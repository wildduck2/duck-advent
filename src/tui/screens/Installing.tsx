import { Box, Text } from "ink";
import Spinner from "ink-spinner";
import React, { useEffect, useState } from "react";
import { hasFreshInstall, markInstallComplete } from "../../cache/install";
import { run } from "../../lib/exec";

interface Props {
  repoRoot: string;
  repoHash: string;
  installCommand: string[];
  onDone: () => void;
  onError: (err: Error) => void;
}

type Phase = "checking" | "installing" | "done" | "skip";

export const Installing: React.FC<Props> = ({
  repoRoot,
  repoHash,
  installCommand,
  onDone,
  onError,
}) => {
  const [phase, setPhase] = useState<Phase>("checking");
  const [tail, setTail] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (hasFreshInstall(repoRoot, repoHash)) {
        setPhase("skip");
        setTimeout(() => onDone(), 250);
        return;
      }
      setPhase("installing");
      try {
        const [cmd, ...args] = installCommand;
        const result = await run(cmd, args, {
          capture: true,
          cwd: repoRoot,
        });
        if (cancelled) return;
        for (const line of result.stdout.split("\n").slice(-6)) {
          setTail((t) => [...t.slice(-5), line]);
        }
        if (result.code !== 0) {
          onError(new Error(`install exited ${result.code}: ${result.stderr.slice(0, 400)}`));
          return;
        }
        markInstallComplete(repoRoot, repoHash);
        setPhase("done");
        setTimeout(() => onDone(), 400);
      } catch (e) {
        if (!cancelled) onError(e as Error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [installCommand, onDone, onError, repoHash, repoRoot]);

  return (
    <Box flexDirection="column" paddingX={2} paddingY={1}>
      <Text bold color="cyanBright">
        Installing dependencies
      </Text>
      <Box marginTop={1}>
        {phase === "checking" ? (
          <>
            <Spinner type="dots" />
            <Text> checking lockfile…</Text>
          </>
        ) : phase === "skip" ? (
          <Text color="greenBright">✓ already installed (cache hit)</Text>
        ) : phase === "installing" ? (
          <>
            <Spinner type="dots" />
            <Text> running {installCommand.join(" ")}…</Text>
          </>
        ) : (
          <Text color="greenBright">✓ install complete</Text>
        )}
      </Box>
      {tail.length > 0 ? (
        <Box marginTop={1} flexDirection="column">
          {tail.map((l, i) => (
            <Text key={`${i}-${l}`} dimColor>
              {l}
            </Text>
          ))}
        </Box>
      ) : null}
    </Box>
  );
};
