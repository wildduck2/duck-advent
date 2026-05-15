import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

/**
 * The .gentleduck cache lives under the user's home directory so multiple
 * advent-code projects on the same machine share a single store.
 */
export function gentleduckRoot(): string {
  return resolve(homedir(), ".gentleduck");
}

export function repoCacheDir(repoHash: string): string {
  const dir = resolve(gentleduckRoot(), "cache", repoHash);
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function repoStateDir(repoHash: string): string {
  const dir = resolve(gentleduckRoot(), "state", repoHash);
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function logDir(): string {
  const dir = resolve(gentleduckRoot(), "log");
  mkdirSync(dir, { recursive: true });
  return dir;
}
