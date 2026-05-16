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

  pub fn prev_before(&self, slug: &str) -> Option<&QuestStep> {
    let idx = self.quests.iter().position(|q| q.slug == slug)?;
    if idx == 0 { None } else { self.quests.get(idx - 1) }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn step(n: u32, slug: &str) -> QuestStep {
    QuestStep {
      number: n,
      slug: slug.into(),
      title: slug.into(),
      tier: None,
      difficulty: None,
      briefing: format!("docs/{slug}.md"),
      workdir: format!("src/{slug}"),
      test_filter: None,
      services: Vec::new(),
      seed: None,
      hints: Vec::new(),
    }
  }

  fn config(slugs: &[&str]) -> QuestConfig {
    QuestConfig {
      name: "Test".into(),
      description: None,
      package_manager: default_package_manager(),
      install_command: vec!["bun".into(), "install".into()],
      test_command: vec!["bunx".into(), "vitest".into()],
      branch_prefix: default_branch_prefix(),
      cache_dir: default_cache_dir(),
      validators: Vec::new(),
      services: BTreeMap::new(),
      quests: slugs.iter().enumerate().map(|(i, s)| step(i as u32 + 1, s)).collect(),
    }
  }

  #[test]
  fn first_returns_index_zero() {
    let c = config(&["a", "b", "c"]);
    assert_eq!(c.first().slug, "a");
  }

  #[test]
  fn find_by_slug_branch_match() {
    let c = config(&["a", "b", "c"]);
    assert_eq!(c.find_by_slug("b").map(|q| &q.slug), Some(&"b".to_string()));
    assert_eq!(c.find_by_branch("b").map(|q| &q.slug), Some(&"b".to_string()));
    assert!(c.find_by_slug("missing").is_none());
    assert!(c.find_by_branch("missing").is_none());
  }

  #[test]
  fn next_after_walks_forward() {
    let c = config(&["a", "b", "c"]);
    assert_eq!(c.next_after("a").map(|q| q.slug.as_str()), Some("b"));
    assert_eq!(c.next_after("b").map(|q| q.slug.as_str()), Some("c"));
    assert!(c.next_after("c").is_none(), "last quest has no next");
    assert!(c.next_after("missing").is_none());
  }

  #[test]
  fn prev_before_walks_backward() {
    let c = config(&["a", "b", "c"]);
    assert!(c.prev_before("a").is_none(), "first quest has no prev");
    assert_eq!(c.prev_before("b").map(|q| q.slug.as_str()), Some("a"));
    assert_eq!(c.prev_before("c").map(|q| q.slug.as_str()), Some("b"));
    assert!(c.prev_before("missing").is_none());
  }

  #[test]
  fn single_quest_config_has_no_neighbours() {
    let c = config(&["only"]);
    assert_eq!(c.first().slug, "only");
    assert!(c.next_after("only").is_none());
    assert!(c.prev_before("only").is_none());
  }
}
