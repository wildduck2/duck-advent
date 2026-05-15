use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level shape of a `quest.config.ts` after the bun bridge dumps it to JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestConfig {
  pub name: String,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default = "default_package_manager")]
  pub package_manager: String,
  pub install_command: Vec<String>,
  pub test_command: Vec<String>,
  #[serde(default = "default_branch_prefix")]
  pub branch_prefix: String,
  #[serde(default = "default_cache_dir")]
  pub cache_dir: String,
  #[serde(default)]
  pub validators: Vec<ValidatorSpec>,
  #[serde(default)]
  pub services: BTreeMap<String, ServiceSpec>,
  /// Accept either `quests` (preferred) or `chapters` (legacy). The bun
  /// bridge normalizes both into this field before serializing.
  #[serde(alias = "chapters")]
  pub quests: Vec<QuestStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestStep {
  pub number: u32,
  pub slug: String,
  pub title: String,
  #[serde(default)]
  pub tier: Option<String>,
  #[serde(default)]
  pub difficulty: Option<u8>,
  pub briefing: String,
  pub workdir: String,
  #[serde(default)]
  pub test_filter: Option<String>,
  #[serde(default)]
  pub services: Vec<String>,
  #[serde(default)]
  pub seed: Option<String>,
  #[serde(default)]
  pub hints: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceSpec {
  pub compose: String,
  pub container: String,
  #[serde(default)]
  pub ready_check: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorSpec {
  pub id: String,
  pub label: String,
  pub cmd: Vec<String>,
  #[serde(default)]
  pub min: Option<String>,
  #[serde(default)]
  pub optional: bool,
}

fn default_package_manager() -> String {
  "bun".into()
}
fn default_branch_prefix() -> String {
  "chapter-".into()
}
fn default_cache_dir() -> String {
  ".gentleduck".into()
}

impl QuestConfig {
  pub fn find_by_branch(&self, branch: &str) -> Option<&QuestStep> {
    self.quests.iter().find(|q| q.slug == branch)
  }

  pub fn find_by_slug(&self, slug: &str) -> Option<&QuestStep> {
    self.quests.iter().find(|q| q.slug == slug)
  }

  pub fn first(&self) -> &QuestStep {
    &self.quests[0]
  }

  pub fn next_after(&self, slug: &str) -> Option<&QuestStep> {
    let idx = self.quests.iter().position(|q| q.slug == slug)?;
    self.quests.get(idx + 1)
  }
}
