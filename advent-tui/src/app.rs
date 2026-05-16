//! Finite-state TUI driver. Phase transitions are explicit; all I/O happens
//! through awaited helpers so the render loop never blocks.

use std::{
  path::PathBuf,
  time::{Duration, Instant},
};

use advent_cache::{
  ValidatorOutcome, add_elapsed, bump_attempts, bump_hints, complete_quest, read_progress, set_current_quest,
};
use advent_config::LoadedConfig;
use advent_core::{AdventError, AdventResult, ProgressState, QuestStep};
use advent_quest::tests::TestOutcome;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::layout::Rect;
use tokio::{sync::oneshot, time};

use crate::{
  confetti::Confetti,
  screens,
  terminal::{Tui, enter, leave},
  workspace::Workspace,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
  Splash,
  Installing,
  Validating,
  Briefing,
  Workspace,
  /// Tests are running asynchronously; the UI shows a spinner overlay and
  /// keeps painting so it never feels frozen.
  RunningTests,
  /// Working tree has user edits that block branch switching. Modal offers
  /// commit / stash / discard / cancel.
  DirtyPrompt,
  /// Generic "working on it" spinner for short async transitions (git commit,
  /// branch switch, etc.) so the UI shows feedback instead of going silent.
  Working,
  HintOverlay,
  Celebrate,
  Complete,
  Error,
}

/// What the user wants done with their dirty tree before advancing.
#[derive(Clone, Copy, Debug)]
enum DirtyChoice {
  Commit,
  Stash,
  Discard,
}

pub struct App {
  cfg: LoadedConfig,
  cli_version: String,
  progress: ProgressState,
  quest: QuestStep,
  phase: Phase,

  install_status: String,
  install_tail: Vec<String>,
  validator_results: Vec<ValidatorOutcome>,
  validator_busy: Option<usize>,

  briefing_md: String,
  briefing_scroll: u16,

  workspace: Option<Workspace>,
  confetti: Confetti,
  celebrate_secs: u64,
  is_last_quest: bool,

  hint_text: String,
  hint_index: usize,

  /// When set, the next keystroke is interpreted as a duck-advent command
  /// instead of being forwarded to the focused pane. Cleared on every key
  /// (whether it matched a command or not) and on a 1s timeout via the tick.
  leader_pending: bool,
  /// Tick counter at which the leader auto-clears if no key follows.
  leader_deadline: Option<std::time::Instant>,

  /// One-shot channel waiting on the background test runner. Polled on each
  /// tick so the UI stays responsive while vitest does its thing.
  test_rx: Option<oneshot::Receiver<AdventResult<TestOutcome>>>,
  /// Streaming output channel — every captured line lands here. UI renders
  /// the most recent N inside the modal so the user sees live progress.
  test_lines_rx: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
  /// Tail of the captured lines (newest at the end).
  test_tail: std::collections::VecDeque<String>,
  /// Co-operative cancel flag the streaming runner polls.
  test_cancel: std::sync::Arc<tokio::sync::Mutex<bool>>,
  /// Spinner counter for the "running tests" overlay.
  test_spin: u32,
  /// Tick counter at the moment we kicked the test runner.
  test_started_tick: u32,
  /// Global tick counter — incremented on every `on_tick`.
  tick_count: u32,

  /// Message rendered in the generic "Working" modal during async transitions.
  working_msg: String,

  error_msg: String,

  /// Wall-clock instant when the current quest session began (last
  /// enter_workspace for this slug). `None` while we are outside a quest
  /// workspace (splash, install, validating, briefing-before-first-enter).
  quest_session_start: Option<Instant>,
  /// Whole seconds of the current session that have already been flushed to
  /// `~/.gentleduck/state/.../progress.json`. The delta between this and
  /// `quest_session_start.elapsed().as_secs()` is what the next flush adds.
  session_flushed_secs: u64,
  /// Cached `elapsed_seconds` from progress.json at the start of this session,
  /// so the live timer = `session_baseline + session_start.elapsed()` without
  /// hitting disk on every render.
  session_baseline_secs: u64,
}

pub async fn run(cfg: LoadedConfig, cli_version: String) -> AdventResult<()> {
  let mut app = App::new(cfg, cli_version)?;
  let mut terminal = enter().map_err(AdventError::BareIo)?;
  // When resuming, skip the splash and kick the install/validate flow
  // immediately so the user lands back inside their workspace fast.
  let prime = if app.phase == Phase::Installing { kick_install(&mut app).await } else { Ok(()) };
  let result = match prime {
    Ok(()) => main_loop(&mut terminal, &mut app).await,
    Err(e) => Err(e),
  };
  // Final flush so the last few seconds of the session are not lost on quit.
  flush_session_timer(&mut app);
  leave(&mut terminal).ok();
  result
}

impl App {
  fn new(cfg: LoadedConfig, cli_version: String) -> AdventResult<Self> {
    let progress = read_progress(&cfg.repo_hash)?;
    let is_resume = progress.current_quest.is_some();
    let resume_slug = progress.current_quest.clone().unwrap_or_else(|| cfg.config.first().slug.clone());
    let quest = cfg.config.find_by_slug(&resume_slug).cloned().unwrap_or_else(|| cfg.config.first().clone());
    // First-run shows splash + install + validate + briefing. Resumes jump
    // straight into the workspace via Validating (which fast-paths through
    // cached install + validators when the lockfile/config hashes match).
    let initial_phase = if is_resume { Phase::Installing } else { Phase::Splash };
    Ok(Self {
      cfg,
      cli_version,
      progress,
      quest,
      phase: initial_phase,
      install_status: "checking lockfile".into(),
      install_tail: Vec::new(),
      validator_results: Vec::new(),
      validator_busy: None,
      briefing_md: String::new(),
      briefing_scroll: 0,
      workspace: None,
      confetti: Confetti::new(80, 24),
      celebrate_secs: 0,
      is_last_quest: false,
      hint_text: String::new(),
      hint_index: 0,
      leader_pending: false,
      leader_deadline: None,
      test_rx: None,
      test_lines_rx: None,
      test_tail: std::collections::VecDeque::with_capacity(64),
      test_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
      test_spin: 0,
      test_started_tick: 0,
      tick_count: 0,
      working_msg: String::new(),
      error_msg: String::new(),
      quest_session_start: None,
      session_flushed_secs: 0,
      session_baseline_secs: 0,
    })
  }

  /// Live elapsed for the current quest = persisted baseline + current
  /// session. Returns 0 when there is no quest session active (e.g. splash).
  pub fn quest_elapsed_secs(&self) -> u64 {
    let session = self.quest_session_start.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    self.session_baseline_secs.saturating_add(session)
  }

  fn fail(&mut self, msg: impl Into<String>) {
    self.error_msg = msg.into();
    self.phase = Phase::Error;
  }

  fn hints_for(&self, slug: &str) -> u32 {
    self.progress.quests.get(slug).map(|q| q.hints_used).unwrap_or(0)
  }

  fn attempts_for(&self, slug: &str) -> u32 {
    self.progress.quests.get(slug).map(|q| q.attempts).unwrap_or(0)
  }
}

async fn main_loop(terminal: &mut Tui, app: &mut App) -> AdventResult<()> {
  let mut events = EventStream::new();
  let mut tick = time::interval(Duration::from_millis(40));
  loop {
    terminal.draw(|f| draw(f, app)).map_err(AdventError::BareIo)?;
    tokio::select! {
      maybe_event = events.next() => {
        if let Some(Ok(event)) = maybe_event
          && handle_event(app, event).await? { break; }
      }
      _ = tick.tick() => {
        on_tick(app).await?;
      }
    }
  }
  Ok(())
}

async fn on_tick(app: &mut App) -> AdventResult<()> {
  app.tick_count = app.tick_count.wrapping_add(1);
  // Auto-clear leader_pending after 1s so a stray Ctrl-a doesn't eat the
  // next real keystroke forever.
  if app.leader_pending
    && let Some(deadline) = app.leader_deadline
    && std::time::Instant::now() >= deadline
  {
    app.leader_pending = false;
    app.leader_deadline = None;
  }
  if matches!(app.phase, Phase::Celebrate | Phase::Complete) {
    app.confetti.tick();
  }
  // Flush the per-quest timer to disk every ~5 s so a SIGKILL only loses a
  // handful of seconds. Tick is 40 ms, so 125 ticks ≈ 5 s.
  if app.tick_count.is_multiple_of(125) {
    flush_session_timer(app);
  }
  if matches!(app.phase, Phase::RunningTests) {
    app.test_spin = app.test_spin.wrapping_add(1);
    // Drain any new lines from the streaming reader (non-blocking).
    if let Some(rx) = app.test_lines_rx.as_mut() {
      while let Ok(line) = rx.try_recv() {
        if app.test_tail.len() == 64 {
          app.test_tail.pop_front();
        }
        app.test_tail.push_back(line);
      }
    }
    if let Some(rx) = app.test_rx.as_mut() {
      match rx.try_recv() {
        Ok(Ok(outcome)) => {
          app.test_rx = None;
          on_tests_finished(app, outcome).await?;
        },
        Ok(Err(err)) => {
          app.test_rx = None;
          app.fail(format!("test runner errored: {err}"));
        },
        Err(oneshot::error::TryRecvError::Empty) => {},
        Err(oneshot::error::TryRecvError::Closed) => {
          app.test_rx = None;
          app.fail("test runner task vanished".to_string());
        },
      }
    }
  }
  Ok(())
}

async fn handle_event(app: &mut App, event: Event) -> AdventResult<bool> {
  if let Event::Resize(w, h) = event {
    app.confetti.resize(w, h);
    if let Some(ws) = app.workspace.as_mut() {
      ws.relayout(Rect { x: 0, y: 0, width: w, height: h }).ok();
    }
    return Ok(false);
  }
  let Event::Key(key) = event else { return Ok(false) };
  if key.kind == KeyEventKind::Release {
    return Ok(false);
  }

  // Global quit shortcut.
  if matches!(key.code, KeyCode::Char('q' | 'Q')) && key.modifiers.contains(KeyModifiers::CONTROL) {
    return Ok(true);
  }

  match app.phase {
    Phase::Splash => splash_keys(app, key).await,
    Phase::Installing | Phase::Validating | Phase::Working => Ok(false),
    Phase::RunningTests => running_keys(app, key).await,
    Phase::Briefing => briefing_keys(app, key).await,
    Phase::Workspace => workspace_keys(app, key).await,
    Phase::DirtyPrompt => dirty_prompt_keys(app, key).await,
    Phase::HintOverlay => hint_keys(app, key),
    Phase::Celebrate => celebrate_keys(app, key).await,
    Phase::Complete => Ok(matches!(key.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q'))),
    Phase::Error => Ok(true),
  }
}

async fn running_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
    *app.test_cancel.lock().await = true;
  }
  Ok(false)
}

async fn dirty_prompt_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  match key.code {
    KeyCode::Char('c' | 'C') => apply_dirty_choice(app, DirtyChoice::Commit).await?,
    KeyCode::Char('s' | 'S') => apply_dirty_choice(app, DirtyChoice::Stash).await?,
    KeyCode::Char('d' | 'D') => apply_dirty_choice(app, DirtyChoice::Discard).await?,
    KeyCode::Esc | KeyCode::Char('x' | 'X') => {
      // Cancel — drop back into the workspace; user can keep editing.
      app.phase = Phase::Workspace;
    },
    _ => {},
  }
  Ok(false)
}

async fn splash_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  match key.code {
    KeyCode::Enter | KeyCode::Char(' ') => {
      app.phase = Phase::Installing;
      kick_install(app).await?;
    },
    KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
    _ => {},
  }
  Ok(false)
}

async fn briefing_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  match key.code {
    // Close the briefing. If a workspace is already alive (briefing was
    // opened from inside one via <leader> p) just flip back — no respawn.
    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
      if app.workspace.is_some() {
        app.phase = Phase::Workspace;
      } else {
        enter_workspace(app).await?;
      }
    },
    KeyCode::Char('j') | KeyCode::Down => app.briefing_scroll = app.briefing_scroll.saturating_add(1),
    KeyCode::Char('k') | KeyCode::Up => app.briefing_scroll = app.briefing_scroll.saturating_sub(1),
    KeyCode::Char('g') => app.briefing_scroll = 0,
    KeyCode::Char('G') => app.briefing_scroll = app.briefing_scroll.saturating_add(200),
    KeyCode::Char(' ') | KeyCode::PageDown => app.briefing_scroll = app.briefing_scroll.saturating_add(10),
    KeyCode::PageUp => app.briefing_scroll = app.briefing_scroll.saturating_sub(10),
    _ => {},
  }
  Ok(false)
}

async fn workspace_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  // 1. Direct focus toggle — F2 always swaps panes without going through the
  //    leader. Picked because F-keys never collide with nvim or terminal
  //    multiplexers like tmux/screen.
  if matches!(key.code, KeyCode::F(2)) {
    if let Some(ws) = app.workspace.as_mut() {
      if let Err(e) = ws.toggle_focus() {
        app.fail(format!("pane resize failed: {e}"));
      }
    }
    return Ok(false);
  }
  // F3 — toggle full-screen for the focused pane. Resizes the embedded child
  // immediately so vitest/nvim re-flow to the new width.
  if matches!(key.code, KeyCode::F(3)) {
    if let Some(ws) = app.workspace.as_mut() {
      if let Err(e) = ws.toggle_zoom() {
        app.fail(format!("zoom resize failed: {e}"));
      }
    }
    return Ok(false);
  }

  // 2. Resolve leader prefix. Either `Ctrl-a` (default) or `Ctrl-Space`
  //    (works inside tmux sessions that have Ctrl-a bound). Auto-clears after
  //    1s so a stray prefix doesn't eat the next real keystroke.
  if app.leader_pending {
    app.leader_pending = false;
    app.leader_deadline = None;
    return dispatch_leader(app, key).await;
  }
  let is_leader_trigger = key.modifiers.contains(KeyModifiers::CONTROL)
    && (matches!(key.code, KeyCode::Char('a' | 'A')) || matches!(key.code, KeyCode::Char(' ')));
  if is_leader_trigger {
    app.leader_pending = true;
    app.leader_deadline = Some(std::time::Instant::now() + std::time::Duration::from_millis(1000));
    return Ok(false);
  }

  let Some(ws) = app.workspace.as_mut() else {
    return Ok(false);
  };

  // 2. Tests pane is read-only. Navigation keys scroll the vt100 scrollback;
  //    everything else is silently swallowed (no stdin to vitest).
  if matches!(ws.focus, crate::workspace::Focus::Tests) {
    let _ = handle_tests_scroll(ws, key);
    return Ok(false);
  }

  // 3. Editor pane gets the raw key.
  if let Err(e) = ws.forward_key(key) {
    app.fail(format!("editor pty closed: {e}"));
  }
  Ok(false)
}

/// `<leader>` is `Ctrl-a`. Following keystrokes invoke duck commands without
/// touching nvim/vitest. Single-letter mnemonics — n=next, r=repeat, b=switch
/// focus, h=hint, p=briefing, q=quit, c=cancel.
async fn dispatch_leader(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  match key.code {
    KeyCode::Char('n' | 'N') => {
      validate_and_celebrate(app)?;
    },
    KeyCode::Char('r' | 'R') => {
      repeat_current(app).await?;
    },
    KeyCode::Char('b' | 'B') => {
      if let Some(ws) = app.workspace.as_mut() {
        if let Err(e) = ws.toggle_focus() {
          app.fail(format!("pane resize failed: {e}"));
        }
      }
    },
    KeyCode::Char('z' | 'Z') => {
      if let Some(ws) = app.workspace.as_mut() {
        if let Err(e) = ws.toggle_zoom() {
          app.fail(format!("zoom resize failed: {e}"));
        }
      }
    },
    KeyCode::Char('h' | 'H') => {
      let used = app.hints_for(&app.quest.slug) as usize;
      let total = app.quest.hints.len();
      if total == 0 {
        app.hint_text = "this quest has no hints — you're on your own".into();
        app.hint_index = 0;
      } else if used >= total {
        app.hint_text = format!("you've used all {total} hints — re-read the briefing with <leader> p");
        app.hint_index = total.saturating_sub(1);
      } else {
        app.hint_text = app.quest.hints[used].clone();
        app.hint_index = used;
        let _ = bump_hints(&app.cfg.repo_hash, &app.quest.slug);
        app.progress = read_progress(&app.cfg.repo_hash)?;
      }
      app.phase = Phase::HintOverlay;
    },
    KeyCode::Char('p' | 'P') => {
      load_briefing(app).await?;
      app.phase = Phase::Briefing;
    },
    KeyCode::Char('q' | 'Q') => return Ok(true),
    // `[` / `]` — non-destructive quest navigation. No validation, just jump.
    // Mirrors vim's `[`/`]` motions. Dirty tree pops the same prompt the
    // forward path uses.
    KeyCode::Char('[') => {
      if let Some(prev) = app.cfg.config.prev_before(&app.quest.slug).cloned() {
        goto_quest(app, prev.slug).await?;
      }
    },
    KeyCode::Char(']') => {
      if let Some(next) = app.cfg.config.next_after(&app.quest.slug).cloned() {
        goto_quest(app, next.slug).await?;
      }
    },
    // c / Esc / Ctrl-a again = cancel the leader without firing a command.
    KeyCode::Char('c' | 'C') | KeyCode::Esc => {},
    _ => {},
  }
  Ok(false)
}

/// Returns true when the key was consumed by scrolling. Read-only — does
/// not forward to vitest stdin.
fn handle_tests_scroll(ws: &crate::workspace::Workspace, key: KeyEvent) -> bool {
  let pane = &ws.tests;
  match key.code {
    KeyCode::Char('j') | KeyCode::Down => {
      pane.scroll_down(1);
      true
    },
    KeyCode::Char('k') | KeyCode::Up => {
      pane.scroll_up(1);
      true
    },
    KeyCode::Char(' ') | KeyCode::PageDown => {
      pane.scroll_down(10);
      true
    },
    KeyCode::PageUp => {
      pane.scroll_up(10);
      true
    },
    KeyCode::Char('g') => {
      pane.scroll_top();
      true
    },
    KeyCode::Char('G') => {
      pane.scroll_bottom();
      true
    },
    KeyCode::End => {
      pane.scroll_bottom();
      true
    },
    KeyCode::Home => {
      pane.scroll_top();
      true
    },
    _ => false,
  }
}

fn hint_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
    app.phase = Phase::Workspace;
  }
  Ok(false)
}

async fn celebrate_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  match key.code {
    KeyCode::Char('n' | 'N') | KeyCode::Enter => advance_quest(app).await?,
    KeyCode::Char('r' | 'R') => {
      repeat_current(app).await?;
    },
    KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
    _ => {},
  }
  Ok(false)
}

// ---------- phase actions ------------------------------------------------

async fn kick_install(app: &mut App) -> AdventResult<()> {
  use advent_cache::{has_fresh_install, mark_install_complete};
  if has_fresh_install(&app.cfg.repo_root, &app.cfg.repo_hash)? {
    app.install_status = "✓ cached".into();
    app.phase = Phase::Validating;
    kick_validators(app).await?;
    return Ok(());
  }
  let cmd = app.cfg.config.install_command.clone();
  app.install_status = format!("running {}", cmd.join(" "));
  let out = tokio::process::Command::new(&cmd[0])
    .args(&cmd[1..])
    .current_dir(&app.cfg.repo_root)
    .output()
    .await
    .map_err(AdventError::BareIo)?;
  let stdout = String::from_utf8_lossy(&out.stdout);
  app.install_tail = stdout.lines().rev().take(6).map(str::to_string).collect::<Vec<_>>();
  app.install_tail.reverse();
  if !out.status.success() {
    app.fail(format!("install failed: {}", String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>()));
    return Ok(());
  }
  mark_install_complete(&app.cfg.repo_root, &app.cfg.repo_hash)?;
  app.install_status = "✓ installed".into();
  app.phase = Phase::Validating;
  kick_validators(app).await?;
  Ok(())
}

async fn kick_validators(app: &mut App) -> AdventResult<()> {
  use advent_cache::{read_validator_cache, write_validator_cache};
  if let Some(cached) = read_validator_cache(&app.cfg.repo_hash, &app.cfg.config_hash)? {
    app.validator_results = cached;
    if let Some(v) = app
      .cfg
      .config
      .validators
      .iter()
      .find(|v| !v.optional && !app.validator_results.iter().any(|r| r.id == v.id && r.passed))
    {
      app.fail(format!("validator failed (cached): {}", v.label));
      return Ok(());
    }
    load_briefing(app).await?;
    app.phase = Phase::Briefing;
    return Ok(());
  }

  let mut results: Vec<ValidatorOutcome> = Vec::new();
  for (i, v) in app.cfg.config.validators.clone().iter().enumerate() {
    app.validator_busy = Some(i);
    let outcome = match tokio::process::Command::new(&v.cmd[0]).args(&v.cmd[1..]).output().await {
      Ok(o) => ValidatorOutcome {
        id: v.id.clone(),
        passed: o.status.success(),
        output: format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
          .chars()
          .take(300)
          .collect(),
        checked_at: chrono::Utc::now().to_rfc3339(),
      },
      Err(e) => ValidatorOutcome {
        id: v.id.clone(),
        passed: false,
        output: e.to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
      },
    };
    results.push(outcome);
    app.validator_results = results.clone();
  }
  app.validator_busy = None;
  write_validator_cache(&app.cfg.repo_hash, &app.cfg.config_hash, &results)?;
  if let Some(v) =
    app.cfg.config.validators.iter().find(|v| !v.optional && !results.iter().any(|r| r.id == v.id && r.passed))
  {
    app.fail(format!("validator failed: {}", v.label));
    return Ok(());
  }
  load_briefing(app).await?;
  app.phase = Phase::Briefing;
  Ok(())
}

async fn load_briefing(app: &mut App) -> AdventResult<()> {
  let path: PathBuf = app.cfg.repo_root.join(&app.quest.briefing);
  app.briefing_md = tokio::fs::read_to_string(&path)
    .await
    .unwrap_or_else(|_| format!("# Quest {:02} — {}\n\n_(briefing missing)_", app.quest.number, app.quest.title));
  app.briefing_scroll = 0;
  Ok(())
}

/// Idempotent workspace entry. If a Workspace is already alive on the right
/// branch we just flip the phase — never respawn nvim/vitest needlessly. Only
/// spawns fresh when there's no workspace yet or the user is on a different
/// branch than the active quest.
async fn enter_workspace(app: &mut App) -> AdventResult<()> {
  use advent_quest::git;

  let current = git::current_branch(&app.cfg.repo_root).await?;
  let needs_checkout = current != app.quest.slug;

  if needs_checkout {
    if !git::working_tree_clean(&app.cfg.repo_root).await? {
      app.phase = Phase::DirtyPrompt;
      return Ok(());
    }
    if !git::branch_exists(&app.cfg.repo_root, &app.quest.slug).await? {
      app.fail(format!("branch \"{}\" does not exist", app.quest.slug));
      return Ok(());
    }
    git::checkout(&app.cfg.repo_root, &app.quest.slug).await?;
  }
  app.progress = set_current_quest(&app.cfg.repo_hash, &app.quest.slug)?;

  // Re-use existing workspace when we did not change branches. Saves nvim
  // restart + preserves user's open buffers, cursor position, undo history.
  if !needs_checkout && app.workspace.is_some() {
    app.phase = Phase::Workspace;
    return Ok(());
  }

  drop(app.workspace.take());
  let (cols, rows) = crossterm::terminal::size().map_err(AdventError::BareIo)?;
  let area = Rect { x: 0, y: 0, width: cols, height: rows };
  let ws = Workspace::spawn(&app.cfg.repo_root, &app.cfg.config, &app.quest, area)
    .map_err(|e| AdventError::Git(e.to_string()))?;
  app.workspace = Some(ws);
  begin_quest_session(app);
  app.phase = Phase::Workspace;
  Ok(())
}

/// Spawn the test runner on a background task and flip into `RunningTests`
/// so the UI stays responsive while vitest churns. Live stdout/stderr lines
/// stream into `test_lines_rx` so the modal can render the tail.
fn validate_and_celebrate(app: &mut App) -> AdventResult<()> {
  let _ = bump_attempts(&app.cfg.repo_hash, &app.quest.slug)?;
  let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
  let (done_tx, done_rx) = oneshot::channel();
  let cancel = std::sync::Arc::new(tokio::sync::Mutex::new(false));
  app.test_cancel = std::sync::Arc::clone(&cancel);
  let config = app.cfg.config.clone();
  let repo_root = app.cfg.repo_root.clone();
  let quest = app.quest.clone();
  tokio::spawn(async move {
    let result = advent_quest::tests::run_streaming(&config, &repo_root, &quest, cancel, out_tx).await;
    let _ = done_tx.send(result);
  });
  app.test_rx = Some(done_rx);
  app.test_lines_rx = Some(out_rx);
  app.test_tail.clear();
  app.test_spin = 0;
  app.test_started_tick = app.tick_count;
  app.phase = Phase::RunningTests;
  Ok(())
}

async fn on_tests_finished(app: &mut App, outcome: TestOutcome) -> AdventResult<()> {
  app.test_lines_rx = None;
  if outcome.cancelled {
    app.phase = Phase::Workspace;
    return Ok(());
  }
  if outcome.timed_out {
    app.fail("tests timed out after 3 minutes — check the runner config".to_string());
    return Ok(());
  }
  if !outcome.passed {
    let combined: Vec<String> = outcome.stdout.lines().chain(outcome.stderr.lines()).map(str::to_string).collect();
    let tail = combined.iter().rev().take(20).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    app.fail(format!("tests failed:\n{tail}"));
    return Ok(());
  }
  app.progress = complete_quest(&app.cfg.repo_hash, &app.quest.slug)?;
  app.is_last_quest = app.cfg.config.next_after(&app.quest.slug).is_none();
  app.celebrate_secs = duration_for(&app.progress, &app.quest.slug);
  app.phase = Phase::Celebrate;
  Ok(())
}

async fn advance_quest(app: &mut App) -> AdventResult<()> {
  let Some(next) = app.cfg.config.next_after(&app.quest.slug).cloned() else {
    app.phase = Phase::Complete;
    return Ok(());
  };
  // If the user has uncommitted edits we can't blindly `git checkout` away.
  // Pop a DirtyPrompt modal asking what to do instead of panicking.
  if !advent_quest::git::working_tree_clean(&app.cfg.repo_root).await? {
    app.phase = Phase::DirtyPrompt;
    return Ok(());
  }
  flush_session_timer(app);
  drop(app.workspace.take());
  app.quest = next;
  load_briefing(app).await?;
  enter_workspace(app).await?;
  Ok(())
}

async fn apply_dirty_choice(app: &mut App, choice: DirtyChoice) -> AdventResult<()> {
  let workdir = app.quest.workdir.clone();
  let repo = app.cfg.repo_root.clone();
  let msg = format!("complete quest {:02}: {}", app.quest.number, app.quest.title);
  app.phase = Phase::Working;
  app.working_msg = format!("applying: {choice:?}…");
  match choice {
    DirtyChoice::Commit => {
      // Stage EVERYTHING (not just workdir) — otherwise the next
      // `working_tree_clean` check loops back into the prompt because edits
      // outside the quest's workdir remain unstaged.
      run_git(&repo, &["add", "-A"]).await?;
      // `--allow-empty` so `c` succeeds even if every change was inside an
      // ignored path. `-n` skips pre-commit hooks that would block the flow.
      ensure_git_identity(&repo).await?;
      run_git(&repo, &["commit", "-m", &msg, "--allow-empty", "--no-verify"]).await?;
    },
    DirtyChoice::Stash => {
      run_git(&repo, &["stash", "push", "-u", "-m", &msg, "--", &workdir]).await?;
    },
    DirtyChoice::Discard => {
      advent_quest::git::discard_workdir(&repo, &workdir).await?;
    },
  }
  // After the cleanup, re-attempt the advance — this time the tree is clean.
  advance_quest(app).await
}

async fn run_git(cwd: &std::path::Path, args: &[&str]) -> AdventResult<()> {
  let out = tokio::process::Command::new("git").args(args).current_dir(cwd).output().await?;
  if !out.status.success() {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let context = args.join(" ");
    return Err(AdventError::Git(format!("`git {context}` failed: {stderr}")));
  }
  Ok(())
}

/// Make sure `git commit` will not fail with "please tell me who you are".
/// We probe for `user.email` and inject sensible repo-local defaults when the
/// user has not configured one. Quest commits are local-only by design.
async fn ensure_git_identity(cwd: &std::path::Path) -> AdventResult<()> {
  let email = tokio::process::Command::new("git")
    .args(["config", "user.email"])
    .current_dir(cwd)
    .output()
    .await?;
  if email.status.success() && !email.stdout.is_empty() {
    return Ok(());
  }
  run_git(cwd, &["config", "user.email", "you@example.com"]).await?;
  run_git(cwd, &["config", "user.name", "duck-advent quester"]).await?;
  Ok(())
}

/// Persist the unflushed delta of the current quest session and slide the
/// flush watermark forward. Safe to call when no session is active.
fn flush_session_timer(app: &mut App) {
  let Some(started) = app.quest_session_start else {
    return;
  };
  let session_secs = started.elapsed().as_secs();
  let delta = session_secs.saturating_sub(app.session_flushed_secs);
  if delta == 0 {
    return;
  }
  if let Ok(total) = add_elapsed(&app.cfg.repo_hash, &app.quest.slug, delta) {
    app.session_flushed_secs = session_secs;
    if let Some(q) = app.progress.quests.get_mut(&app.quest.slug) {
      q.elapsed_seconds = total;
    }
  }
}

/// Snapshot the persisted elapsed_seconds for the current quest and start a
/// fresh session clock. Called every time we land in the workspace on a new
/// quest so the live timer continues from where the user left off.
fn begin_quest_session(app: &mut App) {
  app.session_baseline_secs = app.progress.quests.get(&app.quest.slug).map(|q| q.elapsed_seconds).unwrap_or(0);
  app.session_flushed_secs = 0;
  app.quest_session_start = Some(Instant::now());
}

/// Switch to a quest by slug without running the validator. Flushes the
/// in-flight timer so the move never robs the user of recorded time. Used by
/// the `<leader> [` / `<leader> ]` chords.
async fn goto_quest(app: &mut App, slug: String) -> AdventResult<()> {
  let Some(target) = app.cfg.config.find_by_slug(&slug).cloned() else {
    app.fail(format!("quest {slug} not in config"));
    return Ok(());
  };
  if target.slug == app.quest.slug {
    return Ok(());
  }
  if !advent_quest::git::working_tree_clean(&app.cfg.repo_root).await? {
    app.phase = Phase::DirtyPrompt;
    return Ok(());
  }
  flush_session_timer(app);
  drop(app.workspace.take());
  app.quest = target;
  load_briefing(app).await?;
  enter_workspace(app).await?;
  Ok(())
}

async fn repeat_current(app: &mut App) -> AdventResult<()> {
  let workdir = app.quest.workdir.clone();
  if let Some(ws) = app.workspace.as_mut() {
    ws.editor.kill();
    ws.tests.kill();
  }
  app.workspace = None;
  advent_quest::git::discard_workdir(&app.cfg.repo_root, &workdir).await?;
  enter_workspace(app).await?;
  Ok(())
}

fn duration_for(progress: &ProgressState, slug: &str) -> u64 {
  let Some(q) = progress.quests.get(slug) else {
    return 0;
  };
  // Prefer the active-time accumulator (skips idle/closed-laptop time). Fall
  // back to wall-clock between start and completion for legacy progress files
  // written before `elapsed_seconds` existed.
  if q.elapsed_seconds > 0 {
    return q.elapsed_seconds;
  }
  let Some(start) = q.started_at else {
    return 0;
  };
  let end = q.completed_at.unwrap_or_else(chrono::Utc::now);
  (end - start).num_seconds().max(0) as u64
}

// ---------- draw ---------------------------------------------------------

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
  let area = frame.area();
  match app.phase {
    Phase::Splash => {
      screens::splash::draw(frame, area, &app.cfg.config.name, app.cfg.config.description.as_deref(), &app.cli_version)
    },
    Phase::Installing => {
      screens::install::draw(frame, area, &app.cfg.config.install_command, &app.install_tail, &app.install_status);
    },
    Phase::Validating => {
      screens::validators::draw(frame, area, &app.cfg.config.validators, &app.validator_results, app.validator_busy);
    },
    Phase::Briefing => {
      // Draw workspace underneath when available so the user sees the test
      // pane keep running while they read the briefing.
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
      screens::briefing::draw(frame, area, &app.quest, &app.briefing_md, app.briefing_scroll);
    },
    Phase::Workspace => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
    },
    Phase::HintOverlay => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
      screens::hint::draw(frame, area, &app.hint_text, app.hint_index, app.quest.hints.len());
    },
    Phase::RunningTests => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
      let elapsed_ticks = app.tick_count.wrapping_sub(app.test_started_tick);
      let tail: Vec<&str> = app.test_tail.iter().map(String::as_str).collect();
      screens::running::draw(
        frame,
        area,
        &app.cfg.config.test_command,
        (app.test_spin & 0xFF) as u8,
        elapsed_ticks,
        &tail,
      );
    },
    Phase::DirtyPrompt => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
      screens::dirty::draw(frame, area, &app.quest.title);
    },
    Phase::Working => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(frame, area, &app.quest, app.cfg.config.quests.len(), hints, app.leader_pending, app.quest_elapsed_secs());
      }
      screens::working::draw(frame, area, &app.working_msg, (app.test_spin & 0xFF) as u8);
    },
    Phase::Celebrate => {
      let view = screens::celebrate::CelebrateView {
        quest: &app.quest,
        confetti: &app.confetti,
        hints_used: app.hints_for(&app.quest.slug),
        attempts: app.attempts_for(&app.quest.slug),
        duration_secs: app.celebrate_secs,
        is_last: app.is_last_quest,
      };
      screens::celebrate::draw(frame, area, &view);
    },
    Phase::Complete => {
      let total_dur = (chrono::Utc::now() - app.progress.started_at).num_seconds().max(0) as u64;
      let total_hints: u32 = app.progress.quests.values().map(|q| q.hints_used).sum();
      screens::complete::draw(
        frame,
        area,
        &app.cfg.config.name,
        &app.confetti,
        (app.cfg.config.quests.len(), total_hints, total_dur),
      );
    },
    Phase::Error => screens::error::draw(frame, area, &app.error_msg),
  }
}
