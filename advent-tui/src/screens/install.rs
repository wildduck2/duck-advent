use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, install_cmd: &[String], tail: &[String], status: &str) {
  let mut lines: Vec<Line<'static>> = vec![Line::raw("")];
  lines.push(Line::from(vec![
    Span::styled(status.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    Span::raw(" · "),
    Span::styled(install_cmd.join(" "), Style::default().fg(Color::White)),
  ]));
  lines.push(Line::raw(""));
  for l in tail {
    lines.push(Line::styled(l.clone(), Style::default().fg(Color::DarkGray)));
  }
  modal(frame, centered(area, 80, 16), "Installing dependencies", Color::Cyan, lines);
}
