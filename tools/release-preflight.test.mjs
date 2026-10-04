import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const releaseScript = fileURLToPath(new URL("./release.ps1", import.meta.url));

test("el release aborta antes de compilar con variables de pruebas activas (OPS-07)", () => {
  const reportDirectory = mkdtempSync(join(tmpdir(), "columnia-release-test-"));
  try {
    const result = spawnSync("powershell.exe", [
      "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command",
      `[Console]::OutputEncoding = [Text.Encoding]::UTF8; & '${releaseScript}' -DryRun -SkipPackage -ReportPath '${join(reportDirectory, "summary.json")}'`,
    ], { encoding: "utf8", env: { ...process.env, COLUMNIA_TEST_HARNESS_MANIFEST: "1" } });
    const output = `${result.stdout}${result.stderr}`.replace(/\s+/g, " ");
    assert.notEqual(result.status, 0, output);
    assert.match(output, /variables de pruebas activas: COLUMNIA_TEST_HARNESS_MANIFEST/);
    assert.doesNotMatch(output, /Toolchains/);
  } finally {
    rmSync(reportDirectory, { recursive: true, force: true });
  }
});
