use advent_config::LoadedConfig;
use anyhow::Result;

pub async fn run(loaded: LoadedConfig) -> Result<()> {
  let branch = advent_quest::git::current_branch(&loaded.repo_root).await?;
  let progress = advent_cache::read_progress(&loaded.repo_hash)?;
  let total = loaded.config.quests.len();
  println!("journey:   {}", loaded.config.name);
  println!("branch:    {branch}");
  println!("repo:      {}", loaded.repo_root.display());
  if let Some(q) = loaded.config.find_by_branch(&branch) {
    println!("quest:     {}/{} — {}", q.number, total, q.title);
    if let Some(t) = &q.tier {
      println!("tier:      {t}");
    }
    if let Some(d) = q.difficulty {
      println!("difficulty: {d}/5");
    }
    let used = progress.quests.get(&q.slug).map(|s| s.hints_used).unwrap_or(0);
    println!("hints:     {used}/{}", q.hints.len());
  } else {
    println!("quest:     (not on a recognized quest branch — run `duck-advent` to begin)");
  }
  println!("completed: {}/{}", progress.completed.len(), total);
  Ok(())
}
