import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

// QA-50: the contract script also runs under `node --test` (npm run test:tools).
test("el contrato del updater rechaza cada caso inseguro", () => {
  const script = fileURLToPath(new URL("./test-updater-manifest.mjs", import.meta.url));
  const run = spawnSync(process.execPath, [script], { encoding: "utf8", windowsHide: true });
  assert.equal(run.status, 0, run.stderr || run.stdout);
  assert.match(run.stdout, /otra clave, SHA distinto/);
});
