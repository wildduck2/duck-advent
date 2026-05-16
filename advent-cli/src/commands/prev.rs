use advent_config::LoadedConfig;
use anyhow::{Result, bail};

pub async fn run(loaded: LoadedConfig) -> Result<()> {
  let branch = advent_quest::git::current_branch(&loaded.repo_root).await?;
  let current = loaded
    .config
    .find_by_branch(&branch)
    .ok_or_else(|| anyhow::anyhow!("not on a recognized quest branch — run `duck-advent` to begin"))?;
  let Some(prev) = loaded.config.prev_before(&current.slug) else {
    bail!("already at the first quest ({}) — no previous quest to switch to", current.title);
  };
  if !advent_quest::git::working_tree_clean(&loaded.repo_root).await? {
    bail!("working tree has edits — commit, stash, or run `duck-advent repeat` before switching");
  }
  advent_quest::git::checkout(&loaded.repo_root, &prev.slug).await?;
  let _ = advent_cache::set_current_quest(&loaded.repo_hash, &prev.slug)?;
  println!("← switched back to {}: {}", prev.slug, prev.title);
  Ok(())
}
