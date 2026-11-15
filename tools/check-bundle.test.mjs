import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { buildTargetWarning, checkFrontendBudget, FRONTEND_LIMITS, parseArguments, reportProducedArtifacts, snapshotArtifacts } from "./check-bundle.mjs";

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
  writeFileSync(join(dist, "index.html"), '<script type="module" src="/assets/app.js"></script>');
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

test("avisa si el dist no se compiló con el target de Windows (AUD-079)", () => withDirectory((root) => {
  const dist = join(root, "dist");
  const mark = join(root, "mark.json");
  assert.match(buildTargetWarning(dist, mark), /No hay marca/);
  writeFileSync(mark, JSON.stringify({ target: "safari13", outDir: dist }));
  assert.match(buildTargetWarning(dist, mark), /safari13.*TAURI_ENV_PLATFORM=windows/);
  writeFileSync(mark, JSON.stringify({ target: "chrome105", outDir: join(root, "otro") }));
  assert.match(buildTargetWarning(dist, mark), /otro directorio/);
  writeFileSync(mark, JSON.stringify({ target: "chrome105", outDir: dist }));
  assert.equal(buildTargetWarning(dist, mark), null);
}));

// Dos límites: la carga inicial (index.html + imports estáticos) y el total (todo el JS+CSS).
// El chunk lazy pesa lo bastante para que el total y la carga inicial se distingan.
function makeDist(root) {
  const dist = join(root, "dist");
  mkdirSync(join(dist, "assets"), { recursive: true });
  writeFileSync(join(dist, "index.html"),
    '<!doctype html><script type="module" crossorigin src="/assets/entry.js"></script>' +
    '<link rel="stylesheet" crossorigin href="/assets/app.css">');
  writeFileSync(join(dist, "assets", "entry.js"), 'import { shared } from "./shared.js";\nconsole.log(shared, import("./lazy.js"));\n');
  writeFileSync(join(dist, "assets", "shared.js"), "export const shared = 1;\n");
  writeFileSync(join(dist, "assets", "app.css"), "body{margin:0}\n");
  writeFileSync(join(dist, "assets", "lazy.js"), `/*${randomBytes(100 * 1024).toString("base64")}*/\n`);
  return dist;
}

function measure(root, dist) {
  const output = join(root, "measure.json");
  checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({ initialLoad: { rawBytes: 1e9, gzipBytes: 1e9 }, total: { rawBytes: 1e9, gzipBytes: 1e9 } }));
  return JSON.parse(readFileSync(output, "utf8"));
}

function limitsWith(overrides) {
  return { ...FRONTEND_LIMITS, ...overrides };
}

test("la carga inicial solo cuenta index.html y sus imports estáticos, no los chunks lazy", () => withDirectory((root) => {
  const dist = makeDist(root);
  const evidence = measure(root, dist);
  assert.deepEqual(evidence.initialLoad.files, ["assets/app.css", "assets/entry.js", "assets/shared.js"]);
  assert.ok(evidence.totals.rawBytes > evidence.initialLoad.rawBytes + 100 * 1024);
}));

test("la carga inicial que excede su límite falla aunque el total quepa", () => withDirectory((root) => {
  const dist = makeDist(root);
  const initial = measure(root, dist).initialLoad;
  const output = join(root, "budget.json");
  assert.throws(() => checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({
    initialLoad: { rawBytes: initial.rawBytes - 1, gzipBytes: 1e9 },
    total: { rawBytes: 1e9, gzipBytes: 1e9 },
  })), /Carga inicial raw .* excede/);
  assert.throws(() => checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({
    initialLoad: { rawBytes: 1e9, gzipBytes: initial.gzipBytes - 1 },
    total: { rawBytes: 1e9, gzipBytes: 1e9 },
  })), /Carga inicial gzip .* excede/);
  assert.equal(JSON.parse(readFileSync(output, "utf8")).status, "failed");
}));

test("el total que excede su límite falla aunque la carga inicial quepa", () => withDirectory((root) => {
  const dist = makeDist(root);
  const measured = measure(root, dist);
  const output = join(root, "budget.json");
  assert.throws(() => checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({
    initialLoad: { rawBytes: measured.initialLoad.rawBytes, gzipBytes: measured.initialLoad.gzipBytes },
    total: { rawBytes: measured.totals.rawBytes - 1, gzipBytes: 1e9 },
  })), (error) => /Total raw .* excede/.test(error.message) && !/Carga inicial (raw|gzip) \d+ excede/.test(error.message));
  assert.throws(() => checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({
    initialLoad: { rawBytes: 1e9, gzipBytes: 1e9 },
    total: { rawBytes: 1e9, gzipBytes: measured.totals.gzipBytes - 1 },
  })), /Total gzip .* excede/);
}));

test("carga inicial y total dentro de sus límites pasan y se imprimen con su límite", () => withDirectory((root) => {
  const dist = makeDist(root);
  const measured = measure(root, dist);
  const output = join(root, "budget.json");
  checkFrontendBudget(dist, output, join(root, "no-mark.json"), limitsWith({
    initialLoad: { rawBytes: measured.initialLoad.rawBytes, gzipBytes: measured.initialLoad.gzipBytes },
    total: { rawBytes: measured.totals.rawBytes, gzipBytes: measured.totals.gzipBytes },
  }));
  const evidence = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(evidence.status, "passed");
  assert.equal(evidence.limits.initialLoad.rawBytes, measured.initialLoad.rawBytes);
}));
