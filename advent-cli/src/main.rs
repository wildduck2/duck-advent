use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod commands;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "duck-advent", version = VERSION, about = "TUI quest runner with embedded nvim + tests.")]
struct Cli {
  #[command(subcommand)]
  command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
  /// Open the TUI for the repo in the current directory (default).
  Open,
  /// Validate tests pass, then advance to the next quest.
  Next,
  /// Print current journey state.
  Status,
  /// Run all validators (bypass cache).
  Doctor,
  /// Discard your edits inside the current quest's workdir.
  Repeat,
  /// Scaffold a quest.config.ts in the current directory.
  Init,
}

#[tokio::main]
async fn main() -> ExitCode {
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .with_writer(std::io::stderr)
    .init();

  let cmd = Cli::parse().command.unwrap_or(Cmd::Open);
  match commands::dispatch(cmd, VERSION).await {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("✗ {err}");
      ExitCode::from(1)
    },
  }
}
