//! 70/30 workspace: nvim (editor) + test runner (vitest et al). Both run as
//! `PtyPane`s; we forward all keystrokes to whichever pane is focused.

use crate::nvim;
use advent_core::{QuestConfig, QuestStep};
use advent_pty::{PtyPane, PtyView, encode_key};
use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
  Frame,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Paragraph},
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub enum Focus {
  Editor,
  Tests,
}

/// View-model bundle for [`Workspace::draw`]. Folds the seven render
/// parameters into one struct so callers don't have to thread a long
/// positional argument list and clippy stops flagging too_many_arguments.
pub struct WorkspaceView<'a> {
  pub quest: &'a QuestStep,
  pub total: usize,
  pub hints_used: u32,
  pub leader_pending: bool,
  pub elapsed_secs: u64,
}

impl Focus {
  pub fn toggle(self) -> Self {
    match self {
      Self::Editor => Self::Tests,
      Self::Tests => Self::Editor,
    }
  }
}

pub struct Workspace {
  pub editor: PtyPane,
  pub tests: PtyPane,
  pub focus: Focus,
  /// When true the focused pane fills the full pane area; the other is hidden.
  /// Toggled via `F3` or `<leader> z`.
  pub zoomed: bool,
  last_area: Rect,
  // Stored so we can respawn the tests child at a new column width when the
  // pane is zoomed/un-zoomed — vitest only re-flows its watch output on
  // process start, so a SIGWINCH alone is not enough.
  repo_root: PathBuf,
  config: QuestConfig,
  quest: QuestStep,
}

impl Workspace {
  pub fn spawn(repo_root: &Path, config: &QuestConfig, quest: &QuestStep, area: Rect) -> Result<Self> {
    let (editor_rect, tests_rect) = split(area);
    let workdir = repo_root.join(&quest.workdir);
    let editor_argv = nvim::argv(&workdir);
    let editor = PtyPane::spawn(
      "nvim",
      &editor_argv,
      repo_root,
      editor_rect.height.saturating_sub(2).max(1),
      editor_rect.width.saturating_sub(2).max(1),
      &[],
    )?;

    let test_argv = advent_quest::tests::watch_argv(config, quest);
    let (bin, args) = test_argv.split_first().expect("test_command was validated by config schema");
    let tests = PtyPane::spawn(
      bin,
      args,
      repo_root,
      tests_rect.height.saturating_sub(2).max(1),
      tests_rect.width.saturating_sub(2).max(1),
      &[],
    )?;

    Ok(Self {
      editor,
      tests,
      focus: Focus::Editor,
      zoomed: false,
      last_area: area,
      repo_root: repo_root.to_path_buf(),
      config: config.clone(),
      quest: quest.clone(),
    })
  }

  /// Kill and re-spawn the vitest child at the current pane size. Used when
  /// zoom/focus changes the visible width — vitest's watch UI is rendered
  /// once at startup and never reflowed afterwards, so a fresh process is the
  /// only way to redraw cleanly at the new width.
  fn respawn_tests(&mut self, rows: u16, cols: u16) -> Result<()> {
    let argv = advent_quest::tests::watch_argv(&self.config, &self.quest);
    let (bin, args) = argv.split_first().expect("test_command was validated by config schema");
    let fresh = PtyPane::spawn(bin, args, &self.repo_root, rows.max(4), cols.max(20), &[])?;
    // Drop the old pane (which kills its child in Drop) only after the new
    // one is alive, so we never sit with a dead tests pane on a transient
    // spawn error.
    let old = std::mem::replace(&mut self.tests, fresh);
    drop(old);
    Ok(())
  }

  pub fn relayout(&mut self, area: Rect) -> Result<()> {
    self.last_area = area;
    if self.zoomed {
      // Focused pane takes the inner area (minus our status/hint rows + border).
      let inner = inner_panes_area(area);
      let r = inner.height.saturating_sub(2).max(1);
      let c = inner.width.saturating_sub(2).max(1);
      match self.focus {
        Focus::Editor => self.editor.resize(r, c)?,
        Focus::Tests => self.tests.resize(r, c)?,
      }
      return Ok(());
    }
    let (editor_rect, tests_rect) = split(area);
    self.editor.resize(editor_rect.height.saturating_sub(2).max(1), editor_rect.width.saturating_sub(2).max(1))?;
    self.tests.resize(tests_rect.height.saturating_sub(2).max(1), tests_rect.width.saturating_sub(2).max(1))?;
    Ok(())
  }

  /// Toggle full-screen for the focused pane. Resizes the affected pane(s) so
  /// the embedded child re-flows immediately.
  pub fn toggle_zoom(&mut self) -> Result<()> {
    self.zoomed = !self.zoomed;
    let area = self.last_area;
    self.relayout(area)?;
    self.respawn_tests_for_current_layout()
  }

  /// Swap focus. Auto-zooms the tests pane on entry (the pane is narrow in the
  /// 70/30 split, so anything beyond a short line wraps ugly); restores the
  /// split when focus goes back to the editor. Respawns vitest at the new
  /// width so the output redraws cleanly.
  pub fn toggle_focus(&mut self) -> Result<()> {
    self.focus = self.focus.toggle();
    self.zoomed = matches!(self.focus, Focus::Tests);
    let area = self.last_area;
    self.relayout(area)?;
    self.respawn_tests_for_current_layout()
  }

  fn respawn_tests_for_current_layout(&mut self) -> Result<()> {
    let inner = inner_panes_area(self.last_area);
    let (rows, cols) = if self.zoomed {
      (inner.height.saturating_sub(2).max(1), inner.width.saturating_sub(2).max(1))
    } else {
      let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);
      let r = panes[1];
      (r.height.saturating_sub(2).max(1), r.width.saturating_sub(2).max(1))
    };
    self.respawn_tests(rows, cols)
  }

  pub fn forward_key(&self, key: KeyEvent) -> Result<()> {
    let bytes = encode_key(key);
    match self.focus {
      Focus::Editor => self.editor.write_input(&bytes),
      Focus::Tests => self.tests.write_input(&bytes),
    }
  }

  pub fn draw(&self, frame: &mut Frame<'_>, area: Rect, view: &WorkspaceView<'_>) {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
      .split(area);
    self.draw_status(frame, chunks[0], view);
    self.draw_panes(frame, chunks[1]);
    self.draw_hints(frame, chunks[2], view.leader_pending);
  }

  fn draw_status(&self, frame: &mut Frame<'_>, area: Rect, view: &WorkspaceView<'_>) {
    let mut spans = vec![
      Span::styled("  duck ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
      Span::styled("· ", Style::default().fg(Color::DarkGray)),
      Span::styled(
        format!("Quest {:02}/{:02}", view.quest.number, view.total),
        Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
      ),
      Span::raw("  "),
      Span::styled(&view.quest.title, Style::default().fg(Color::White)),
      Span::raw("  "),
      Span::styled(view.quest.tier.clone().unwrap_or_default(), Style::default().fg(Color::DarkGray)),
      Span::raw("  · hints "),
      Span::styled(format!("{}/{}", view.hints_used, view.quest.hints.len()), Style::default().fg(Color::Yellow)),
      Span::raw("  · "),
      Span::styled(format_elapsed(view.elapsed_secs), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
    ];
    if view.leader_pending {
      spans.push(Span::raw("  "));
      spans.push(Span::styled(
        " LEADER ",
        Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD),
      ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
  }

  fn draw_panes(&self, frame: &mut Frame<'_>, area: Rect) {
    let editor_alive = self.editor.is_alive();
    let tests_alive = self.tests.is_alive();
    if self.zoomed {
      let zoom_tag = " · zoomed ";
      match self.focus {
        Focus::Editor => {
          self.draw_pane(frame, area, &format!(" editor{zoom_tag}"), true, editor_alive, &self.editor)
        },
        Focus::Tests => {
          self.draw_pane(frame, area, &format!(" tests{zoom_tag}"), true, tests_alive, &self.tests)
        },
      }
      return;
    }
    let panes = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
      .split(area);
    self.draw_pane(frame, panes[0], " editor ", matches!(self.focus, Focus::Editor), editor_alive, &self.editor);
    self.draw_pane(frame, panes[1], " tests ", matches!(self.focus, Focus::Tests), tests_alive, &self.tests);
  }

  fn draw_pane(
    &self,
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    focused: bool,
    alive: bool,
    pane: &PtyPane,
  ) {
    let color = match (focused, alive) {
      (_, false) => Color::Red,
      (true, true) => Color::Cyan,
      (false, true) => Color::DarkGray,
    };
    let status = if alive { "" } else { " · exited" };
    let block = Block::default()
      .borders(Borders::ALL)
      .border_style(Style::default().fg(color))
      .title(format!("{title}{status}"))
      .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(PtyView(pane), inner);
  }

  fn draw_hints(&self, frame: &mut Frame<'_>, area: Rect, leader_pending: bool) {
    let line = if leader_pending {
      Line::from(vec![
        Span::styled(" LEADER ", Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::raw(" press · "),
        Span::styled("n ", Style::default().fg(Color::Green)),
        Span::raw("next · "),
        Span::styled("r ", Style::default().fg(Color::Magenta)),
        Span::raw("repeat · "),
        Span::styled("b ", Style::default().fg(Color::Cyan)),
        Span::raw("switch · "),
        Span::styled("[ ", Style::default().fg(Color::Blue)),
        Span::raw("prev · "),
        Span::styled("] ", Style::default().fg(Color::Blue)),
        Span::raw("next · "),
        Span::styled("z ", Style::default().fg(Color::Magenta)),
        Span::raw("zoom · "),
        Span::styled("h ", Style::default().fg(Color::Yellow)),
        Span::raw("hint · "),
        Span::styled("p ", Style::default().fg(Color::LightCyan)),
        Span::raw("briefing · "),
        Span::styled("q ", Style::default().fg(Color::Red)),
        Span::raw("quit · "),
        Span::styled("esc ", Style::default().fg(Color::DarkGray)),
        Span::raw("cancel"),
      ])
    } else {
      Line::from(vec![
        Span::styled(" F2 ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::raw(" switch · "),
        Span::styled(" F3 ", Style::default().bg(Color::Magenta).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::raw(" zoom · "),
        Span::styled(" ⌃a ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::raw("/"),
        Span::styled(" ⌃␣ ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::raw(" leader · "),
        Span::styled(" j/k g G ", Style::default().fg(Color::White).bg(Color::DarkGray)),
        Span::raw(" scroll tests · "),
        Span::styled("⌃q ", Style::default().fg(Color::Red)),
        Span::raw("quit"),
      ])
    };
    frame.render_widget(Paragraph::new(line), area);
  }
}

/// Format a quest timer. Uses `mm:ss` under one hour and `h:mm:ss` above so
/// the status bar stays narrow on short quests but never truncates on long ones.
fn format_elapsed(total_secs: u64) -> String {
  let h = total_secs / 3600;
  let m = (total_secs % 3600) / 60;
  let s = total_secs % 60;
  if h > 0 { format!("⏱ {h}:{m:02}:{s:02}") } else { format!("⏱ {m:02}:{s:02}") }
}

fn split(area: Rect) -> (Rect, Rect) {
  let inner = inner_panes_area(area);
  let panes = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
    .split(inner);
  (panes[0], panes[1])
}

/// The middle stripe of the workspace (between status bar and hint bar) — the
/// region the panes themselves are drawn into.
fn inner_panes_area(area: Rect) -> Rect {
  let inner = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
    .split(area);
  inner[1]
}

#[cfg(test)]
mod tests {
  use super::format_elapsed;

  #[test]
  fn format_elapsed_under_one_minute() {
    assert_eq!(format_elapsed(0), "⏱ 00:00");
    assert_eq!(format_elapsed(7), "⏱ 00:07");
    assert_eq!(format_elapsed(59), "⏱ 00:59");
  }

  #[test]
  fn format_elapsed_minutes_pad_two_digits() {
    assert_eq!(format_elapsed(60), "⏱ 01:00");
    assert_eq!(format_elapsed(125), "⏱ 02:05");
    assert_eq!(format_elapsed(3599), "⏱ 59:59");
  }

  #[test]
  fn format_elapsed_switches_to_hour_format_at_one_hour() {
    assert_eq!(format_elapsed(3600), "⏱ 1:00:00");
    assert_eq!(format_elapsed(3661), "⏱ 1:01:01");
    assert_eq!(format_elapsed(7325), "⏱ 2:02:05");
  }

  #[test]
  fn format_elapsed_handles_very_large_values() {
    // 100 hours — should not truncate or panic.
    let s = format_elapsed(360_000);
    assert_eq!(s, "⏱ 100:00:00");
  }
}
