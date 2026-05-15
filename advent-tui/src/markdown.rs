//! Minimal markdown → `Line` converter. Pulldown-cmark gives us tokens; we
//! emit styled spans for headings, lists, code, emphasis, and links.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

pub fn render(source: &str) -> Vec<Line<'static>> {
  let mut opts = Options::empty();
  opts.insert(Options::ENABLE_STRIKETHROUGH);
  opts.insert(Options::ENABLE_TABLES);
  let parser = Parser::new_ext(source, opts);

  let mut state = State::default();
  let mut lines: Vec<Line<'static>> = vec![Line::raw("")];
  let mut current: Vec<Span<'static>> = Vec::new();

  let flush = |lines: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>| {
    if !current.is_empty() {
      lines.push(Line::from(std::mem::take(current)));
    }
  };

  for ev in parser {
    match ev {
      Event::Start(Tag::Heading { level, .. }) => {
        flush(&mut lines, &mut current);
        state.heading = Some(level);
      },
      Event::End(TagEnd::Heading(_)) => {
        flush(&mut lines, &mut current);
        lines.push(Line::raw(""));
        state.heading = None;
      },
      Event::Start(Tag::Paragraph) => {},
      Event::End(TagEnd::Paragraph) => {
        flush(&mut lines, &mut current);
        lines.push(Line::raw(""));
      },
      Event::Start(Tag::CodeBlock(_)) => {
        flush(&mut lines, &mut current);
        state.in_code_block = true;
      },
      Event::End(TagEnd::CodeBlock) => {
        flush(&mut lines, &mut current);
        state.in_code_block = false;
        lines.push(Line::raw(""));
      },
      Event::Start(Tag::List(_)) => {
        flush(&mut lines, &mut current);
        state.list_depth += 1;
      },
      Event::End(TagEnd::List(_)) => {
        flush(&mut lines, &mut current);
        state.list_depth = state.list_depth.saturating_sub(1);
        if state.list_depth == 0 {
          lines.push(Line::raw(""));
        }
      },
      Event::Start(Tag::Item) => {
        flush(&mut lines, &mut current);
        let indent = "  ".repeat(state.list_depth.saturating_sub(1) as usize);
        current.push(Span::styled(format!("{indent}• "), Style::default().fg(Color::Yellow)));
      },
      Event::End(TagEnd::Item) => flush(&mut lines, &mut current),
      Event::Start(Tag::Emphasis) => state.italic = true,
      Event::End(TagEnd::Emphasis) => state.italic = false,
      Event::Start(Tag::Strong) => state.bold = true,
      Event::End(TagEnd::Strong) => state.bold = false,
      Event::Start(Tag::Link { .. }) => state.in_link = true,
      Event::End(TagEnd::Link) => state.in_link = false,
      Event::Code(t) => {
        current.push(Span::styled(t.into_string(), Style::default().fg(Color::LightMagenta)));
      },
      Event::Text(t) if state.in_code_block => {
        for line in t.split('\n') {
          lines.push(Line::styled(format!("  {line}"), Style::default().fg(Color::Gray)));
        }
      },
      Event::Text(t) => current.push(Span::styled(t.into_string(), style_for(&state))),
      Event::HardBreak | Event::SoftBreak => flush(&mut lines, &mut current),
      Event::Rule => {
        flush(&mut lines, &mut current);
        lines.push(Line::styled("─".repeat(60), Style::default().fg(Color::DarkGray)));
      },
      _ => {},
    }
  }
  flush(&mut lines, &mut current);
  lines
}

#[derive(Default)]
struct State {
  in_code_block: bool,
  list_depth: u8,
  bold: bool,
  italic: bool,
  in_link: bool,
  heading: Option<HeadingLevel>,
}

fn style_for(state: &State) -> Style {
  let mut s = Style::default();
  if let Some(h) = state.heading {
    s = match h {
      HeadingLevel::H1 => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
      HeadingLevel::H2 => Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
      _ => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    };
  }
  if state.bold {
    s = s.add_modifier(Modifier::BOLD);
  }
  if state.italic {
    s = s.add_modifier(Modifier::ITALIC);
  }
  if state.in_link {
    s = s.fg(Color::Blue).add_modifier(Modifier::UNDERLINED);
  }
  s
}
