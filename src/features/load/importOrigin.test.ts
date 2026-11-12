import { describe, expect, it } from "vitest";

import type { DatabaseImportResult, DatasetSourceInspection, ImportOrigin } from "../../bridge";
import { importOriginFromResult, importOriginNotice, importOriginSummaryItems } from "./importOrigin";

const result = (overrides: Partial<DatabaseImportResult>): DatabaseImportResult => ({
  inspection: {} as DatasetSourceInspection,
  rowsImported: 100_000,
  moreRowsAvailable: false,
  truncatedColumns: [],
  replacedColumns: [],
  ...overrides,
});

describe("origen de una importación (AUD-061)", () => {
  it("una importación completa y sin textos cambiados no deja nada que decir", () => {
    expect(importOriginFromResult(result({}), "dbo.Ventas")).toBeNull();
  });

  it("recuerda el límite solo si la tabla tenía más filas, y los textos recortados o sustituidos", () => {
    expect(importOriginFromResult(result({ moreRowsAvailable: true }), "dbo.Ventas")).toEqual({
      table: "dbo.Ventas", rowsImported: 100_000, partial: true, truncatedColumns: [], replacedColumns: [],
    });
    expect(importOriginFromResult(result({ truncatedColumns: ["Notas"], replacedColumns: ["Texto"] }), "Ventas")).toEqual({
      table: "Ventas", rowsImported: 100_000, partial: false, truncatedColumns: ["Notas"], replacedColumns: ["Texto"],
    });
  });

  it("escribe las líneas del resumen y el aviso breve de la barra", () => {
    const origin: ImportOrigin = {
      table: "dbo.Ventas", rowsImported: 100_000, partial: true, truncatedColumns: ["Notas", "Detalle"], replacedColumns: ["Texto"],
    };
    expect(importOriginSummaryItems(origin)).toEqual([
      { label: "Origen", value: `primeras ${(100_000).toLocaleString()} filas de dbo.Ventas` },
      { label: "Columnas con textos recortados", value: "Notas, Detalle" },
      { label: "Columnas con caracteres sustituidos", value: "Texto" },
    ]);
    expect(importOriginNotice(origin)?.text).toBe(`Parte de la tabla: primeras ${(100_000).toLocaleString()} filas de dbo.Ventas`);
    expect(importOriginNotice(origin)?.title).toContain("Columnas con textos recortados: Notas, Detalle");
    expect(importOriginNotice({ ...origin, partial: false })?.text).toBe("Textos modificados en la importación");
    expect(importOriginSummaryItems({ ...origin, partial: false })[0]?.label).toBe("Columnas con textos recortados");
  });

  it("sin origen o sin nada que avisar no hay líneas ni aviso", () => {
    expect(importOriginSummaryItems(null)).toEqual([]);
    expect(importOriginNotice(undefined)).toBeNull();
    expect(importOriginNotice({ table: "t", rowsImported: 1, partial: false, truncatedColumns: [], replacedColumns: [] })).toBeNull();
  });
});
