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
