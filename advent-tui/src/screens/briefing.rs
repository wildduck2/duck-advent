use super::MODAL_BG;
use crate::markdown;
use advent_core::QuestStep;
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

/// Centered 70%-of-screen briefing modal with opaque bg so the workspace
/// underneath does not bleed through.
fn briefing_area(area: Rect) -> Rect {
  let w = (area.width as u32 * 70 / 100) as u16;
  let h = (area.height as u32 * 80 / 100) as u16;
  let w = w.max(60).min(area.width.saturating_sub(2));
  let h = h.max(12).min(area.height.saturating_sub(2));
  Rect {
    x: area.x + (area.width.saturating_sub(w)) / 2,
    y: area.y + (area.height.saturating_sub(h)) / 2,
    width: w,
    height: h,
  }
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, quest: &QuestStep, source: &str, scroll: u16) {
  let outer = briefing_area(area);
  frame.render_widget(Clear, outer);
  let bg = Style::default().bg(MODAL_BG).fg(Color::White);
  let outer_block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(Color::Cyan).bg(MODAL_BG))
    .style(bg)
    .title(format!(" Quest {:02} — {} ", quest.number, quest.title))
    .title_style(Style::default().fg(Color::LightCyan).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = outer_block.inner(outer);
  frame.render_widget(outer_block, outer);

  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(2), Constraint::Min(0), Constraint::Length(1)])
    .split(inner);

  let meta = Line::from(vec![
    Span::styled(
      quest.tier.as_deref().map(|t| t.to_string()).unwrap_or_default(),
      Style::default().fg(Color::LightMagenta).bg(MODAL_BG),
    ),
    Span::styled(
      quest.difficulty.map(|d| format!("  ·  difficulty {d}/5")).unwrap_or_default(),
      Style::default().fg(Color::DarkGray).bg(MODAL_BG),
    ),
  ])
  .alignment(Alignment::Center);
  frame.render_widget(Paragraph::new(vec![meta, Line::raw("")]).style(bg), rows[0]);

  let lines = markdown::render(source);
  frame.render_widget(
    Paragraph::new(lines).style(bg).wrap(Wrap { trim: false }).scroll((scroll, 0)),
    rows[1],
  );

  let hint = Line::from(vec![
    Span::styled(" j/k ", Style::default().fg(Color::Yellow).bg(MODAL_BG)),
    Span::styled("scroll · ", Style::default().fg(Color::Gray).bg(MODAL_BG)),
    Span::styled("g/G ", Style::default().fg(Color::Yellow).bg(MODAL_BG)),
    Span::styled("top/bottom · ", Style::default().fg(Color::Gray).bg(MODAL_BG)),
    Span::styled("⏎/esc ", Style::default().fg(Color::Green).bg(MODAL_BG)),
    Span::styled("enter workspace", Style::default().fg(Color::Gray).bg(MODAL_BG)),
  ])
  .alignment(Alignment::Center);
  frame.render_widget(Paragraph::new(hint).style(bg), rows[2]);
}
