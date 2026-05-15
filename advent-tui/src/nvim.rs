//! Build the argv passed to nvim when it spawns inside the workspace pane.

use std::path::{Path, PathBuf};

const ENTRY_SUFFIXES: &[&str] =
  &[".service.ts", ".controller.ts", ".worker.ts", ".gateway.ts", ".guard.ts", ".ticker.ts", ".module.ts", ".ts"];

const NETRW_SETUP: &str = "let g:netrw_banner=0 | let g:netrw_winsize=22 | let g:netrw_liststyle=3";

const OPEN_TREE: &str = "if exists(':NvimTreeOpen') | NvimTreeOpen \
   | elseif exists(':Neotree') | Neotree show \
   | elseif exists(':Oil') | Oil \
   | else | silent! runtime! plugin/netrwPlugin.vim | silent! Lexplore | endif";

pub fn argv(workdir: &Path) -> Vec<String> {
  let entry = pick_entry_file(workdir).unwrap_or_else(|| workdir.to_path_buf());
  vec![
    "-c".into(),
    NETRW_SETUP.into(),
    "-c".into(),
    OPEN_TREE.into(),
    "-c".into(),
    "wincmd p".into(),
    entry.to_string_lossy().into_owned(),
  ]
}

fn pick_entry_file(workdir: &Path) -> Option<PathBuf> {
  let entries: Vec<PathBuf> = std::fs::read_dir(workdir)
    .ok()?
    .filter_map(Result::ok)
    .map(|e| e.path())
    .filter(|p| p.is_file())
    .filter(|p| {
      p.extension().and_then(|s| s.to_str()) == Some("ts")
        && p.file_name().and_then(|s| s.to_str()).is_some_and(|n| !n.ends_with(".spec.ts"))
    })
    .collect();
  for suffix in ENTRY_SUFFIXES {
    if let Some(hit) =
      entries.iter().find(|p| p.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.ends_with(suffix)))
    {
      return Some(hit.clone());
    }
  }
  entries.into_iter().next()
}
