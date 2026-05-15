import { resolve } from "node:path";
import { render } from "ink";
import React from "react";
import { readProgress, setCurrentChapter } from "../cache/progress";
import { loadConfig } from "../config/loader";
import type { LoadedConfig, QuestStep } from "../config/schema";
import { composeUp } from "../quest/docker";
import { checkout, currentBranch } from "../quest/git";
import {
  durationForQuest,
  findQuestByBranch,
  firstQuest,
  readBriefing,
  tearDownAndAdvance,
} from "../quest/runner";
import {
  ensureLayout,
  focusLayout,
  type LayoutTarget,
  refocusEditor,
  respawnEditorPane,
  respawnTestPane,
} from "../session/layout";
import { writeRuntimeStatus } from "../session/status";
import { Briefing } from "../tui/screens/Briefing";
import { Celebrate } from "../tui/screens/Celebrate";
import { Final } from "../tui/screens/Final";

/**
 * `duck-advent next` semantics:
 *  - On a recognized quest branch with tests green: tear down, advance, celebrate.
 *  - On a recognized quest branch with tests red: print message, exit non-zero.
 *  - On any other branch (e.g. main) with no progress: start quest 1 — git
 *    checkout, services up, briefing, then auto-switch the user's tmux client
 *    into the duck-quest window.
 */
export async function nextCommand(cwd: string): Promise<void> {
  const loaded = await loadConfig(cwd);
  const branch = await currentBranch(loaded.repoRoot);
  const current = findQuestByBranch(loaded, branch);

  if (!current) {
    const target = firstQuest(loaded);
    console.log(`✦ starting first quest: ${target.title}`);
    await checkout(loaded.repoRoot, target.slug);
    await composeUp(loaded.config, loaded.repoRoot, target.services);
    setCurrentChapter(loaded.repoHash, target.slug);
    const layout = await openWorkspaceFor(loaded, target);
    await showBriefing(loaded, target);
    await focusLayout(layout);
    return;
  }

  const result = await tearDownAndAdvance(loaded, current);
  if (result.status === "failed") {
    console.error(`✗ ${result.failure}. Fix the failing tests and try again.`);
    process.exit(1);
  }

  const durationMs = durationForQuest(loaded, current.slug);
  const stats = readProgress(loaded.repoHash).chapters[current.slug];

  if (result.status === "complete") {
    const state = readProgress(loaded.repoHash);
    await new Promise<void>((res) => {
      const app = render(
        React.createElement(Final, {
          questName: loaded.config.name,
          totalChapters: loaded.config.quests.length,
          durationMs: Date.parse(state.lastUpdatedAt) - Date.parse(state.startedAt),
          totalHints: Object.values(state.chapters).reduce(
            (a, b) => a + (b.hintsUsed ?? 0),
            0,
          ),
          onExit: () => {
            app.unmount();
            res();
          },
        }),
      );
    });
    return;
  }

  await new Promise<void>((res) => {
    const app = render(
      React.createElement(Celebrate, {
        chapterNumber: current.number,
        chapterTitle: current.title,
        hintsUsed: stats?.hintsUsed ?? 0,
        attempts: stats?.attempts ?? 1,
        durationMs,
        onContinue: () => {
          app.unmount();
          res();
        },
      }),
    );
  });

  const upcoming = result.next;
  if (!upcoming) return;
  const layout = await openWorkspaceFor(loaded, upcoming);
  await showBriefing(loaded, upcoming);
  await focusLayout(layout);
}

async function openWorkspaceFor(
  loaded: LoadedConfig,
  quest: QuestStep,
): Promise<LayoutTarget> {
  const target = await ensureLayout({
    repoRoot: loaded.repoRoot,
    repoHash: loaded.repoHash,
    config: loaded.config,
    chapter: quest,
    cliPath: process.argv[1] ?? "duck-advent",
  });

  const workdirAbs = resolve(loaded.repoRoot, quest.workdir);
  const baseCmd = loaded.config.testCommand.map((p: string) => (p.includes(" ") ? `'${p}'` : p));
  if (quest.testFilter) baseCmd.push(quest.testFilter);
  await respawnEditorPane(target, loaded.repoRoot, workdirAbs).catch(() => undefined);
  await respawnTestPane(target, loaded.repoRoot, baseCmd.join(" ")).catch(() => undefined);
  await refocusEditor(target).catch(() => undefined);

  writeRuntimeStatus(loaded.repoRoot, loaded.config.cacheDir, {
    chapterNumber: quest.number,
    totalChapters: loaded.config.quests.length,
    title: quest.title,
    hintsUsed: 0,
    tier: quest.tier,
  });
  return target;
}

async function showBriefing(loaded: LoadedConfig, quest: QuestStep): Promise<void> {
  await new Promise<void>((res) => {
    const md = readBriefing(loaded, quest);
    const app = render(
      React.createElement(Briefing, {
        chapter: quest,
        markdown: md,
        onClose: () => {
          app.unmount();
          res();
        },
      }),
    );
  });
}
