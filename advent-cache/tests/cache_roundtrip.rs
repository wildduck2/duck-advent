//! End-to-end tests for the `~/.gentleduck` cache layer.
//!
//! Every test redirects the cache root into a fresh tempdir via
//! `advent_cache::set_root_override`, so the real user state is never touched.
//! The override is a process-wide RwLock, so tests share state — we serialise
//! through a Mutex to keep the override stable while a test runs.

use std::{
  path::PathBuf,
  sync::{Mutex, OnceLock},
};

use advent_cache::{
  ValidatorOutcome, add_elapsed, bump_attempts, bump_hints, complete_quest, enforce_completion_sigs,
  has_fresh_install, log_dir, mark_install_complete, progress_path, read_progress, read_validator_cache,
  repo_hash, reset_attempt, root, set_current_quest, set_root_override, sign_completion, verify_completion,
  write_progress, write_validator_cache,
};
use advent_core::ProgressState;
use tempfile::TempDir;

/// All cache calls go through a shared global root. Tests run in parallel by
/// default; this Mutex makes the override stable for the duration of each.
fn lock() -> std::sync::MutexGuard<'static, ()> {
  static M: OnceLock<Mutex<()>> = OnceLock::new();
  M.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|p| p.into_inner())
}

/// Hold the tempdir alive AND the lock guard for the test's lifetime. Dropping
/// the guard clears the override so the user's real cache is untouched.
struct CacheSandbox {
  _dir: TempDir,
  _guard: std::sync::MutexGuard<'static, ()>,
}

impl CacheSandbox {
  fn new() -> Self {
    let guard = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    set_root_override(Some(dir.path().to_path_buf()));
    Self { _dir: dir, _guard: guard }
  }
}

impl Drop for CacheSandbox {
  fn drop(&mut self) {
    set_root_override(None);
  }
}

fn fake_repo_root(sandbox: &TempDir, name: &str) -> PathBuf {
  let p = sandbox.path().join(name);
  std::fs::create_dir_all(&p).unwrap();
  p
}

#[test]
fn override_redirects_root_into_tempdir() {
  let _sb = CacheSandbox::new();
  let r = root().expect("root resolves under override");
  assert!(r.starts_with(std::env::temp_dir()), "override should land inside the OS tempdir, got {r:?}");
}

#[test]
fn log_dir_is_created_on_demand() {
  let _sb = CacheSandbox::new();
  let p = log_dir().expect("log dir");
  assert!(p.exists(), "log dir should be created");
  assert!(p.ends_with("log"));
}

#[test]
fn repo_hash_is_deterministic_for_same_path() {
  let dir = tempfile::tempdir().unwrap();
  let repo = fake_repo_root(&dir, "alpha");
  let a = repo_hash(&repo);
  let b = repo_hash(&repo);
  assert_eq!(a, b, "same path produces same hash across calls");
  assert_eq!(a.len(), 16, "16 hex chars = 8 bytes of sha256");
}

#[test]
fn repo_hash_differs_across_paths() {
  let dir = tempfile::tempdir().unwrap();
  let a = repo_hash(&fake_repo_root(&dir, "alpha"));
  let b = repo_hash(&fake_repo_root(&dir, "beta"));
  assert_ne!(a, b);
}

#[test]
fn install_cache_records_and_invalidates_on_lockfile_change() {
  let _sb = CacheSandbox::new();
  let dir = tempfile::tempdir().unwrap();
  let repo = fake_repo_root(&dir, "repo");
  std::fs::write(repo.join("package.json"), "{\"name\":\"x\"}").unwrap();
  std::fs::write(repo.join("bun.lock"), "version 1").unwrap();
  let hash = repo_hash(&repo);

  assert!(!has_fresh_install(&repo, &hash).unwrap(), "no install yet");
  mark_install_complete(&repo, &hash).unwrap();
  assert!(has_fresh_install(&repo, &hash).unwrap(), "should be fresh right after marking");

  // Mutating the lockfile invalidates the cache.
  std::fs::write(repo.join("bun.lock"), "version 2").unwrap();
  assert!(!has_fresh_install(&repo, &hash).unwrap(), "lockfile change invalidates");
}

#[test]
fn validator_cache_keyed_by_config_hash() {
  let _sb = CacheSandbox::new();
  let repo_hash = "deadbeef";
  let outcomes = vec![ValidatorOutcome {
    id: "bun".into(),
    passed: true,
    output: "1.1.42".into(),
    checked_at: "2026-05-16T00:00:00Z".into(),
  }];

  write_validator_cache(repo_hash, "cfg-v1", &outcomes).unwrap();
  // Same config hash: cache hits.
  let hit = read_validator_cache(repo_hash, "cfg-v1").unwrap();
  assert!(hit.is_some());
  assert_eq!(hit.unwrap()[0].id, "bun");

  // Different config hash (user edited quest.config.ts): cache misses.
  let miss = read_validator_cache(repo_hash, "cfg-v2").unwrap();
  assert!(miss.is_none(), "config hash mismatch should be a miss");
}

#[test]
fn read_progress_returns_empty_when_file_missing() {
  let _sb = CacheSandbox::new();
  let state = read_progress("nonexistent").unwrap();
  assert!(state.current_quest.is_none());
  assert!(state.quests.is_empty());
}

#[test]
fn set_current_quest_seeds_started_at_once() {
  let _sb = CacheSandbox::new();
  let hash = "repo1";
  let s1 = set_current_quest(hash, "chapter-01").unwrap();
  let started_at_1 = s1.quests.get("chapter-01").and_then(|q| q.started_at).expect("started_at set");
  // Calling again must NOT overwrite started_at — it's a one-shot.
  std::thread::sleep(std::time::Duration::from_millis(5));
  let s2 = set_current_quest(hash, "chapter-01").unwrap();
  let started_at_2 = s2.quests.get("chapter-01").and_then(|q| q.started_at).unwrap();
  assert_eq!(started_at_1, started_at_2, "started_at must not be reset on re-entry");
}

#[test]
fn complete_quest_marks_and_dedupes() {
  let _sb = CacheSandbox::new();
  let hash = "repo2";
  set_current_quest(hash, "ch1").unwrap();
  let (s1, _) = complete_quest(hash, "ch1", None).unwrap();
  assert_eq!(s1.completed, vec!["ch1".to_string()]);
  assert!(s1.quests.get("ch1").and_then(|q| q.completed_at).is_some());
  // Idempotent — completing twice does not duplicate in `completed`.
  let (s2, _) = complete_quest(hash, "ch1", None).unwrap();
  assert_eq!(s2.completed, vec!["ch1".to_string()], "second complete must not duplicate");
}

#[test]
fn complete_quest_tracks_best_time_and_resets_attempt() {
  let _sb = CacheSandbox::new();
  let hash = "repo-bt";
  set_current_quest(hash, "ch1").unwrap();

  // First attempt: 30s. complete -> best = 30, attempt reset.
  add_elapsed(hash, "ch1", 30).unwrap();
  let (s, o) = complete_quest(hash, "ch1", None).unwrap();
  assert_eq!(o.attempt_seconds, 30);
  assert!(o.set_new_best, "first completion always sets best");
  assert_eq!(o.best_seconds, Some(30));
  assert_eq!(s.quests.get("ch1").unwrap().best_time_seconds, Some(30));
  assert_eq!(s.quests.get("ch1").unwrap().attempt_elapsed_seconds, 0, "attempt timer resets on complete");
  // Cumulative untouched.
  assert_eq!(s.quests.get("ch1").unwrap().elapsed_seconds, 30);

  // Second attempt: 45s (slower) — best stays 30.
  add_elapsed(hash, "ch1", 45).unwrap();
  let (s, o) = complete_quest(hash, "ch1", None).unwrap();
  assert!(!o.set_new_best);
  assert_eq!(o.best_seconds, Some(30));
  assert_eq!(s.quests.get("ch1").unwrap().best_time_seconds, Some(30));
  // Cumulative now 75.
  assert_eq!(s.quests.get("ch1").unwrap().elapsed_seconds, 75);

  // Third attempt: 20s — new best.
  add_elapsed(hash, "ch1", 20).unwrap();
  let (s, o) = complete_quest(hash, "ch1", None).unwrap();
  assert!(o.set_new_best);
  assert_eq!(o.best_seconds, Some(20));
  assert_eq!(s.quests.get("ch1").unwrap().best_time_seconds, Some(20));
}

#[test]
fn reset_attempt_clears_only_attempt_timer() {
  let _sb = CacheSandbox::new();
  let hash = "repo-ra";
  add_elapsed(hash, "ch1", 90).unwrap();
  let pre = read_progress(hash).unwrap();
  assert_eq!(pre.quests.get("ch1").unwrap().elapsed_seconds, 90);
  assert_eq!(pre.quests.get("ch1").unwrap().attempt_elapsed_seconds, 90);

  let after = reset_attempt(hash, "ch1").unwrap();
  assert_eq!(after.quests.get("ch1").unwrap().elapsed_seconds, 90, "cumulative untouched");
  assert_eq!(after.quests.get("ch1").unwrap().attempt_elapsed_seconds, 0);
}

#[test]
fn bump_helpers_return_latest_state_and_match_disk() {
  let _sb = CacheSandbox::new();
  let hash = "repo3";
  let s1 = bump_hints(hash, "ch1").unwrap();
  let s2 = bump_hints(hash, "ch1").unwrap();
  let s3 = bump_attempts(hash, "ch1").unwrap();
  assert_eq!(s1.quests.get("ch1").unwrap().hints_used, 1);
  assert_eq!(s2.quests.get("ch1").unwrap().hints_used, 2);
  assert_eq!(s3.quests.get("ch1").unwrap().attempts, 1);
  assert_eq!(s3.quests.get("ch1").unwrap().hints_used, 2, "attempts bump preserves hints");
  // Returned state matches what a fresh read sees.
  let disk = read_progress(hash).unwrap();
  assert_eq!(disk.quests.get("ch1").unwrap().hints_used, 2);
  assert_eq!(disk.quests.get("ch1").unwrap().attempts, 1);
}

#[test]
fn add_elapsed_accumulates_and_zero_delta_is_a_noop() {
  let _sb = CacheSandbox::new();
  let hash = "repo4";
  let s1 = add_elapsed(hash, "ch1", 30).unwrap();
  let s2 = add_elapsed(hash, "ch1", 45).unwrap();
  assert_eq!(s1.quests.get("ch1").unwrap().elapsed_seconds, 30);
  assert_eq!(s2.quests.get("ch1").unwrap().elapsed_seconds, 75);
  // Zero delta: returns current state, must not bump.
  let s3 = add_elapsed(hash, "ch1", 0).unwrap();
  assert_eq!(s3.quests.get("ch1").unwrap().elapsed_seconds, 75);
}

#[test]
fn progress_file_path_lives_inside_state_dir() {
  let _sb = CacheSandbox::new();
  let p = progress_path("xyz").unwrap();
  assert!(p.ends_with("state/xyz/progress.json"), "unexpected path: {p:?}");
  assert!(p.parent().unwrap().exists(), "state/xyz/ should be created");
}

#[test]
fn write_progress_round_trip_preserves_elapsed_seconds() {
  let _sb = CacheSandbox::new();
  let hash = "repo5";
  let mut state = ProgressState::empty();
  state.current_quest = Some("ch1".into());
  let q = state.ensure_quest("ch1");
  q.elapsed_seconds = 4242;
  write_progress(hash, &mut state).unwrap();
  let back = read_progress(hash).unwrap();
  assert_eq!(back.current_quest.as_deref(), Some("ch1"));
  assert_eq!(back.quests.get("ch1").unwrap().elapsed_seconds, 4242);
}

#[test]
fn sign_and_verify_completion_round_trip() {
  let secret = "deadbeef";
  let sig = sign_completion(secret, "ch1", "repohash", 1717_000_000);
  assert!(verify_completion(secret, "ch1", "repohash", 1717_000_000, &sig));
  // Any field change invalidates.
  assert!(!verify_completion(secret, "ch2", "repohash", 1717_000_000, &sig));
  assert!(!verify_completion(secret, "ch1", "otherrepo", 1717_000_000, &sig));
  assert!(!verify_completion(secret, "ch1", "repohash", 1717_000_001, &sig));
  assert!(!verify_completion("wrong-secret", "ch1", "repohash", 1717_000_000, &sig));
}

#[test]
fn signed_completion_round_trips_through_cache() {
  let _sb = CacheSandbox::new();
  let hash = "repo-sig";
  let secret = "s3cret";
  set_current_quest(hash, "ch1").unwrap();
  add_elapsed(hash, "ch1", 10).unwrap();
  let (state, _) = complete_quest(hash, "ch1", Some(secret)).unwrap();
  let sig = state.quests.get("ch1").unwrap().completion_sig.clone().expect("sig set");
  let ts = state.quests.get("ch1").unwrap().completed_at.unwrap().timestamp();
  assert!(verify_completion(secret, "ch1", hash, ts, &sig));
}

#[test]
fn enforce_completion_sigs_drops_forged_entries() {
  let _sb = CacheSandbox::new();
  let hash = "repo-forge";
  let secret = "real";
  // Legit completion.
  set_current_quest(hash, "ch1").unwrap();
  add_elapsed(hash, "ch1", 5).unwrap();
  let (mut state, _) = complete_quest(hash, "ch1", Some(secret)).unwrap();

  // Forged: add ch2 to completed with a fake sig.
  state.completed.push("ch2".to_string());
  let q = state.ensure_quest("ch2");
  q.completed_at = Some(chrono::Utc::now());
  q.completion_sig = Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into());
  // Also forge ch3 with no sig at all (pre-manifest style edit).
  state.completed.push("ch3".to_string());
  state.ensure_quest("ch3").completed_at = Some(chrono::Utc::now());

  let forged = enforce_completion_sigs(&mut state, hash, secret);
  assert!(forged.contains(&"ch2".to_string()));
  assert!(forged.contains(&"ch3".to_string()));
  assert!(!forged.contains(&"ch1".to_string()), "honest completion stays");
  assert_eq!(state.completed, vec!["ch1".to_string()]);
  assert!(state.quests.get("ch2").unwrap().completed_at.is_none());
  assert!(state.quests.get("ch3").unwrap().completed_at.is_none());
}
