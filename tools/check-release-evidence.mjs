import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const baselinePath = join(projectRoot, "fixtures", "accessibility", "release-evidence-baseline-v1.json");
const evidenceRoot = join(projectRoot, ".local", "validation", "release-evidence");
const checkStamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const checkDirectory = join(projectRoot, ".local", "validation", "release-evidence-check", checkStamp);
const checkOutput = join(checkDirectory, "summary.json");
const updateBaseline = process.argv.includes("--update-baseline");
const requestedSummary = process.argv.find((argument) => argument.startsWith("--summary="))?.slice("--summary=".length);

function fail(message) {
  throw new Error(message);
}

function relativePath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function gitOutput(argumentsList) {
  return execFileSync("git", argumentsList, { cwd: projectRoot, encoding: "utf8" }).trim();
}

function isBaselineCommit(currentCommit, evidenceCommit) {
  const parents = gitOutput(["rev-list", "--parents", "-n", "1", currentCommit]).split(/\s+/);
  if (parents.length !== 2 || parents[1] !== evidenceCommit) return false;
  const changedFiles = gitOutput(["diff-tree", "--no-commit-id", "--name-only", "-r", currentCommit])
    .split(/\r?\n/)
    .filter(Boolean);
  return changedFiles.length === 1 && changedFiles[0] === relativePath(baselinePath);
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function latestSummary() {
  if (requestedSummary) return resolve(projectRoot, requestedSummary);
  const entries = await readdir(evidenceRoot, { withFileTypes: true });
  const candidates = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const summaryPath = join(evidenceRoot, entry.name, "summary.json");
    try {
      candidates.push({ summaryPath, mtime: (await stat(summaryPath)).mtimeMs });
    } catch {
      // Ignore incomplete evidence directories.
    }
  }
  candidates.sort((left, right) => right.mtime - left.mtime);
  if (candidates.length === 0) fail("No existe evidencia release para comparar.");
  return candidates[0].summaryPath;
}

const checks = [];
let status = "failed";
let error = null;
let summaryPath = null;

try {
  const baseline = await readJson(baselinePath);
  summaryPath = await latestSummary();
  const summary = await readJson(summaryPath);
  const packageManifest = await readJson(join(projectRoot, "package.json"));
  const currentGit = {
    commit: gitOutput(["rev-parse", "HEAD"]),
    branch: gitOutput(["branch", "--show-current"]),
    dirty: gitOutput(["status", "--porcelain"]).length > 0,
  };
  if (currentGit.dirty) fail("El árbol Git debe estar limpio para validar evidencia release.");
  const currentCommitMatchesEvidence = summary.git?.commit === currentGit.commit
    || isBaselineCommit(currentGit.commit, summary.git?.commit);
  if (!currentCommitMatchesEvidence || summary.git?.branch !== currentGit.branch || summary.git?.dirty !== false) {
    fail("La evidencia release no corresponde al HEAD limpio actual.");
  }
  if (!updateBaseline && baseline.git?.commit && summary.git.commit !== baseline.git.commit) {
    fail("La evidencia release no corresponde al commit aprobado por el baseline.");
  }
  if (!updateBaseline && (summary.lockfiles?.packageLockSha256 !== baseline.lockfiles?.packageLockSha256 || summary.lockfiles?.cargoLockSha256 !== baseline.lockfiles?.cargoLockSha256)) {
    fail("Los lockfiles de la evidencia release no coinciden con el baseline.");
  }
  if (summary.status !== "passed") fail(`La evidencia release no está aprobada: ${summary.status}.`);
  if (summary.schemaVersion !== baseline.schemaVersion || summary.captureVersion !== baseline.captureVersion) {
    fail("La versión del contrato de evidencia release no coincide.");
  }
  if (summary.source !== baseline.source) fail("La evidencia no proviene del binario release.");
  if (summary.projectVersion !== packageManifest.version || summary.projectVersion !== baseline.projectVersion) {
    fail("La versión de evidencia no coincide con el baseline y package.json.");
  }
  if (summary.fixture?.path !== baseline.fixturePath) fail("La fixture de evidencia no coincide con el baseline.");
  if (!/^[a-f0-9]{64}$/.test(summary.binary?.sha256 ?? "")) fail("El binario no tiene SHA-256 válido.");
  if (!/^[a-f0-9]{64}$/.test(summary.fixture?.sha256 ?? "")) fail("La fixture no tiene SHA-256 válido.");
  if (!Number.isInteger(summary.binary?.sizeBytes) || summary.binary.sizeBytes <= 0) fail("El tamaño del binario no es válido.");
  if (!Number.isInteger(summary.fixture?.sizeBytes) || summary.fixture.sizeBytes <= 0) fail("El tamaño de la fixture no es válido.");

  const actualCases = Array.isArray(summary.cases) ? summary.cases : [];
  if (JSON.stringify(actualCases.map((captureCase) => captureCase.name)) !== JSON.stringify(baseline.requiredCases.map((captureCase) => captureCase.name))) {
    fail("Los casos de captura no coinciden con el baseline.");
  }
  for (const expectedCase of baseline.requiredCases) {
    const actualCase = actualCases.find((captureCase) => captureCase.name === expectedCase.name);
    if (!actualCase) fail(`Falta el caso ${expectedCase.name}.`);
    if (JSON.stringify(actualCase.viewport) !== JSON.stringify(expectedCase.viewport)) fail(`${expectedCase.name}: viewport cambió.`);
    if (actualCase.deviceScaleFactor !== expectedCase.deviceScaleFactor || actualCase.forcedColors !== expectedCase.forcedColors) {
      fail(`${expectedCase.name}: escala o forced-colors cambió.`);
    }
    if ((actualCase.zoom ?? 1) !== (expectedCase.zoom ?? 1)) fail(`${expectedCase.name}: zoom cambió.`);
    if (actualCase.valid !== true) fail(`${expectedCase.name}: contrato visual inválido.`);
    const inspection = actualCase.inspection ?? {};
    for (const landmark of baseline.contract.landmarks) {
      if (inspection.landmarks?.[landmark] !== 1) fail(`${expectedCase.name}: falta landmark ${landmark}.`);
    }
    if (inspection.minimumTargetSize < baseline.contract.minimumTargetSize) fail(`${expectedCase.name}: target menor al mínimo.`);
    if (baseline.contract.requireVisibleFocus && (inspection.focus?.outlineStyle === "none" || inspection.focus?.outlineWidth === "0px")) {
      fail(`${expectedCase.name}: foco no visible.`);
    }
    if (baseline.contract.requireNoHorizontalOverflow && inspection.noHorizontalOverflow !== true) fail(`${expectedCase.name}: overflow horizontal.`);
    const screenshotPath = resolve(projectRoot, actualCase.screenshot);
    const screenshotSha256 = createHash("sha256").update(await readFile(screenshotPath)).digest("hex");
    if (screenshotSha256 !== actualCase.screenshotSha256) fail(`${expectedCase.name}: hash del PNG no coincide con el sumario.`);
    const expectedHash = baseline.screenshots?.[expectedCase.name];
    if (!expectedHash) {
      if (!updateBaseline) fail(`El baseline no tiene hash para ${expectedCase.name}; ejecuta --update-baseline tras revisar las imágenes.`);
    } else if (expectedHash !== actualCase.screenshotSha256 && !updateBaseline) {
      fail(`${expectedCase.name}: diferencia visual detectada (${expectedHash} -> ${actualCase.screenshotSha256}).`);
    }
    checks.push({ name: expectedCase.name, status: "passed", screenshot: relativePath(screenshotPath), screenshotSha256: actualCase.screenshotSha256 });
  }
  if (updateBaseline) {
    baseline.approvedAt = new Date().toISOString();
    baseline.projectVersion = summary.projectVersion;
    baseline.fixturePath = summary.fixture.path;
    baseline.git = summary.git;
    baseline.lockfiles = summary.lockfiles;
    baseline.screenshots = Object.fromEntries(checks.map((check) => [check.name, check.screenshotSha256]));
    await writeFile(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`, "utf8");
  }
  status = "passed";
} catch (caught) {
  error = caught instanceof Error ? caught.message : String(caught);
}

await mkdir(dirname(checkOutput), { recursive: true });
await writeFile(checkOutput, `${JSON.stringify({
  schemaVersion: 1,
  status,
  generatedAt: new Date().toISOString(),
  baseline: relativePath(baselinePath),
  sourceSummary: summaryPath ? relativePath(summaryPath) : null,
  baselineUpdated: updateBaseline,
  checks,
  error,
}, null, 2)}\n`, "utf8");

if (status !== "passed") {
  console.error(`Baseline release falló: ${error ?? "error desconocido"}. Evidencia: ${relativePath(checkOutput)}`);
  process.exitCode = 1;
} else {
  console.log(`Baseline release aprobado: ${relativePath(checkOutput)}`);
}
