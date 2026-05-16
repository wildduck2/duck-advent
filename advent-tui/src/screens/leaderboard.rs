use super::{MODAL_BG, full_modal_area};
use advent_core::{ProgressState, QuestStep};
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// Render a sortable, read-only view of every quest with its best time,
/// attempts, hints used, and lock state. Triggered via `<leader> l`.
pub fn draw(frame: &mut Frame<'_>, area: Rect, quests: &[QuestStep], progress: &ProgressState, current_slug: &str) {
  let outer = full_modal_area(area, 6, 2);
  frame.render_widget(Clear, outer);
  let bg = Style::default().bg(MODAL_BG);
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(Color::Cyan).bg(MODAL_BG))
    .style(bg)
    .title(format!(" 🏆 leaderboard · {}/{} done ", progress.completed.len(), quests.len()))
    .title_style(Style::default().fg(Color::LightCyan).bg(MODAL_BG).add_modifier(Modifier::BOLD));
  let inner = block.inner(outer);
  frame.render_widget(block, outer);

  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(2), Constraint::Min(0), Constraint::Length(2)])
    .split(inner);

  // Header
  let header = Line::from(vec![
    Span::styled(
      format!("  {:>3}  {:<26}  {:<14}  {:>10}  {:>8}  {:>7}  status", "#", "title", "tier", "best", "attempts", "hints"),
      Style::default().fg(Color::DarkGray).bg(MODAL_BG).add_modifier(Modifier::BOLD),
    ),
  ]);
  frame.render_widget(Paragraph::new(vec![header, Line::raw("")]).style(bg), rows[0]);

  // Body — one row per quest, with the current quest highlighted.
  let mut lines: Vec<Line<'static>> = Vec::with_capacity(quests.len());
  let completed: std::collections::HashSet<&str> = progress.completed.iter().map(String::as_str).collect();
  for (i, q) in quests.iter().enumerate() {
    let stats = progress.quests.get(&q.slug);
    let best = stats.and_then(|s| s.best_time_seconds);
    let attempts = stats.map(|s| s.attempts).unwrap_or(0);
    let hints = stats.map(|s| s.hints_used).unwrap_or(0);
    let is_done = completed.contains(q.slug.as_str());
    let is_current = q.slug == current_slug;
    let predecessor_done = i == 0 || completed.contains(quests[i - 1].slug.as_str());
    let is_locked = !is_done && !is_current && !predecessor_done;

    let status_label = if is_done {
      Span::styled(" ✓ done   ", Style::default().fg(Color::Green).bg(MODAL_BG).add_modifier(Modifier::BOLD))
    } else if is_current {
      Span::styled(" ▶ active ", Style::default().fg(Color::LightYellow).bg(MODAL_BG).add_modifier(Modifier::BOLD))
    } else if is_locked {
      Span::styled(" 🔒 locked", Style::default().fg(Color::DarkGray).bg(MODAL_BG))
    } else {
      Span::styled(" ○ open   ", Style::default().fg(Color::White).bg(MODAL_BG))
    };
    let best_txt = best.map(format_short).unwrap_or_else(|| "—".into());
    let title_color = if is_locked { Color::DarkGray } else { Color::White };
    let tier = q.tier.clone().unwrap_or_default();
    lines.push(Line::from(vec![
      Span::styled(
        if is_current { "  ▶ " } else { "    " },
        Style::default().fg(Color::LightYellow).bg(MODAL_BG).add_modifier(Modifier::BOLD),
      ),
      Span::styled(format!("{:>2}  ", q.number), Style::default().fg(Color::DarkGray).bg(MODAL_BG)),
      Span::styled(format!("{:<26}", truncate(&q.title, 26)), Style::default().fg(title_color).bg(MODAL_BG)),
      Span::styled(format!("  {:<14}", truncate(&tier, 14)), Style::default().fg(Color::Magenta).bg(MODAL_BG)),
      Span::styled(format!("  {:>10}", best_txt), Style::default().fg(Color::Green).bg(MODAL_BG)),
      Span::styled(format!("  {:>8}", attempts), Style::default().fg(Color::White).bg(MODAL_BG)),
      Span::styled(format!("  {:>7}", format!("{}/{}", hints, q.hints.len())), Style::default().fg(Color::Yellow).bg(MODAL_BG)),
      Span::raw("  "),
      status_label,
    ]));
  }
  frame.render_widget(Paragraph::new(lines).style(bg), rows[1]);

  let footer = Line::from(vec![
    Span::styled(" esc / enter / q ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
    Span::styled("  close", Style::default().fg(Color::White).bg(MODAL_BG)),
  ])
  .alignment(Alignment::Center);
  frame.render_widget(Paragraph::new(footer).style(bg), rows[2]);
}

/// Compact mm:ss / h:mm:ss for the best-time column. Returns "—" for None
/// upstream so this only runs on Some values.
fn format_short(secs: u64) -> String {
  let h = secs / 3600;
  let m = (secs % 3600) / 60;
  let s = secs % 60;
  if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

fn truncate(s: &str, max: usize) -> String {
  if s.chars().count() <= max {
    return s.to_string();
  }
  let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
  t.push('…');
  t
}
