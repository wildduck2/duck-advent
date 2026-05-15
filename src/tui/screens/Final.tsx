import BigText from "ink-big-text";
import Gradient from "ink-gradient";
import { Box, Text, useInput } from "ink";
import React from "react";

interface Props {
  questName: string;
  totalChapters: number;
  durationMs: number;
  totalHints: number;
  onExit: () => void;
}

function fmtDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export const Final: React.FC<Props> = ({
  questName,
  totalChapters,
  durationMs,
  totalHints,
  onExit,
}) => {
  useInput((_, key) => {
    if (key.return || key.escape) onExit();
  });

  return (
    <Box flexDirection="column" alignItems="center" paddingY={1}>
      <Gradient name="pastel">
        <BigText text="QUEST" font="block" />
      </Gradient>
      <Gradient name="atlas">
        <BigText text="COMPLETE" font="block" />
      </Gradient>
      <Box marginTop={1} flexDirection="column" alignItems="center">
        <Text bold color="cyanBright">
          {questName}
        </Text>
        <Box marginTop={1}>
          <Text>
            <Text dimColor>chapters </Text>
            <Text color="white">{totalChapters}/{totalChapters}</Text>
            <Text dimColor> · total time </Text>
            <Text color="white">{fmtDuration(durationMs)}</Text>
            <Text dimColor> · hints </Text>
            <Text color="white">{totalHints}</Text>
          </Text>
        </Box>
      </Box>
      <Box marginTop={2}>
        <Text dimColor>press ⏎ to exit</Text>
      </Box>
    </Box>
  );
};
