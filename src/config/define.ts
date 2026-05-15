import type { QuestConfig } from "./schema";

/**
 * Type-only helper that gives users autocomplete + structural validation when
 * authoring their `quest.config.ts`. The CLI loads the same module and re-parses
 * the exported object with the zod schema at runtime — defineQuest is purely a
 * compile-time aid.
 */
export function defineQuest(config: QuestConfig): QuestConfig {
  return config;
}

export type { QuestConfig, ChapterConfig, ServiceConfig, ValidatorConfig } from "./schema";
