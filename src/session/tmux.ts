import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { run, runCapture } from "../lib/exec";

const HERE = fileURLToPath(new URL(".", import.meta.url));
export const TMUX_CONF = resolve(HERE, "..", "..", "tmux", "duck-quest.tmux.conf");

/**
 * True when the orchestrator is running inside an existing tmux client. In
 * that case we attach a new *window* to the current session instead of
 * spawning a fresh server — killing or replacing the user's session would be
 * catastrophic.
 */
export function isInsideTmux(): boolean {
  return Boolean(process.env.TMUX);
}

export async function currentSessionName(): Promise<string | null> {
  if (!isInsideTmux()) return null;
  const { code, stdout } = await runCapture("tmux", ["display-message", "-p", "#{session_name}"]);
  if (code !== 0) return null;
  return stdout.trim() || null;
}

export function sessionName(repoHash: string): string {
  return `duck-quest-${repoHash.slice(0, 10)}`;
}

export function windowName(repoHash: string): string {
  return `duck-quest-${repoHash.slice(0, 8)}`;
}

export async function sessionExists(name: string): Promise<boolean> {
  const { code } = await runCapture("tmux", ["has-session", "-t", name]);
  return code === 0;
}

export async function windowExists(session: string, window: string): Promise<boolean> {
  const { code, stdout } = await runCapture("tmux", [
    "list-windows",
    "-t",
    session,
    "-F",
    "#{window_name}",
  ]);
  if (code !== 0) return false;
  return stdout.split("\n").some((line) => line.trim() === window);
}

export async function killSession(name: string): Promise<void> {
  await run("tmux", ["kill-session", "-t", name], { capture: true });
}

export async function tmuxRun(args: string[]): Promise<void> {
  const { code, stderr } = await runCapture("tmux", args);
  if (code !== 0) throw new Error(`tmux ${args.join(" ")} failed: ${stderr.trim()}`);
}

export async function tmuxCapture(args: string[]): Promise<string> {
  const { code, stdout, stderr } = await runCapture("tmux", args);
  if (code !== 0) throw new Error(`tmux ${args.join(" ")} failed: ${stderr.trim()}`);
  return stdout;
}
