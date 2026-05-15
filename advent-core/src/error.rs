use std::path::PathBuf;
use thiserror::Error;

pub type AdventResult<T> = Result<T, AdventError>;

#[derive(Debug, Error)]
pub enum AdventError {
  #[error("config not found: walked up from {start} but no quest.config.{{ts,mjs,js}} was found")]
  ConfigNotFound { start: PathBuf },

  #[error("config parse error: {0}")]
  ConfigParse(String),

  #[error("config bridge failed: bun exited with code {code}: {stderr}")]
  BunBridge { code: i32, stderr: String },

  #[error("git error: {0}")]
  Git(String),

  #[error("docker error: {0}")]
  Docker(String),

  #[error("working tree not clean — commit or stash before switching quests")]
  DirtyTree,

  #[error("validator failed: {id} — {message}")]
  Validator { id: String, message: String },

  #[error("not on a recognized quest branch ({branch})")]
  NotOnQuestBranch { branch: String },

  #[error("tests failed (exit {code})")]
  TestsFailed { code: i32 },

  #[error("i/o error at {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },

  #[error(transparent)]
  BareIo(#[from] std::io::Error),

  #[error(transparent)]
  Json(#[from] serde_json::Error),
}

impl AdventError {
  pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
    Self::Io { path: path.into(), source }
  }
}
