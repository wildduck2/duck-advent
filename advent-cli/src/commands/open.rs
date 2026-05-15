use advent_config::LoadedConfig;
use anyhow::Result;

pub async fn run(loaded: LoadedConfig, version: &str) -> Result<()> {
  advent_tui::run(loaded, version.to_string()).await?;
  Ok(())
}
