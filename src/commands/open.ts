import { render } from "ink";
import React from "react";
import { readProgress } from "../cache/progress";
import { loadConfig } from "../config/loader";
import { findChapterBySlug, firstChapter } from "../quest/runner";
import { ensureLayout, focusLayout } from "../session/layout";
import { App } from "../tui/App";

export interface OpenOpts {
  cwd: string;
  cliVersion: string;
  cliPath: string;
  revalidate?: boolean;
}

export async function openQuest(opts: OpenOpts): Promise<void> {
  const loaded = await loadConfig(opts.cwd);

  let launchSlug: string | null = null;

  await new Promise<void>((resolveOuter, rejectOuter) => {
    const app = render(
      React.createElement(App, {
        loaded,
        cliVersion: opts.cliVersion,
        revalidate: opts.revalidate,
        onLaunch: async (slug) => {
          launchSlug = slug;
        },
      }),
    );
    app
      .waitUntilExit()
      .then(() => resolveOuter())
      .catch((e) => rejectOuter(e as Error));
  });

  if (!launchSlug) return;

  const chapter = findChapterBySlug(loaded, launchSlug) ?? firstChapter(loaded);
  const target = await ensureLayout({
    repoRoot: loaded.repoRoot,
    repoHash: loaded.repoHash,
    config: loaded.config,
    chapter,
    cliPath: opts.cliPath,
  });

  readProgress(loaded.repoHash); // touch state
  await focusLayout(target);
}
