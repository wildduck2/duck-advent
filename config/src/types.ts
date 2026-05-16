import type { CONSTANTS } from "./constants";

export type IPackageManager = "bun" | "pnpm" | "npm" | "yarn";

/** 1 (easiest) — 5 (hardest). */
export type IDifficulty = 1 | 2 | 3 | 4 | 5;

export interface IValidator {
  /** Stable id used for caching results. */
  readonly id: string;
  /** Human-readable label for the validator row. */
  readonly label: string;
  /** Argv to run. Zero exit code passes. */
  readonly cmd: readonly string[];
  /** Optional minimum-version string the user can self-compare. */
  readonly min?: string;
  /** When true, a failing run does not block the quest from starting. */
  readonly optional?: boolean;
}

export interface IServiceSpec {
  /** Path (repo-relative) to a docker compose file that brings the service up. */
  readonly compose: string;
  /** Container name the spec declares — used for health probes and shutdown. */
  readonly container: string;
  /** Optional argv duck-advent runs to confirm the service is reachable. */
  readonly readyCheck?: readonly string[];
}

export interface IQuestStep {
  /** 1-based ordinal in the journey. */
  readonly number: number;
  /** Branch name. duck-advent runs `git checkout <slug>`. */
  readonly slug: string;
  /** Display title. */
  readonly title: string;
  /** Optional tier label (e.g. "Strings", "Sets"). */
  readonly tier?: string;
  /** 1—5 difficulty. */
  readonly difficulty?: IDifficulty;
  /** Path (repo-relative) to the briefing markdown file. */
  readonly briefing: string;
  /** Path (repo-relative) nvim opens when entering the workspace. */
  readonly workdir: string;
  /** Substring passed to the test runner to scope what runs. */
  readonly testFilter?: string;
  /** Names of top-level `services` entries this quest depends on. */
  readonly services?: readonly string[];
  /** Optional path to a seed script duck-advent runs before the quest starts. */
  readonly seed?: string;
  /** Hints surfaced one-by-one in the workspace via ⌃h. */
  readonly hints?: readonly string[];
}

export interface IQuestConfig {
  /** Journey name shown on splash, status bar, and final screen. */
  readonly name: string;
  /** Short description shown on splash. */
  readonly description?: string;
  /** Which package manager runs `installCommand`. Informational. */
  readonly packageManager?: IPackageManager;
  /** Argv duck-advent invokes during the first-run install phase. */
  readonly installCommand: readonly string[];
  /** Argv for the test runner. Keep `--watch` here — duck-advent strips it
   *  for one-shot validation runs. */
  readonly testCommand: readonly string[];
  /** All quest branches start with this prefix (e.g. `chapter-`). */
  readonly branchPrefix?: string;
  /** Folder name (repo-relative) for per-repo cache + runtime state. */
  readonly cacheDir?: typeof CONSTANTS.DEFAULT_CACHE_DIR | string;
  /** Environment checks run on first launch and again with `duck-advent doctor`. */
  readonly validators?: readonly IValidator[];
  /** Service specs the quests may depend on (per-quest `services: [...]`). */
  readonly services?: Readonly<Record<string, IServiceSpec>>;
  /** Ordered list of quests. */
  readonly quests: readonly IQuestStep[];
}
