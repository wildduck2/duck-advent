import { z } from "zod";

export const QuestStepSchema = z.object({
  number: z.number().int().positive(),
  slug: z.string().min(1),
  title: z.string().min(1),
  tier: z.string().optional(),
  difficulty: z.number().int().min(1).max(5).optional(),
  briefing: z.string().min(1),
  workdir: z.string().min(1),
  testFilter: z.string().optional(),
  services: z.array(z.string()).default([]),
  seed: z.string().optional(),
  hints: z.array(z.string()).default([]),
});
export type QuestStep = z.infer<typeof QuestStepSchema>;
// Back-compat alias.
export const ChapterSchema = QuestStepSchema;
export type ChapterConfig = QuestStep;

export const ServiceSchema = z.object({
  compose: z.string().min(1),
  container: z.string().min(1),
  readyCheck: z.array(z.string()).optional(),
});
export type ServiceConfig = z.infer<typeof ServiceSchema>;

export const ValidatorSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  cmd: z.array(z.string()).min(1),
  min: z.string().optional(),
  optional: z.boolean().default(false),
});
export type ValidatorConfig = z.infer<typeof ValidatorSchema>;

/**
 * The CLI accepts either `quests: [...]` (preferred) or `chapters: [...]`
 * (legacy). The loader normalizes to `quests` after parsing.
 */
const QuestConfigShape = z.object({
  name: z.string().min(1),
  description: z.string().optional(),
  packageManager: z.enum(["bun", "pnpm", "npm", "yarn"]).default("bun"),
  installCommand: z.array(z.string()).min(1),
  testCommand: z.array(z.string()).min(1),
  branchPrefix: z.string().default("chapter-"),
  cacheDir: z.string().default(".gentleduck"),
  validators: z.array(ValidatorSchema).default([]),
  services: z.record(z.string(), ServiceSchema).default({}),
  quests: z.array(QuestStepSchema).optional(),
  chapters: z.array(QuestStepSchema).optional(),
});

export const QuestConfigSchema = QuestConfigShape.transform((cfg) => {
  const steps = cfg.quests ?? cfg.chapters ?? [];
  if (steps.length === 0) {
    throw new Error("quest config must define `quests: [...]` (or legacy `chapters: [...]`).");
  }
  return { ...cfg, quests: steps, chapters: steps };
});
export type QuestConfig = z.infer<typeof QuestConfigSchema>;

export interface LoadedConfig {
  repoRoot: string;
  configPath: string;
  config: QuestConfig;
  repoHash: string;
  configHash: string;
}
