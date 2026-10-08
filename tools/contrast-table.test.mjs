import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { contrastRatio, contrastTable, themeBlocks, withContrastTable } from "./contrast-table.mjs";

const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const design = readFileSync(new URL("../DESIGN.md", import.meta.url), "utf8").replace(/\r\n/g, "\n");

test("DESIGN.md cita la tabla que sale de src/styles.css (UI-02)", () => {
  assert.equal(withContrastTable(design, css), design);
});

test("cambiar un color en src/styles.css cambia la tabla sin tocar otro archivo (UI-02)", () => {
  const changed = css.replace("--control-border: #7d8b8e;", "--control-border: #6b797c;");
  assert.notEqual(contrastTable(changed), contrastTable(css));
  assert.ok(themeBlocks(css).length >= 8);
  assert.equal(contrastRatio("#000000", "#ffffff").toFixed(0), "21");
});

test("cada par de la tabla alcanza su mínimo (UI-02)", () => {
  for (const line of contrastTable(css).split("\n").slice(2)) {
    const [ratio, minimum] = [...line.matchAll(/(\d+(?:\.\d+)?):1/g)].map((match) => Number(match[1]));
    assert.ok(ratio >= minimum, line);
  }
});
