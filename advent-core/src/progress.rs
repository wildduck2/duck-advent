use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressState {
  pub current_quest: Option<String>,
  #[serde(default)]
  pub completed: Vec<String>,
  pub started_at: DateTime<Utc>,
  pub last_updated_at: DateTime<Utc>,
  #[serde(default)]
  pub quests: BTreeMap<String, QuestStats>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuestStats {
  pub started_at: Option<DateTime<Utc>>,
  pub completed_at: Option<DateTime<Utc>>,
  #[serde(default)]
  pub hints_used: u32,
  #[serde(default)]
  pub attempts: u32,
  /// Total seconds spent on this quest, cumulative across attempts + sessions.
  /// Never resets — `<leader> r` keeps adding to this. Persisted so closing
  /// and reopening the workspace continues the same cumulative count.
  #[serde(default)]
  pub elapsed_seconds: u64,
  /// Seconds on the CURRENT attempt only. Resets to 0 on `<leader> r` and
  /// on successful completion. Drives `best_time_seconds`.
  #[serde(default)]
  pub attempt_elapsed_seconds: u64,
  /// Fastest single-attempt solve time recorded for this quest. `None` until
  /// first completion. Updated only when a later attempt beats it.
  #[serde(default)]
  pub best_time_seconds: Option<u64>,
  /// HMAC-SHA256 of `"complete:<slug>:<repo_hash>:<completed_at_unix>"` keyed
  /// by the manifest secret. Set on successful completion; read on load and
  /// any entry with a missing or mismatched sig is treated as incomplete.
  /// Hex-encoded.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub completion_sig: Option<String>,
}

impl ProgressState {
  pub fn empty() -> Self {
    let now = Utc::now();
    Self { current_quest: None, completed: Vec::new(), started_at: now, last_updated_at: now, quests: BTreeMap::new() }
  }

  pub fn touch(&mut self) {
    self.last_updated_at = Utc::now();
  }

  pub fn ensure_quest(&mut self, slug: &str) -> &mut QuestStats {
    self.quests.entry(slug.to_string()).or_default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_initialises_timestamps_consistently() {
    let s = ProgressState::empty();
    assert!(s.current_quest.is_none());
    assert!(s.completed.is_empty());
    assert!(s.quests.is_empty());
    assert!(s.last_updated_at >= s.started_at, "last_updated never precedes started");
  }

  #[test]
  fn ensure_quest_creates_default_then_returns_existing() {
    let mut s = ProgressState::empty();
    let q = s.ensure_quest("alpha");
    assert_eq!(q.hints_used, 0);
    assert_eq!(q.attempts, 0);
    assert_eq!(q.elapsed_seconds, 0);
    q.attempts = 5;
    // Second call must return the same entry, not overwrite.
    let q2 = s.ensure_quest("alpha");
    assert_eq!(q2.attempts, 5);
  }

  #[test]
  fn touch_moves_last_updated_forward() {
    let mut s = ProgressState::empty();
    let before = s.last_updated_at;
    std::thread::sleep(std::time::Duration::from_millis(5));
    s.touch();
    assert!(s.last_updated_at > before);
  }

  #[test]
  fn legacy_progress_without_elapsed_seconds_deserialises_with_default() {
    // Snapshot of a progress.json written before the timer feature existed.
    let legacy = r#"{
      "current_quest": "chapter-01",
      "completed": [],
      "started_at": "2026-05-10T12:00:00Z",
      "last_updated_at": "2026-05-10T12:30:00Z",
      "quests": {
        "chapter-01": {
          "started_at": "2026-05-10T12:00:00Z",
          "completed_at": null,
          "hints_used": 2,
          "attempts": 3
        }
      }
    }"#;
    let state: ProgressState = serde_json::from_str(legacy).expect("legacy progress should still parse");
    let q = state.quests.get("chapter-01").expect("quest entry survives");
    assert_eq!(q.hints_used, 2);
    assert_eq!(q.attempts, 3);
    assert_eq!(q.elapsed_seconds, 0, "missing field defaults to 0, never panics");
  }

  #[test]
  fn quest_stats_round_trips_through_serde() {
    let original = QuestStats { hints_used: 1, attempts: 2, elapsed_seconds: 360, ..QuestStats::default() };
    let json = serde_json::to_string(&original).unwrap();
    let back: QuestStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.hints_used, 1);
    assert_eq!(back.attempts, 2);
    assert_eq!(back.elapsed_seconds, 360);
  }
}
