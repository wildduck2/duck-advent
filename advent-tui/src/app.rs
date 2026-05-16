//! Finite-state TUI driver.
//!
//! State management invariants (read before touching):
//!
//! 1. `app.progress` mirrors `~/.gentleduck/state/<repo>/progress.json` at all
//!    times. Every cache mutator (set_current_quest, complete_quest,
//!    bump_hints, bump_attempts, add_elapsed) returns the freshly written
//!    ProgressState; all call sites must replace `app.progress` with it.
//!
//! 2. The per-quest timer flushes through `flush_session_timer`. That helper
//!    is idempotent and cheap, so we flush it at *every* boundary that might
//!    end a session (quest switch, repeat, quit, fail, complete, test pass).
//!    The periodic 5s tick is the safety net, not the primary mechanism.
//!
//! 3. Quest transitions go through ONE entry point: `transition_to_quest`.
//!    Direct calls to `git::checkout` or `enter_workspace` from action
//!    handlers are forbidden. The single funnel guarantees: dirty-tree
//!    handling, branch existence check, timer flush, workspace respawn,
//!    progress refresh, session reseed — all happen, in the right order,
//!    every time.
//!
//! 4. When a transition cannot complete because the working tree is dirty,
//!    the user's *intent* is preserved in `pending_intent` so the
//!    DirtyPrompt resolution replays the exact action they asked for. Prior
//!    to this, every dirty resolution fell through to `advance_quest` which
//!    silently converted a backward `<leader> [` into a forward jump.

use std::{
  path::PathBuf,
  time::{Duration, Instant},
};

use advent_cache::{
  CompletionOutcome, ValidatorOutcome, add_elapsed, bump_attempts, bump_hints, complete_quest, read_progress,
  reset_attempt, set_current_quest,
};
use advent_config::LoadedConfig;
use advent_core::{AdventError, AdventResult, ProgressState, QuestManifest, QuestStep};
use advent_quest::tests::TestOutcome;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::screens::dirty::DirtyPromptKind;
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
  /// Read-only overlay listing every quest with best time + status.
  /// Triggered via `<leader> l`. Dismiss with Esc/Enter/q.
  Leaderboard,
  /// Soft, recoverable advisory modal — drawn over the workspace and
  /// dismissed with any key. Used for lock gating, manifest drift, and
  /// other "you can fix this in-place" cases.
  Notice,
  Error,
}

/// What the user wants done with their dirty tree before advancing.
#[derive(Clone, Copy, Debug)]
enum DirtyChoice {
  Commit,
  Stash,
  Discard,
}

/// The action that triggered DirtyPrompt — replayed after the user picks a
/// resolution. Without this, every dirty resolution used to fall through to
/// `advance_quest`, silently converting backward jumps into forward ones.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingIntent {
  /// Validate-and-advance (the `<leader> n` happy path, or Enter on the
  /// celebrate screen). Goes to the next quest after the current one.
  AdvanceNext,
  /// Jump to the next quest without validation (`<leader> ]`).
  GotoNext(String),
  /// Jump to the previous quest (`<leader> [`).
  GotoPrev(String),
  /// Jump to an arbitrary quest by slug (programmatic).
  GotoQuest(String),
  /// Discard edits in the current quest's workdir and re-enter the same
  /// quest. Used by `<leader> r` and the celebrate-screen `r`.
  Repeat,
}

impl PendingIntent {
  /// Map the intent to the right DirtyPrompt copy so a backward jump never
  /// reads "switching to the next quest".
  fn dirty_kind(&self) -> DirtyPromptKind {
    match self {
      PendingIntent::AdvanceNext => DirtyPromptKind::AdvanceForward,
      PendingIntent::GotoNext(_) => DirtyPromptKind::GoForward,
      PendingIntent::GotoPrev(_) => DirtyPromptKind::GoBackward,
      PendingIntent::GotoQuest(_) => DirtyPromptKind::GoForward,
      PendingIntent::Repeat => DirtyPromptKind::Repeat,
    }
  }
}

/// Pure mapping from a leader-armed keystroke to the action the user means.
/// Kept side-effect-free so it can be unit-tested without ratatui/pty/git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaderAction {
  ValidateNext,
  Repeat,
  ToggleFocus,
  ToggleZoom,
  Hint,
  Briefing,
  Quit,
  PrevQuest,
  NextQuestNoValidate,
  /// Open the per-quest leaderboard overlay.
  Leaderboard,
  /// Explicit cancel — `c`/`C`/`Esc`. Clears the leader without firing.
  Cancel,
  /// Any other key — leader prefix is consumed but nothing fires.
  Unknown,
}

/// Build a [`LeaderAction`] from the keystroke that followed the leader
/// prefix. No side effects, no allocation, no I/O. Pure mapping table.
pub fn leader_action_for(key: KeyCode) -> LeaderAction {
  match key {
    KeyCode::Char('n' | 'N') => LeaderAction::ValidateNext,
    KeyCode::Char('r' | 'R') => LeaderAction::Repeat,
    KeyCode::Char('b' | 'B') => LeaderAction::ToggleFocus,
    KeyCode::Char('z' | 'Z') => LeaderAction::ToggleZoom,
    KeyCode::Char('h' | 'H') => LeaderAction::Hint,
    KeyCode::Char('p' | 'P') => LeaderAction::Briefing,
    KeyCode::Char('q' | 'Q') => LeaderAction::Quit,
    KeyCode::Char('l' | 'L') => LeaderAction::Leaderboard,
    KeyCode::Char('[') => LeaderAction::PrevQuest,
    KeyCode::Char(']') => LeaderAction::NextQuestNoValidate,
    KeyCode::Char('c' | 'C') | KeyCode::Esc => LeaderAction::Cancel,
    _ => LeaderAction::Unknown,
  }
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
  /// Body of the active Notice modal (title + message). Cleared on dismiss.
  notice_title: String,
  notice_msg: String,
  /// Phase to return to when the leaderboard overlay is dismissed. Set by
  /// every entry point (workspace leader chord, celebrate, briefing) so the
  /// modal feels stackable instead of teleporting the user.
  leaderboard_return: Option<Phase>,

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

  /// What the user *wanted* to do when DirtyPrompt fired. Replayed by
  /// `apply_dirty_choice` after the working tree is cleaned. Cleared on
  /// resolution OR cancel so a stale intent never fires later.
  pending_intent: Option<PendingIntent>,

  /// Outcome of the most recent completion. Drives the NEW BEST badge +
  /// "personal best" line on the celebrate screen. `None` until first solve
  /// of the session; cleared on quest advance.
  last_completion: Option<CompletionOutcome>,

  /// Optional repo integrity manifest. When present, every `<leader> n`
  /// re-hashes the active chapter's test files and refuses to run if
  /// they drifted; every completion is HMAC-signed with the manifest
  /// secret so progress.json edits can't forge a solve. `None` means the
  /// repo hasn't run `duck-advent manifest gen` — best-effort mode, no
  /// integrity guarantees.
  manifest: Option<QuestManifest>,
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
    let manifest = QuestManifest::load(&cfg.repo_root)?;
    let mut progress = read_progress(&cfg.repo_hash)?;
    // Strip any forged or pre-manifest completion entries up front so the
    // unlock gate, leaderboard, and celebrate screen all see the true state.
    if let Some(m) = manifest.as_ref() {
      let _forged = advent_cache::enforce_completion_sigs(&mut progress, &cfg.repo_hash, &m.secret);
      // Persist the cleaned state so the on-disk view matches what we hold.
      advent_cache::write_progress(&cfg.repo_hash, &mut progress)?;
    }
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
      notice_title: String::new(),
      notice_msg: String::new(),
      leaderboard_return: None,
      quest_session_start: None,
      session_flushed_secs: 0,
      session_baseline_secs: 0,
      pending_intent: None,
      last_completion: None,
      manifest,
    })
  }

  /// Live elapsed for the current quest = persisted baseline + current
  /// session. Returns 0 when there is no quest session active (e.g. splash).
  pub fn quest_elapsed_secs(&self) -> u64 {
    let session = self.quest_session_start.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    self.session_baseline_secs.saturating_add(session)
  }

  /// Single point that flushes any unrecorded session time, terminates the
  /// fail-state machinery, and lands the app on the Error screen. Used by
  /// every error path so we never leave seconds unrecorded.
  ///
  /// Reserve `fail` for FATAL conditions that should end the session
  /// (config-load failure, missing branch, IO error). For recoverable
  /// advisories the user can fix without restarting (lock gate, manifest
  /// drift, hint-already-used), use `notice` instead — it overlays the
  /// workspace and dismisses on any key.
  fn fail(&mut self, msg: impl Into<String>) {
    flush_session_timer(self);
    self.error_msg = msg.into();
    self.phase = Phase::Error;
  }

  /// Pop a soft, dismissable advisory on top of the workspace. Does NOT
  /// flush the session timer (the user is still actively working — they
  /// just hit a guard rail).
  fn notice(&mut self, title: impl Into<String>, msg: impl Into<String>) {
    self.notice_title = title.into();
    self.notice_msg = msg.into();
    self.phase = Phase::Notice;
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
  // handful of seconds. Tick is 40 ms, so 125 ticks ≈ 5 s. This is the
  // safety net — every transition handler also flushes explicitly so a
  // crash between ticks still records most of the session.
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

  // Global quit shortcut. Flush before bailing so the final seconds count.
  if matches!(key.code, KeyCode::Char('q' | 'Q')) && key.modifiers.contains(KeyModifiers::CONTROL) {
    flush_session_timer(app);
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
    Phase::Leaderboard => leaderboard_keys(app, key),
    Phase::Notice => notice_keys(app, key),
    Phase::Error => Ok(true),
  }
}

fn notice_keys(app: &mut App, _key: KeyEvent) -> AdventResult<bool> {
  // Any key dismisses — drop back into the workspace if one is alive,
  // otherwise fall through to the briefing (early-launch case).
  app.notice_title.clear();
  app.notice_msg.clear();
  app.phase = if app.workspace.is_some() { Phase::Workspace } else { Phase::Briefing };
  Ok(false)
}

fn leaderboard_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'l' | 'L')) {
    // Restore the phase the user came from. Falls back to Workspace if no
    // return was recorded (defensive — shouldn't happen).
    app.phase = app.leaderboard_return.take().unwrap_or(Phase::Workspace);
  }
  Ok(false)
}

/// Refresh progress from disk and pop the leaderboard overlay on top of
/// whichever phase the user invoked it from. `return_to` is restored on
/// dismiss so the overlay feels stackable (celebrate → board → celebrate,
/// briefing → board → briefing, etc.).
fn open_leaderboard(app: &mut App, return_to: Phase) -> AdventResult<()> {
  app.progress = read_progress(&app.cfg.repo_hash)?;
  app.leaderboard_return = Some(return_to);
  app.phase = Phase::Leaderboard;
  Ok(())
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
      // Cancel — drop back into the workspace; user can keep editing. Clear
      // the stashed intent so it cannot fire later.
      app.pending_intent = None;
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
        // First entry into the current quest. Funnel through the same single
        // transition path used by every other quest change.
        let slug = app.quest.slug.clone();
        transition_to_quest(app, slug, PendingIntent::GotoQuest(app.quest.slug.clone())).await?;
      }
    },
    KeyCode::Char('j') | KeyCode::Down => app.briefing_scroll = app.briefing_scroll.saturating_add(1),
    KeyCode::Char('k') | KeyCode::Up => app.briefing_scroll = app.briefing_scroll.saturating_sub(1),
    KeyCode::Char('g') => app.briefing_scroll = 0,
    KeyCode::Char('G') => app.briefing_scroll = app.briefing_scroll.saturating_add(200),
    KeyCode::Char(' ') | KeyCode::PageDown => app.briefing_scroll = app.briefing_scroll.saturating_add(10),
    KeyCode::PageUp => app.briefing_scroll = app.briefing_scroll.saturating_sub(10),
    // Pop the leaderboard right on top of the briefing. Returning closes
    // the board back into the briefing, not the workspace.
    KeyCode::Char('l' | 'L') => open_leaderboard(app, Phase::Briefing)?,
    _ => {},
  }
  Ok(false)
}

async fn workspace_keys(app: &mut App, key: KeyEvent) -> AdventResult<bool> {
  // 1. Direct focus toggle — F2 always swaps panes without going through the
  //    leader. Picked because F-keys never collide with nvim or terminal
  //    multiplexers like tmux/screen.
  if matches!(key.code, KeyCode::F(2)) {
    if let Some(ws) = app.workspace.as_mut()
      && let Err(e) = ws.toggle_focus()
    {
      app.fail(format!("pane resize failed: {e}"));
    }
    return Ok(false);
  }
  // F3 — toggle full-screen for the focused pane. Resizes the embedded child
  // immediately so vitest/nvim re-flow to the new width.
  if matches!(key.code, KeyCode::F(3)) {
    if let Some(ws) = app.workspace.as_mut()
      && let Err(e) = ws.toggle_zoom()
    {
      app.fail(format!("zoom resize failed: {e}"));
    }
    return Ok(false);
  }

  // 2. Resolve leader prefix. Either `Ctrl-a` (default) or `Ctrl-Space`
  //    (works inside tmux sessions that have Ctrl-a bound). Auto-clears after
  //    1s so a stray prefix doesn't eat the next real keystroke.
  if app.leader_pending {
    app.leader_pending = false;
    app.leader_deadline = None;
    return apply_leader_action(app, leader_action_for(key.code)).await;
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

  // 3. Tests pane is read-only. Navigation keys scroll the vt100 scrollback;
  //    everything else is silently swallowed (no stdin to vitest).
  if matches!(ws.focus, crate::workspace::Focus::Tests) {
    let _ = handle_tests_scroll(ws, key);
    return Ok(false);
  }

  // 4. Editor pane gets the raw key.
  if let Err(e) = ws.forward_key(key) {
    app.fail(format!("editor pty closed: {e}"));
  }
  Ok(false)
}

/// Run the effect for a `LeaderAction`. The key→action mapping lives in the
/// pure `leader_action_for`; this function is the impure half. Returns `true`
/// when the event loop should exit.
async fn apply_leader_action(app: &mut App, action: LeaderAction) -> AdventResult<bool> {
  match action {
    LeaderAction::ValidateNext => validate_and_celebrate(app)?,
    LeaderAction::Repeat => transition_to_quest(app, app.quest.slug.clone(), PendingIntent::Repeat).await?,
    LeaderAction::ToggleFocus => {
      if let Some(ws) = app.workspace.as_mut()
        && let Err(e) = ws.toggle_focus()
      {
        app.fail(format!("pane resize failed: {e}"));
      }
    },
    LeaderAction::ToggleZoom => {
      if let Some(ws) = app.workspace.as_mut()
        && let Err(e) = ws.toggle_zoom()
      {
        app.fail(format!("zoom resize failed: {e}"));
      }
    },
    LeaderAction::Hint => show_hint(app)?,
    LeaderAction::Briefing => {
      load_briefing(app).await?;
      app.phase = Phase::Briefing;
    },
    LeaderAction::Quit => {
      flush_session_timer(app);
      return Ok(true);
    },
    LeaderAction::PrevQuest => {
      if let Some(prev) = app.cfg.config.prev_before(&app.quest.slug).cloned() {
        transition_to_quest(app, prev.slug.clone(), PendingIntent::GotoPrev(prev.slug)).await?;
      }
    },
    LeaderAction::NextQuestNoValidate => {
      if let Some(next) = app.cfg.config.next_after(&app.quest.slug).cloned() {
        transition_to_quest(app, next.slug.clone(), PendingIntent::GotoNext(next.slug)).await?;
      }
    },
    LeaderAction::Leaderboard => open_leaderboard(app, Phase::Workspace)?,
    LeaderAction::Cancel | LeaderAction::Unknown => {},
  }
  Ok(false)
}

/// Bump the hint counter, refresh `app.progress` with the post-write state,
/// and stage the hint overlay text.
fn show_hint(app: &mut App) -> AdventResult<()> {
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
    // bump_hints returns the post-write ProgressState — keep our snapshot
    // synchronized with disk in one round-trip.
    app.progress = bump_hints(&app.cfg.repo_hash, &app.quest.slug)?;
  }
  app.phase = Phase::HintOverlay;
  Ok(())
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
    KeyCode::Char('n' | 'N') | KeyCode::Enter => advance_after_completion(app).await?,
    KeyCode::Char('r' | 'R') => {
      transition_to_quest(app, app.quest.slug.clone(), PendingIntent::Repeat).await?
    },
    // Stackable leaderboard. Dismissing the board returns to the celebrate
    // modal so the user can hit `n` to advance.
    KeyCode::Char('l' | 'L') => open_leaderboard(app, Phase::Celebrate)?,
    KeyCode::Char('q') | KeyCode::Esc => {
      flush_session_timer(app);
      return Ok(true);
    },
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

/// Single entry point for every quest change. Handles, in order:
///   1. Validate the target slug exists in config.
///   2. Verify the branch exists in git (fail-fast — no dirty-prompt
///      detour for nonexistent branches like the previous code did).
///   3. If staying on the same quest AND the same branch is already checked
///      out, short-circuit cheaply: discard workdir if this is a Repeat,
///      otherwise just re-enter the workspace.
///   4. If we need to switch branches and the working tree is dirty, stash
///      the [`PendingIntent`] so the dirty-prompt resolution replays this
///      exact action — never converts a backward jump into a forward one.
///   5. Flush the in-flight timer (so we never rob the user of seconds at a
///      transition), kill the old workspace, checkout, refresh progress,
///      respawn the workspace, reseed the session clock.
async fn transition_to_quest(app: &mut App, target_slug: String, intent: PendingIntent) -> AdventResult<()> {
  use advent_quest::git;
  // (1) Resolve the target quest from config — invalid slugs fail loud.
  let Some(target) = app.cfg.config.find_by_slug(&target_slug).cloned() else {
    app.fail(format!("quest {target_slug} not in config"));
    return Ok(());
  };

  // (2) Lock gate. A quest is unlocked iff it's the first OR its predecessor
  // (or the quest itself) is in `progress.completed`. Forward jumps and
  // explicit "go to slug" actions get gated. Repeat (same quest) and prev
  // (always going to an earlier — therefore already-unlocked — quest) are
  // exempt. AdvanceNext fires from the celebrate screen AFTER complete_quest
  // marked the predecessor done, so it naturally passes.
  let gate_required = matches!(
    intent,
    PendingIntent::AdvanceNext | PendingIntent::GotoNext(_) | PendingIntent::GotoQuest(_)
  );
  if gate_required && !app.cfg.config.is_unlocked(&target.slug, &app.progress.completed) {
    let prev_label = app
      .cfg
      .config
      .prev_before(&target.slug)
      .map(|p| format!("{:02} ({})", p.number, p.title))
      .unwrap_or_else(|| "the previous quest".into());
    // Soft advisory — the user just bumped the unlock gate. Keep them in
    // the workspace; they should finish the current quest then `<leader>
    // n` to advance naturally.
    app.notice(
      "Locked",
      format!("🔒 Quest {:02} — {} is locked.\n\nComplete quest {} first, then run `<leader> n` from there to advance.", target.number, target.title, prev_label),
    );
    return Ok(());
  }

  // (3) Branch existence — check BEFORE dirty-prompt. A missing branch is a
  // config/repo bug; popping a dirty prompt for it would be misleading.
  let current_branch = git::current_branch(&app.cfg.repo_root).await?;
  let needs_checkout = current_branch != target.slug;
  if needs_checkout && !git::branch_exists(&app.cfg.repo_root, &target.slug).await? {
    app.fail(format!("branch \"{}\" does not exist", target.slug));
    return Ok(());
  }

  // (3) Same-quest fast path. Repeat discards the workdir; everything else
  // just flips back into the existing workspace (preserves nvim buffers).
  if !needs_checkout && app.workspace.is_some() && app.quest.slug == target.slug {
    match intent {
      PendingIntent::Repeat => {
        flush_session_timer(app);
        // Wipe the per-attempt clock so this fresh attempt is timed cleanly.
        // Cumulative `elapsed_seconds` is untouched — the leaderboard's
        // "total time" view still reflects everything spent here.
        app.progress = reset_attempt(&app.cfg.repo_hash, &target.slug)?;
        app.last_completion = None;
        if let Some(ws) = app.workspace.as_mut() {
          ws.editor.kill();
          ws.tests.kill();
        }
        app.workspace = None;
        git::discard_workdir(&app.cfg.repo_root, &target.workdir).await?;
        spawn_workspace(app, target).await?;
      },
      _ => {
        app.phase = Phase::Workspace;
      },
    }
    app.pending_intent = None;
    return Ok(());
  }

  // (4) Dirty-tree guard — only when actually switching branches. Stash the
  // intent so the resolution path knows what to replay.
  if needs_checkout && !git::working_tree_clean(&app.cfg.repo_root).await? {
    app.pending_intent = Some(intent);
    app.phase = Phase::DirtyPrompt;
    return Ok(());
  }

  // (5) The happy path — flush, checkout, refresh, respawn, reseed.
  flush_session_timer(app);
  if needs_checkout {
    git::checkout(&app.cfg.repo_root, &target.slug).await?;
  }
  app.progress = set_current_quest(&app.cfg.repo_hash, &target.slug)?;
  app.quest = target.clone();
  // Quest changed — last completion belongs to the previous quest. Clear it
  // so a stale NEW BEST badge can't flash on the next celebrate.
  app.last_completion = None;
  load_briefing(app).await?;
  spawn_workspace(app, target).await?;
  // After landing in the workspace, pop the briefing overlay automatically
  // for explicit user-driven cross-quest transitions (advance from celebrate,
  // <leader> ] / [). The user dismisses with Enter/Esc to start coding.
  //
  // Skipped intents:
  //   - GotoQuest: programmatic — used by the very-first briefing dismiss
  //     and by Repeat-style same-quest re-enters. Showing the briefing
  //     again here would loop back into the briefing screen on first run.
  //   - Repeat: handled in step (3), same quest, no new briefing to read.
  if matches!(intent, PendingIntent::AdvanceNext | PendingIntent::GotoNext(_) | PendingIntent::GotoPrev(_)) {
    app.phase = Phase::Briefing;
  }
  app.pending_intent = None;
  Ok(())
}

/// Kill any existing workspace, spawn a fresh one for `quest`, reseed the
/// session timer from the persisted baseline. Used only by
/// `transition_to_quest`; not a public entry point.
async fn spawn_workspace(app: &mut App, quest: QuestStep) -> AdventResult<()> {
  drop(app.workspace.take());
  let (cols, rows) = crossterm::terminal::size().map_err(AdventError::BareIo)?;
  let area = Rect { x: 0, y: 0, width: cols, height: rows };
  let ws = Workspace::spawn(&app.cfg.repo_root, &app.cfg.config, &quest, area)
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
  // Manifest integrity check: when a manifest is present, every test file
  // recorded for this chapter must still hash to its recorded value. Any
  // drift means the user edited the spec — refuse to validate so they can't
  // turn the suite into `it.skip(...)` for a free completion.
  if let Some(manifest) = app.manifest.as_ref() {
    let drift = manifest
      .verify_chapter(&app.cfg.repo_root, &app.quest.slug)
      .map_err(|e| AdventError::ConfigParse(format!("manifest verify failed: {e}")))?;
    if !drift.is_empty() {
      app.notice(
        "Test file changed",
        format!(
          "🛑 {} test file(s) drifted from the integrity manifest:\n  {}\n\nRestore originals: `git checkout HEAD -- <path>`\nOr regenerate the manifest if the change was intentional: `duck-advent manifest gen`",
          drift.len(),
          drift.join("\n  ")
        ),
      );
      return Ok(());
    }
  }

  // bump_attempts now returns the full ProgressState — keep our snapshot in
  // sync with disk so the attempt count rendered on the celebrate screen
  // reflects this run, not the previous one.
  app.progress = bump_attempts(&app.cfg.repo_hash, &app.quest.slug)?;
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
  // Tests passed. Flush any final session seconds BEFORE complete_quest
  // writes so this attempt's `attempt_elapsed_seconds` reflects the full
  // active time, not just what the last periodic tick captured.
  flush_session_timer(app);
  let secret = app.manifest.as_ref().map(|m| m.secret.as_str());
  let (state, outcome) = complete_quest(&app.cfg.repo_hash, &app.quest.slug, secret)?;
  app.progress = state;
  app.last_completion = Some(outcome);
  app.is_last_quest = app.cfg.config.next_after(&app.quest.slug).is_none();
  // Prefer the just-recorded attempt time — that's what the user actually
  // experienced for this solve. Fall back to cumulative for legacy entries.
  app.celebrate_secs = if outcome.attempt_seconds > 0 {
    outcome.attempt_seconds
  } else {
    duration_for(&app.progress, &app.quest.slug)
  };
  // Freeze the live workspace timer — the user is now on the celebrate
  // modal, not actively working. Any seconds spent looking at the modal
  // should NOT count toward the cumulative `elapsed_seconds`.
  app.quest_session_start = None;
  app.session_flushed_secs = 0;
  app.phase = Phase::Celebrate;
  Ok(())
}

/// `Enter` / `n` on the celebrate screen. Walks to the next quest or lands
/// on Complete when there is no next.
async fn advance_after_completion(app: &mut App) -> AdventResult<()> {
  let Some(next) = app.cfg.config.next_after(&app.quest.slug).cloned() else {
    flush_session_timer(app);
    app.phase = Phase::Complete;
    return Ok(());
  };
  transition_to_quest(app, next.slug.clone(), PendingIntent::AdvanceNext).await
}

/// Apply the user's dirty-tree choice, then replay whatever intent triggered
/// the prompt. Previously:
///   1. hard-coded `advance_quest` — backward `<leader> [` got converted into
///      a forward jump after the user picked Commit;
///   2. stash + discard operated only on the quest's workdir, so any edit
///      outside it (nvim swap, scratch files, cross-cutting fixes) survived
///      the resolution; the next `working_tree_clean` check failed and we
///      bounced right back into DirtyPrompt — looking to the user like the
///      action just lagged or did nothing.
/// All three resolutions now operate on the FULL working tree, and the post-
/// resolution path asserts the tree is clean before replaying — if it isn't
/// (e.g. git refused to stash, weird permissions), we surface a real error
/// instead of an infinite prompt loop.
async fn apply_dirty_choice(app: &mut App, choice: DirtyChoice) -> AdventResult<()> {
  let repo = app.cfg.repo_root.clone();
  let msg = format!("duck-advent: snapshot before leaving quest {:02} {}", app.quest.number, app.quest.title);
  app.phase = Phase::Working;
  app.working_msg = match choice {
    DirtyChoice::Commit => "committing edits…".into(),
    DirtyChoice::Stash => "stashing edits…".into(),
    DirtyChoice::Discard => "discarding edits…".into(),
  };
  match choice {
    DirtyChoice::Commit => {
      run_git(&repo, &["add", "-A"]).await?;
      ensure_git_identity(&repo).await?;
      run_git(&repo, &["commit", "-m", &msg, "--allow-empty", "--no-verify"]).await?;
    },
    DirtyChoice::Stash => {
      // `-u` includes untracked. Full tree (no `-- <pathspec>`) so the
      // resolution actually clears working_tree_clean for the next check.
      run_git(&repo, &["stash", "push", "-u", "-m", &msg]).await?;
    },
    DirtyChoice::Discard => {
      // Hard reset to HEAD wipes tracked changes; `clean -fd` removes
      // untracked files + dirs. Both restricted to the repo root.
      run_git(&repo, &["reset", "--hard", "HEAD"]).await?;
      run_git(&repo, &["clean", "-fd"]).await?;
    },
  }

  // Loop-guard: if the chosen action did not actually clean the tree (git
  // refused for some reason — wrong branch, locked index, etc.) we'd
  // otherwise bounce straight back into DirtyPrompt and look like a lag.
  // Fail explicitly so the user sees the real cause.
  if !advent_quest::git::working_tree_clean(&repo).await? {
    app.pending_intent = None;
    app.fail(format!("`{choice:?}` did not clean the working tree — run `git status` to inspect"));
    return Ok(());
  }

  // Replay the original intent against the cleaned tree. Defensive fallback
  // to AdvanceNext if the intent vanished (shouldn't happen).
  let intent = app.pending_intent.take().unwrap_or(PendingIntent::AdvanceNext);
  replay_intent(app, intent).await
}

async fn replay_intent(app: &mut App, intent: PendingIntent) -> AdventResult<()> {
  match intent {
    PendingIntent::AdvanceNext => {
      let next = app.cfg.config.next_after(&app.quest.slug).cloned();
      match next {
        Some(n) => transition_to_quest(app, n.slug.clone(), PendingIntent::AdvanceNext).await,
        None => {
          flush_session_timer(app);
          app.phase = Phase::Complete;
          Ok(())
        },
      }
    },
    PendingIntent::GotoNext(slug) => transition_to_quest(app, slug.clone(), PendingIntent::GotoNext(slug)).await,
    PendingIntent::GotoPrev(slug) => transition_to_quest(app, slug.clone(), PendingIntent::GotoPrev(slug)).await,
    PendingIntent::GotoQuest(slug) => transition_to_quest(app, slug.clone(), PendingIntent::GotoQuest(slug)).await,
    PendingIntent::Repeat => transition_to_quest(app, app.quest.slug.clone(), PendingIntent::Repeat).await,
  }
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
/// flush watermark forward. Safe (and cheap — no disk IO when delta=0) to
/// call from any boundary that ends a session.
fn flush_session_timer(app: &mut App) {
  let Some(started) = app.quest_session_start else {
    return;
  };
  let session_secs = started.elapsed().as_secs();
  let delta = session_secs.saturating_sub(app.session_flushed_secs);
  if delta == 0 {
    return;
  }
  if let Ok(state) = add_elapsed(&app.cfg.repo_hash, &app.quest.slug, delta) {
    app.session_flushed_secs = session_secs;
    app.progress = state;
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
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::briefing::draw(frame, area, &app.quest, &app.briefing_md, app.briefing_scroll);
    },
    Phase::Workspace => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
    },
    Phase::HintOverlay => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::hint::draw(frame, area, &app.hint_text, app.hint_index, app.quest.hints.len());
    },
    Phase::RunningTests => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
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
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      let kind = app.pending_intent.as_ref().map(PendingIntent::dirty_kind).unwrap_or(DirtyPromptKind::AdvanceForward);
      screens::dirty::draw(frame, area, &app.quest.title, kind);
    },
    Phase::Working => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::working::draw(frame, area, &app.working_msg, (app.test_spin & 0xFF) as u8);
    },
    Phase::Celebrate => {
      let set_new_best = app.last_completion.map(|o| o.set_new_best).unwrap_or(false);
      let best_seconds = app
        .last_completion
        .and_then(|o| o.best_seconds)
        .or_else(|| app.progress.quests.get(&app.quest.slug).and_then(|q| q.best_time_seconds));
      let view = screens::celebrate::CelebrateView {
        quest: &app.quest,
        confetti: &app.confetti,
        hints_used: app.hints_for(&app.quest.slug),
        attempts: app.attempts_for(&app.quest.slug),
        duration_secs: app.celebrate_secs,
        is_last: app.is_last_quest,
        set_new_best,
        best_seconds,
      };
      screens::celebrate::draw(frame, area, &view);
    },
    Phase::Leaderboard => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::leaderboard::draw(frame, area, &app.cfg.config.quests, &app.progress, &app.quest.slug);
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
    Phase::Notice => {
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::notice::draw(frame, area, &app.notice_title, &app.notice_msg);
    },
    Phase::Error => {
      // Even fatal errors should keep the workspace visible underneath so
      // the user has context for what was on screen when things broke.
      if let Some(ws) = app.workspace.as_ref() {
        let hints = app.hints_for(&app.quest.slug);
        ws.draw(
          frame,
          area,
          &crate::workspace::WorkspaceView {
            quest: &app.quest,
            total: app.cfg.config.quests.len(),
            hints_used: hints,
            leader_pending: app.leader_pending,
            elapsed_secs: app.quest_elapsed_secs(),
          },
        );
      }
      screens::error::draw(frame, area, &app.error_msg);
    },
  }
}

// ---------- tests --------------------------------------------------------

#[cfg(test)]
mod tests {
  //! Pure-logic tests only. Anything that touches git, pty, ratatui, the
  //! filesystem, or `~/.gentleduck` belongs in the integration-test crate
  //! `advent-cache/tests/` or as an end-to-end harness.

  use super::*;

  #[test]
  fn leader_action_table_covers_every_documented_chord() {
    // Lowercase chord set documented in the hint bar.
    assert_eq!(leader_action_for(KeyCode::Char('n')), LeaderAction::ValidateNext);
    assert_eq!(leader_action_for(KeyCode::Char('r')), LeaderAction::Repeat);
    assert_eq!(leader_action_for(KeyCode::Char('b')), LeaderAction::ToggleFocus);
    assert_eq!(leader_action_for(KeyCode::Char('z')), LeaderAction::ToggleZoom);
    assert_eq!(leader_action_for(KeyCode::Char('h')), LeaderAction::Hint);
    assert_eq!(leader_action_for(KeyCode::Char('p')), LeaderAction::Briefing);
    assert_eq!(leader_action_for(KeyCode::Char('q')), LeaderAction::Quit);
    assert_eq!(leader_action_for(KeyCode::Char('l')), LeaderAction::Leaderboard);
    assert_eq!(leader_action_for(KeyCode::Char('[')), LeaderAction::PrevQuest);
    assert_eq!(leader_action_for(KeyCode::Char(']')), LeaderAction::NextQuestNoValidate);
    assert_eq!(leader_action_for(KeyCode::Char('c')), LeaderAction::Cancel);
    assert_eq!(leader_action_for(KeyCode::Esc), LeaderAction::Cancel);
  }

  #[test]
  fn leader_action_uppercase_aliases_match_lowercase() {
    for (lower, upper) in
      [('n', 'N'), ('r', 'R'), ('b', 'B'), ('z', 'Z'), ('h', 'H'), ('p', 'P'), ('q', 'Q'), ('l', 'L'), ('c', 'C')]
    {
      assert_eq!(
        leader_action_for(KeyCode::Char(lower)),
        leader_action_for(KeyCode::Char(upper)),
        "uppercase {upper} should alias to lowercase {lower}"
      );
    }
  }

  #[test]
  fn leader_action_unknown_for_unbound_keys() {
    assert_eq!(leader_action_for(KeyCode::Char('x')), LeaderAction::Unknown);
    assert_eq!(leader_action_for(KeyCode::Tab), LeaderAction::Unknown);
    assert_eq!(leader_action_for(KeyCode::F(5)), LeaderAction::Unknown);
  }

  #[test]
  fn duration_for_prefers_elapsed_seconds() {
    use chrono::{Duration as ChronoDuration, Utc};
    let mut state = ProgressState::empty();
    let q = state.ensure_quest("slug");
    q.elapsed_seconds = 42;
    q.started_at = Some(Utc::now() - ChronoDuration::seconds(9999));
    q.completed_at = Some(Utc::now());
    // elapsed_seconds wins over the wall-clock fallback.
    assert_eq!(duration_for(&state, "slug"), 42);
  }

  #[test]
  fn duration_for_falls_back_to_wallclock_when_elapsed_zero() {
    use chrono::{Duration as ChronoDuration, Utc};
    let mut state = ProgressState::empty();
    let now = Utc::now();
    let q = state.ensure_quest("slug");
    q.elapsed_seconds = 0;
    q.started_at = Some(now - ChronoDuration::seconds(120));
    q.completed_at = Some(now);
    let secs = duration_for(&state, "slug");
    // Allow 2s slack — Utc::now() is called twice across the helper.
    assert!((118..=122).contains(&secs), "expected ~120s, got {secs}");
  }

  #[test]
  fn duration_for_returns_zero_when_quest_absent() {
    let state = ProgressState::empty();
    assert_eq!(duration_for(&state, "nonexistent"), 0);
  }

  #[test]
  fn pending_intent_round_trips_through_clone() {
    let intents = [
      PendingIntent::AdvanceNext,
      PendingIntent::GotoQuest("chapter-03".into()),
      PendingIntent::Repeat,
    ];
    for i in &intents {
      assert_eq!(i.clone(), *i);
    }
  }
}
