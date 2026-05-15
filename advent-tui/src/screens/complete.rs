use super::{MODAL_BG, full_modal_area};
use crate::confetti::Confetti;
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, name: &str, confetti: &Confetti, totals: (usize, u32, u64)) {
  let outer = full_modal_area(area, 4, 1);
  frame.render_widget(Clear, outer);
  let bg = Style::default().bg(MODAL_BG);
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(Color::Magenta).bg(MODAL_BG))
    .style(bg)
    .title(" 🌟 journey complete ")
    .title_style(Style::default().fg(Color::Magenta).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = block.inner(outer);
  frame.render_widget(block, outer);

  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(0), Constraint::Length(8)])
    .split(inner);
  frame.render_widget(Paragraph::new(confetti.render()).style(bg), rows[0]);

  let (chapters, hints, dur) = totals;
  let mins = dur / 60;
  let secs = dur % 60;
  let footer = vec![
    Line::raw(""),
    Line::styled(
      "JOURNEY COMPLETE",
      Style::default().fg(Color::Magenta).bg(MODAL_BG).add_modifier(Modifier::BOLD),
    ),
    Line::styled(name.to_string(), Style::default().fg(Color::LightCyan).bg(MODAL_BG)),
    Line::raw(""),
    Line::from(vec![
      Span::styled("  quests ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{chapters}/{chapters}"), Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled("  · hints ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{hints}"), Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled("  · total time ", Style::default().bg(MODAL_BG)),
      Span::styled(format!("{mins}m {secs}s"), Style::default().fg(Color::White).bg(MODAL_BG)),
    ]),
    Line::raw(""),
    Line::styled("⏎/q to exit", Style::default().fg(Color::DarkGray).bg(MODAL_BG)),
  ];
  frame.render_widget(Paragraph::new(footer).style(bg).alignment(Alignment::Center), rows[1]);
}
