import { tmuxRun } from "./tmux";

/**
 * Open a centered popup that runs the given shell command. Closes when the
 * command exits. Uses tmux's display-popup (requires tmux >= 3.2).
 */
export async function popup(
  session: string,
  cmd: string,
  opts: { width?: string; height?: string; title?: string } = {},
): Promise<void> {
  const args = ["display-popup", "-E", "-t", session];
  if (opts.width) args.push("-w", opts.width);
  else args.push("-w", "85%");
  if (opts.height) args.push("-h", opts.height);
  else args.push("-h", "80%");
  if (opts.title) args.push("-T", opts.title);
  args.push(cmd);
  await tmuxRun(args);
}
