<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/wildduck2/duck-ui/master/apps/duck-ui-docs/public/logo-dark.svg">
    <img src="https://raw.githubusercontent.com/wildduck2/duck-ui/master/apps/duck-ui-docs/public/LOGO.svg" alt="gentleduck" width="160">
  </picture>
</p>

<h1 align="center">duck-advent</h1>

<p align="center">
  Advent-of-Code-style quest runner. Single binary, single terminal.
  Embeds <code>nvim</code> and the test runner inside a Ratatui TUI via PTY.
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.90+-orange.svg" alt="rust"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="license"></a>
</p>

---

## What it does

- Reads `quest.config.ts` from any TS project (Bun, pnpm, npm).
- Walks the user through N quests. Each quest is a git branch.
- Inside the TUI: 70% nvim (your real nvim, your LSP, your file tree) + 30% test runner pane.
- `<leader> n` runs the test suite once. Green advances; red shows the failing tail.
- `<leader> r` discards working-tree edits in the quest workdir for a clean restart.

No tmux. No external windows. No shell environment management.

## Workspace

| Crate | Role |
|---|---|
| `advent-core` | Domain types (config, progress, errors) |
| `advent-cache` | `~/.gentleduck` install / validator / progress cache |
| `advent-config` | Loads `quest.config.ts` via a `bun -e` (or `node --import tsx`) bridge |
| `advent-quest` | Git branch ops + test runner spawn |
| `advent-pty` | `portable-pty` + `vt100-ctt` to Ratatui widget (`PtyView`) |
| `advent-tui` | Phase state machine + screens + workspace layout |
| `advent-cli` | `clap` entrypoint, produces the `duck-advent` binary |

The TS package under `config/` ships `@gentleduck/advent-config`: type-safe `defineConfig` and the `Duck` namespace.

## Build

```bash
cargo build --release
cp target/release/duck-advent ~/.local/bin/
```

Requires Rust 1.90+. Target repo needs one of: `bun`, or `node` + `tsx`.

## Use

```bash
cd your-repo-with-quest.config.ts
duck-advent           # opens TUI, resumes current quest
duck-advent next      # validate + advance (CLI, no TUI)
duck-advent status    # progress overview
duck-advent doctor    # run validators
duck-advent repeat    # discard edits in current quest workdir
duck-advent init      # scaffold a quest.config.ts in cwd
```

## Keymap

Inside the workspace:

| Keys | Action |
|---|---|
| editor pane keys | Forwarded to nvim, your full config works |
| tests pane keys | Scroll only: `j`/`k`, `g`/`G`, `Home`/`End`, `PgUp`/`PgDn`. No stdin to vitest |
| `Ctrl-q` | Quit immediately |
| `Ctrl-a` | Arm the LEADER, next keystroke runs a duck command |

After `Ctrl-a` (LEADER armed, 1s timeout):

| Key | Action |
|---|---|
| `n` | Validate (run tests once) + celebrate + advance |
| `r` | Repeat, discard edits in workdir |
| `b` | Switch focus between editor and tests pane |
| `h` | Show next hint |
| `p` | Re-open briefing |
| `q` | Quit |
| `c` / `Esc` | Cancel the leader |

## Config

See [`config/README.md`](./config/README.md) for the `@gentleduck/advent-config` package surface. Minimal example:

```ts
import { defineConfig, Duck } from "@gentleduck/advent-config";

export default defineConfig({
  name: "Redis Quest",
  packageManager: "bun",
  installCommand: ["bun", "install"],
  testCommand: ["bunx", "vitest", "--watch"],
  branchPrefix: "chapter-",
  validators: [
    { id: Duck.CONSTANTS.STANDARD_VALIDATORS.BUN, label: "bun", cmd: ["bun", "--version"] },
  ],
  quests: [
    {
      number: 1,
      slug: "chapter-01-counter",
      title: "Counter Service",
      tier: "Strings",
      difficulty: 1,
      briefing: "docs/01-counter.md",
      workdir: "src/challenges/chapter-01-counter",
      testFilter: "chapter-01",
      hints: ["..."],
    },
  ],
});
```

## State

Per-repo state lives under `~/.gentleduck/`:

- `cache/<repo-hash>/install.json` — lockfile fingerprint + completed timestamp
- `cache/<repo-hash>/validators.json` — last validator outcomes, keyed by config hash
- `state/<repo-hash>/progress.json` — current quest, completed list, attempts, hints used

`<repo-hash>` is `sha256(canonical-path)[..16]` — survives `package.json` edits.

## License

MIT.
