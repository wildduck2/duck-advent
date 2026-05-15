import { Box, Text, useInput } from "ink";
import React, { useMemo, useState } from "react";
import { renderMarkdown } from "../../lib/markdown";

interface Props {
  chapter: { number: number; title: string; difficulty?: number; tier?: string };
  markdown: string;
  onClose: () => void;
}

export const Briefing: React.FC<Props> = ({ chapter, markdown, onClose }) => {
  const lines = useMemo(() => renderMarkdown(markdown).split("\n"), [markdown]);
  const [scroll, setScroll] = useState(0);
  const pageSize = Math.max(10, (process.stdout.rows ?? 30) - 8);
  const maxScroll = Math.max(0, lines.length - pageSize);

  useInput((input, key) => {
    if (key.escape || input === "q") onClose();
    else if (input === "j" || key.downArrow) setScroll((s) => Math.min(maxScroll, s + 1));
    else if (input === "k" || key.upArrow) setScroll((s) => Math.max(0, s - 1));
    else if (input === " " || key.pageDown)
      setScroll((s) => Math.min(maxScroll, s + Math.floor(pageSize / 2)));
    else if (key.pageUp) setScroll((s) => Math.max(0, s - Math.floor(pageSize / 2)));
    else if (input === "g") setScroll(0);
    else if (input === "G") setScroll(maxScroll);
  });

  const slice = lines.slice(scroll, scroll + pageSize);
  const tier = chapter.tier ? ` · ${chapter.tier}` : "";
  const diff = chapter.difficulty ? ` · ${chapter.difficulty}/5` : "";

  return (
    <Box flexDirection="column">
      <Box borderStyle="round" borderColor="cyan" paddingX={1}>
        <Text bold color="cyanBright">
          Quest {String(chapter.number).padStart(2, "0")} — {chapter.title}
        </Text>
        <Text dimColor>
          {tier}
          {diff}
        </Text>
      </Box>
      <Box flexDirection="column" paddingX={1}>
        {slice.map((l, i) => (
          <Text key={`${scroll}-${i}`}>{l}</Text>
        ))}
      </Box>
      <Box borderStyle="single" borderColor="gray" paddingX={1} marginTop={1}>
        <Text dimColor>
          j/k scroll · space pgdn · g/G top/bottom · q/esc close ({scroll + slice.length}/
          {lines.length})
        </Text>
      </Box>
    </Box>
  );
};
