import { existsSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import type { ChapterConfig, QuestConfig } from "../config/schema";
import { run } from "../lib/exec";
import {
  currentSessionName,
  isInsideTmux,
  sessionExists,
  sessionName,
  TMUX_CONF,
  tmuxCapture,
  tmuxRun,
  windowExists,
  windowName,
} from "./tmux";

export interface LayoutArgs {
  repoRoot: string;
  repoHash: string;
  config: QuestConfig;
  chapter: ChapterConfig;
  cliPath: string;
}

export interface LayoutTarget {
  mode: "session" | "window";
  session: string;
  window?: string;
  address: string;
  editorPaneId: string;
  testPaneId: string;
}

/** Glob preference order — first match wins. Picks the most "interesting" file
 * for the quest so nvim has a real buffer open, not just a directory netrw. */
const ENTRY_PATTERNS: Array<RegExp> = [
  /\.service\.ts$/, // most quests have a *.service.ts
  /\.controller\.ts$/,
  /\.worker\.ts$/,
  /\.gateway\.ts$/,
  /\.guard\.ts$/,
  /\.ticker\.ts$/,
  /\.module\.ts$/,
  /\.ts$/,
];

function findEntryFile(workdirAbs: string): string | null {
  if (!existsSync(workdirAbs)) return null;
  let entries: string[];
  try {
    entries = readdirSync(workdirAbs);
  } catch {
    return null;
  }
  const ts = entries.filter((f) => f.endsWith(".ts") && !f.endsWith(".spec.ts"));
  for (const pat of ENTRY_PATTERNS) {
    const hit = ts.find((f) => pat.test(f));
    if (hit) return resolve(workdirAbs, hit);
  }
  return null;
}

/**
 * Build the argv for nvim so that:
 *   - the chosen entry file is open in the main buffer
 *   - the netrw file tree is open on the left (Lexplore)
 *   - focus is back on the entry file buffer (wincmd p)
 *
 * Falls back to opening the workdir directly when no .ts file is found.
 */
function nvimArgs(workdirAbs: string): string[] {
  const entry = findEntryFile(workdirAbs);
  // Detect whatever file-tree plugin the user has, falling back to bundled
  // netrw. This way nvim-tree / neo-tree / oil users get their familiar UI,
  // and bare nvim still gets a working tree on the left.
  const netrwSetup =
    "let g:netrw_banner=0 | let g:netrw_winsize=22 | let g:netrw_liststyle=3";
  const openTree =
    'if exists(":NvimTreeOpen") | NvimTreeOpen | ' +
    'elseif exists(":Neotree") | Neotree show | ' +
    'elseif exists(":Oil") | Oil | ' +
    "else | silent! runtime! plugin/netrwPlugin.vim | silent! Lexplore | endif";
  const target = entry ?? workdirAbs;
  return ["-c", netrwSetup, "-c", openTree, "-c", "wincmd p", target];
}

export async function ensureLayout(args: LayoutArgs): Promise<LayoutTarget> {
  const workdirAbs = resolve(args.repoRoot, args.chapter.workdir);
  const testCmd = renderTestCommand(args.config.testCommand, args.chapter);
  const editorArgv = nvimArgs(workdirAbs);

  if (isInsideTmux()) {
    const session = (await currentSessionName()) ?? "0";
    const window = windowName(args.repoHash);
    const address = `${session}:${window}`;
    let editorPaneId: string;
    let testPaneId: string;

    if (await windowExists(session, window)) {
      const panes = await readPaneIds(address);
      editorPaneId = panes[0] ?? "";
      testPaneId = panes[1] ?? "";
    } else {
      editorPaneId = (
        await tmuxCapture([
          "new-window",
          "-d",
          "-t",
          session,
          "-n",
          window,
          "-c",
          args.repoRoot,
          "-P",
          "-F",
          "#{pane_id}",
          "nvim",
          ...editorArgv,
        ])
      ).trim();
      testPaneId = (
        await tmuxCapture([
          "split-window",
          "-h",
          "-l",
          "30%",
          "-t",
          editorPaneId,
          "-c",
          args.repoRoot,
          "-P",
          "-F",
          "#{pane_id}",
          "sh",
          "-c",
          testCmd,
        ])
      ).trim();
      await tmuxRun(["select-pane", "-t", editorPaneId]);
    }
    return { mode: "window", session, window, address, editorPaneId, testPaneId };
  }

  const session = sessionName(args.repoHash);
  let editorPaneId: string;
  let testPaneId: string;

  if (!(await sessionExists(session))) {
    await tmuxRun([
      "-f",
      TMUX_CONF,
      "new-session",
      "-d",
      "-s",
      session,
      "-x",
      String(process.stdout.columns || 200),
      "-y",
      String(process.stdout.rows || 50),
      "-c",
      args.repoRoot,
      "nvim",
      ...editorArgv,
    ]);
    editorPaneId = (await tmuxCapture(["list-panes", "-t", session, "-F", "#{pane_id}"]))
      .split("\n")[0]
      .trim();
    testPaneId = (
      await tmuxCapture([
        "split-window",
        "-h",
        "-l",
        "30%",
        "-t",
        editorPaneId,
        "-c",
        args.repoRoot,
        "-P",
        "-F",
        "#{pane_id}",
        "sh",
        "-c",
        testCmd,
      ])
    ).trim();
    await tmuxRun(["select-pane", "-t", editorPaneId]);
  } else {
    const panes = await readPaneIds(session);
    editorPaneId = panes[0] ?? "";
    testPaneId = panes[1] ?? "";
  }
  return { mode: "session", session, address: session, editorPaneId, testPaneId };
}

async function readPaneIds(target: string): Promise<string[]> {
  const out = await tmuxCapture(["list-panes", "-t", target, "-F", "#{pane_id}"]);
  return out
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

/**
 * Switch the active tmux client to the duck-quest window/session. Call this
 * after the briefing modal closes so the user lands inside the workspace.
 */
export async function focusLayout(target: LayoutTarget): Promise<void> {
  if (target.mode === "window") {
    await tmuxRun(["select-window", "-t", target.address]);
    if (target.editorPaneId) {
      await tmuxRun(["select-pane", "-t", target.editorPaneId]).catch(() => undefined);
    }
    return;
  }
  await run("tmux", ["attach-session", "-t", target.session], { capture: false });
}

export async function refocusEditor(target: LayoutTarget): Promise<void> {
  if (!target.editorPaneId) return;
  await tmuxRun(["select-pane", "-t", target.editorPaneId]);
}

export async function respawnTestPane(
  target: LayoutTarget,
  repoRoot: string,
  cmd: string,
): Promise<void> {
  if (!target.testPaneId) return;
  await tmuxRun([
    "respawn-pane",
    "-k",
    "-t",
    target.testPaneId,
    "-c",
    repoRoot,
    "sh",
    "-c",
    cmd,
  ]);
}

export async function respawnEditorPane(
  target: LayoutTarget,
  repoRoot: string,
  workdirAbs: string,
): Promise<void> {
  if (!target.editorPaneId) return;
  await tmuxRun([
    "respawn-pane",
    "-k",
    "-t",
    target.editorPaneId,
    "-c",
    repoRoot,
    "nvim",
    ...nvimArgs(workdirAbs),
  ]);
}

function renderTestCommand(base: string[], chapter: ChapterConfig): string {
  const parts = [...base];
  if (chapter.testFilter && !parts.some((p) => p.includes(chapter.testFilter ?? ""))) {
    parts.push(chapter.testFilter);
  }
  return parts.map((p) => (p.includes(" ") ? `'${p}'` : p)).join(" ");
}
