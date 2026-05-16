use crate::{Cmd, ManifestCmd};
use anyhow::Result;

mod doctor;
mod init;
mod manifest;
mod next;
mod open;
mod prev;
mod repeat;
mod status;

pub async fn dispatch(cmd: Cmd, version: &str) -> Result<()> {
  // Init is the only command that runs WITHOUT a quest.config.ts — it
  // creates one. Every other command requires a loaded config.
  if matches!(cmd, Cmd::Init) {
    return init::run(&std::env::current_dir()?).await;
  }
  let cwd = std::env::current_dir()?;
  let loaded = advent_config::load(&cwd).await?;
  match cmd {
    Cmd::Open => open::run(loaded, version).await,
    Cmd::Next => next::run(loaded).await,
    Cmd::Prev => prev::run(loaded).await,
    Cmd::Status => status::run(loaded).await,
    Cmd::Doctor => doctor::run(loaded).await,
    Cmd::Repeat => repeat::run(loaded).await,
    Cmd::Manifest(sub) => match sub {
      ManifestCmd::Gen => manifest::run_gen(loaded).await,
      ManifestCmd::Verify => manifest::run_verify(loaded).await,
    },
    Cmd::Init => unreachable!("handled above"),
  }
}
