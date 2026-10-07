import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./generate-sbom.ps1", import.meta.url));

test("el SBOM incluye npm y distingue lo distribuido de lo que no (LIM-14)", () => {
  const root = mkdtempSync(join(tmpdir(), "columnia-sbom-"));
  try {
    const output = join(root, "sbom.json");
    const run = spawnSync(
      "powershell",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-OutputPath", output],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr || run.stdout);
    const bom = JSON.parse(readFileSync(output, "utf8").replace(/^﻿/, ""));
    const scopeOf = (ecosystem, name) => bom.components
      .filter((component) => component.name === name && component.purl.startsWith(`pkg:${ecosystem}/`))
      .map((component) => component.scope);
    assert.deepEqual([...new Set(scopeOf("npm", "react"))], ["required"]);
    assert.deepEqual([...new Set(scopeOf("npm", "vitest"))], ["excluded"]);
    assert.deepEqual([...new Set(scopeOf("cargo", "polars"))], ["required"]);
    // Only built for macOS: not part of the Windows installer.
    assert.deepEqual([...new Set(scopeOf("cargo", "core-foundation"))], ["excluded"]);
    assert.ok(bom.components.every((component) => ["required", "excluded"].includes(component.scope)));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
