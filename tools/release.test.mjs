import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./release.ps1", import.meta.url));
const source = readFileSync(script, "utf8");

test("al fallar, el release imprime el reporte y sale con código 1 (OPS-18)", () => {
  const root = mkdtempSync(join(tmpdir(), "columnia-release-"));
  try {
    const report = join(root, "summary.json");
    // A test-harness variable makes the release stop at its first check,
    // before any gate or build runs.
    const run = spawnSync(
      "powershell",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-DryRun", "-SkipPackage", "-ReportPath", report],
      { encoding: "utf8", env: { ...process.env, COLUMNIA_TEST_LOCALE: "es" } },
    );
    assert.equal(run.status, 1, run.stderr);
    assert.match(run.stdout, /Reporte: /);
    assert.match(run.stderr, /COLUMNIA_TEST_LOCALE/);
    const summary = JSON.parse(readFileSync(report, "utf8").replace(/^﻿/, ""));
    assert.equal(summary.status, "failed");
    assert.equal(summary.dryRun, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("cada paso parte de un código de salida limpio y el manifiesto se comprueba (OPS-19)", () => {
  assert.match(source, /\$global:LASTEXITCODE = 0\s+& \$Command/);
  assert.match(source, /& node @ManifestArguments\s+#[^\n]*\n\s+if \(\$LASTEXITCODE -ne 0\) \{\s+throw "tools\/generate-updater-manifest\.mjs/);
  assert.match(source, /check-updater-manifest\.mjs[^\n]*\n\s+if \(\$LASTEXITCODE -ne 0\)/);
  assert.match(source, /\.PARAMETER DryRun/);
});
