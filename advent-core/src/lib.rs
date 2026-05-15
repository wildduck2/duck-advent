//! Domain types shared across the duck-advent workspace.
//!
//! Everything in this crate is plain data + small helpers — no I/O, no async,
//! no global state. The CLI, TUI, cache, and config crates depend on these
//! types so the schema stays in one place.

pub mod config;
pub mod error;
pub mod progress;

pub use config::{QuestConfig, QuestStep, ServiceSpec, ValidatorSpec};
pub use error::{AdventError, AdventResult};
pub use progress::{ProgressState, QuestStats};
