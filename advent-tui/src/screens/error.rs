use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Style},
  text::Line,
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, message: &str) {
  let mut body = vec![Line::raw("")];
  for line in message.lines() {
    body.push(Line::styled(line.to_string(), Style::default().fg(Color::White)));
  }
  body.push(Line::raw(""));
  body.push(Line::styled("press any key to exit", Style::default().fg(Color::DarkGray)));
  let lines = body.len() as u16 + 2;
  let h = lines.clamp(8, area.height.saturating_sub(4).max(8));
  modal(frame, centered(area, area.width.saturating_sub(8).min(120), h), "Error", Color::Red, body);
}
