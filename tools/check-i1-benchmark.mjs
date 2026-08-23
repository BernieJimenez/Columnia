import { readdir, readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const evidenceRoot = join(projectRoot, ".local", "validation", "i1-benchmark");
const baselinePath = join(projectRoot, "fixtures", "performance", "i1-benchmark-baseline-v1.json");

function fail(message) {
  throw new Error(message);
}

async function latestSummary() {
  const entries = await readdir(evidenceRoot, { withFileTypes: true });
  const candidates = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const summaryPath = join(evidenceRoot, entry.name, "summary.json");
    try {
      candidates.push({ summaryPath, mtimeMs: (await stat(summaryPath)).mtimeMs });
    } catch {
      // Ignore incomplete evidence directories.
    }
  }
  candidates.sort((left, right) => right.mtimeMs - left.mtimeMs);
  if (candidates.length === 0) fail("No existe evidencia del benchmark I1.");
  return candidates[0].summaryPath;
}

const parseJson = (contents) => JSON.parse(contents.replace(/^\uFEFF/, ""));
const baseline = parseJson(await readFile(baselinePath, "utf8"));
const summary = parseJson(await readFile(await latestSummary(), "utf8"));
if (summary.schemaVersion !== baseline.schemaVersion) fail("schemaVersion I1 no soportada.");
if (summary.status !== "passed") fail(`El benchmark I1 no está aprobado: ${summary.error ?? "error desconocido"}.`);
if (summary.cleanupConfirmed !== true) fail("El benchmark I1 no confirmó cleanup.");
if (summary.input?.sizeBytes < baseline.minInputBytes) fail("El input I1 está por debajo del tamaño mínimo.");
if (summary.input?.rowCount <= 0 || summary.input?.columnCount !== baseline.requiredColumnCount) {
  fail("Las dimensiones del input I1 no cumplen el contrato.");
}

const runs = Array.isArray(summary.runs) ? summary.runs : [];
for (const engine of baseline.requiredEngines) {
  const run = runs.find((candidate) => candidate?.engine === engine);
  if (!run || run.status !== "passed") fail(`Falta una ejecución aprobada para ${engine}.`);
  if (!(run.durationMs > 0) || !(run.peakWorkingSetBytes > 0)) {
    fail(`${engine} no reportó tiempo y RAM positivos.`);
  }
}
if (typeof summary.comparison?.durationRatioColumniaOverDataprep !== "number" ||
    typeof summary.comparison?.peakWorkingSetRatioColumniaOverDataprep !== "number") {
  fail("La comparación I1 no contiene ratios numéricos.");
}

console.log(`Benchmark I1 aprobado: ${summary.input.sizeBytes} bytes, ${summary.input.rowCount} filas, ${runs.length} motores.`);
