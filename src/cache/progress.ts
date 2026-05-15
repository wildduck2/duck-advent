import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { repoStateDir } from "./paths";

export interface ChapterStats {
  startedAt?: string;
  completedAt?: string;
  hintsUsed: number;
  attempts: number;
}

export interface ProgressState {
  currentChapter: string | null;
  completed: string[];
  startedAt: string;
  lastUpdatedAt: string;
  chapters: Record<string, ChapterStats>;
}

function file(repoHash: string): string {
  return resolve(repoStateDir(repoHash), "progress.json");
}

function empty(): ProgressState {
  const now = new Date().toISOString();
  return {
    currentChapter: null,
    completed: [],
    startedAt: now,
    lastUpdatedAt: now,
    chapters: {},
  };
}

export function readProgress(repoHash: string): ProgressState {
  const f = file(repoHash);
  if (!existsSync(f)) return empty();
  try {
    return JSON.parse(readFileSync(f, "utf8")) as ProgressState;
  } catch {
    return empty();
  }
}

export function writeProgress(repoHash: string, state: ProgressState): void {
  state.lastUpdatedAt = new Date().toISOString();
  writeFileSync(file(repoHash), `${JSON.stringify(state, null, 2)}\n`);
}

export function setCurrentChapter(repoHash: string, slug: string): ProgressState {
  const state = readProgress(repoHash);
  state.currentChapter = slug;
  const c = state.chapters[slug] ?? { hintsUsed: 0, attempts: 0 };
  if (!c.startedAt) c.startedAt = new Date().toISOString();
  state.chapters[slug] = c;
  writeProgress(repoHash, state);
  return state;
}

export function completeChapter(repoHash: string, slug: string): ProgressState {
  const state = readProgress(repoHash);
  if (!state.completed.includes(slug)) state.completed.push(slug);
  const c = state.chapters[slug] ?? { hintsUsed: 0, attempts: 0 };
  c.completedAt = new Date().toISOString();
  state.chapters[slug] = c;
  writeProgress(repoHash, state);
  return state;
}

export function bumpHints(repoHash: string, slug: string): number {
  const state = readProgress(repoHash);
  const c = state.chapters[slug] ?? { hintsUsed: 0, attempts: 0 };
  c.hintsUsed += 1;
  state.chapters[slug] = c;
  writeProgress(repoHash, state);
  return c.hintsUsed;
}

export function bumpAttempts(repoHash: string, slug: string): number {
  const state = readProgress(repoHash);
  const c = state.chapters[slug] ?? { hintsUsed: 0, attempts: 0 };
  c.attempts += 1;
  state.chapters[slug] = c;
  writeProgress(repoHash, state);
  return c.attempts;
}
