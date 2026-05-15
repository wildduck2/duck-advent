import { Box, Text } from "ink";
import Spinner from "ink-spinner";
import React, { useEffect, useState } from "react";
import {
  readValidatorCache,
  writeValidatorCache,
  type ValidatorResult,
} from "../../cache/validators";
import { runCapture } from "../../lib/exec";
import type { ValidatorConfig } from "../../config/schema";

interface Props {
  validators: ValidatorConfig[];
  repoHash: string;
  configHash: string;
  revalidate: boolean;
  onDone: () => void;
  onError: (err: Error) => void;
}

interface State {
  status: "pending" | "running" | "passed" | "failed";
  output?: string;
}

export const Validating: React.FC<Props> = ({
  validators,
  repoHash,
  configHash,
  revalidate,
  onDone,
  onError,
}) => {
  const [state, setState] = useState<Record<string, State>>(() => {
    const init: Record<string, State> = {};
    for (const v of validators) init[v.id] = { status: "pending" };
    return init;
  });
  const [skipped, setSkipped] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!revalidate) {
        const cached = readValidatorCache(repoHash, configHash);
        if (cached) {
          const next: Record<string, State> = {};
          for (const v of validators) {
            const r = cached.results[v.id];
            next[v.id] = r
              ? { status: r.passed ? "passed" : "failed", output: r.output }
              : { status: "pending" };
          }
          setState(next);
          setSkipped(true);
          const failed = validators.find((v) => !v.optional && !cached.results[v.id]?.passed);
          if (failed) {
            onError(new Error(`validator failed (cached): ${failed.label}`));
            return;
          }
          setTimeout(() => onDone(), 400);
          return;
        }
      }

      const results: ValidatorResult[] = [];
      for (const v of validators) {
        if (cancelled) return;
        setState((s) => ({ ...s, [v.id]: { status: "running" } }));
        const [cmd, ...args] = v.cmd;
        const r = await runCapture(cmd, args);
        const passed = r.code === 0;
        const output = (r.stdout + r.stderr).slice(0, 300);
        results.push({
          id: v.id,
          passed,
          output,
          checkedAt: new Date().toISOString(),
        });
        setState((s) => ({
          ...s,
          [v.id]: { status: passed ? "passed" : "failed", output },
        }));
      }
      if (cancelled) return;
      writeValidatorCache(repoHash, configHash, results);
      const failed = results.find((r) => {
        const v = validators.find((vv) => vv.id === r.id);
        return v && !v.optional && !r.passed;
      });
      if (failed) {
        onError(new Error(`validator failed: ${failed.id}`));
        return;
      }
      setTimeout(() => onDone(), 400);
    })();
    return () => {
      cancelled = true;
    };
  }, [validators, repoHash, configHash, revalidate, onDone, onError]);

  return (
    <Box flexDirection="column" paddingX={2} paddingY={1}>
      <Text bold color="cyanBright">
        Validating environment{skipped ? " (cached)" : ""}
      </Text>
      <Box marginTop={1} flexDirection="column">
        {validators.map((v) => {
          const s = state[v.id];
          return (
            <Box key={v.id}>
              <Text>
                {s.status === "passed" ? (
                  <Text color="greenBright">✓</Text>
                ) : s.status === "failed" ? (
                  <Text color="redBright">✗</Text>
                ) : s.status === "running" ? (
                  <Spinner type="dots" />
                ) : (
                  <Text dimColor>○</Text>
                )}
                {"  "}
                {v.label}
                {v.optional ? <Text dimColor> (optional)</Text> : null}
              </Text>
            </Box>
          );
        })}
      </Box>
    </Box>
  );
};
