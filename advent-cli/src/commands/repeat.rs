use advent_config::LoadedConfig;
use anyhow::{Result, bail};

pub async fn run(loaded: LoadedConfig) -> Result<()> {
  let branch = advent_quest::git::current_branch(&loaded.repo_root).await?;
  let Some(quest) = loaded.config.find_by_branch(&branch) else {
    bail!("not on a recognized quest branch");
  };
  advent_quest::git::discard_workdir(&loaded.repo_root, &quest.workdir).await?;
  println!("✓ discarded edits in {} — quest {} reset", quest.workdir, quest.title);
  Ok(())
}
