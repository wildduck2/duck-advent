//! Pseudo-terminal embedded as a Ratatui widget.
//!
//! `PtyPane` owns the child process and a background reader thread that feeds
//! bytes into a `vt100-ctt::Parser`. The TUI snapshots the parser each frame
//! and renders styled spans via `PtyView`. Keystrokes go through `keys::encode`
//! and into the master's writer.

mod keys;
mod pane;
mod view;

pub use keys::encode as encode_key;
pub use pane::PtyPane;
pub use view::PtyView;
