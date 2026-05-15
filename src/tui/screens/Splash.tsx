import BigText from "ink-big-text";
import Gradient from "ink-gradient";
import { Box, Text, useInput } from "ink";
import React from "react";

interface Props {
  questName: string;
  description?: string;
  version: string;
  onContinue: () => void;
}

export const Splash: React.FC<Props> = ({ questName, description, version, onContinue }) => {
  useInput((_, key) => {
    if (key.return || key.escape) onContinue();
  });

  return (
    <Box flexDirection="column" alignItems="center" paddingY={1}>
      <Gradient name="vice">
        <BigText text="duck advent" font="block" />
      </Gradient>
      <Box marginTop={1} flexDirection="column" alignItems="center">
        <Text bold color="cyanBright">
          {questName}
        </Text>
        {description ? <Text dimColor>{description}</Text> : null}
        <Text dimColor>v{version}</Text>
      </Box>
      <Box marginTop={2}>
        <Text color="yellowBright">press </Text>
        <Text bold color="white">
          ⏎
        </Text>
        <Text color="yellowBright"> to begin · </Text>
        <Text bold color="white">
          esc
        </Text>
        <Text color="yellowBright"> to skip</Text>
      </Box>
    </Box>
  );
};
