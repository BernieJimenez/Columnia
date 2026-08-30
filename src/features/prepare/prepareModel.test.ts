import { describe, expect, it } from "vitest";

import type { TransformRecipe } from "../../bridge";
import {
  EMPTY_HISTORY,
  changeProgressMessage,
  isLoadedRecipe,
  requiresImpactConfirmation,
} from "./prepareModel";

const emptyRecipe: TransformRecipe = {
  renames: [],
  casts: [],
  dateParses: [],
  filters: [],
  calculatedColumn: null,
  findReplace: null,
  keepColumns: null,
  splitColumn: null,
  mergeColumns: null,
  outlierTreatments: [],
  groupSummary: null,
  contactNormalizations: [],
  textExtractions: [],
};

describe("modelo de preparación", () => {
  it("parte de un historial disponible pero vacío", () => {
    expect(EMPTY_HISTORY).toMatchObject({
      snapshotsEnabled: true,
      canUndo: false,
      canRedo: false,
      entryCount: 0,
    });
  });

  it("reconoce únicamente documentos de receta v1 completos", () => {
    expect(isLoadedRecipe({
      version: 1,
      name: "Limpieza",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
    })).toBe(true);
    expect(isLoadedRecipe({ version: 1, name: "Incompleta", savedAt: "ahora", recipe: {} })).toBe(false);
    expect(isLoadedRecipe(null)).toBe(false);
  });

  it("acepta metadatos estructurales de sesión y rechaza listas mal formadas", () => {
    const migrationReport = {
      artifactSha256: null,
      sourceFormat: "dataprep" as const,
      sourceVersion: 1,
      convertedItems: 1,
      omittedItems: 0,
      warningCount: 0,
      convertedOperations: ["renames"],
      omittedOperations: [],
      warnings: [],
      manualActions: [],
      session: {
        hasSourceReference: true,
        hasSnapshotReference: false,
        sheetName: "Datos",
        stageLabel: "Preparar",
        appliedOperationCount: 1,
        qualityRuleCount: 0,
        analysisCheckCount: 1,
        appliedOperations: ["rename_text"],
        analysisChecks: ["completeness"],
        analysisSampled: true,
        analysisSampleRowCount: 2,
        analysisTotalRowCount: 3,
        historySnapshotCount: 2,
        historyCursor: 1,
        nonPortableArtifacts: ["analysis_results", "caches"],
      },
    };

    expect(isLoadedRecipe({
      version: 1,
      name: "Sesión",
      savedAt: "2026-08-29T00:00:00Z",
      recipe: emptyRecipe,
      migrationReport,
    })).toBe(true);
    expect(isLoadedRecipe({
      version: 1,
      name: "Sesión inválida",
      savedAt: "2026-08-29T00:00:00Z",
      recipe: emptyRecipe,
      migrationReport: {
        ...migrationReport,
        session: { ...migrationReport.session, appliedOperations: ["rename_text", 1] },
      },
    })).toBe(false);
  });

  it.each([
    ["un conteo negativo", { appliedOperationCount: -1 }],
    ["un conteo fraccionario", { historyCursor: 0.5 }],
    ["un conteo no finito", { analysisTotalRowCount: Number.NaN }],
    ["una bandera mal formada", { analysisSampled: "yes" }],
    ["un artefacto mal formado", { nonPortableArtifacts: ["caches", 1] }],
  ])("rechaza %s en metadatos de sesión", (_name, sessionPatch) => {
    const migrationReport = {
      artifactSha256: null,
      sourceFormat: "dataprep" as const,
      sourceVersion: 1,
      convertedItems: 1,
      omittedItems: 0,
      warningCount: 0,
      convertedOperations: ["renames"],
      omittedOperations: [],
      warnings: [],
      manualActions: [],
      session: {
        hasSourceReference: true,
        hasSnapshotReference: false,
        sheetName: "Datos",
        stageLabel: "Preparar",
        appliedOperationCount: 1,
        qualityRuleCount: 0,
        analysisCheckCount: 0,
        ...sessionPatch,
      },
    };

    expect(isLoadedRecipe({
      version: 1,
      name: "Sesión inválida",
      savedAt: "2026-08-29T00:00:00Z",
      recipe: emptyRecipe,
      migrationReport,
    })).toBe(false);
  });

  it.each([
    ["filtros", { filters: [{ column: "nombre", operator: "eq", value: "Ana" }] }],
    ["eliminación de columnas", { keepColumns: ["nombre"] }],
    ["división destructiva", { splitColumn: { source: "nombre", delimiter: " ", names: ["a", "b"], dropSource: true } }],
    ["outliers", { outlierTreatments: [{ column: "total", action: "cap" }] }],
    ["resumen", { groupSummary: { groupBy: ["nombre"], aggregations: [{ column: "total", operation: "sum" }] } }],
    ["contactos", { contactNormalizations: [{ column: "nombre", kind: "email" }] }],
  ] satisfies Array<[string, Partial<TransformRecipe>]>)
  ("exige confirmación para %s", (_name, change) => {
    expect(requiresImpactConfirmation({ ...emptyRecipe, ...change })).toBe(true);
  });

  it("permite aplicar directamente una receta sin impacto destructivo", () => {
    expect(requiresImpactConfirmation({
      ...emptyRecipe,
      renames: [{ from: "nombre", to: "cliente" }],
    })).toBe(false);
  });

  it("mantiene mensajes explícitos para deshacer y rehacer", () => {
    expect(changeProgressMessage("undo")).toBe("Deshaciendo cambio…");
    expect(changeProgressMessage("redo")).toBe("Rehaciendo cambio…");
  });
});
