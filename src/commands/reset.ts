import { loadConfig } from "../config/loader";
import { findChapterByBranch, resetChapter } from "../quest/runner";
import { currentBranch } from "../quest/git";

export async function resetCommand(cwd: string): Promise<void> {
  const loaded = await loadConfig(cwd);
  const branch = await currentBranch(loaded.repoRoot);
  const chapter = findChapterByBranch(loaded, branch);
  if (!chapter) {
    console.error(`✗ not on a recognized chapter branch (${branch}).`);
    process.exit(1);
  }
  console.log(`tearing down + bringing up services for ${chapter.title}…`);
  await resetChapter(loaded, chapter);
  console.log("✓ reset complete.");
}
