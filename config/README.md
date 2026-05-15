# @gentleduck/advent-config

Type-safe `quest.config.ts` authoring for [duck-advent](https://github.com/gentleduck/duck-advent).

## Install

```bash
bun add -d @gentleduck/advent-config
```

## Usage

```ts
import { defineConfig, Duck } from "@gentleduck/advent-config";

export default defineConfig({
  name: "Redis Quest",
  packageManager: "bun",
  installCommand: ["bun", "install"],
  testCommand: ["bunx", "vitest", "--watch"],
  branchPrefix: "chapter-",
  validators: [
    { id: Duck.CONSTANTS.STANDARD_VALIDATORS.BUN, label: "bun >= 1.1", cmd: ["bun", "--version"], min: "1.1" },
  ],
  quests: [
    {
      number: 1,
      slug: "chapter-01-intro",
      title: "Intro",
      tier: "Warmup",
      difficulty: 1,
      briefing: "docs/01-intro.md",
      workdir: "src/challenges/chapter-01-intro",
      testFilter: "chapter-01",
      hints: ["read the briefing", "look at the test"],
    },
  ],
});
```

## Surface

- `defineConfig(config)` — identity helper, gives full TypeScript inference.
- `Duck.IQuestConfig` / `Duck.IQuestStep` / `Duck.IValidator` / `Duck.IPackageManager` / `Duck.IDifficulty` — public interfaces (all `I`-prefixed).
- `Duck.CONSTANTS` — defaults and well-known IDs the Rust loader recognises.
