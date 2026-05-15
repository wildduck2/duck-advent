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
