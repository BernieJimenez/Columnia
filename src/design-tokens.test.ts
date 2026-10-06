import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** Every `.css` and `.tsx` file under `src`, where tokens are defined or used. */
function sources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sources(path);
    return /\.(css|tsx)$/.test(entry.name) ? [path] : [];
  });
}

describe("design tokens", () => {
  it("uses no CSS custom property that is never defined (TXT-03)", () => {
    const text = sources("src").map((path) => readFileSync(path, "utf8")).join("\n");
    const defined = new Set([...text.matchAll(/(--[a-z0-9-]+)\s*:/gi)].map((match) => match[1]));
    const used = new Set([...text.matchAll(/var\((--[a-z0-9-]+)/gi)].map((match) => match[1]));
    expect([...used].filter((token) => !defined.has(token))).toEqual([]);
  });
});
