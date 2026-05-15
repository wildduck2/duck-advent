import { resolve } from "node:path";
import { run, runCapture } from "../lib/exec";
import type { QuestConfig } from "../config/schema";

function composeArgs(config: QuestConfig, repoRoot: string, services: string[]): string[] {
  const files = services
    .map((s) => {
      const svc = config.services[s];
      if (!svc) throw new Error(`unknown service "${s}" — not in quest.config.ts`);
      return ["-f", resolve(repoRoot, svc.compose)];
    })
    .flat();
  return ["compose", ...files];
}

export async function composeUp(
  config: QuestConfig,
  repoRoot: string,
  services: string[],
): Promise<void> {
  if (services.length === 0) return;
  const args = [...composeArgs(config, repoRoot, services), "up", "-d", "--remove-orphans"];
  const { code, stderr } = await run("docker", args, { capture: true });
  if (code !== 0) throw new Error(`docker compose up failed: ${stderr.trim()}`);
}

export async function composeDown(
  config: QuestConfig,
  repoRoot: string,
  services: string[],
): Promise<void> {
  if (services.length === 0) return;
  const args = [...composeArgs(config, repoRoot, services), "down", "-v", "--remove-orphans"];
  const { code, stderr } = await run("docker", args, { capture: true });
  if (code !== 0) throw new Error(`docker compose down failed: ${stderr.trim()}`);
}

export async function waitForContainer(name: string, timeoutMs = 30_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const { code, stdout } = await runCapture("docker", ["exec", name, "redis-cli", "ping"]);
    if (code === 0 && stdout.trim() === "PONG") return;
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`container ${name} did not become healthy in ${timeoutMs}ms`);
}
