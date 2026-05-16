use super::{centered, modal};
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Style},
  text::Line,
};

/// Soft, recoverable advisory modal. Drawn ON TOP of the workspace (so the
/// user keeps context). Dismissed with any key. Use this for lock gating,
/// manifest drift, hint exhaustion, and anything else the user can fix
/// in-place — reserve [`crate::screens::error::draw`] for fatal errors that
/// abort the session.
pub fn draw(frame: &mut Frame<'_>, area: Rect, title: &str, message: &str) {
  let mut body = vec![Line::raw("")];
  for line in message.lines() {
    body.push(Line::styled(line.to_string(), Style::default().fg(Color::White)));
  }
  body.push(Line::raw(""));
  body.push(Line::styled("press any key to dismiss", Style::default().fg(Color::DarkGray)));
  let lines = body.len() as u16 + 2;
  let h = lines.clamp(8, area.height.saturating_sub(4).max(8));
  let w = area.width.saturating_sub(8).min(90);
  modal(frame, centered(area, w, h), title, Color::Yellow, body);
}
