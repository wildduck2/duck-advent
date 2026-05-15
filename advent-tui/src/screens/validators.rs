use super::{centered, modal};
use advent_cache::ValidatorOutcome;
use advent_core::ValidatorSpec;
use ratatui::{
  Frame,
  layout::Rect,
  style::{Color, Style},
  text::{Line, Span},
};

pub fn draw(
  frame: &mut Frame<'_>,
  area: Rect,
  validators: &[ValidatorSpec],
  results: &[ValidatorOutcome],
  busy: Option<usize>,
) {
  let mut lines: Vec<Line<'static>> = vec![Line::raw("")];
  for (i, v) in validators.iter().enumerate() {
    let outcome = results.iter().find(|r| r.id == v.id);
    let icon = if Some(i) == busy {
      Span::styled("⠿", Style::default().fg(Color::Yellow))
    } else if let Some(r) = outcome {
      if r.passed {
        Span::styled("✓", Style::default().fg(Color::Green))
      } else {
        Span::styled("✗", Style::default().fg(Color::Red))
      }
    } else {
      Span::styled("○", Style::default().fg(Color::DarkGray))
    };
    let mut spans = vec![Span::raw("  "), icon, Span::raw("  "), Span::raw(v.label.clone())];
    if v.optional {
      spans.push(Span::styled(" (optional)", Style::default().fg(Color::DarkGray)));
    }
    lines.push(Line::from(spans));
  }
  let h = (validators.len() as u16) + 6;
  modal(frame, centered(area, 70, h), "Validating environment", Color::Cyan, lines);
}
