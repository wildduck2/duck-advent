import { Box, Text, useApp } from "ink";
import React, { useCallback, useEffect, useState } from "react";
import { setCurrentChapter, readProgress } from "../cache/progress";
import type { LoadedConfig } from "../config/schema";
import { composeUp } from "../quest/docker";
import {
  findChapterBySlug,
  firstChapter,
  nextChapter,
  readBriefing,
} from "../quest/runner";
import { writeRuntimeStatus } from "../session/status";
import { Briefing } from "./screens/Briefing";
import { Installing } from "./screens/Installing";
import { Splash } from "./screens/Splash";
import { Validating } from "./screens/Validating";

type Phase = "splash" | "installing" | "validating" | "briefing" | "launching" | "error";

interface Props {
  loaded: LoadedConfig;
  cliVersion: string;
  /**
   * If true, jump straight to validation/launch (skip splash). Used when
   * re-invoking duck-advent from inside an active tmux session.
   */
  skipSplash?: boolean;
  revalidate?: boolean;
  /**
   * Called once the user has cleared the briefing and the session is ready to
   * launch tmux. The caller is responsible for the tmux attach.
   */
  onLaunch: (chapterSlug: string) => Promise<void>;
}

export const App: React.FC<Props> = ({
  loaded,
  cliVersion,
  skipSplash,
  revalidate,
  onLaunch,
}) => {
  const { exit } = useApp();
  const [phase, setPhase] = useState<Phase>(skipSplash ? "installing" : "splash");
  const [errorMsg, setErrorMsg] = useState<string>("");
  const [chapterSlug, setChapterSlug] = useState<string>(() => {
    const progress = readProgress(loaded.repoHash);
    return progress.currentChapter ?? firstChapter(loaded).slug;
  });

  const chapter = findChapterBySlug(loaded, chapterSlug) ?? firstChapter(loaded);

  const fail = useCallback(
    (err: Error) => {
      setErrorMsg(err.message);
      setPhase("error");
    },
    [],
  );

  useEffect(() => {
    if (phase !== "launching") return;
    setCurrentChapter(loaded.repoHash, chapter.slug);
    writeRuntimeStatus(loaded.repoRoot, loaded.config.cacheDir, {
      chapterNumber: chapter.number,
      totalChapters: loaded.config.quests.length,
      title: chapter.title,
      hintsUsed: readProgress(loaded.repoHash).chapters[chapter.slug]?.hintsUsed ?? 0,
      tier: chapter.tier,
    });
    composeUp(loaded.config, loaded.repoRoot, chapter.services)
      .then(() => onLaunch(chapter.slug))
      .then(() => exit())
      .catch((err) => fail(err as Error));
  }, [chapter, exit, fail, loaded, onLaunch, phase]);

  if (phase === "splash") {
    return (
      <Splash
        questName={loaded.config.name}
        description={loaded.config.description}
        version={cliVersion}
        onContinue={() => setPhase("installing")}
      />
    );
  }

  if (phase === "installing") {
    return (
      <Installing
        repoRoot={loaded.repoRoot}
        repoHash={loaded.repoHash}
        installCommand={loaded.config.installCommand}
        onDone={() => setPhase("validating")}
        onError={fail}
      />
    );
  }

  if (phase === "validating") {
    return (
      <Validating
        validators={loaded.config.validators}
        repoHash={loaded.repoHash}
        configHash={loaded.configHash}
        revalidate={!!revalidate}
        onDone={() => setPhase("briefing")}
        onError={fail}
      />
    );
  }

  if (phase === "briefing") {
    const md = readBriefing(loaded, chapter);
    return (
      <Briefing
        chapter={chapter}
        markdown={md}
        onClose={() => setPhase("launching")}
      />
    );
  }

  if (phase === "launching") {
    return (
      <Box padding={1}>
        <Text dimColor>preparing services for {chapter.title}…</Text>
      </Box>
    );
  }

  return (
    <Box padding={1} flexDirection="column">
      <Text color="redBright" bold>
        ✗ {errorMsg}
      </Text>
      <Text dimColor>fix the issue above, then re-run duck-advent.</Text>
    </Box>
  );
};
