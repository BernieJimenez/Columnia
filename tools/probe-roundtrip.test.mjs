import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./probe-roundtrip.ps1", import.meta.url));

function runBattery(probeBody) {
  const root = mkdtempSync(join(tmpdir(), "columnia-roundtrip-"));
  try {
    const fixtures = join(root, "fixtures");
    mkdirSync(fixtures);
    writeFileSync(join(fixtures, "a.csv"), "id\n1\n");
    writeFileSync(join(fixtures, "b.csv"), "id\n2\n");
    const probe = join(root, "probe.ps1");
    writeFileSync(probe, `param([switch]$RunNativeSelectors, [switch]$RunPrepareFlow, [string]$NativeDatasetPath, [int]$TimeoutSeconds)\n${probeBody}\n`);
    return spawnSync(
      "powershell",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-ProbePath", probe, "-FixtureDirectory", fixtures],
      { encoding: "utf8" },
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("una sonda que sale con código 1 sin excepción marca el fixture como fallido (QA-47)", () => {
  const run = runBattery('if ($NativeDatasetPath.EndsWith("b.csv")) { exit 1 }');
  assert.notEqual(run.status, 0);
  // Windows PowerShell writes the console code page, so the accents are not compared.
  assert.match(run.stderr + run.stdout, / en: b\.csv\. Revisa/);
});

test("la batería aprueba cuando cada sonda termina bien (QA-47)", () => {
  const run = runBattery("exit 0");
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /aprobada: 2 fixtures/);
});
