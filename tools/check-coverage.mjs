import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const criticalFiles = {
  "/src/App.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/delivery/DeliveryPhase.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/prepare/PreparePhase.tsx": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/prepare/usePrepareController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/review/useReviewController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/delivery/useDeliveryController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
  "/src/features/projects/useProjectsController.ts": { statements: 80, lines: 80, branches: 75, functions: 75 },
  // T10-18: ratchet floors taken from the 2026-09-24 measurement (rounded down);
  // raise them as tests grow, never lower them.
  "/src/features/delivery/deliveryModel.ts": { statements: 78, lines: 79, branches: 79, functions: 95 },
  "/src/features/review/ReviewPhase.tsx": { statements: 87, lines: 89, branches: 81, functions: 91 },
  // QA-26: floors from the 2026-10-04 measurement.
  "/src/features/load/LoadPhase.tsx": { statements: 98, lines: 98, branches: 95, functions: 98 },
  "/src/features/explore/ExplorePhase.tsx": { statements: 89, lines: 91, branches: 87, functions: 89 },
  "/src/features/review/DatasetComparisonSection.tsx": { statements: 93, lines: 95, branches: 94, functions: 97 },
};
// QA-26: the whole front end, not only the files above (2026-10-04, rounded down).
const globalFloor = { statements: 88, lines: 92, branches: 83, functions: 89 };

/**
 * Problems of a coverage summary: older than the newest source file (a stale
 * `coverage/` folder), or below a global or per-file floor.
 */
export function coverageProblems(summary, summaryModifiedMs, newestSourceModifiedMs) {
  const failures = [];
  if (summaryModifiedMs < newestSourceModifiedMs) {
    failures.push("el resumen de cobertura es anterior a los fuentes; vuelve a ejecutar las pruebas con cobertura");
  }
  for (const [metric, threshold] of Object.entries(globalFloor)) {
    const actual = summary.total?.[metric]?.pct;
    if (typeof actual !== "number" || actual < threshold) {
      failures.push(`total ${metric} ${actual ?? "n/a"}% < ${threshold}%`);
    }
  }
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
  return failures;
}

async function newestModifiedMs(directory) {
  let newest = 0;
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      newest = Math.max(newest, await newestModifiedMs(path));
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      newest = Math.max(newest, (await stat(path)).mtimeMs);
    }
  }
  return newest;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    const summaryPath = "coverage/coverage-summary.json";
    const summary = JSON.parse(await readFile(summaryPath, "utf8"));
    const failures = coverageProblems(summary, (await stat(summaryPath)).mtimeMs, await newestModifiedMs("src"));
    if (failures.length) throw new Error(failures.join("; "));
    console.log(`Cobertura aprobada: total y ${Object.keys(criticalFiles).length} archivos con umbrales, sobre un resumen actual.`);
  } catch (error) {
    console.error(`Gate de cobertura crítica falló: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
