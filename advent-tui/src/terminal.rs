use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn enter() -> std::io::Result<Tui> {
  enable_raw_mode()?;
  let mut stdout = std::io::stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  Terminal::new(CrosstermBackend::new(stdout))
}

pub fn leave(terminal: &mut Tui) -> std::io::Result<()> {
  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
  terminal.show_cursor()?;
  Ok(())
}
