import { describe, expect, it } from "vitest";

import type { DatasetPreview, DatasetSourceInspection, ImportProfile } from "../../bridge";
import {
  createImportProfile,
  importProfileApplicability,
  parseImportProfileMismatch,
} from "./importProfile";

const source: DatasetSourceInspection = {
  selectionId: "ephemeral-123",
  fileName: "private-source.xlsx",
  fileSizeBytes: 128,
  format: "excel",
  sheets: [{ id: "selection-sheet-id", name: "Resumen" }],
  defaultSheetId: "selection-sheet-id",
  isCompressedContainer: true,
  resourceEstimate: {
    processingPath: "inMemory",
    estimatedMaterializationRamBytes: 256 * 1024 * 1024,
    estimatedTemporaryDiskBytes: 128,
  },
};

const dataset: DatasetPreview = {
  fileName: "private-source.xlsx",
  fileSizeBytes: 128,
  rowCount: 1,
  columnCount: 1,
  columns: [{ name: "importe", dataType: "String" }],
  rows: [["valor-privado"]],
};

describe("perfiles reutilizables de importación", () => {
  it("guarda solo las convenciones y el esquema, nunca rutas, ids temporales ni filas", () => {
    const profile = createImportProfile(
      source,
      dataset,
      { sheetId: "selection-sheet-id", headerMode: "firstRow" },
      { dateConvention: "dmy", numberConvention: "commaDecimalDotGrouping" },
    );

    expect(profile).toEqual({
      version: 1,
      format: "excel",
      sheetName: "Resumen",
      headerMode: "firstRow",
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
      schema: [{ name: "importe", dataType: "String" }],
    });
    const serialized = JSON.stringify(profile);
    expect(serialized).not.toMatch(/selectionId|selection-sheet-id|private-source|valor-privado|sourcePath|fileName/);
  });

  it("reutiliza una hoja por su nombre exacto y no cae a otra hoja cuando falta", () => {
    const profile: ImportProfile = {
      version: 1,
      format: "excel",
      sheetName: "Resumen",
      headerMode: "generated",
      schema: [{ name: "column_1", dataType: "String" }],
    };
    expect(importProfileApplicability(profile, source)).toEqual({
      kind: "applicable",
      sheetId: "selection-sheet-id",
      headerMode: "generated",
    });
    expect(importProfileApplicability(profile, { ...source, sheets: [{ id: "other", name: "Detalle" }] }))
      .toEqual({ kind: "sheet_missing" });
    expect(importProfileApplicability({ ...profile, version: 2 } as unknown as ImportProfile, source))
      .toEqual({ kind: "format_mismatch" });
  });

  it("solo interpreta errores estructurados del guard de esquema", () => {
    const mismatch = {
      code: "importProfileSchemaMismatch",
      missingColumns: ["periodo"],
      addedColumns: ["region"],
      changedTypes: [{ column: "importe", expected: "Int64", actual: "String" }],
    } as const;
    expect(parseImportProfileMismatch(new Error(`__columnia_import_profile_mismatch__:${JSON.stringify(mismatch)}`)))
      .toEqual(mismatch);
    expect(parseImportProfileMismatch(new Error("fallo general de lectura"))).toBeNull();
    expect(parseImportProfileMismatch(new Error("__columnia_import_profile_mismatch__:{\"code\":\"bad\"}")))
      .toBeNull();
  });
});
