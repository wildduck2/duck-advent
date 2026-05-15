import BigText from "ink-big-text";
import Gradient from "ink-gradient";
import { Box, Text, useInput } from "ink";
import React, { useEffect, useState } from "react";

interface Props {
  chapterNumber: number;
  chapterTitle: string;
  hintsUsed: number;
  attempts: number;
  durationMs: number;
  onContinue: () => void;
}

const CONFETTI = ["✦", "✧", "✺", "✹", "✸", "✷", "★", "✦", "❉", "❋", "❅", "❆"];
const COLORS = [
  "redBright",
  "greenBright",
  "yellowBright",
  "blueBright",
  "magentaBright",
  "cyanBright",
  "whiteBright",
] as const;

interface Piece {
  ch: string;
  color: (typeof COLORS)[number];
  x: number;
  y: number;
}

function fmtDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  if (m === 0) return `${s}s`;
  return `${m}m ${s % 60}s`;
}

export const Celebrate: React.FC<Props> = ({
  chapterNumber,
  chapterTitle,
  hintsUsed,
  attempts,
  durationMs,
  onContinue,
}) => {
  const cols = process.stdout.columns ?? 80;
  const rows = 8;
  const [pieces, setPieces] = useState<Piece[]>([]);

  useEffect(() => {
    const id = setInterval(() => {
      setPieces((p) =>
        [
          ...p
            .map((x) => ({ ...x, y: x.y + 1 }))
            .filter((x) => x.y < rows),
          ...Array.from({ length: 6 }, () => ({
            ch: CONFETTI[Math.floor(Math.random() * CONFETTI.length)],
            color: COLORS[Math.floor(Math.random() * COLORS.length)],
            x: Math.floor(Math.random() * cols),
            y: 0,
          })),
        ].slice(-60),
      );
    }, 120);
    const timeout = setTimeout(() => onContinue(), 4500);
    return () => {
      clearInterval(id);
      clearTimeout(timeout);
    };
  }, [cols, onContinue]);

  useInput((_, key) => {
    if (key.return || key.escape) onContinue();
  });

  const grid: string[][] = Array.from({ length: rows }, () => Array.from({ length: cols }, () => " "));
  const colorGrid: ((typeof COLORS)[number] | undefined)[][] = Array.from({ length: rows }, () =>
    Array.from({ length: cols }, () => undefined),
  );
  for (const p of pieces) {
    if (p.y >= 0 && p.y < rows && p.x >= 0 && p.x < cols) {
      grid[p.y][p.x] = p.ch;
      colorGrid[p.y][p.x] = p.color;
    }
  }

  return (
    <Box flexDirection="column">
      <Box flexDirection="column">
        {grid.map((row, ri) => (
          <Box key={ri}>
            {row.map((cell, ci) => {
              const color = colorGrid[ri][ci];
              return (
                <Text key={ci} color={color}>
                  {cell}
                </Text>
              );
            })}
          </Box>
        ))}
      </Box>
      <Box flexDirection="column" alignItems="center" marginTop={1}>
        <Gradient name="rainbow">
          <BigText text={`CH ${chapterNumber}`} font="block" />
        </Gradient>
        <Text bold color="greenBright">
          Complete — {chapterTitle}
        </Text>
        <Box marginTop={1}>
          <Text>
            <Text dimColor>time </Text>
            <Text color="white">{fmtDuration(durationMs)}</Text>
            <Text dimColor> · attempts </Text>
            <Text color="white">{attempts}</Text>
            <Text dimColor> · hints </Text>
            <Text color="white">
              {hintsUsed}/3
            </Text>
          </Text>
        </Box>
        <Box marginTop={1}>
          <Text dimColor>press ⏎ to continue…</Text>
        </Box>
      </Box>
    </Box>
  );
};
