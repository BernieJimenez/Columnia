import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// COD-02: the stylesheets grew by patches; these checks keep the cascade
// from collecting undefined tokens or the same rule twice again.
const files = ["src/styles.css", "src/workflow-styles.css", "src/features/explore/explore.css"];
const sources = files.map((file) => [file, readFileSync(join(process.cwd(), file), "utf8")] as const);

/** Every rule as "context | selector | sorted declarations". */
function ruleKeys(css: string): string[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const keys: string[] = [];
  const stack: string[] = [];
  let buffer = "";
  for (const character of text) {
    if (character === "{") {
      stack.push(buffer.trim().replace(/\s+/g, " "));
      buffer = "";
    } else if (character === "}") {
      const selector = stack.pop() ?? "";
      const declarations = buffer.split(";").map((part) => part.trim()).filter(Boolean).sort();
      if (declarations.length > 0 && !selector.startsWith("@")) {
        keys.push(`${stack.join(" > ")} | ${selector} | ${declarations.join("; ")}`);
      }
      buffer = "";
    } else {
      buffer += character;
    }
  }
  return keys;
}

describe("higiene de las hojas de estilo (COD-02)", () => {
  it("toda variable usada está definida", () => {
    const all = sources.map(([, css]) => css).join("\n");
    const defined = new Set([...all.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1]));
    const undefinedTokens = [...new Set([...all.matchAll(/var\((--[\w-]+)/g)].map((match) => match[1]))]
      .filter((token) => !defined.has(token));
    expect(undefinedTokens).toEqual([]);
  });

  it("ninguna regla se repite con las mismas declaraciones", () => {
    for (const [file, css] of sources) {
      const seen = new Set<string>();
      const repeated = ruleKeys(css).filter((key) => (seen.has(key) ? true : (seen.add(key), false)));
      expect(repeated, file).toEqual([]);
    }
  });
});
