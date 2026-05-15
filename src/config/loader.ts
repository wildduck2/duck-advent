import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { QuestConfigSchema, type LoadedConfig } from "./schema";

const CONFIG_FILES = ["quest.config.ts", "quest.config.mjs", "quest.config.js"];

export function findRepoRoot(startDir: string): { repoRoot: string; configPath: string } {
  let dir = resolve(startDir);
  while (true) {
    for (const name of CONFIG_FILES) {
      const candidate = resolve(dir, name);
      if (existsSync(candidate)) {
        return { repoRoot: dir, configPath: candidate };
      }
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  throw new Error(
    `No quest.config.{ts,mjs,js} found from ${startDir}. Run \`duck-advent init\` to create one.`,
  );
}

function sha256(input: string): string {
  return createHash("sha256").update(input).digest("hex");
}

export async function loadConfig(startDir: string): Promise<LoadedConfig> {
  const { repoRoot, configPath } = findRepoRoot(startDir);
  const rawText = readFileSync(configPath, "utf8");

  const moduleUrl = pathToFileURL(configPath).href;
  const imported = (await import(moduleUrl)) as { default?: unknown };
  const fromDefault = imported.default;
  if (!fromDefault || typeof fromDefault !== "object") {
    throw new Error(
      `${configPath} must export a default value created by defineQuest({ ... }).`,
    );
  }
  const parsed = QuestConfigSchema.parse(fromDefault);

  const repoHash = sha256(repoRoot).slice(0, 16);
  const configHash = sha256(rawText).slice(0, 16);

  return {
    repoRoot,
    configPath,
    config: parsed,
    repoHash,
    configHash,
  };
}

export function resolveRepoPath(repoRoot: string, p: string): string {
  return isAbsolute(p) ? p : resolve(repoRoot, p);
}
