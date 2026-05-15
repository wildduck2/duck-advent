use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::Paragraph,
};

const LOGO: &[&str] = &[
  "  ╔╦╗╦ ╦╔═╗╦╔═  ╔═╗╔╦╗╦  ╦╔═╗╔╗╔╔╦╗",
  "   ║║║ ║║  ╠╩╗  ╠═╣ ║║╚╗╔╝║╣ ║║║ ║ ",
  "  ═╩╝╚═╝╚═╝╩ ╩  ╩ ╩═╩╝ ╚╝ ╚═╝╝╚╝ ╩ ",
];

pub fn draw(frame: &mut Frame<'_>, area: Rect, name: &str, description: Option<&str>, version: &str) {
  let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Min(0),
      Constraint::Length(3),
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(2),
      Constraint::Length(1),
      Constraint::Min(0),
    ])
    .split(area);

  let logo: Vec<Line<'static>> = LOGO
    .iter()
    .map(|l| Line::styled((*l).to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
    .collect();
  frame.render_widget(Paragraph::new(logo).alignment(Alignment::Center), rows[1]);

  frame.render_widget(
    Paragraph::new(Line::styled(name.to_string(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)))
      .alignment(Alignment::Center),
    rows[3],
  );
  if let Some(d) = description {
    frame.render_widget(
      Paragraph::new(Line::styled(d.to_string(), Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
      rows[4],
    );
  }
  frame.render_widget(
    Paragraph::new(Line::styled(format!("v{version}"), Style::default().fg(Color::DarkGray)))
      .alignment(Alignment::Center),
    rows[5],
  );
  frame.render_widget(
    Paragraph::new(Line::from(vec![
      Span::styled("press ", Style::default().fg(Color::Yellow)),
      Span::styled("⏎", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::styled(" to begin · ", Style::default().fg(Color::Yellow)),
      Span::styled("q", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
      Span::styled(" to quit", Style::default().fg(Color::Yellow)),
    ]))
    .alignment(Alignment::Center),
    rows[6],
  );
}
