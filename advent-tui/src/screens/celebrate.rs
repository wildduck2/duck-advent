use super::{MODAL_BG, full_modal_area};
use crate::confetti::Confetti;
use advent_core::QuestStep;
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub struct CelebrateView<'a> {
  pub quest: &'a QuestStep,
  pub confetti: &'a Confetti,
  pub hints_used: u32,
  pub attempts: u32,
  pub duration_secs: u64,
  pub is_last: bool,
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, view: &CelebrateView<'_>) {
  let outer = full_modal_area(area, 4, 1);
  frame.render_widget(Clear, outer);
  let bg = Style::default().bg(MODAL_BG);
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(Color::Green).bg(MODAL_BG))
    .style(bg)
    .title(" 🎉 quest complete ")
    .title_style(Style::default().fg(Color::Green).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = block.inner(outer);
  frame.render_widget(block, outer);

  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(0), Constraint::Length(8)])
    .split(inner);
  frame.render_widget(Paragraph::new(view.confetti.render()).style(bg), rows[0]);

  let mins = view.duration_secs / 60;
  let secs = view.duration_secs % 60;
  let footer = vec![
    Line::raw(""),
    Line::styled(
      format!("QUEST {:02} COMPLETE", view.quest.number),
      Style::default().fg(Color::Green).bg(MODAL_BG).add_modifier(Modifier::BOLD),
    ),
    Line::styled(view.quest.title.clone(), Style::default().fg(Color::LightCyan).bg(MODAL_BG)),
    Line::raw(""),
    Line::from(vec![
      Span::styled("  hints ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{}", view.hints_used), Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled("  · attempts ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{}", view.attempts), Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled("  · time ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{mins}m {secs}s"), Style::default().fg(Color::White).bg(MODAL_BG)),
    ]),
    Line::raw(""),
    Line::from(vec![
      Span::styled(" n ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
      Span::styled(
        if view.is_last { "  finish" } else { "  next quest" },
        Style::default().fg(Color::White).bg(MODAL_BG),
      ),
      Span::styled("   ", Style::default().bg(MODAL_BG)),
      Span::styled(" r ", Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)),
      Span::styled("  repeat", Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled("   ", Style::default().bg(MODAL_BG)),
      Span::styled(" q ", Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
      Span::styled("  quit", Style::default().fg(Color::White).bg(MODAL_BG)),
    ]),
  ];
  frame.render_widget(Paragraph::new(footer).style(bg).alignment(Alignment::Center), rows[1]);
}
