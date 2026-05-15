import { type SpawnOptions, spawn } from "node:child_process";

export interface ExecResult {
  code: number;
  stdout: string;
  stderr: string;
}

export async function run(
  cmd: string,
  args: string[] = [],
  opts: SpawnOptions & { capture?: boolean } = {},
): Promise<ExecResult> {
  const inherit = !opts.capture;
  const child = spawn(cmd, args, {
    stdio: inherit ? "inherit" : ["ignore", "pipe", "pipe"],
    cwd: opts.cwd,
    env: opts.env ?? process.env,
    shell: opts.shell,
  });
  let stdout = "";
  let stderr = "";
  if (!inherit) {
    child.stdout?.on("data", (d) => {
      stdout += d.toString();
    });
    child.stderr?.on("data", (d) => {
      stderr += d.toString();
    });
  }
  return new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code) => resolve({ code: code ?? 0, stdout, stderr }));
  });
}

export async function runCapture(cmd: string, args: string[] = []): Promise<ExecResult> {
  return run(cmd, args, { capture: true });
}
