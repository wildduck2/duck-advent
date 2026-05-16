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
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
};

type HmacSha256 = Hmac<Sha256>;

/// Build the canonical message HMAC-signed when a completion is recorded.
/// Includes the slug, the repo identity (so a manifest secret + sig pair
/// can't be moved to a different repo), and the completion timestamp in
/// unix seconds (so re-completing the same quest yields a different sig).
fn completion_message(slug: &str, repo_hash: &str, completed_at_unix: i64) -> String {
  format!("complete:{slug}:{repo_hash}:{completed_at_unix}")
}

/// Sign a completion payload with the manifest secret. Returns lowercase hex.
pub fn sign_completion(secret: &str, slug: &str, repo_hash: &str, completed_at_unix: i64) -> String {
  let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
  mac.update(completion_message(slug, repo_hash, completed_at_unix).as_bytes());
  hex::encode(mac.finalize().into_bytes())
}

/// Constant-time verify of a completion entry. Returns `true` iff `sig`
/// matches what `sign_completion` would produce for the same inputs.
pub fn verify_completion(secret: &str, slug: &str, repo_hash: &str, completed_at_unix: i64, sig: &str) -> bool {
  let expected = sign_completion(secret, slug, repo_hash, completed_at_unix);
  // `hmac::Mac::verify_slice` requires a fresh Mac per call. Hex compare is
  // fine here — both strings are the same length when honest, and we don't
  // care about leaking length to a process-local attacker.
  constant_time_eq(expected.as_bytes(), sig.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
  if a.len() != b.len() {
    return false;
  }
  let mut diff: u8 = 0;
  for (x, y) in a.iter().zip(b.iter()) {
    diff |= x ^ y;
  }
  diff == 0
}

/// Walk `state` and drop forgery: any `completion_sig` that doesn't verify
/// against `secret` is cleared, its `completed_at` is reset, and the slug
/// is removed from `completed`. Use after `read_progress` whenever a
/// trustworthy view is required.
pub fn enforce_completion_sigs(state: &mut ProgressState, repo_hash: &str, secret: &str) -> Vec<String> {
  let mut forged: Vec<String> = Vec::new();
  let slugs: Vec<String> = state.quests.keys().cloned().collect();
  for slug in slugs {
    let Some(q) = state.quests.get_mut(&slug) else { continue };
    let Some(completed_at) = q.completed_at else {
      // No claim of completion. Strip any orphan sig defensively.
      if q.completion_sig.is_some() {
        q.completion_sig = None;
        forged.push(slug.clone());
      }
      continue;
    };
    let Some(sig) = q.completion_sig.clone() else {
      // Claim with no sig — forged or pre-manifest. Strip.
      q.completed_at = None;
      forged.push(slug.clone());
      continue;
    };
    let ts = completed_at.timestamp();
    if !verify_completion(secret, &slug, repo_hash, ts, &sig) {
      q.completed_at = None;
      q.completion_sig = None;
      forged.push(slug.clone());
    }
  }
  state.completed.retain(|s| !forged.contains(s));
  forged
}

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

/// Outcome of a `complete_quest` call. The TUI reads `set_new_best` to flash
/// a "NEW BEST" badge on the celebrate screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompletionOutcome {
  /// Wall-clock seconds the user spent on the attempt that just succeeded.
  pub attempt_seconds: u64,
  /// `true` when this attempt set a new personal best (or was the first
  /// completion).
  pub set_new_best: bool,
  /// Personal-best after the update.
  pub best_seconds: Option<u64>,
}

/// Mark `slug` complete. When `secret` is `Some`, the completion is signed
/// with HMAC so a hand-edit of progress.json can't forge a quest as solved.
/// `None` skips signing (repos that haven't run `manifest gen` yet).
pub fn complete_quest(
  repo_hash: &str,
  slug: &str,
  secret: Option<&str>,
) -> AdventResult<(ProgressState, CompletionOutcome)> {
  let mut state = read_progress(repo_hash)?;
  if !state.completed.iter().any(|s| s == slug) {
    state.completed.push(slug.to_string());
  }
  let q = state.ensure_quest(slug);
  let now = Utc::now();
  q.completed_at = Some(now);
  let attempt = q.attempt_elapsed_seconds;
  let mut outcome = CompletionOutcome { attempt_seconds: attempt, set_new_best: false, best_seconds: q.best_time_seconds };
  if attempt > 0 {
    let beat_prev = match q.best_time_seconds {
      Some(prev) => attempt < prev,
      None => true,
    };
    if beat_prev {
      q.best_time_seconds = Some(attempt);
      outcome.best_seconds = Some(attempt);
      outcome.set_new_best = true;
    }
  }
  // Reset the per-attempt window so the next repeat-and-solve loop starts
  // from zero. `elapsed_seconds` (cumulative) stays intact.
  q.attempt_elapsed_seconds = 0;
  // Sign the completion record. The signature binds (slug, repo, timestamp)
  // so the entry can't be replayed onto a different quest or repo.
  q.completion_sig = secret.map(|s| sign_completion(s, slug, repo_hash, now.timestamp()));
  write_progress(repo_hash, &mut state)?;
  Ok((state, outcome))
}

/// Reset the per-attempt timer for `slug` to 0. Called by the TUI when the
/// user presses `<leader> r` so the next solve is timed cleanly. Cumulative
/// `elapsed_seconds` is untouched.
pub fn reset_attempt(repo_hash: &str, slug: &str) -> AdventResult<ProgressState> {
  let mut state = read_progress(repo_hash)?;
  let q = state.ensure_quest(slug);
  if q.attempt_elapsed_seconds == 0 {
    return Ok(state);
  }
  q.attempt_elapsed_seconds = 0;
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
  // Both counters tick together. Cumulative `elapsed_seconds` is monotonic
  // for the lifetime of the quest entry. `attempt_elapsed_seconds` is the
  // per-attempt clock that `reset_attempt` and `complete_quest` zero out.
  q.elapsed_seconds = q.elapsed_seconds.saturating_add(secs);
  q.attempt_elapsed_seconds = q.attempt_elapsed_seconds.saturating_add(secs);
  write_progress(repo_hash, &mut state)?;
  Ok(state)
}
