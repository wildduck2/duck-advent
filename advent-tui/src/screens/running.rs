use super::{MODAL_BG, full_modal_area};
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TICK_MS: u64 = 40;

pub fn draw(frame: &mut Frame<'_>, area: Rect, cmd: &[String], spin: u8, ticks_since_start: u32, tail: &[&str]) {
  let outer = full_modal_area(area, 4, 1);
  frame.render_widget(Clear, outer);
  let bg = Style::default().bg(MODAL_BG);
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(Color::Yellow).bg(MODAL_BG))
    .style(bg)
    .title(" Validating quest ")
    .title_style(Style::default().fg(Color::Yellow).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = block.inner(outer);
  frame.render_widget(block, outer);

  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(7), Constraint::Min(0), Constraint::Length(2)])
    .split(inner);

  let frame_idx = (spin as usize) % SPINNER.len();
  let elapsed = (ticks_since_start as u64 * TICK_MS) / 1000;
  let dots = ".".repeat(((spin / 4) % 4) as usize);

  let header = vec![
    Line::raw(""),
    Line::from(vec![
      Span::styled(
        SPINNER[frame_idx].to_string(),
        Style::default().fg(Color::Yellow).bg(MODAL_BG).add_modifier(Modifier::BOLD),
      ),
      Span::styled("  running tests", Style::default().fg(Color::White).bg(MODAL_BG).add_modifier(Modifier::BOLD)),
      Span::styled(dots, Style::default().fg(Color::Yellow).bg(MODAL_BG)),
    ])
    .alignment(Alignment::Center),
    Line::raw(""),
    Line::styled(format!("elapsed {elapsed}s"), Style::default().fg(Color::LightYellow).bg(MODAL_BG))
      .alignment(Alignment::Center),
    Line::raw(""),
    Line::styled(cmd.join(" "), Style::default().fg(Color::DarkGray).bg(MODAL_BG)).alignment(Alignment::Center),
  ];
  frame.render_widget(Paragraph::new(header).style(bg), rows[0]);

  // Live tail of the child's stdout/stderr.
  let visible = rows[1].height as usize;
  let start = tail.len().saturating_sub(visible);
  let tail_lines: Vec<Line<'static>> = tail[start..]
    .iter()
    .map(|s| Line::styled((*s).to_string(), Style::default().fg(Color::Gray).bg(MODAL_BG)))
    .collect();
  let tail_block = Block::default()
    .borders(Borders::TOP | Borders::BOTTOM)
    .border_style(Style::default().fg(Color::DarkGray).bg(MODAL_BG))
    .style(bg)
    .title(" live output ")
    .title_style(Style::default().fg(Color::DarkGray).bg(MODAL_BG));
  let tail_inner = tail_block.inner(rows[1]);
  frame.render_widget(tail_block, rows[1]);
  frame.render_widget(Paragraph::new(tail_lines).style(bg).wrap(Wrap { trim: false }), tail_inner);

  let footer = Line::from(vec![
    Span::styled(" Esc ", Style::default().bg(Color::DarkGray).fg(Color::White)),
    Span::styled("  cancel", Style::default().fg(Color::DarkGray).bg(MODAL_BG)),
  ])
  .alignment(Alignment::Center);
  frame.render_widget(Paragraph::new(footer).style(bg), rows[2]);
}
