use advent_config::LoadedConfig;
use anyhow::Result;

pub async fn run(loaded: LoadedConfig) -> Result<()> {
  for v in &loaded.config.validators {
    let out = tokio::process::Command::new(&v.cmd[0]).args(&v.cmd[1..]).output().await;
    let icon = match &out {
      Ok(o) if o.status.success() => "✓",
      _ => "✗",
    };
    println!("  {icon} {} — {}", v.label, v.cmd.join(" "));
    if let Ok(o) = &out
      && !o.status.success()
    {
      let err = String::from_utf8_lossy(&o.stderr);
      if !err.trim().is_empty() {
        println!("      {}", err.trim());
      }
    } else if let Err(e) = &out {
      println!("      {e}");
    }
  }
  Ok(())
}
