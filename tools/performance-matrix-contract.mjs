import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const matrixPath = new URL("../fixtures/performance/dataset-scale-matrix-v1.json", import.meta.url);
const matrixDefinition = JSON.parse(readFileSync(matrixPath, "utf8"));

function isPositiveInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function maximum(values) {
  return values.length === 0 ? null : Math.max(...values);
}

export function validatePerformanceMatrixSummary(summary, definition = matrixDefinition) {
  const errors = [];
  const matrix = summary?.scaleMatrix;
  const profile = definition.profiles?.find((candidate) => candidate.id === matrix?.profileId);

  if (definition.schemaVersion !== 1 || definition.sizeComparable !== true || !profile) {
    errors.push("El perfil sintético no pertenece a la matriz versionada.");
  }
  if (!matrix || matrix.schemaVersion !== 1) {
    errors.push("Falta el contrato versionado de escala.");
    return { status: "failed", profileId: matrix?.profileId ?? null, errors };
  }

  const dimensions = matrix.dimensions ?? {};
  const measures = matrix.measures ?? {};
  const commands = Array.isArray(summary.commands) ? summary.commands : [];
  const outputs = Array.isArray(summary.outputs) ? summary.outputs : [];
  const expectedNameDistinctCount = profile && profile.nameCardinality === "row"
    ? dimensions.rowCount
    : Math.min(dimensions.rowCount ?? 0, profile?.nameCardinality ?? 0);
  const commandWorkingSet = commands.map((command) => command.peakWorkingSetBytes);
  const commandDiskPeaks = commands.map((command) => command.peakSampledWorkspaceDiskBytes);
  const commandDurations = commands.map((command) => command.durationMs);
  const recordedOutputBytes = outputs.reduce((total, output) => total + (output.sizeBytes ?? 0), 0);
  const expectedWorkingSet = maximum(commandWorkingSet);
  const expectedDiskPeak = maximum(commandDiskPeaks);
  const expectedMaxDuration = maximum(commandDurations);
  const expectedTotalDuration = commandDurations.reduce((total, duration) => total + (duration ?? 0), 0);

  if (!profile) {
    // The profile error above is sufficient; avoid dereferencing an unknown profile.
    return { status: "failed", profileId: matrix.profileId ?? null, errors };
  }
  if (dimensions.columnCount !== profile.columnCount || summary.input?.columnCount !== profile.columnCount) {
    errors.push("El ancho medido no coincide con el perfil declarado.");
  }
  if (!isPositiveInteger(dimensions.rowCount) || dimensions.rowCount !== summary.input?.rowCount) {
    errors.push("El conteo de filas no coincide con la entrada medida.");
  }
  if (!isPositiveInteger(dimensions.inputSizeBytes) || dimensions.inputSizeBytes !== summary.input?.sizeBytes) {
    errors.push("El tamaño medido no coincide con la entrada.");
  }
  if (dimensions.inputSizeBytes < dimensions.targetMiB * 1024 * 1024) {
    errors.push("La entrada quedó por debajo de su tamaño objetivo comparable.");
  }
  if (dimensions.nameDistinctCount !== expectedNameDistinctCount) {
    errors.push("La cardinalidad de nombre no coincide con el perfil.");
  }
  if (dimensions.idDistinctCount !== dimensions.rowCount) {
    errors.push("La cardinalidad del identificador debe coincidir con las filas.");
  }
  if (dimensions.notesDistinctCount !== 1 || dimensions.notesBytesPerRow !== profile.notesBytesPerRow) {
    errors.push("La longitud del texto no coincide con el perfil.");
  }
  if (dimensions.generatedColumnCount !== profile.columnCount - 4) {
    errors.push("El conteo de columnas sintéticas adicionales es incorrecto.");
  }
  if (!isPositiveInteger(summary.targetMiB) || dimensions.targetMiB !== summary.targetMiB) {
    errors.push("Falta el tamaño objetivo comparable.");
  }
  if (commands.length === 0 || commandWorkingSet.some((value) => !isPositiveInteger(value))) {
    errors.push("Faltan muestras de memoria de los comandos.");
  }
  if (commandDiskPeaks.some((value) => !isPositiveInteger(value))) {
    errors.push("Faltan muestras del pico de disco temporal.");
  }
  if (commandDurations.some((value) => !Number.isFinite(value) || value < 0)) {
    errors.push("Faltan duraciones válidas por comando.");
  }
  if (!isPositiveInteger(measures.peakWorkingSetBytes) || measures.peakWorkingSetBytes !== expectedWorkingSet) {
    errors.push("El máximo de RAM no coincide con las muestras por comando.");
  }
  if (!isPositiveInteger(measures.peakSampledWorkspaceDiskBytes) || measures.peakSampledWorkspaceDiskBytes !== expectedDiskPeak) {
    errors.push("El máximo de disco temporal no coincide con las muestras por comando.");
  }
  if (measures.workspaceDiskMeasurement !== "workspace file sizes sampled while each CLI command runs") {
    errors.push("Falta indicar que el pico de disco corresponde a muestras del workspace.");
  }
  if (!isPositiveInteger(measures.commandCount) || measures.commandCount !== commands.length) {
    errors.push("El número de comandos medidos no coincide.");
  }
  if (!Number.isFinite(measures.maxCommandDurationMs) || measures.maxCommandDurationMs !== expectedMaxDuration ||
      !Number.isFinite(measures.totalCommandDurationMs) || measures.totalCommandDurationMs !== expectedTotalDuration) {
    errors.push("Los resúmenes de duración no coinciden con las muestras por comando.");
  }
  if (!isPositiveInteger(measures.recordedOutputBytes) || measures.recordedOutputBytes !== recordedOutputBytes) {
    errors.push("El tamaño total de salidas no coincide con las salidas medidas.");
  }
  if (measures.cancellation !== "not-measured-by-cli-benchmark") {
    errors.push("La evidencia debe declarar que este benchmark no mide cancelación.");
  }
  if (measures.cleanupConfirmed !== true || summary.cleanupConfirmed !== true) {
    errors.push("La evidencia no confirmó la limpieza del workspace.");
  }

  return {
    status: errors.length === 0 ? "passed" : "failed",
    profileId: matrix.profileId,
    errors,
    observed: {
      targetMiB: dimensions.targetMiB,
      columnCount: dimensions.columnCount,
      rowCount: dimensions.rowCount,
      nameDistinctCount: dimensions.nameDistinctCount,
      notesBytesPerRow: dimensions.notesBytesPerRow,
      peakWorkingSetBytes: measures.peakWorkingSetBytes,
      peakSampledWorkspaceDiskBytes: measures.peakSampledWorkspaceDiskBytes,
      recordedOutputBytes: measures.recordedOutputBytes,
      commandCount: measures.commandCount,
      maxCommandDurationMs: measures.maxCommandDurationMs,
      totalCommandDurationMs: measures.totalCommandDurationMs,
      cancellation: measures.cancellation,
      cleanupConfirmed: measures.cleanupConfirmed,
    },
  };
}

export function summarizePerformanceMatrixSummaries(summaries, definition = matrixDefinition) {
  const groups = new Map();
  for (const summary of summaries) {
    const dimensions = summary?.scaleMatrix?.dimensions;
    const targetMiB = dimensions?.targetMiB;
    const profileId = summary?.scaleMatrix?.profileId;
    if (!isPositiveInteger(targetMiB) || !definition.profiles?.some((profile) => profile.id === profileId)) continue;

    if (!groups.has(targetMiB)) groups.set(targetMiB, new Map());
    const profileRuns = groups.get(targetMiB);
    const validation = validatePerformanceMatrixSummary(summary, definition);
    const candidate = {
      profileId,
      status: summary.status === "passed" && validation.status === "passed" ? "passed" : "failed",
      startedAt: summary.startedAt ?? null,
      evidenceDirectory: summary.evidenceDirectory ?? null,
      dimensions,
      measures: summary.scaleMatrix.measures ?? null,
      validationErrors: validation.errors,
    };
    const current = profileRuns.get(profileId);
    const candidateTime = Date.parse(candidate.startedAt ?? "") || 0;
    const currentTime = Date.parse(current?.startedAt ?? "") || 0;
    if (!current || candidateTime >= currentTime) profileRuns.set(profileId, candidate);
  }

  const comparisons = [...groups.entries()]
    .sort(([left], [right]) => left - right)
    .map(([targetMiB, profileRuns]) => {
      const cases = definition.profiles.map((profile) => profileRuns.get(profile.id) ?? {
        profileId: profile.id,
        status: "not-run",
        startedAt: null,
        evidenceDirectory: null,
        dimensions: null,
        measures: null,
        validationErrors: [],
      });
      const status = cases.some((entry) => entry.status === "failed")
        ? "failed"
        : cases.every((entry) => entry.status === "passed") ? "passed" : "incomplete";
      return { targetMiB, status, cases };
    });
  const status = comparisons.length > 0 && comparisons.every((comparison) => comparison.status === "passed")
    ? "passed"
    : comparisons.some((comparison) => comparison.status === "failed") ? "failed" : "incomplete";

  return {
    schemaVersion: 1,
    status,
    profiles: definition.profiles.map(({ id, columnCount, nameCardinality, notesBytesPerRow }) => ({
      id,
      columnCount,
      nameCardinality,
      notesBytesPerRow,
    })),
    comparisons,
  };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const summaryPath = process.argv[2];
  if (!summaryPath) {
    console.error("Uso: node tools/performance-matrix-contract.mjs <summary.json>");
    process.exitCode = 2;
  } else {
    try {
      const summary = JSON.parse(readFileSync(summaryPath, "utf8").replace(/^\uFEFF/, ""));
      const result = validatePerformanceMatrixSummary(summary);
      console.log(JSON.stringify(result));
      if (result.status !== "passed") process.exitCode = 1;
    } catch (error) {
      console.log(JSON.stringify({
        status: "failed",
        profileId: null,
        errors: [`No se pudo leer la evidencia de escala: ${error.message}`],
      }));
      process.exitCode = 1;
    }
  }
}
