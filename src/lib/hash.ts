import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";

export function sha256(input: string): string {
  return createHash("sha256").update(input).digest("hex");
}

export function fileHash(path: string): string | null {
  try {
    statSync(path);
  } catch {
    return null;
  }
  return sha256(readFileSync(path, "utf8"));
}

export function combinedHash(paths: string[]): string {
  const h = createHash("sha256");
  for (const p of paths) {
    const fh = fileHash(p);
    if (fh) h.update(`${p}:${fh}\n`);
  }
  return h.digest("hex");
}
