import { readProgress } from "../cache/progress";
import { loadConfig } from "../config/loader";
import { currentBranch } from "../quest/git";
import { findQuestByBranch } from "../quest/runner";

export async function statusCommand(cwd: string): Promise<void> {
  const loaded = await loadConfig(cwd);
  const branch = await currentBranch(loaded.repoRoot);
  const quest = findQuestByBranch(loaded, branch);
  const progress = readProgress(loaded.repoHash);
  const total = loaded.config.quests.length;

  console.log(`journey:  ${loaded.config.name}`);
  console.log(`branch:   ${branch}`);
  if (quest) {
    console.log(`quest:    ${quest.number}/${total} — ${quest.title}`);
    if (quest.tier) console.log(`tier:     ${quest.tier}`);
    if (quest.difficulty) console.log(`difficulty: ${quest.difficulty}/5`);
    console.log(`services: ${quest.services.join(", ") || "(none)"}`);
    const hints = progress.chapters[quest.slug]?.hintsUsed ?? 0;
    console.log(`hints used: ${hints}/${quest.hints.length}`);
  } else {
    console.log("quest:    (not on a recognized quest branch — run `duck-advent next` to begin)");
  }
  console.log(`completed: ${progress.completed.length}/${total}`);
}
