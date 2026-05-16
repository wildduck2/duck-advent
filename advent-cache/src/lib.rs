//! `~/.gentleduck/` filesystem state for duck-advent.
//!
//! Layout:
//!   ~/.gentleduck/
//!     cache/<repo-hash>/install.json    -- lockfile hash + completedAt
//!     cache/<repo-hash>/validators.json -- per-validator pass/fail + configHash
//!     state/<repo-hash>/progress.json   -- current quest, completed list, hints
//!     log/duck-advent-<ts>.log          -- orchestrator log

use advent_core::{AdventError, AdventResult, ProgressState};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
};

/// Process-wide root override. Production keeps it `None` so `root()` resolves
/// to `~/.gentleduck`. Tests call `set_root_override` to redirect every cache
/// read/write into a tempdir, leaving the user's real state untouched.
static ROOT_OVERRIDE: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// Redirect every subsequent cache call into `root` (typically a tempdir).
/// Pass `None` to clear and fall back to `$HOME/.gentleduck`. Intended only
/// for tests — the lock is global so concurrent tests must serialize on it.
pub fn set_root_override(root: Option<PathBuf>) {
  *ROOT_OVERRIDE.write().expect("ROOT_OVERRIDE poisoned") = root;
}

/// Root of the user-global cache. Created on demand.
pub fn root() -> AdventResult<PathBuf> {
  if let Some(p) = ROOT_OVERRIDE.read().expect("ROOT_OVERRIDE poisoned").as_ref() {
    return Ok(p.clone());
  }
  let home =
    dirs::home_dir().ok_or_else(|| AdventError::ConfigParse("cannot resolve $HOME for ~/.gentleduck cache".into()))?;
  Ok(home.join(".gentleduck"))
}

pub fn repo_hash(repo_root: &Path) -> String {
  let canon = repo_root.canonicalize().unwrap_or_else(|_| repo_root.to_path_buf());
  let mut h = Sha256::new();
  h.update(canon.to_string_lossy().as_bytes());
  let bytes = h.finalize();
  hex::encode(&bytes[..8])
}

fn ensure_dir(p: &Path) -> AdventResult<()> {
  fs::create_dir_all(p).map_err(|source| AdventError::io(p.to_path_buf(), source))
}

fn cache_dir(repo_hash: &str) -> AdventResult<PathBuf> {
  let dir = root()?.join("cache").join(repo_hash);
  ensure_dir(&dir)?;
  Ok(dir)
}

fn state_dir(repo_hash: &str) -> AdventResult<PathBuf> {
  let dir = root()?.join("state").join(repo_hash);
  ensure_dir(&dir)?;
  Ok(dir)
}

pub fn log_dir() -> AdventResult<PathBuf> {
  let dir = root()?.join("log");
  ensure_dir(&dir)?;
  Ok(dir)
}

fn read_json<T: for<'de> Deserialize<'de>>(p: &Path) -> AdventResult<Option<T>> {
  match fs::read_to_string(p) {
    Ok(s) => Ok(Some(serde_json::from_str(&s).map_err(AdventError::Json)?)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(AdventError::io(p.to_path_buf(), e)),
  }
}

fn write_json<T: Serialize>(p: &Path, value: &T) -> AdventResult<()> {
  let s = serde_json::to_string_pretty(value).map_err(AdventError::Json)?;
  fs::write(p, format!("{s}\n")).map_err(|e| AdventError::io(p.to_path_buf(), e))
}

// ---------- install cache ------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallRecord {
  pub lockfile_hash: String,
  pub completed_at: String,
}

fn lockfile_fingerprint(repo_root: &Path) -> String {
  let mut h = Sha256::new();
  let candidates = ["package.json", "bun.lock", "bun.lockb", "package-lock.json", "pnpm-lock.yaml", "yarn.lock"];
  for name in candidates {
    let p = repo_root.join(name);
    if let Ok(bytes) = fs::read(&p) {
      h.update(name.as_bytes());
      h.update(b":");
      h.update(&bytes);
      h.update(b"\n");
    }
  }
  hex::encode(h.finalize())
}

pub fn has_fresh_install(repo_root: &Path, repo_hash: &str) -> AdventResult<bool> {
  let file = cache_dir(repo_hash)?.join("install.json");
  let Some(rec) = read_json::<InstallRecord>(&file)? else {
    return Ok(false);
  };
  Ok(rec.lockfile_hash == lockfile_fingerprint(repo_root))
}

pub fn mark_install_complete(repo_root: &Path, repo_hash: &str) -> AdventResult<()> {
  let file = cache_dir(repo_hash)?.join("install.json");
  let rec = InstallRecord { lockfile_hash: lockfile_fingerprint(repo_root), completed_at: Utc::now().to_rfc3339() };
  write_json(&file, &rec)
}

// ---------- validator cache ----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorOutcome {
  pub id: String,
  pub passed: bool,
  pub output: String,
  pub checked_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidatorRecord {
  config_hash: String,
  results: Vec<ValidatorOutcome>,
}

pub fn read_validator_cache(repo_hash: &str, config_hash: &str) -> AdventResult<Option<Vec<ValidatorOutcome>>> {
  let file = cache_dir(repo_hash)?.join("validators.json");
  let Some(rec) = read_json::<ValidatorRecord>(&file)? else {
    return Ok(None);
  };
  if rec.config_hash != config_hash {
    return Ok(None);
  }
  Ok(Some(rec.results))
}

pub fn write_validator_cache(repo_hash: &str, config_hash: &str, results: &[ValidatorOutcome]) -> AdventResult<()> {
  let file = cache_dir(repo_hash)?.join("validators.json");
  let rec = ValidatorRecord { config_hash: config_hash.to_string(), results: results.to_vec() };
  write_json(&file, &rec)
}

// ---------- progress -----------------------------------------------------

pub fn progress_path(repo_hash: &str) -> AdventResult<PathBuf> {
  Ok(state_dir(repo_hash)?.join("progress.json"))
}

pub fn read_progress(repo_hash: &str) -> AdventResult<ProgressState> {
  let p = progress_path(repo_hash)?;
  Ok(read_json::<ProgressState>(&p)?.unwrap_or_else(ProgressState::empty))
}

pub fn write_progress(repo_hash: &str, state: &mut ProgressState) -> AdventResult<()> {
  state.touch();
  let p = progress_path(repo_hash)?;
  write_json(&p, state)
}

pub fn set_current_quest(repo_hash: &str, slug: &str) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  state.current_quest = Some(slug.to_string());
  let q = state.ensure_quest(slug);
  if q.started_at.is_none() {
    q.started_at = Some(Utc::now());
  }
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}

pub fn complete_quest(repo_hash: &str, slug: &str) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  if !state.completed.iter().any(|s| s == slug) {
    state.completed.push(slug.to_string());
  }
  state.ensure_quest(slug).completed_at = Some(Utc::now());
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}

/// Increment the per-quest hint counter. Returns the full ProgressState so
/// the caller can refresh its in-memory snapshot in one disk hit.
pub fn bump_hints(repo_hash: &str, slug: &str) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  state.ensure_quest(slug).hints_used += 1;
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}

/// Increment the per-quest attempt counter (one per `<leader> n` invocation).
pub fn bump_attempts(repo_hash: &str, slug: &str) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  state.ensure_quest(slug).attempts += 1;
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}

/// Add `secs` to the per-quest elapsed timer and persist. Called on a periodic
/// tick from the TUI so the timer survives SIGKILL / lid-close with at most a
/// few seconds of drift. Returns the full ProgressState so the TUI can avoid
/// a follow-up read.
pub fn add_elapsed(repo_hash: &str, slug: &str, secs: u64) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  if secs == 0 {
    return Ok(state);
  }
  let q = state.ensure_quest(slug);
  q.elapsed_seconds = q.elapsed_seconds.saturating_add(secs);
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}
