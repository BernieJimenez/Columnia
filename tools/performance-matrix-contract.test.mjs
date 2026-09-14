import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  summarizePerformanceMatrixSummaries,
  validatePerformanceMatrixSummary,
} from "./performance-matrix-contract.mjs";

const definition = JSON.parse(readFileSync(new URL("../fixtures/performance/dataset-scale-matrix-v1.json", import.meta.url), "utf8"));

function createSummary(profile) {
  const rowCount = 120;
  const workingSets = [40_000_000, 48_000_000];
  const diskPeaks = [9_000_000, 13_000_000];
  const outputs = [{ sizeBytes: 700 }, { sizeBytes: 900 }];
  const commands = workingSets.map((peakWorkingSetBytes, index) => ({
    name: `transform-${index + 1}`,
    durationMs: 120 + index,
    peakWorkingSetBytes,
    peakSampledWorkspaceDiskBytes: diskPeaks[index],
  }));
  return {
    status: "passed",
    startedAt: "2026-09-14T12:00:00.000Z",
    evidenceDirectory: `.local/validation/performance-benchmark/${profile.id}`,
    targetMiB: 10,
    input: {
      rowCount,
      columnCount: profile.columnCount,
      sizeBytes: 10 * 1024 * 1024,
    },
    commands,
    outputs,
    cleanupConfirmed: true,
    scaleMatrix: {
      schemaVersion: 1,
      profileId: profile.id,
      dimensions: {
        targetMiB: 10,
        inputSizeBytes: 10 * 1024 * 1024,
        rowCount,
        columnCount: profile.columnCount,
        idDistinctCount: rowCount,
        nameDistinctCount: profile.nameCardinality === "row"
          ? rowCount
          : Math.min(rowCount, profile.nameCardinality),
        notesDistinctCount: 1,
        notesBytesPerRow: profile.notesBytesPerRow,
        generatedColumnCount: profile.columnCount - 4,
      },
      measures: {
        peakWorkingSetBytes: Math.max(...workingSets),
        peakSampledWorkspaceDiskBytes: Math.max(...diskPeaks),
        workspaceDiskMeasurement: "workspace file sizes sampled while each CLI command runs",
        recordedOutputBytes: outputs.reduce((total, output) => total + output.sizeBytes, 0),
        commandCount: commands.length,
        maxCommandDurationMs: Math.max(...commands.map((command) => command.durationMs)),
        totalCommandDurationMs: commands.reduce((total, command) => total + command.durationMs, 0),
        cancellation: "not-measured-by-cli-benchmark",
        cleanupConfirmed: true,
      },
    },
  };
}

test("acepta todos los perfiles con dimensiones y medidas consistentes", () => {
  for (const profile of definition.profiles) {
    const result = validatePerformanceMatrixSummary(createSummary(profile), definition);
    assert.equal(result.status, "passed", `${profile.id}: ${result.errors.join("; ")}`);
  }
});

test("rechaza un ancho declarado distinto al ancho observado", () => {
  const summary = createSummary(definition.profiles.find((profile) => profile.id === "wide"));
  summary.input.columnCount = 4;
  const result = validatePerformanceMatrixSummary(summary, definition);
  assert.equal(result.status, "failed");
  assert.ok(result.errors.some((error) => error.includes("ancho")));
});

test("rechaza cardinalidad, texto o medidas faltantes", () => {
  const summary = createSummary(definition.profiles.find((profile) => profile.id === "low-cardinality"));
  summary.scaleMatrix.dimensions.nameDistinctCount = 17;
  summary.scaleMatrix.dimensions.notesBytesPerRow = 77;
  summary.commands[0].peakSampledWorkspaceDiskBytes = undefined;
  const result = validatePerformanceMatrixSummary(summary, definition);
  assert.equal(result.status, "failed");
  assert.ok(result.errors.some((error) => error.includes("cardinalidad")));
  assert.ok(result.errors.some((error) => error.includes("longitud")));
  assert.ok(result.errors.some((error) => error.includes("disco temporal")));
});

test("exige declarar explícitamente límites de medición y limpieza", () => {
  const summary = createSummary(definition.profiles.find((profile) => profile.id === "standard"));
  summary.scaleMatrix.measures.cancellation = "passed";
  summary.scaleMatrix.measures.cleanupConfirmed = false;
  const result = validatePerformanceMatrixSummary(summary, definition);
  assert.equal(result.status, "failed");
  assert.ok(result.errors.some((error) => error.includes("cancelación")));
  assert.ok(result.errors.some((error) => error.includes("limpieza")));
});

test("compara perfiles del mismo tamaño y deja visibles los perfiles pendientes", () => {
  const summaries = definition.profiles.slice(0, 2).map(createSummary);
  const result = summarizePerformanceMatrixSummaries(summaries, definition);
  assert.equal(result.status, "incomplete");
  assert.equal(result.comparisons.length, 1);
  assert.equal(result.comparisons[0].targetMiB, 10);
  assert.deepEqual(result.comparisons[0].cases.map((entry) => entry.status), [
    "passed",
    "passed",
    "not-run",
    "not-run",
  ]);
});

test("selecciona la evidencia más reciente de cada perfil y rechaza la corrida fallida", () => {
  const standard = definition.profiles.find((profile) => profile.id === "standard");
  const earlier = createSummary(standard);
  const later = createSummary(standard);
  earlier.startedAt = "2026-09-13T12:00:00.000Z";
  later.startedAt = "2026-09-14T12:00:00.000Z";
  later.status = "failed";
  later.evidenceDirectory = ".local/validation/performance-benchmark/latest-failed";
  const result = summarizePerformanceMatrixSummaries([earlier, later], definition);
  const standardCase = result.comparisons[0].cases[0];
  assert.equal(standardCase.status, "failed");
  assert.equal(standardCase.evidenceDirectory, ".local/validation/performance-benchmark/latest-failed");
  assert.equal(result.status, "failed");
});
