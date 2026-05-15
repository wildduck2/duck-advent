pub mod briefing;
pub mod celebrate;
pub mod complete;
pub mod dirty;
pub mod error;
pub mod hint;
pub mod install;
pub mod running;
pub mod splash;
pub mod validators;
pub mod working;

use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::Line,
  widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

/// Background colour the modal-as-overlay paints across its full area. Picked
/// so it covers the workspace (nvim + tests) underneath cleanly without
/// looking aggressively black.
pub const MODAL_BG: Color = Color::Rgb(0x12, 0x12, 0x1c);

/// Shared "modal box with title" helper. Renders a `Clear` to wipe whatever
/// was drawn underneath, fills the area with a dark bg, draws a rounded
/// border, then the body paragraph. Used by install/validators/hint/etc.
pub fn modal(frame: &mut Frame<'_>, area: Rect, title: &str, color: Color, body: Vec<Line<'static>>) {
  frame.render_widget(Clear, area);
  let bg = Style::default().bg(MODAL_BG).fg(Color::White);
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(color).bg(MODAL_BG))
    .style(bg)
    .title(format!(" {title} "))
    .title_style(Style::default().fg(color).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = block.inner(area);
  frame.render_widget(block, area);
  frame.render_widget(Paragraph::new(body).style(bg).wrap(Wrap { trim: false }), inner);
}

/// Centre a child rect of `w × h` inside `area`.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
  let w = w.min(area.width.saturating_sub(2));
  let h = h.min(area.height.saturating_sub(2));
  Rect {
    x: area.x + (area.width.saturating_sub(w)) / 2,
    y: area.y + (area.height.saturating_sub(h)) / 2,
    width: w,
    height: h,
  }
}

/// Modal that covers the entire viewport with margins. Used for big phases
/// like `Validating`, `Celebrate`, and `Complete` where the underlying
/// workspace should be visually replaced.
pub fn full_modal_area(area: Rect, h_margin: u16, v_margin: u16) -> Rect {
  Rect {
    x: area.x + h_margin.min(area.width.saturating_sub(4) / 2),
    y: area.y + v_margin.min(area.height.saturating_sub(4) / 2),
    width: area.width.saturating_sub(h_margin * 2).max(20),
    height: area.height.saturating_sub(v_margin * 2).max(6),
  }
}
