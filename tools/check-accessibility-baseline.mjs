import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const validationRoot = join(projectRoot, ".local", "validation", "accessibility-visual");
const baselinePath = join(projectRoot, "fixtures", "accessibility", "visual-baseline-v1.json");
const evidenceStamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const evidenceRelativePath = `.local/validation/accessibility-baseline/${evidenceStamp}`;
const evidenceDirectory = join(projectRoot, evidenceRelativePath.replaceAll("/", "\\"));
const outputPath = join(evidenceDirectory, "summary.json");

function fail(message) {
  throw new Error(message);
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function findLatestSummary() {
  const entries = await readdir(validationRoot, { withFileTypes: true });
  const candidates = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const summaryPath = join(validationRoot, entry.name, "summary.json");
    try {
      const details = await stat(summaryPath);
      candidates.push({ summaryPath, mtimeMs: details.mtimeMs });
    } catch {
      // Ignore incomplete evidence directories.
    }
  }
  candidates.sort((left, right) => right.mtimeMs - left.mtimeMs);
  if (candidates.length === 0) fail("No existe evidencia de accessibility:visual para comparar.");
  return candidates[0].summaryPath;
}

function relativePath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function assertEqual(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} no coincide con el baseline.`);
  }
}

const checks = [];
let status = "failed";
let error = null;
let summaryPath = null;

try {
  const baseline = await readJson(baselinePath);
  summaryPath = process.argv[2]
    ? resolve(projectRoot, process.argv[2])
    : await findLatestSummary();
  const summary = await readJson(summaryPath);
  if (summary.schemaVersion !== baseline.schemaVersion) fail("schemaVersion de la evidencia visual no soportada.");
  if (summary.captureVersion !== baseline.captureVersion) fail("captureVersion de la evidencia visual no soportada.");
  if (summary.status !== "passed") fail(`La evidencia visual no está aprobada: ${summary.status}.`);

  const actualCases = Array.isArray(summary.cases) ? summary.cases : [];
  assertEqual(
    actualCases.map((captureCase) => captureCase.name),
    baseline.requiredCases.map((captureCase) => captureCase.name),
    "Casos visuales",
  );

  for (const expectedCase of baseline.requiredCases) {
    const actualCase = actualCases.find((captureCase) => captureCase.name === expectedCase.name);
    if (!actualCase) fail(`Falta el caso visual ${expectedCase.name}.`);
    assertEqual(actualCase.viewport, expectedCase.viewport, `${expectedCase.name}.viewport`);
    assertEqual(actualCase.deviceScaleFactor, expectedCase.deviceScaleFactor, `${expectedCase.name}.deviceScaleFactor`);
    assertEqual(actualCase.forcedColors, expectedCase.forcedColors, `${expectedCase.name}.forcedColors`);
    if (actualCase.valid !== true) fail(`El caso visual ${expectedCase.name} no está marcado como válido.`);
    const inspection = actualCase.inspection ?? {};
    for (const landmark of baseline.contract.landmarks) {
      const count = inspection.landmarks?.[landmark];
      if (count !== 1) fail(`${expectedCase.name} no cumple el landmark ${landmark}.`);
    }
    if (inspection.minimumTargetSize < baseline.contract.minimumTargetSize) {
      fail(`${expectedCase.name} tiene targets menores al mínimo.`);
    }
    if (baseline.contract.requireVisibleFocus && (inspection.focus?.outlineStyle === "none" || inspection.focus?.outlineWidth === "0px")) {
      fail(`${expectedCase.name} perdió el foco visible.`);
    }
    if (baseline.contract.requireNoHorizontalOverflow && inspection.noHorizontalOverflow !== true) {
      fail(`${expectedCase.name} tiene overflow horizontal.`);
    }
    const screenshotPath = resolve(projectRoot, actualCase.screenshot);
    const screenshotBytes = await readFile(screenshotPath);
    const screenshotSha256 = createHash("sha256").update(screenshotBytes).digest("hex");
    if (!/^[a-f0-9]{64}$/.test(actualCase.screenshotSha256 ?? "")) {
      fail(`${expectedCase.name} no tiene SHA-256 de captura válido.`);
    }
    if (screenshotSha256 !== actualCase.screenshotSha256) {
      fail(`${expectedCase.name} tiene una captura cuyo hash no coincide.`);
    }
    checks.push({ name: expectedCase.name, status: "passed", screenshot: relativePath(screenshotPath) });
  }
  status = "passed";
} catch (caught) {
  error = caught instanceof Error ? caught.message : String(caught);
}

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify({
  schemaVersion: 1,
  status,
  generatedAt: new Date().toISOString(),
  baseline: relativePath(baselinePath),
  sourceSummary: summaryPath ? relativePath(summaryPath) : null,
  checks,
  error,
  evidenceDirectory: evidenceRelativePath,
}, null, 2)}\n`, "utf8");

if (status !== "passed") {
  console.error(`Baseline visual falló: ${error ?? "error desconocido"}. Evidencia: ${evidenceRelativePath}`);
  process.exitCode = 1;
} else {
  console.log(`Baseline visual aprobado: ${evidenceRelativePath}`);
}
