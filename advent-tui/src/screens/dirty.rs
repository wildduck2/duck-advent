use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, quest_title: &str) {
  let body = vec![
    Line::raw(""),
    Line::from(vec![
      Span::styled("Quest complete: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
      Span::styled(quest_title.to_string(), Style::default().fg(Color::White)),
    ]),
    Line::raw(""),
    Line::raw("You have uncommitted edits in the workdir."),
    Line::raw("Pick how to clean up before switching to the next quest:"),
    Line::raw(""),
    Line::from(vec![
      Span::styled(" c ", Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD)),
      Span::raw(" commit edits + advance"),
    ]),
    Line::from(vec![
      Span::styled(" s ", Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::raw(" stash edits + advance (recover with `git stash pop`)"),
    ]),
    Line::from(vec![
      Span::styled(" d ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::raw(" discard edits + advance"),
    ]),
    Line::from(vec![
      Span::styled(" x / esc ", Style::default().fg(Color::DarkGray)),
      Span::raw(" cancel (stay in this quest)"),
    ]),
  ];
  modal(frame, centered(area, 70, 14), "Uncommitted changes", Color::Yellow, body);
}
