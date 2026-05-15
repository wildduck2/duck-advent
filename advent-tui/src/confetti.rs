//! Cheap celebration animation. State is a small particle list updated on
//! every UI tick.

use ratatui::{
  style::{Color, Style},
  text::{Line, Span},
};
use std::time::{Duration, Instant};

const SHAPES: &[&str] = &["✦", "✧", "✺", "✹", "★", "❉", "❅", "❆", "·"];
const COLORS: &[Color] =
  &[Color::LightRed, Color::LightGreen, Color::LightYellow, Color::LightBlue, Color::LightMagenta, Color::LightCyan];

#[derive(Clone, Copy)]
struct Piece {
  ch: usize,
  color: usize,
  x: u16,
  y: f32,
  vy: f32,
}

pub struct Confetti {
  pieces: Vec<Piece>,
  cols: u16,
  rows: u16,
  last_tick: Instant,
  rng: u64,
}

impl Confetti {
  pub fn new(cols: u16, rows: u16) -> Self {
    Self { pieces: Vec::new(), cols, rows, last_tick: Instant::now(), rng: 0xCAFEF00DBAADF00D }
  }

  fn next_rand(&mut self) -> u64 {
    let mut x = self.rng;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.rng = x;
    x
  }

  pub fn resize(&mut self, cols: u16, rows: u16) {
    self.cols = cols;
    self.rows = rows;
  }

  pub fn tick(&mut self) {
    let now = Instant::now();
    if now.duration_since(self.last_tick) < Duration::from_millis(60) {
      return;
    }
    self.last_tick = now;
    let rows = self.rows as f32;
    self.pieces.retain_mut(|p| {
      p.y += p.vy;
      p.y < rows
    });
    let new = (self.cols / 18).max(2);
    for _ in 0..new {
      let r1 = self.next_rand();
      let r2 = self.next_rand();
      let r3 = self.next_rand();
      self.pieces.push(Piece {
        ch: (r1 as usize) % SHAPES.len(),
        color: (r2 as usize) % COLORS.len(),
        x: (r3 as u16) % self.cols.max(1),
        y: 0.0,
        vy: 0.5 + ((r1 >> 8) as f32 % 100.0) / 100.0,
      });
    }
  }

  pub fn render(&self) -> Vec<Line<'static>> {
    let rows = self.rows as usize;
    let cols = self.cols as usize;
    let mut grid: Vec<Vec<Option<(usize, usize)>>> = vec![vec![None; cols]; rows];
    for p in &self.pieces {
      let y = p.y as usize;
      let x = p.x as usize;
      if y < rows && x < cols {
        grid[y][x] = Some((p.ch, p.color));
      }
    }
    grid
      .into_iter()
      .map(|row| {
        let spans: Vec<Span<'static>> = row
          .into_iter()
          .map(|cell| match cell {
            Some((ci, color)) => Span::styled(SHAPES[ci].to_string(), Style::default().fg(COLORS[color])),
            None => Span::raw(" "),
          })
          .collect();
        Line::from(spans)
      })
      .collect()
  }
}
