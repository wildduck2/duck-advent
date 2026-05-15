import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { repoCacheDir } from "./paths";

export interface ValidatorResult {
  id: string;
  passed: boolean;
  output: string;
  checkedAt: string;
}

interface ValidatorRecord {
  configHash: string;
  results: Record<string, ValidatorResult>;
}

function file(repoHash: string): string {
  return resolve(repoCacheDir(repoHash), "validators.json");
}

export function readValidatorCache(repoHash: string, configHash: string): ValidatorRecord | null {
  const f = file(repoHash);
  if (!existsSync(f)) return null;
  try {
    const record = JSON.parse(readFileSync(f, "utf8")) as ValidatorRecord;
    if (record.configHash !== configHash) return null;
    return record;
  } catch {
    return null;
  }
}

export function writeValidatorCache(
  repoHash: string,
  configHash: string,
  results: ValidatorResult[],
): void {
  const record: ValidatorRecord = {
    configHash,
    results: Object.fromEntries(results.map((r) => [r.id, r])),
  };
  writeFileSync(file(repoHash), `${JSON.stringify(record, null, 2)}\n`);
}

export function clearValidatorCache(repoHash: string): void {
  const f = file(repoHash);
  if (existsSync(f)) writeFileSync(f, "");
}
