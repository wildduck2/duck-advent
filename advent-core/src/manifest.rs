//! Quest integrity manifest.
//!
//! Lives at the repo root as `.duck-manifest.json`. Pins per-chapter test
//! file contents (sha256) and carries a HMAC secret derived from those
//! hashes. The TUI verifies hashes before each `<leader> n` and refuses to
//! validate when a test file was tampered with; the cache layer signs every
//! completion entry with HMAC so a user editing `progress.json` by hand
//! can't forge a quest as solved.
//!
//! The manifest is per-branch — each chapter branch has its own copy whose
//! `chapters` map only lists that branch's test files. `duck-advent manifest
//! gen` regenerates it after intentional test-file changes.

use crate::error::{AdventError, AdventResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

pub const MANIFEST_FILENAME: &str = ".duck-manifest.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestManifest {
  pub version: u32,
  /// HMAC secret used to sign completion records. Derived deterministically
  /// at `gen` time as `sha256(canonical_chapter_hashes)`, then base64. This
  /// means any test-file edit invalidates every prior HMAC — cheating by
  /// editing a test wipes your own progress.
  pub secret: String,
  pub chapters: BTreeMap<String, ChapterManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChapterManifest {
  /// Map of repo-relative path → `sha256:<hex>` of that file's bytes.
  pub test_files: BTreeMap<String, String>,
}

impl QuestManifest {
  /// Read + parse the manifest from `repo_root`. Returns `Ok(None)` when the
  /// file is missing — callers can choose to fall back to "no integrity
  /// checks" mode (default for repos that haven't run `manifest gen` yet).
  pub fn load(repo_root: &Path) -> AdventResult<Option<Self>> {
    let p = repo_root.join(MANIFEST_FILENAME);
    match std::fs::read_to_string(&p) {
      Ok(s) => Ok(Some(serde_json::from_str(&s).map_err(AdventError::Json)?)),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(e) => Err(AdventError::io(p, e)),
    }
  }

  pub fn save(&self, repo_root: &Path) -> AdventResult<PathBuf> {
    let p = repo_root.join(MANIFEST_FILENAME);
    let s = serde_json::to_string_pretty(self).map_err(AdventError::Json)?;
    std::fs::write(&p, format!("{s}\n")).map_err(|e| AdventError::io(p.clone(), e))?;
    Ok(p)
  }

  /// Verify every recorded test file's contents still hashes to the recorded
  /// value for `slug`. Returns the list of drifted paths (empty = all good).
  /// Missing files count as drift too.
  pub fn verify_chapter(&self, repo_root: &Path, slug: &str) -> AdventResult<Vec<String>> {
    let Some(chapter) = self.chapters.get(slug) else {
      // No record for this slug — treat as "nothing to verify". The caller
      // decides whether that's an error.
      return Ok(Vec::new());
    };
    let mut drift = Vec::new();
    for (path, expected) in &chapter.test_files {
      let full = repo_root.join(path);
      match std::fs::read(&full) {
        Ok(bytes) => {
          let actual = file_hash(&bytes);
          if &actual != expected {
            drift.push(path.clone());
          }
        },
        Err(_) => drift.push(path.clone()),
      }
    }
    Ok(drift)
  }
}

/// `sha256:<hex>` representation for one file's contents.
pub fn file_hash(bytes: &[u8]) -> String {
  let mut h = Sha256::new();
  h.update(bytes);
  format!("sha256:{}", hex::encode(h.finalize()))
}

/// Derive the manifest secret from the sorted (slug, path, hash) tuples.
/// Deterministic — same chapter contents → same secret across machines.
pub fn derive_secret(chapters: &BTreeMap<String, ChapterManifest>) -> String {
  use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
  let mut h = Sha256::new();
  for (slug, ch) in chapters {
    h.update(slug.as_bytes());
    h.update(b"\n");
    for (path, hash) in &ch.test_files {
      h.update(path.as_bytes());
      h.update(b":");
      h.update(hash.as_bytes());
      h.update(b"\n");
    }
  }
  URL_SAFE_NO_PAD.encode(h.finalize())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn ch(files: &[(&str, &str)]) -> ChapterManifest {
    ChapterManifest {
      test_files: files.iter().map(|(p, h)| (p.to_string(), format!("sha256:{h}"))).collect(),
    }
  }

  #[test]
  fn file_hash_is_stable_and_prefixed() {
    let a = file_hash(b"hello");
    let b = file_hash(b"hello");
    assert_eq!(a, b);
    assert!(a.starts_with("sha256:"));
  }

  #[test]
  fn derive_secret_is_deterministic_for_same_input() {
    let mut a = BTreeMap::new();
    a.insert("ch1".into(), ch(&[("test/a.ts", "deadbeef")]));
    a.insert("ch2".into(), ch(&[("test/b.ts", "cafef00d")]));
    let s1 = derive_secret(&a);
    let s2 = derive_secret(&a);
    assert_eq!(s1, s2);
  }

  #[test]
  fn derive_secret_changes_when_any_hash_changes() {
    let mut a = BTreeMap::new();
    a.insert("ch1".into(), ch(&[("test/a.ts", "deadbeef")]));
    let s1 = derive_secret(&a);
    a.get_mut("ch1").unwrap().test_files.insert("test/a.ts".into(), "sha256:0000".into());
    let s2 = derive_secret(&a);
    assert_ne!(s1, s2);
  }
}
