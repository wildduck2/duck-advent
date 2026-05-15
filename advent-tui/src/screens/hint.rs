use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::Line,
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, hint: &str, index: usize, total: usize) {
  let lines = vec![
    Line::raw(""),
    Line::styled(
      format!("Hint {}/{}", index + 1, total),
      Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ),
    Line::raw(""),
    Line::raw(hint.to_string()),
    Line::raw(""),
    Line::styled("⏎/esc to dismiss", Style::default().fg(Color::DarkGray)),
  ];
  modal(frame, centered(area, 70, 9), "Hint", Color::Yellow, lines);
}
