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
