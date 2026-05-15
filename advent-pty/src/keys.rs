use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Translate a crossterm key event into bytes that a vt100 program (nvim,
/// vitest, a shell) accepts on stdin.
pub fn encode(key: KeyEvent) -> Vec<u8> {
  if key.kind == KeyEventKind::Release {
    return Vec::new();
  }
  let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
  let alt = key.modifiers.contains(KeyModifiers::ALT);
  let shift = key.modifiers.contains(KeyModifiers::SHIFT);

  let mut out = Vec::with_capacity(8);
  if alt {
    out.push(0x1b);
  }

  match key.code {
    KeyCode::Char(c) => encode_char(c, ctrl, shift, &mut out),
    KeyCode::Enter => out.push(b'\r'),
    KeyCode::Tab => out.push(b'\t'),
    KeyCode::BackTab => out.extend_from_slice(b"\x1b[Z"),
    KeyCode::Backspace => out.push(0x7f),
    KeyCode::Esc => out.push(0x1b),
    KeyCode::Left => out.extend_from_slice(b"\x1b[D"),
    KeyCode::Right => out.extend_from_slice(b"\x1b[C"),
    KeyCode::Up => out.extend_from_slice(b"\x1b[A"),
    KeyCode::Down => out.extend_from_slice(b"\x1b[B"),
    KeyCode::Home => out.extend_from_slice(b"\x1b[H"),
    KeyCode::End => out.extend_from_slice(b"\x1b[F"),
    KeyCode::PageUp => out.extend_from_slice(b"\x1b[5~"),
    KeyCode::PageDown => out.extend_from_slice(b"\x1b[6~"),
    KeyCode::Delete => out.extend_from_slice(b"\x1b[3~"),
    KeyCode::Insert => out.extend_from_slice(b"\x1b[2~"),
    KeyCode::F(n) => match n {
      1 => out.extend_from_slice(b"\x1bOP"),
      2 => out.extend_from_slice(b"\x1bOQ"),
      3 => out.extend_from_slice(b"\x1bOR"),
      4 => out.extend_from_slice(b"\x1bOS"),
      n => out.extend_from_slice(format!("\x1b[{};1~", 10 + n).as_bytes()),
    },
    _ => {},
  }
  out
}

fn encode_char(c: char, ctrl: bool, shift: bool, out: &mut Vec<u8>) {
  if ctrl {
    let lower = c.to_ascii_lowercase();
    let code = match lower {
      'a'..='z' => (lower as u8) - b'a' + 1,
      ' ' => 0,
      '\\' => 0x1c,
      ']' => 0x1d,
      '^' => 0x1e,
      '_' => 0x1f,
      _ => c as u8,
    };
    out.push(code);
    return;
  }
  if shift && c.is_ascii_alphabetic() {
    out.extend_from_slice(c.to_ascii_uppercase().encode_utf8(&mut [0; 4]).as_bytes());
    return;
  }
  let mut buf = [0u8; 4];
  out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}
