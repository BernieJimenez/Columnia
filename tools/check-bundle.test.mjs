import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { checkFrontendBudget, parseArguments, reportProducedArtifacts, snapshotArtifacts } from "./check-bundle.mjs";

function withDirectory(run) {
  const root = mkdtempSync(join(tmpdir(), "columnia-bundle-test-"));
  return Promise.resolve(run(root)).finally(() => rmSync(root, { recursive: true, force: true }));
}

test("snapshot y artifacts nombran la opción que falta (COD-19)", () => {
  assert.throws(() => parseArguments(["snapshot"]), /snapshot requiere --output/);
  assert.throws(() => parseArguments(["artifacts", "--output", "a.json"]), /artifacts requiere --snapshot/);
  assert.equal(parseArguments(["budget"]).command, "budget");
});

test("el presupuesto mide también fuentes y rechaza source maps (COD-19)", () => withDirectory((root) => {
  const dist = join(root, "dist");
  mkdirSync(join(dist, "assets"), { recursive: true });
  writeFileSync(join(dist, "assets", "app.js"), "console.log(1);\n");
  writeFileSync(join(dist, "assets", "font.woff2"), Buffer.alloc(600 * 1024));
  writeFileSync(join(dist, "assets", "app.js.map"), "{}");
  const output = join(root, "budget.json");
  assert.throws(() => checkFrontendBudget(dist, output), /source maps|exceden/);
  const evidence = JSON.parse(readFileSync(output, "utf8"));
  assert.ok(evidence.files.some((file) => file.path === "assets/font.woff2" && file.kind === "other"));
  assert.equal(evidence.violations.length, 3);
}));

test("un artefacto con otra fecha pero el mismo contenido no cuenta como producido (COD-19)", () => withDirectory(async (root) => {
  const bundle = join(root, "bundle");
  mkdirSync(bundle);
  const installer = join(bundle, "Columnia_setup.exe");
  writeFileSync(installer, "same bytes");
  const snapshot = join(root, "snapshot.json");
  await snapshotArtifacts(root, bundle, snapshot);
  utimesSync(installer, new Date(2030, 0, 1), new Date(2030, 0, 1));
  const output = join(root, "artifacts.json");
  await assert.rejects(reportProducedArtifacts(root, bundle, snapshot, output), /sin producir/);
  assert.equal(JSON.parse(readFileSync(output, "utf8")).touchedWithoutChanges.length, 1);
  writeFileSync(installer, "new bytes");
  await reportProducedArtifacts(root, bundle, snapshot, output);
  assert.equal(JSON.parse(readFileSync(output, "utf8")).artifacts.length, 1);
}));
