import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

/**
 * The tmux status-left command reads this file every status-interval seconds.
 * Update it from the orchestrator whenever chapter/progress changes.
 */
export function writeRuntimeStatus(
  repoRoot: string,
  cacheDirName: string,
  state: {
    chapterNumber: number;
    totalChapters: number;
    title: string;
    hintsUsed: number;
    tier?: string;
  },
): void {
  const file = resolve(repoRoot, cacheDirName, "runtime-status");
  mkdirSync(dirname(file), { recursive: true });
  const tier = state.tier ? `· ${state.tier} ` : "";
  const line = `ch ${String(state.chapterNumber).padStart(2, "0")}/${state.totalChapters} ${tier}· ${state.title} · hints ${state.hintsUsed}/3`;
  writeFileSync(file, `${line}\n`);
}
