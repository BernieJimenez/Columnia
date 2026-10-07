import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./probe-webview2-native-selectors.mjs", import.meta.url), "utf8");

test("el probe solo usa %TEMP% para su carpeta temporal, que borra al terminar (SEG-12)", () => {
  const uses = source.split(/\r?\n/).filter((line) => line.includes("tmpdir()"));
  assert.deepEqual(uses.map((line) => line.trim()), [
    'temporaryDirectory = mkdtempSync(join(tmpdir(), "columnia-native-selectors-"));',
  ]);
  assert.match(source, /rmSync\(temporaryDirectory, \{ recursive: true, force: true \}\)/);
  assert.doesNotMatch(source, /columnia-prepare-[\w-]+\.png/);
});

test("el código de salida sigue al estado del resultado y no exige 4 columnas (QA-54)", () => {
  assert.match(source, /process\.exitCode = result\?\.status === "passed" \? 0 : 1;/);
  assert.doesNotMatch(source, /columnCount === 4/);
});
