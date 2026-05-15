import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { combinedHash } from "../lib/hash";
import { repoCacheDir } from "./paths";

interface InstallRecord {
  lockfileHash: string;
  completedAt: string;
}

const LOCKFILES = ["bun.lock", "bun.lockb", "package-lock.json", "pnpm-lock.yaml", "yarn.lock"];

function lockfileFingerprint(repoRoot: string): string {
  const paths = ["package.json", ...LOCKFILES].map((p) => resolve(repoRoot, p));
  return combinedHash(paths);
}

export function installRecordPath(repoHash: string): string {
  return resolve(repoCacheDir(repoHash), "install.json");
}

export function hasFreshInstall(repoRoot: string, repoHash: string): boolean {
  const file = installRecordPath(repoHash);
  if (!existsSync(file)) return false;
  try {
    const record = JSON.parse(readFileSync(file, "utf8")) as InstallRecord;
    return record.lockfileHash === lockfileFingerprint(repoRoot);
  } catch {
    return false;
  }
}

export function markInstallComplete(repoRoot: string, repoHash: string): void {
  const record: InstallRecord = {
    lockfileHash: lockfileFingerprint(repoRoot),
    completedAt: new Date().toISOString(),
  };
  writeFileSync(installRecordPath(repoHash), `${JSON.stringify(record, null, 2)}\n`);
}
