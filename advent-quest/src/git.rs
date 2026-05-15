use advent_core::{AdventError, AdventResult};
use std::path::Path;
use tokio::process::Command;

async fn capture(args: &[&str], cwd: &Path) -> AdventResult<(i32, String, String)> {
  let out =
    Command::new("git").args(args).current_dir(cwd).output().await.map_err(|e| AdventError::Git(e.to_string()))?;
  Ok((
    out.status.code().unwrap_or(-1),
    String::from_utf8_lossy(&out.stdout).into_owned(),
    String::from_utf8_lossy(&out.stderr).into_owned(),
  ))
}

pub async fn current_branch(cwd: &Path) -> AdventResult<String> {
  let (code, stdout, stderr) = capture(&["rev-parse", "--abbrev-ref", "HEAD"], cwd).await?;
  if code != 0 {
    return Err(AdventError::Git(stderr.trim().to_string()));
  }
  Ok(stdout.trim().to_string())
}

pub async fn working_tree_clean(cwd: &Path) -> AdventResult<bool> {
  let (_, stdout, _) = capture(&["status", "--porcelain"], cwd).await?;
  Ok(stdout.trim().is_empty())
}

pub async fn branch_exists(cwd: &Path, branch: &str) -> AdventResult<bool> {
  let arg = format!("refs/heads/{branch}");
  let (code, _, _) = capture(&["rev-parse", "--verify", &arg], cwd).await?;
  Ok(code == 0)
}

pub async fn checkout(cwd: &Path, branch: &str) -> AdventResult<()> {
  let (code, _, stderr) = capture(&["checkout", branch], cwd).await?;
  if code != 0 {
    return Err(AdventError::Git(stderr.trim().to_string()));
  }
  Ok(())
}

/// Discard the user's working-tree edits inside `workdir` (and remove
/// untracked files within it). Used by the `repeat` action so the quest
/// resets cleanly without touching the rest of the repo.
pub async fn discard_workdir(cwd: &Path, workdir: &str) -> AdventResult<()> {
  let (code, _, stderr) = capture(&["checkout", "HEAD", "--", workdir], cwd).await?;
  if code != 0 {
    return Err(AdventError::Git(stderr.trim().to_string()));
  }
  let (code, _, stderr) = capture(&["clean", "-fd", "--", workdir], cwd).await?;
  if code != 0 {
    return Err(AdventError::Git(stderr.trim().to_string()));
  }
  Ok(())
}
