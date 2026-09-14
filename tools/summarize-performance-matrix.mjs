import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { summarizePerformanceMatrixSummaries } from "./performance-matrix-contract.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const evidenceRoot = resolve(process.argv[2] ?? join(projectRoot, ".local", "validation", "performance-benchmark"));
const outputPath = join(projectRoot, ".local", "validation", "performance-matrix", "summary.json");

function listJsonFiles(directory) {
  let entries;
  try {
    entries = readdirSync(directory, { withFileTypes: true });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
  return entries.flatMap((entry) => {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) return listJsonFiles(entryPath);
    return entry.isFile() && entry.name === "summary.json" ? [entryPath] : [];
  });
}

const summaries = [];
for (const summaryPath of listJsonFiles(evidenceRoot)) {
  try {
    summaries.push(JSON.parse(readFileSync(summaryPath, "utf8").replace(/^\uFEFF/, "")));
  } catch {
    // A malformed historical run cannot contribute a comparable profile.
  }
}
const comparison = summarizePerformanceMatrixSummaries(summaries);
const result = {
  ...comparison,
  generatedAt: new Date().toISOString(),
  evidenceRoot: relative(projectRoot, evidenceRoot).replaceAll("\\", "/"),
};

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");
console.log(`Matriz de escala ${result.status}: ${outputPath}`);
for (const comparisonGroup of result.comparisons) {
  const cases = comparisonGroup.cases.map((entry) => {
    const measure = entry.measures;
    return measure
      ? `${entry.profileId}=${entry.status} (${entry.dimensions.columnCount} cols, ${entry.dimensions.rowCount} filas, ${measure.peakWorkingSetBytes} B RAM, ${measure.peakSampledWorkspaceDiskBytes} B disco, ${measure.maxCommandDurationMs} ms máx.)`
      : `${entry.profileId}=${entry.status}`;
  });
  console.log(`${comparisonGroup.targetMiB} MiB: ${cases.join(" | ")}`);
}
