import { run, runCapture } from "../lib/exec";

export async function currentBranch(cwd: string): Promise<string> {
  const { stdout } = await runCapture("git", ["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"]);
  return stdout.trim();
}

export async function isWorkingTreeClean(cwd: string): Promise<boolean> {
  const { stdout } = await runCapture("git", ["-C", cwd, "status", "--porcelain"]);
  return stdout.trim().length === 0;
}

export async function branchExists(cwd: string, branch: string): Promise<boolean> {
  const { code } = await runCapture("git", [
    "-C",
    cwd,
    "rev-parse",
    "--verify",
    `refs/heads/${branch}`,
  ]);
  return code === 0;
}

export async function checkout(cwd: string, branch: string): Promise<void> {
  const { code, stderr } = await run("git", ["-C", cwd, "checkout", branch], { capture: true });
  if (code !== 0) throw new Error(`git checkout ${branch} failed: ${stderr.trim()}`);
}
