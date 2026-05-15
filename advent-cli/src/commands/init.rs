use anyhow::{Result, bail};
use std::path::Path;

const TEMPLATE: &str = r#"// Authoring with @gentleduck/advent-config gives you full TS inference.
// The Rust binary loads this file via bun (or node+tsx) and validates against
// the same schema mirrored in advent-core::QuestConfig.
import { defineConfig, Duck } from "@gentleduck/advent-config";

export default defineConfig({
  name: "Your Quest",
  description: "Short tagline shown on the splash screen.",
  packageManager: "bun",
  installCommand: ["bun", "install"],
  testCommand: ["bunx", "vitest", "--watch"],
  branchPrefix: "chapter-",
  validators: [
    { id: Duck.CONSTANTS.STANDARD_VALIDATORS.BUN, label: "bun >= 1.1", cmd: ["bun", "--version"], min: "1.1" },
    { id: Duck.CONSTANTS.STANDARD_VALIDATORS.NVIM, label: "nvim available", cmd: ["nvim", "--version"] },
    { id: Duck.CONSTANTS.STANDARD_VALIDATORS.GIT, label: "git available", cmd: ["git", "--version"] },
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
      hints: [
        "Read the briefing.",
        "Look at the failing test.",
        "Start with the smallest possible change.",
      ],
    },
  ],
});
"#;

pub async fn run(cwd: &Path) -> Result<()> {
  let target = cwd.join("quest.config.ts");
  if target.exists() {
    bail!("{} already exists", target.display());
  }
  tokio::fs::write(&target, TEMPLATE).await?;
  println!("✓ wrote {}", target.display());
  println!("→ edit it, then run: duck-advent");
  Ok(())
}
