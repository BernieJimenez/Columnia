import { describe, expect, it } from "vitest";

import type { TransformRecipe } from "../../bridge";
import {
  EMPTY_HISTORY,
  appliedPlanChanges,
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

  it("reconoce documentos completos de receta v1 y v2, con o sin esquema legado", () => {
    expect(isLoadedRecipe({
      version: 1,
      name: "Limpieza",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
    })).toBe(true);
    expect(isLoadedRecipe({
      version: 2,
      name: "Receta con esquema",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
      sourceSchema: [{ name: "id", dataType: "Int64" }],
    })).toBe(true);
    expect(isLoadedRecipe({
      version: 2,
      name: "Receta automatizada",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
    })).toBe(true);
    expect(isLoadedRecipe({
      version: 2,
      name: "Esquema incompleto",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
      sourceSchema: [{ name: "id", dataType: "" }],
    })).toBe(false);
    expect(isLoadedRecipe({
      version: 1,
      name: "v1 con campo v2",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: emptyRecipe,
      sourceSchema: [{ name: "id", dataType: "Int64" }],
    })).toBe(false);
    expect(isLoadedRecipe({ version: 1, name: "Incompleta", savedAt: "ahora", recipe: {} })).toBe(false);
    expect(isLoadedRecipe(null)).toBe(false);
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

describe("appliedPlanChanges", () => {
  const options = { trimText: true, normalizeSentinels: true, normalizeColumnNames: false, removeDuplicates: true, imputeMissing: true };
  const none = { changedCellCount: 0, renamedColumnCount: 0, removedRowCount: 0, typedColumnCount: 0, imputedCellCount: 0 };

  it("lists what the proposal applied, one line per kind, in singular or plural", () => {
    expect(appliedPlanChanges(options, { ...none, changedCellCount: 58, removedRowCount: 5268, typedColumnCount: 2, imputedCellCount: 1 })).toEqual([
      `58 celdas limpiadas (espacios y marcadores «sin dato»)`,
      `${(5268).toLocaleString()} filas duplicadas quitadas`,
      "2 columnas convertidas a número",
      "1 valor vacío rellenado",
    ]);
  });

  it("names the cleaning that was selected and leaves out what changed nothing", () => {
    expect(appliedPlanChanges({ ...options, normalizeSentinels: false }, { ...none, changedCellCount: 1 })).toEqual([
      "1 celda con espacios recortados",
    ]);
    expect(appliedPlanChanges({ ...options, trimText: false }, { ...none, changedCellCount: 3 })).toEqual([
      "3 marcadores «sin dato» convertidos en vacíos",
    ]);
    expect(appliedPlanChanges(options, none)).toEqual([]);
  });
});
