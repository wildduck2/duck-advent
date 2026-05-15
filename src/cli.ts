#!/usr/bin/env node
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { openQuest } from "./commands/open";
import { nextCommand } from "./commands/next";
import { hintCommand } from "./commands/hint";
import { briefingCommand } from "./commands/briefing";
import { resetCommand } from "./commands/reset";
import { doctorCommand } from "./commands/doctor";
import { initCommand } from "./commands/init";
import { statusCommand } from "./commands/status";
import { DUCK_ADVENT_VERSION } from "./version";

const PKG = { version: DUCK_ADVENT_VERSION };

async function main(): Promise<void> {
  await yargs(hideBin(process.argv))
    .scriptName("duck-advent")
    .version(PKG.version)
    .command(
      ["open", "$0"],
      "open the quest TUI for the repo in cwd",
      (y) =>
        y.option("revalidate", {
          type: "boolean",
          default: false,
          describe: "re-run validators, bypass cached results",
        }),
      async (args) =>
        openQuest({
          cwd: process.cwd(),
          cliVersion: PKG.version,
          cliPath: process.argv[1] ?? "duck-advent",
          revalidate: args.revalidate as boolean,
        }),
    )
    .command(
      "next",
      "validate tests, mark passed, advance to next chapter",
      () => undefined,
      async () => nextCommand(process.cwd()),
    )
    .command(
      "hint",
      "reveal the next hint for current chapter",
      () => undefined,
      async () => hintCommand(process.cwd()),
    )
    .command(
      "briefing",
      "re-show briefing for current chapter",
      () => undefined,
      async () => briefingCommand(process.cwd()),
    )
    .command(
      "reset",
      "tear down + bring up services for current chapter",
      () => undefined,
      async () => resetCommand(process.cwd()),
    )
    .command(
      "doctor",
      "run validators (use --revalidate to bypass cache)",
      (y) =>
        y.option("revalidate", {
          type: "boolean",
          default: true,
          describe: "re-run validators, bypass cached results",
        }),
      async (args) => doctorCommand(process.cwd(), args.revalidate as boolean),
    )
    .command(
      "status",
      "show current branch, chapter, and progress",
      () => undefined,
      async () => statusCommand(process.cwd()),
    )
    .command(
      "init",
      "scaffold a quest.config.ts in the current repo",
      () => undefined,
      async () => initCommand(process.cwd()),
    )
    .strict()
    .help()
    .parseAsync();
}

main().catch((err: Error) => {
  console.error(`✗ ${err.message}`);
  process.exit(1);
});
