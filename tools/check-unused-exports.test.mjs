import assert from "node:assert/strict";
import test from "node:test";

import { unusedExports } from "./check-unused-exports.mjs";

test("detecta una exportación que nadie usa (LIM-03)", () => {
  const files = new Map([
    ["src/a.ts", "export function used() {}\nexport const lonely = 1;\n"],
    ["src/b.ts", "import { used } from './a';\nused();\n"],
    ["src/a.test.ts", "export const fromTest = 2;\n"],
  ]);
  assert.deepEqual(unusedExports(files), ["src/a.ts: lonely"]);
});
