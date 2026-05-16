use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

/// What the user wanted to do when the dirty-tree prompt popped. Drives the
/// language inside the modal so a backward jump doesn't read "switching to
/// the next quest" and a repeat doesn't read "Quest complete".
#[derive(Clone, Copy, Debug)]
pub enum DirtyPromptKind {
  /// `<leader> n` (or Enter on celebrate) — the user just passed the quest
  /// and is heading forward to the next one.
  AdvanceForward,
  /// `<leader> ]` — explicit forward jump without test validation.
  GoForward,
  /// `<leader> [` — explicit backward jump.
  GoBackward,
  /// `<leader> r` — restart the current quest.
  Repeat,
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, quest_title: &str, kind: DirtyPromptKind) {
  let (header_label, header_value, action_word) = match kind {
    DirtyPromptKind::AdvanceForward => (
      "Quest complete: ",
      quest_title.to_string(),
      "advance to the next quest",
    ),
    DirtyPromptKind::GoForward => ("Jumping forward from: ", quest_title.to_string(), "switch to the next quest"),
    DirtyPromptKind::GoBackward => ("Jumping back from: ", quest_title.to_string(), "switch to the previous quest"),
    DirtyPromptKind::Repeat => ("Restarting: ", quest_title.to_string(), "restart this quest"),
  };
  let header_color = match kind {
    DirtyPromptKind::AdvanceForward => Color::Green,
    DirtyPromptKind::GoForward => Color::Cyan,
    DirtyPromptKind::GoBackward => Color::Blue,
    DirtyPromptKind::Repeat => Color::Magenta,
  };
  let body = vec![
    Line::raw(""),
    Line::from(vec![
      Span::styled(header_label, Style::default().fg(header_color).add_modifier(Modifier::BOLD)),
      Span::styled(header_value, Style::default().fg(Color::White)),
    ]),
    Line::raw(""),
    Line::raw("You have uncommitted edits in the working tree."),
    Line::from(vec![Span::raw("Pick how to clean up before we "), Span::styled(action_word, Style::default().fg(header_color)), Span::raw(":")]),
    Line::raw(""),
    Line::from(vec![
      Span::styled(" c ", Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD)),
      Span::raw(" commit edits (stages everything, makes a local commit)"),
    ]),
    Line::from(vec![
      Span::styled(" s ", Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::raw(" stash edits (recover later with `git stash pop`)"),
    ]),
    Line::from(vec![
      Span::styled(" d ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::raw(" discard edits (irreversible)"),
    ]),
    Line::from(vec![
      Span::styled(" x / esc ", Style::default().fg(Color::DarkGray)),
      Span::raw(" cancel (stay in this quest)"),
    ]),
  ];
  modal(frame, centered(area, 78, 15), "Uncommitted changes", Color::Yellow, body);
}
