import { render } from "ink";
import React from "react";
import { bumpHints, readProgress } from "../cache/progress";
import { loadConfig } from "../config/loader";
import { findChapterByBranch } from "../quest/runner";
import { currentBranch } from "../quest/git";
import { HintModal } from "../tui/screens/HintModal";

export async function hintCommand(cwd: string): Promise<void> {
  const loaded = await loadConfig(cwd);
  const branch = await currentBranch(loaded.repoRoot);
  const chapter = findChapterByBranch(loaded, branch);
  if (!chapter) {
    console.error(`✗ not on a recognized quest branch (${branch}).`);
    process.exit(1);
  }
  const used = readProgress(loaded.repoHash).chapters[chapter.slug]?.hintsUsed ?? 0;
  if (used >= chapter.hints.length) {
    console.log(`no more hints (${used}/${chapter.hints.length} used). you've got this.`);
    return;
  }
  const hint = chapter.hints[used];
  bumpHints(loaded.repoHash, chapter.slug);

  await new Promise<void>((res) => {
    const app = render(
      React.createElement(HintModal, {
        hint,
        index: used,
        total: chapter.hints.length,
        onClose: () => {
          app.unmount();
          res();
        },
      }),
    );
  });
}
