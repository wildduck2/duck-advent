use anyhow::{Context, Result};
use parking_lot::Mutex;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
  io::{Read, Write},
  path::Path,
  sync::Arc,
  thread,
};

/// A child process running inside a pty whose output drives a vt100 parser.
pub struct PtyPane {
  pub(crate) parser: Arc<Mutex<vt100_ctt::Parser>>,
  master: Box<dyn MasterPty + Send>,
  writer: Mutex<Box<dyn Write + Send>>,
  child: Mutex<Box<dyn Child + Send + Sync>>,
  rows: u16,
  cols: u16,
}

impl PtyPane {
  /// Spawn `program` with `args` inside a `rows × cols` pty.
  pub fn spawn(
    program: &str,
    args: &[String],
    cwd: &Path,
    rows: u16,
    cols: u16,
    env: &[(String, String)],
  ) -> Result<Self> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).context("openpty failed")?;

    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    cmd.cwd(cwd);
    cmd.env("TERM", std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()));
    cmd.env("COLORTERM", "truecolor");
    for (k, v) in env {
      cmd.env(k, v);
    }

    let child = pair.slave.spawn_command(cmd).context("spawn_command failed")?;
    drop(pair.slave);

    let parser = Arc::new(Mutex::new(vt100_ctt::Parser::new(rows, cols, 2048)));
    let master = pair.master;
    let writer = master.take_writer().context("take_writer failed")?;
    let mut reader = master.try_clone_reader().context("clone_reader failed")?;

    let parser_clone = Arc::clone(&parser);
    thread::Builder::new()
      .name("advent-pty-reader".into())
      .spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
          if n == 0 {
            break;
          }
          parser_clone.lock().process(&buf[..n]);
        }
      })
      .context("spawn reader thread")?;

    Ok(Self { parser, master, writer: Mutex::new(writer), child: Mutex::new(child), rows, cols })
  }

  pub fn write_input(&self, bytes: &[u8]) -> Result<()> {
    let mut w = self.writer.lock();
    w.write_all(bytes).context("pty write failed")?;
    w.flush().ok();
    Ok(())
  }

  pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    if rows == self.rows && cols == self.cols {
      return Ok(());
    }
    self.rows = rows;
    self.cols = cols;
    self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).context("pty resize failed")?;
    self.parser.lock().screen_mut().set_size(rows, cols);
    Ok(())
  }

  pub fn is_alive(&self) -> bool {
    matches!(self.child.lock().try_wait(), Ok(None))
  }

  pub fn scroll_up(&self, rows: usize) {
    let mut p = self.parser.lock();
    let cur = p.screen().scrollback();
    p.screen_mut().set_scrollback(cur.saturating_add(rows));
  }

  pub fn scroll_down(&self, rows: usize) {
    let mut p = self.parser.lock();
    let cur = p.screen().scrollback();
    p.screen_mut().set_scrollback(cur.saturating_sub(rows));
  }

  pub fn scroll_top(&self) {
    let mut p = self.parser.lock();
    p.screen_mut().set_scrollback(usize::MAX);
  }

  pub fn scroll_bottom(&self) {
    let mut p = self.parser.lock();
    p.screen_mut().set_scrollback(0);
  }

  pub fn scrollback_offset(&self) -> usize {
    self.parser.lock().screen().scrollback()
  }

  pub fn kill(&mut self) {
    let _ = self.child.lock().kill();
  }
}

impl Drop for PtyPane {
  fn drop(&mut self) {
    let _ = self.child.lock().kill();
  }
}
