import { Box, Text, useInput } from "ink";
import React from "react";

interface Props {
  hint: string;
  index: number;
  total: number;
  onClose: () => void;
}

export const HintModal: React.FC<Props> = ({ hint, index, total, onClose }) => {
  useInput((input, key) => {
    if (key.return || key.escape || input === "q") onClose();
  });

  return (
    <Box flexDirection="column" alignItems="center" paddingY={2}>
      <Box borderStyle="double" borderColor="yellow" paddingX={3} paddingY={1} flexDirection="column">
        <Text bold color="yellowBright">
          Hint {index + 1} of {total}
        </Text>
        <Box marginTop={1}>
          <Text>{hint}</Text>
        </Box>
      </Box>
      <Box marginTop={1}>
        <Text dimColor>⏎/esc to close</Text>
      </Box>
    </Box>
  );
};
