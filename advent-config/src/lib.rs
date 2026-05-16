//! Loads `quest.config.ts` from the target repo by subprocessing `bun`.
//!
//! Approach: spawn `bun -e "...bridge..."` with the path of the user's
//! quest.config.ts. The bridge imports the default export, projects the
//! camelCase TS shape into the snake_case JSON serde expects, and writes to
//! stdout. No legacy aliases are accepted — the config MUST be authored with
//! `defineConfig({...})` exported as default.

use advent_core::{AdventError, AdventResult, QuestConfig};
use sha2::{Digest, Sha256};
use std::{
  path::{Path, PathBuf},
  process::Stdio,
};
use tokio::process::Command;

/// Locate the nearest `quest.config.{ts,mjs,js}` walking up from `start`.
pub fn find_config(start: &Path) -> AdventResult<PathBuf> {
  let candidates = ["quest.config.ts", "quest.config.mjs", "quest.config.js"];
  let mut dir = start.to_path_buf();
  loop {
    for name in candidates {
      let p = dir.join(name);
      if p.is_file() {
        return Ok(p);
      }
    }
    if !dir.pop() {
      break;
    }
  }
  Err(AdventError::ConfigNotFound { start: start.to_path_buf() })
}

pub fn repo_root_for(config_path: &Path) -> PathBuf {
  config_path.parent().unwrap_or(Path::new(".")).to_path_buf()
}

pub fn config_hash(content: &str) -> String {
  let mut h = Sha256::new();
  h.update(content.as_bytes());
  hex::encode(h.finalize())
}

/// Bridge script: `bun` evaluates this. We pass the config path via the
/// `DUCK_CONFIG` env var because `bun -e` argv handling is inconsistent.
///
/// Authoring contract (strict, no backwards-compat):
///   - default export MUST come from `defineConfig({...})`
///   - top-level keys use camelCase: `installCommand`, `testCommand`,
///     `branchPrefix`, `cacheDir`, `packageManager`
///   - per-quest keys use camelCase: `testFilter`
///   - the list of quests MUST be called `quests` (not `chapters`)
/// Any legacy snake_case top-level key or `chapters` alias is a hard error.
const BRIDGE: &str = r#"
const path = process.env.DUCK_CONFIG;
if (!path) {
  console.error('DUCK_CONFIG env var missing');
  process.exit(1);
}
import(path).then((mod) => {
  const c = mod.default;
  if (!c || typeof c !== 'object') {
    console.error('quest.config: missing default export (use `export default defineConfig({...})`)');
    process.exit(1);
  }
  if ('chapters' in c) {
    console.error('quest.config: `chapters` is no longer supported — rename it to `quests`');
    process.exit(1);
  }
  if (!Array.isArray(c.quests) || c.quests.length === 0) {
    console.error('quest.config: `quests` must be a non-empty array');
    process.exit(1);
  }
  for (const k of ['installCommand', 'testCommand']) {
    if (!Array.isArray(c[k])) {
      console.error(`quest.config: required field \`${k}\` missing or not an array`);
      process.exit(1);
    }
  }
  const out = {
    name: c.name,
    description: c.description,
    package_manager: c.packageManager ?? 'bun',
    install_command: c.installCommand,
    test_command: c.testCommand,
    branch_prefix: c.branchPrefix ?? 'chapter-',
    cache_dir: c.cacheDir ?? '.gentleduck',
    validators: c.validators ?? [],
    services: c.services ?? {},
    quests: c.quests.map((q) => ({
      number: q.number,
      slug: q.slug,
      title: q.title,
      tier: q.tier ?? null,
      difficulty: q.difficulty ?? null,
      briefing: q.briefing,
      workdir: q.workdir,
      test_filter: q.testFilter ?? null,
      services: q.services ?? [],
      seed: q.seed ?? null,
      hints: q.hints ?? [],
    })),
  };
  process.stdout.write(JSON.stringify(out));
}).catch((err) => {
  console.error(err && err.message ? err.message : String(err));
  process.exit(1);
});
"#;

#[derive(Debug, Clone)]
pub struct LoadedConfig {
  pub repo_root: PathBuf,
  pub config_path: PathBuf,
  pub repo_hash: String,
  pub config_hash: String,
  pub config: QuestConfig,
}

/// Each entry describes how to invoke a TypeScript-capable runtime so that
/// the bridge script in `BRIDGE` ends up running with the user's
/// `quest.config.ts` reachable via the `DUCK_CONFIG` env var.
///
/// We probe runtimes in order and fall back to the next one when the binary
/// isn't on PATH. Bun is preferred (no install dance, native TS), node is the
/// graceful fallback for environments without bun.
fn runtimes() -> &'static [(&'static str, &'static [&'static str])] {
  // `node --import tsx/esm` lets node load .ts files via tsx's loader hook. We
  // pass our bridge as a -e string in both cases.
  &[
    ("bun", &["-e"]),
    ("node", &["--import", "tsx/esm", "-e"]),
  ]
}

pub async fn load(start: &Path) -> AdventResult<LoadedConfig> {
  let config_path = find_config(start)?;
  let repo_root = repo_root_for(&config_path);
  let raw = tokio::fs::read_to_string(&config_path).await.map_err(|e| AdventError::io(config_path.clone(), e))?;
  let config_hash = config_hash(&raw);
  let repo_hash = advent_cache::repo_hash(&repo_root);

  let mut last_err: Option<String> = None;
  let mut output: Option<std::process::Output> = None;
  for (bin, args) in runtimes() {
    let mut cmd = Command::new(bin);
    cmd.args(*args)
      .arg(BRIDGE)
      .env("DUCK_CONFIG", &config_path)
      .current_dir(&repo_root)
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    match cmd.output().await {
      Ok(out) => {
        output = Some(out);
        break;
      },
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        last_err = Some(format!("runtime `{bin}` not on PATH"));
        continue;
      },
      Err(e) => return Err(AdventError::ConfigParse(format!("failed to spawn {bin}: {e}"))),
    }
  }
  let output = output.ok_or_else(|| {
    AdventError::ConfigParse(format!(
      "no TypeScript runtime found (install bun or node+tsx) — {}",
      last_err.unwrap_or_else(|| "no candidates".into())
    ))
  })?;

  if !output.status.success() {
    return Err(AdventError::BunBridge {
      code: output.status.code().unwrap_or(-1),
      stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    });
  }

  let parsed: QuestConfig = serde_json::from_slice(&output.stdout).map_err(AdventError::Json)?;
  Ok(LoadedConfig { repo_root, config_path, repo_hash, config_hash, config: parsed })
}
