/**
 * Constants surfaced under `Duck.CONSTANTS` — defaults the Rust loader applies
 * when the corresponding field is omitted.
 */
export const CONSTANTS = {
  DEFAULT_PACKAGE_MANAGER: "bun",
  DEFAULT_BRANCH_PREFIX: "chapter-",
  DEFAULT_CACHE_DIR: ".gentleduck",
  /** Maximum number of hints stored per quest. Surfaced one at a time. */
  MAX_HINTS_PER_QUEST: 3,
  /** Standard validator IDs duck-advent expects but does not require. */
  STANDARD_VALIDATORS: {
    BUN: "bun",
    NODE: "node",
    NVIM: "nvim",
    GIT: "git",
  },
} as const;
