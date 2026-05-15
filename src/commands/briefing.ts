import { render } from "ink";
import React from "react";
import { loadConfig } from "../config/loader";
import { findChapterByBranch, readBriefing } from "../quest/runner";
import { currentBranch } from "../quest/git";
import { Briefing } from "../tui/screens/Briefing";

export async function briefingCommand(cwd: string): Promise<void> {
  const loaded = await loadConfig(cwd);
  const branch = await currentBranch(loaded.repoRoot);
  const chapter = findChapterByBranch(loaded, branch);
  if (!chapter) {
    console.error(`✗ not on a recognized chapter branch (${branch}).`);
    process.exit(1);
  }
  const md = readBriefing(loaded, chapter);
  await new Promise<void>((res) => {
    const app = render(
      React.createElement(Briefing, {
        chapter,
        markdown: md,
        onClose: () => {
          app.unmount();
          res();
        },
      }),
    );
  });
}
