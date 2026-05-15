use crate::pane::PtyPane;
use ratatui::{
  buffer::Buffer,
  layout::Rect,
  style::{Color, Modifier, Style},
  widgets::Widget,
};

/// Borrowing widget that paints the pane's current vt100 screen snapshot.
pub struct PtyView<'a>(pub &'a PtyPane);

impl<'a> Widget for PtyView<'a> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let parser = self.0.parser.lock();
    let screen = parser.screen();
    let height = area.height.min(screen.size().0);
    let width = area.width.min(screen.size().1);

    for row in 0..height {
      for col in 0..width {
        let x = area.x + col;
        let y = area.y + row;
        if x >= buf.area.right() || y >= buf.area.bottom() {
          continue;
        }
        let cell = buf.cell_mut((x, y));
        match (cell, screen.cell(row, col)) {
          (Some(cell_mut), Some(src)) => {
            let contents = src.contents();
            let glyph = if contents.is_empty() { " " } else { &contents };
            cell_mut.set_symbol(glyph);
            cell_mut.set_style(map_style(src));
          },
          (Some(cell_mut), None) => {
            cell_mut.set_symbol(" ");
            cell_mut.set_style(Style::default());
          },
          _ => {},
        }
      }
    }

    if screen.hide_cursor() {
      return;
    }
    let (cy, cx) = screen.cursor_position();
    let x = area.x + cx.min(width.saturating_sub(1));
    let y = area.y + cy.min(height.saturating_sub(1));
    if x < buf.area.right()
      && y < buf.area.bottom()
      && let Some(cell) = buf.cell_mut((x, y))
    {
      cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
    }
  }
}

fn map_style(cell: &vt100_ctt::Cell) -> Style {
  let mut style = Style::default();
  style = style.fg(map_color(cell.fgcolor()));
  let bg = map_color(cell.bgcolor());
  if !matches!(bg, Color::Reset) {
    style = style.bg(bg);
  }
  if cell.bold() {
    style = style.add_modifier(Modifier::BOLD);
  }
  if cell.italic() {
    style = style.add_modifier(Modifier::ITALIC);
  }
  if cell.underline() {
    style = style.add_modifier(Modifier::UNDERLINED);
  }
  if cell.inverse() {
    style = style.add_modifier(Modifier::REVERSED);
  }
  style
}

fn map_color(c: vt100_ctt::Color) -> Color {
  match c {
    vt100_ctt::Color::Default => Color::Reset,
    vt100_ctt::Color::Idx(i) => Color::Indexed(i),
    vt100_ctt::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
  }
}
