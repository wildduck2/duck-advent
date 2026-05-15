//! Ratatui TUI for duck-advent.
//!
//! Responsibility split:
//!  * `app`       — phase machine + event/tick loop, owns terminal handle
//!  * `screens`   — pure render fns, one file per phase
//!  * `workspace` — embedded nvim + tests panes with PTY input forwarding
//!  * `markdown`  — minimal markdown → styled `Line` converter
//!  * `confetti`  — celebration animation

mod app;
mod confetti;
mod markdown;
mod nvim;
mod screens;
mod terminal;
mod workspace;

pub use app::run;
