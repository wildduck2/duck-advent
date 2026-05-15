import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  bumpAttempts,
  completeChapter,
  readProgress,
  setCurrentChapter,
} from "../cache/progress";
import type { LoadedConfig, QuestStep } from "../config/schema";
import { run } from "../lib/exec";
import { composeDown, composeUp } from "./docker";
import { branchExists, checkout, currentBranch, isWorkingTreeClean } from "./git";

export function findQuestByBranch(config: LoadedConfig, branch: string): QuestStep | undefined {
  return config.config.quests.find((c) => c.slug === branch);
}

export function findQuestBySlug(config: LoadedConfig, slug: string): QuestStep | undefined {
  return config.config.quests.find((c) => c.slug === slug);
}

export function firstQuest(config: LoadedConfig): QuestStep {
  return config.config.quests[0];
}

export function nextQuestAfter(config: LoadedConfig, current: QuestStep): QuestStep | undefined {
  const idx = config.config.quests.findIndex((c) => c.slug === current.slug);
  if (idx < 0 || idx === config.config.quests.length - 1) return undefined;
  return config.config.quests[idx + 1];
}

// Back-compat aliases — older callers used `chapter` naming.
export const findChapterByBranch = findQuestByBranch;
export const findChapterBySlug = findQuestBySlug;
export const firstChapter = firstQuest;
export const nextChapter = nextQuestAfter;

export function readBriefing(loaded: LoadedConfig, quest: QuestStep): string {
  const path = resolve(loaded.repoRoot, quest.briefing);
  if (!existsSync(path)) return `# ${quest.title}\n\n_(briefing missing)_`;
  return readFileSync(path, "utf8");
}

export async function switchToQuest(loaded: LoadedConfig, quest: QuestStep): Promise<void> {
  if (!(await isWorkingTreeClean(loaded.repoRoot))) {
    throw new Error("working tree not clean — commit or stash before switching quests");
  }
  if (!(await branchExists(loaded.repoRoot, quest.slug))) {
    throw new Error(`branch "${quest.slug}" does not exist in repo`);
  }
  const cur = await currentBranch(loaded.repoRoot);
  if (cur !== quest.slug) await checkout(loaded.repoRoot, quest.slug);
  setCurrentChapter(loaded.repoHash, quest.slug);
}

export interface AdvanceResult {
  status: "passed" | "failed" | "complete";
  failure?: string;
  next?: QuestStep;
}

export async function runTestsOnce(loaded: LoadedConfig, quest: QuestStep): Promise<boolean> {
  bumpAttempts(loaded.repoHash, quest.slug);
  const base = loaded.config.testCommand.filter((arg) => arg !== "--watch" && arg !== "-w");
  const cmd = [...base, ...(quest.testFilter ? [quest.testFilter] : [])];
  const [bin, ...args] = cmd;
  const { code } = await run(bin, args, { capture: false, cwd: loaded.repoRoot });
  return code === 0;
}

export async function tearDownAndAdvance(
  loaded: LoadedConfig,
  current: QuestStep,
): Promise<AdvanceResult> {
  const passed = await runTestsOnce(loaded, current);
  if (!passed) return { status: "failed", failure: "tests did not pass" };
  await composeDown(loaded.config, loaded.repoRoot, current.services);
  completeChapter(loaded.repoHash, current.slug);
  const next = nextQuestAfter(loaded, current);
  if (!next) return { status: "complete" };
  await checkout(loaded.repoRoot, next.slug);
  await composeUp(loaded.config, loaded.repoRoot, next.services);
  setCurrentChapter(loaded.repoHash, next.slug);
  return { status: "passed", next };
}

export async function resetQuest(loaded: LoadedConfig, quest: QuestStep): Promise<void> {
  await composeDown(loaded.config, loaded.repoRoot, quest.services);
  await composeUp(loaded.config, loaded.repoRoot, quest.services);
}

// Back-compat alias
export const resetChapter = resetQuest;

export function durationForQuest(loaded: LoadedConfig, slug: string): number {
  const state = readProgress(loaded.repoHash);
  const c = state.chapters[slug];
  if (!c?.startedAt) return 0;
  const end = c.completedAt ?? new Date().toISOString();
  return Date.parse(end) - Date.parse(c.startedAt);
}
export const durationForChapter = durationForQuest;
