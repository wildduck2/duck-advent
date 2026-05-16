use advent_config::LoadedConfig;
use advent_core::{ChapterManifest, QuestManifest, derive_secret, file_hash};
use anyhow::{Result, bail};
use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

/// Scan the repo for test files belonging to each declared quest, then write
/// `.duck-manifest.json` at the repo root. Idempotent — re-running yields the
/// same manifest as long as test contents are unchanged. The HMAC secret is
/// re-derived from the hashes, so any test edit invalidates every existing
/// signed completion (and thus wipes forged progress).
pub async fn run_gen(loaded: LoadedConfig) -> Result<()> {
  let mut chapters: BTreeMap<String, ChapterManifest> = BTreeMap::new();
  let test_root = loaded.repo_root.join("test");

  for quest in &loaded.config.quests {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let filter = quest.test_filter.as_deref().unwrap_or(&quest.slug);
    // Walk `test/` and pick files whose path contains the filter. Cheap +
    // good-enough heuristic for the redis-advent layout where each chapter
    // owns `test/e2e/<filter>.e2e-spec.ts` plus any matching unit files.
    collect_matching(&test_root, &loaded.repo_root, filter, &mut files)?;
    if files.is_empty() {
      // No matching test file means the quest can never be validated. Loud
      // so the author notices instead of shipping a manifest that's silent
      // on tampering.
      eprintln!("warn: quest {} ({}) has no matching test files under test/", quest.number, quest.slug);
    }
    chapters.insert(quest.slug.clone(), ChapterManifest { test_files: files });
  }

  let secret = derive_secret(&chapters);
  let manifest = QuestManifest { version: 1, secret, chapters };
  let path = manifest.save(&loaded.repo_root)?;
  println!("✓ wrote {}", path.display());
  println!("  chapters: {}", manifest.chapters.len());
  let total_files: usize = manifest.chapters.values().map(|c| c.test_files.len()).sum();
  println!("  pinned files: {total_files}");
  Ok(())
}

/// Verify the active chapter's test files. Exits non-zero (via `bail!`) on
/// drift so CI / shell scripts can chain it.
pub async fn run_verify(loaded: LoadedConfig) -> Result<()> {
  let Some(manifest) = QuestManifest::load(&loaded.repo_root)? else {
    bail!("no .duck-manifest.json — run `duck-advent manifest gen` first");
  };
  let branch = advent_quest::git::current_branch(&loaded.repo_root).await?;
  let Some(quest) = loaded.config.find_by_branch(&branch) else {
    bail!("not on a recognized quest branch");
  };
  let drift = manifest.verify_chapter(&loaded.repo_root, &quest.slug)?;
  if drift.is_empty() {
    println!("✓ all {} files match", manifest.chapters.get(&quest.slug).map(|c| c.test_files.len()).unwrap_or(0));
    Ok(())
  } else {
    eprintln!("✗ {} file(s) drifted from manifest:", drift.len());
    for p in &drift {
      eprintln!("    {p}");
    }
    eprintln!();
    eprintln!("Restore the originals with `git checkout HEAD -- <path>`, or");
    eprintln!("regenerate the manifest with `duck-advent manifest gen` if the change is intentional.");
    bail!("manifest verification failed");
  }
}

fn collect_matching(
  dir: &Path,
  repo_root: &Path,
  filter: &str,
  out: &mut BTreeMap<String, String>,
) -> Result<()> {
  if !dir.exists() {
    return Ok(());
  }
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.is_dir() {
      collect_matching(&path, repo_root, filter, out)?;
      continue;
    }
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else { continue };
    // Heuristic: file name contains the filter (e.g. "chapter-04"). Catches
    // `chapter-04.e2e-spec.ts`, `chapter-04.spec.ts`, etc.
    if !name.contains(filter) {
      continue;
    }
    let bytes = std::fs::read(&path)?;
    let rel = path.strip_prefix(repo_root).unwrap_or(&path).to_string_lossy().into_owned();
    out.insert(rel, file_hash(&bytes));
  }
  Ok(())
}

#[allow(dead_code)]
fn _ensure_path_is_absolute(_p: &PathBuf) {}
