use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame<'_>, area: Rect, msg: &str, spin: u8) {
  let frame_idx = (spin as usize) % SPINNER.len();
  let body = vec![
    Line::raw(""),
    Line::raw(""),
    Line::from(vec![
      Span::styled(
        SPINNER[frame_idx].to_string(),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
      ),
      Span::raw("  "),
      Span::styled(msg.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ])
    .alignment(ratatui::layout::Alignment::Center),
    Line::raw(""),
  ];
  modal(frame, centered(area, 60, 7), "Working", Color::Cyan, body);
}
