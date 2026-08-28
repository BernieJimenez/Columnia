import { readFile } from "node:fs/promises";

const criticalFiles = {
  "/src/App.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/delivery/DeliveryPhase.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/prepare/PreparePhase.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/prepare/usePrepareController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/projects/useProjectsController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
};

function fail(message) {
  throw new Error(message);
}

try {
  const summary = JSON.parse(await readFile("coverage/coverage-summary.json", "utf8"));
  const failures = [];
  for (const [suffix, thresholds] of Object.entries(criticalFiles)) {
    const key = Object.keys(summary).find((candidate) => candidate.replaceAll("\\", "/").endsWith(suffix));
    if (!key) {
      failures.push(`${suffix}: no aparece en coverage-summary.json`);
      continue;
    }
    for (const [metric, threshold] of Object.entries(thresholds)) {
      const actual = summary[key][metric]?.pct;
      if (typeof actual !== "number" || actual < threshold) {
        failures.push(`${suffix} ${metric} ${actual ?? "n/a"}% < ${threshold}%`);
      }
    }
  }
  if (failures.length) fail(failures.join("; "));
  console.log(`Cobertura crítica aprobada: ${Object.keys(criticalFiles).length} archivos con umbrales por capa.`);
} catch (error) {
  console.error(`Gate de cobertura crítica falló: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
