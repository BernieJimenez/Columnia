import { describe, expect, it } from "vitest";

import type { DatasetPreview, TransformRecipe } from "../../bridge";
import { buildTransformPreview, visibleColumnNames } from "./transformAdvisor";

const dataset: DatasetPreview = {
  fileName: "clientes.csv",
  fileSizeBytes: 128,
  rowCount: 100,
  columnCount: 3,
  columns: [
    { name: "nombre", dataType: "String" },
    { name: "total", dataType: "Int64" },
    { name: "ciudad", dataType: "String" },
  ],
  rows: [["Ana", "10", "Santo Domingo"], ["Luis", "20", "Santiago"]],
};

const emptyRecipe: TransformRecipe = {
  renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
  findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
  outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
};

describe("buildTransformPreview", () => {
  it("estima filtros sobre la muestra visible y marca la confianza", () => {
    const preview = buildTransformPreview(dataset, {
      ...emptyRecipe,
      filters: [{ column: "total", operator: "gte", value: "15" }],
    });

    expect(preview.afterRows).toBe(50);
    expect(preview.rowsDelta).toBe(-50);
    expect(preview.risk).toBe("high");
    expect(preview.confidenceLabel).toBe("baja");
    expect(preview.basis).toContain("2 filas visibles");
    expect(preview.recoveryOptions.some((option) => option.includes("Historial"))).toBe(true);
    expect(preview.recoveryOptions.some((option) => option.includes("parte"))).toBe(true);
  });

  it("calcula columnas añadidas y mantiene bajo riesgo para una transformación reversible", () => {
    const preview = buildTransformPreview(dataset, {
      ...emptyRecipe,
      renames: [{ from: "nombre", to: "cliente" }],
      calculatedColumn: {
        name: "total_doble",
        source: "total",
        operation: "multiply",
        operand: { kind: "literal", value: "2" },
      },
    });

    expect(preview.beforeColumns).toBe(3);
    expect(preview.afterColumns).toBe(4);
    expect(preview.columnsDelta).toBe(1);
    expect(preview.beforeColumnNames).toContain("nombre");
    expect(preview.afterColumnNames).toContain("cliente");
    expect(preview.afterColumnNames).toContain("total_doble");
    expect(preview.risk).toBe("low");
    expect(preview.afterRows).toBe(dataset.rowCount);
    expect(preview.confidence).toBe(96);
  });

  it("no promete filas cuando un resumen agrupado o outliers pueden cambiar la granularidad", () => {
    const preview = buildTransformPreview(dataset, {
      ...emptyRecipe,
      outlierTreatments: [{ column: "total", action: "drop" }],
    });

    expect(preview.afterRows).toBeNull();
    expect(preview.rowsDelta).toBeNull();
    expect(preview.risk).toBe("high");
    expect(preview.confidence).toBeLessThan(50);
  });
});

describe("visibleColumnNames", () => {
  it("resume listas largas sin romper el panel", () => {
    expect(visibleColumnNames(["a", "b", "c", "d", "e", "f", "g", "h", "i"])).toBe("a, b, c, d, e, f, g, h y 1 más");
  });
});

describe("estimación de filtros con fechas y coma decimal (FUN-23)", () => {
  const dated: DatasetPreview = {
    ...dataset,
    columns: [
      { name: "fecha", dataType: "Date" },
      { name: "importe", dataType: "String" },
      { name: "nota", dataType: "String" },
    ],
    rows: [["2023-12-31", "12,5", "a"], ["2024-02-01", "7,25", ""], ["2024-03-01", "30", "b"], ["2024-04-01", "1", "c"]],
  };

  it("compara fechas ISO y números con coma decimal", () => {
    expect(buildTransformPreview(dated, {
      ...emptyRecipe,
      filters: [{ column: "fecha", operator: "gte", value: "2024-01-01" }],
    }).afterRows).toBe(75);
    expect(buildTransformPreview(dated, {
      ...emptyRecipe,
      filters: [{ column: "importe", operator: "gt", value: "10" }],
    }).afterRows).toBe(50);
  });

  it("dice que no puede estimar cuando el valor no se puede comparar", () => {
    const preview = buildTransformPreview(dated, {
      ...emptyRecipe,
      filters: [{ column: "nota", operator: "gt", value: "b" }],
    });
    expect(preview.afterRows).toBeNull();
    expect(preview.basis).toContain("No se puede estimar");
  });

  it("no cuenta una celda vacía como distinta", () => {
    expect(buildTransformPreview(dated, {
      ...emptyRecipe,
      filters: [{ column: "nota", operator: "neq", value: "a" }],
    }).afterRows).toBe(50);
  });
});

describe("recetas que cambian valores (FUN-25)", () => {
  it("las conversiones y los reemplazos no se presentan como de bajo riesgo", () => {
    const cast = buildTransformPreview(dataset, {
      ...emptyRecipe,
      casts: [{ column: "total", target: "integer" }],
    });
    expect(cast.risk).toBe("medium");
    expect(cast.recommendations.join(" ")).toContain("la receta se detiene sin aplicar nada");
    const replace = buildTransformPreview(dataset, {
      ...emptyRecipe,
      findReplace: { scope: "all_text_columns", column: null, find: "a", replace: "b", regex: false },
    });
    expect(replace.risk).toBe("medium");
    expect(replace.recommendations.join(" ")).toContain("reescribe todas las celdas");
    expect(buildTransformPreview(dataset, emptyRecipe).risk).toBe("low");
  });
});
