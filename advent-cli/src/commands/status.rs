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
    let elapsed = progress.quests.get(&q.slug).map(|s| s.elapsed_seconds).unwrap_or(0);
    println!("time:      {}", fmt_elapsed(elapsed));
  } else {
    println!("quest:     (not on a recognized quest branch — run `duck-advent` to begin)");
  }
  println!("completed: {}/{}", progress.completed.len(), total);
  let journey_secs: u64 = progress.quests.values().map(|q| q.elapsed_seconds).sum();
  println!("journey time: {}", fmt_elapsed(journey_secs));
  Ok(())
}

fn fmt_elapsed(total_secs: u64) -> String {
  let h = total_secs / 3600;
  let m = (total_secs % 3600) / 60;
  let s = total_secs % 60;
  if h > 0 { format!("{h}h {m:02}m {s:02}s") } else { format!("{m:02}m {s:02}s") }
}
