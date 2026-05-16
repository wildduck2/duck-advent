/**
 * Type-safe quest config authoring for duck-advent.
 *
 *   import { defineConfig, Duck } from "@gentleduck/advent-config";
 *
 *   export default defineConfig({
 *     name: "Redis Quest",
 *     packageManager: "bun",
 *     installCommand: ["bun", "install"],
 *     testCommand: ["bunx", "vitest", "--watch"],
 *     branchPrefix: "chapter-",
 *     validators: [
 *       { id: "bun", label: "bun >= 1.1", cmd: ["bun", "--version"], min: "1.1" },
 *     ],
 *     quests: [
 *       {
 *         number: 1,
 *         slug: "chapter-01-intro",
 *         title: "Intro",
 *         tier: "Warmup",
 *         difficulty: 1,
 *         briefing: "docs/01-intro.md",
 *         workdir: "src/challenges/chapter-01-intro",
 *         testFilter: "chapter-01",
 *         hints: ["read the briefing", "look at the test"],
 *       },
 *     ],
 *   });
 *
 * The Rust binary reads this default export via `bun` (or `node --import tsx`)
 * and validates it against the same schema mirrored on the Rust side.
 */

import * as types from "./types";
import * as constants from "./constants";

/** Public namespace — every type users author against is here under `Duck.I*`. */
export namespace Duck {
  export type IQuestConfig = types.IQuestConfig;
  export type IQuestStep = types.IQuestStep;
  export type IValidator = types.IValidator;
  export type IServiceSpec = types.IServiceSpec;
  export type IPackageManager = types.IPackageManager;
  export type IDifficulty = types.IDifficulty;

  export const CONSTANTS = constants.CONSTANTS;
  export type IConstants = typeof constants.CONSTANTS;
}

/**
 * Identity function that gives users full type-checking + autocomplete on the
 * config they author. Returns the value unchanged — the Rust loader does the
 * real validation at run time via the shared schema.
 */
export function defineConfig(config: Duck.IQuestConfig): Duck.IQuestConfig {
  return config;
}

export type { IQuestConfig, IQuestStep, IValidator, IServiceSpec, IPackageManager, IDifficulty } from "./types";
export { CONSTANTS } from "./constants";
