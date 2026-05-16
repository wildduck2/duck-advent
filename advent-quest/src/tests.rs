use advent_core::{AdventError, AdventResult, QuestConfig, QuestStep};
use std::{
  path::Path,
  process::Stdio,
  sync::Arc,
};
use tokio::{
  io::{AsyncBufReadExt, BufReader},
  process::Command,
  sync::{Mutex, mpsc},
  time::{Duration, timeout},
};

/// Outcome of `run_once`. `stdout`/`stderr` hold the captured output for the
/// failure overlay; UI consumers also receive incremental lines via the
/// `mpsc::Receiver<String>` returned by [`run_streaming`].
pub struct TestOutcome {
  pub passed: bool,
  pub stdout: String,
  pub stderr: String,
  pub cancelled: bool,
  pub timed_out: bool,
}

/// Hard upper bound on how long a one-shot validation may take. Vitest cold
/// starts are ~10s; chapters with blocking commands (BLPOP/BRPOP/BLMOVE) can
/// stack up to ~30s per failing test. 60s covers honest runs without letting
/// a hanging worker burn minutes of the user's life.
///
/// Override at runtime with `DUCK_TEST_TIMEOUT_SECS=<n>` for chapters that
/// genuinely need longer (large streaming benchmarks, cluster MEET dances).
const DEFAULT_TIMEOUT_SECS: u64 = 60;

fn resolve_timeout() -> Duration {
  let secs = std::env::var("DUCK_TEST_TIMEOUT_SECS")
    .ok()
    .and_then(|s| s.parse::<u64>().ok())
    .filter(|n| *n > 0)
    .unwrap_or(DEFAULT_TIMEOUT_SECS);
  Duration::from_secs(secs)
}

/// Per-test soft cap passed to vitest via `--testTimeout` and `--hookTimeout`.
/// Stops a single hung test from chewing the full 30s default × N tests.
/// Override with `DUCK_PER_TEST_TIMEOUT_MS=<n>`.
const DEFAULT_PER_TEST_TIMEOUT_MS: u64 = 10_000;

fn resolve_per_test_timeout_ms() -> u64 {
  std::env::var("DUCK_PER_TEST_TIMEOUT_MS")
    .ok()
    .and_then(|s| s.parse::<u64>().ok())
    .filter(|n| *n > 0)
    .unwrap_or(DEFAULT_PER_TEST_TIMEOUT_MS)
}

/// Build the argv for a one-shot validation run.
///
/// * strips `--watch`/`-w` from the user's `test_command`
/// * appends the quest's `testFilter` (if any)
/// * forces vitest into run mode with `--run` when the binary is vitest and
///   no run flag is already present — vitest defaults to watch mode in TTYs
///   and hangs when invoked without `--run`.
/// * injects `--testTimeout` and `--hookTimeout` so a single hanging test
///   can't stack up to multiple minutes. The repo's `vitest.config.ts`
///   default of 30s is generous; for chapters that depend on blocking
///   commands (BLPOP, BLMOVE) a TODO stub stacking 6×30s would wait three
///   minutes per `<leader> n`. Capping at 10s per test keeps the worst-case
///   suite under a minute.
pub fn one_shot_argv(config: &QuestConfig, quest: &QuestStep) -> Vec<String> {
  let mut argv: Vec<String> =
    config.test_command.iter().filter(|a| !matches!(a.as_str(), "--watch" | "-w")).cloned().collect();
  if let Some(f) = &quest.test_filter
    && !argv.iter().any(|a| a == f)
  {
    argv.push(f.clone());
  }
  let is_vitest = argv.iter().any(|a| a == "vitest" || a.ends_with("/vitest"));
  let has_run_flag = argv.iter().any(|a| matches!(a.as_str(), "run" | "--run"));
  if is_vitest && !has_run_flag {
    // Insert `--run` right after the vitest token so positional filters keep
    // being interpreted as patterns.
    let pos = argv.iter().position(|a| a == "vitest" || a.ends_with("/vitest")).unwrap_or(0) + 1;
    argv.insert(pos, "--run".to_string());
  }
  if is_vitest {
    let ms = resolve_per_test_timeout_ms();
    if !argv.iter().any(|a| a.starts_with("--testTimeout")) {
      argv.push(format!("--testTimeout={ms}"));
    }
    if !argv.iter().any(|a| a.starts_with("--hookTimeout")) {
      argv.push(format!("--hookTimeout={ms}"));
    }
  }
  argv
}

/// Watch-mode argv (keeps `--watch`, no auto-injection of `--run`).
pub fn watch_argv(config: &QuestConfig, quest: &QuestStep) -> Vec<String> {
  let mut argv = config.test_command.clone();
  if let Some(f) = &quest.test_filter
    && !argv.iter().any(|a| a == f)
  {
    argv.push(f.clone());
  }
  argv
}

/// Run-once with live line streaming + timeout + cooperative cancel.
///
/// * `cancel`: when set to `true` from another task, the child is killed.
/// * `out_tx`: every captured line is sent here. Caller can render the tail.
///
/// Returns once the child exits (success / failure / killed / timed out).
pub async fn run_streaming(
  config: &QuestConfig,
  repo_root: &Path,
  quest: &QuestStep,
  cancel: Arc<Mutex<bool>>,
  out_tx: mpsc::UnboundedSender<String>,
) -> AdventResult<TestOutcome> {
  let argv = one_shot_argv(config, quest);
  let (bin, args) = argv.split_first().ok_or_else(|| AdventError::ConfigParse("empty test_command".into()))?;

  let mut cmd = Command::new(bin);
  cmd.args(args)
    .current_dir(repo_root)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .env("CI", "1")
    .env("FORCE_COLOR", "0");

  let mut child = cmd.spawn().map_err(AdventError::BareIo)?;
  let stdout = child.stdout.take().expect("stdout piped");
  let stderr = child.stderr.take().expect("stderr piped");

  let stdout_acc: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
  let stderr_acc: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

  let stdout_task = spawn_reader(stdout, "", out_tx.clone(), Arc::clone(&stdout_acc));
  let stderr_task = spawn_reader(stderr, "", out_tx.clone(), Arc::clone(&stderr_acc));

  let kill_handle = child.id();
  let cancel_clone = Arc::clone(&cancel);
  let kill_task = tokio::spawn(async move {
    loop {
      if *cancel_clone.lock().await {
        if let Some(pid) = kill_handle {
          let _ = tokio::process::Command::new("kill").arg("-TERM").arg(pid.to_string()).status().await;
        }
        break;
      }
      tokio::time::sleep(Duration::from_millis(150)).await;
    }
  });

  let wait_result = timeout(resolve_timeout(), child.wait()).await;
  let _ = stdout_task.await;
  let _ = stderr_task.await;
  kill_task.abort();

  let stdout = stdout_acc.lock().await.clone();
  let stderr = stderr_acc.lock().await.clone();
  let cancelled = *cancel.lock().await;
  let timed_out = wait_result.is_err();
  let passed = matches!(&wait_result, Ok(Ok(status)) if status.success()) && !cancelled && !timed_out;
  Ok(TestOutcome { passed, stdout, stderr, cancelled, timed_out })
}

fn spawn_reader(
  stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
  prefix: &'static str,
  tx: mpsc::UnboundedSender<String>,
  acc: Arc<Mutex<String>>,
) -> tokio::task::JoinHandle<()> {
  tokio::spawn(async move {
    let mut reader = BufReader::new(stream).lines();
    while let Ok(Some(line)) = reader.next_line().await {
      acc.lock().await.push_str(&line);
      acc.lock().await.push('\n');
      let _ = tx.send(format!("{prefix}{line}"));
    }
  })
}

/// Back-compat. Internally uses `run_streaming` with a no-op cancel + ignored
/// stream. Kept for the CLI's `duck-advent next` subcommand.
pub async fn run_once(config: &QuestConfig, repo_root: &Path, quest: &QuestStep) -> AdventResult<TestOutcome> {
  let cancel = Arc::new(Mutex::new(false));
  let (tx, _rx) = mpsc::unbounded_channel();
  run_streaming(config, repo_root, quest, cancel, tx).await
}
