import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const source = readFileSync(new URL("./prepare-beta-gate.ps1", import.meta.url), "utf8");

test("la RC se escribe en UTF-8 sin BOM y cuenta los archivos sin seguimiento (QA-48)", () => {
  assert.doesNotMatch(source, /Set-Content[^\n]*-Encoding utf8/i);
  assert.match(source, /"status", "--porcelain", "--untracked-files=all"/);
  const writer = /function Write-Utf8NoBom \{[\s\S]*?\n\}/.exec(source)?.[0];
  assert.ok(writer, "falta Write-Utf8NoBom");
  const root = mkdtempSync(join(tmpdir(), "columnia-beta-gate-"));
  try {
    const target = join(root, "manifest.json");
    const run = spawnSync(
      "powershell",
      ["-NoProfile", "-Command", `${writer}\nWrite-Utf8NoBom '${target}' '{"versión": 1}'`],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    const bytes = readFileSync(target);
    assert.notDeepEqual([...bytes.subarray(0, 3)], [0xef, 0xbb, 0xbf]);
    assert.equal(bytes.toString("utf8"), '{"versión": 1}');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
