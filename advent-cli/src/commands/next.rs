use advent_config::LoadedConfig;
use anyhow::{Result, bail};

pub async fn run(loaded: LoadedConfig) -> Result<()> {
  let branch = advent_quest::git::current_branch(&loaded.repo_root).await?;
  match loaded.config.find_by_branch(&branch) {
    Some(current) => {
      let outcome = advent_quest::tests::run_once(&loaded.config, &loaded.repo_root, current).await?;
      if !outcome.passed {
        if !outcome.stdout.is_empty() {
          eprintln!("{}", outcome.stdout);
        }
        if !outcome.stderr.is_empty() {
          eprintln!("{}", outcome.stderr);
        }
        bail!("tests failed — fix them and try `duck-advent next` again");
      }
      let manifest = advent_core::QuestManifest::load(&loaded.repo_root)?;
      let secret = manifest.as_ref().map(|m| m.secret.as_str());
      let (_state, _outcome) = advent_cache::complete_quest(&loaded.repo_hash, &current.slug, secret)?;
      if let Some(next) = loaded.config.next_after(&current.slug) {
        advent_quest::git::checkout(&loaded.repo_root, &next.slug).await?;
        let _ = advent_cache::set_current_quest(&loaded.repo_hash, &next.slug)?;
        println!("✓ {} complete — advanced to {}", current.title, next.title);
      } else {
        println!("✓ all quests complete!");
      }
    },
    None => {
      let first = loaded.config.first().clone();
      advent_quest::git::checkout(&loaded.repo_root, &first.slug).await?;
      let _ = advent_cache::set_current_quest(&loaded.repo_hash, &first.slug)?;
      println!("✦ starting first quest: {}", first.title);
    },
  }
  Ok(())
}
