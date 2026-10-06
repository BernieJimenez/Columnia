import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import type { DatasetPreview, DatasetSourceInspection, ImportProfile } from "../../bridge";
import {
  LEGACY_ENCODING_PREFIX,
  beginDatasetLoad,
  beginDelimitedHeaderReview,
  beginSchemaPreview,
  completeSchemaPreview,
  failSchemaPreview,
  legacyEncodingExample,
  recoverDatasetLoadCancellationFailure,
  schemaMismatchInspection,
  completeDelimitedHeaderReview,
  createReadyDatasetStatus,
  delimitedHeaderInspection,
  needsResourcePreflight,
  requestDatasetLoadCancellation,
  restoreDatasetAfterLoadFailure,
  setLoadInspectionError,
  updateDatasetLoadProgress,
  updateSheetSelection,
  workbookInspection,
} from "./loadModel";

const dataset: DatasetPreview = {
  fileName: "anterior.csv",
  fileSizeBytes: 10,
  rowCount: 1,
  columnCount: 1,
  columns: [{ name: "id", dataType: "String" }],
  rows: [["1"]],
};

const workbook: DatasetSourceInspection = {
  selectionId: "opaque-selection",
  fileName: "libro.xlsx",
  fileSizeBytes: 2048,
  format: "excel",
  isCompressedContainer: true,
  defaultSheetId: "sheet-2",
  sheets: [
    { id: "sheet-1", name: "Enero" },
    { id: "sheet-2", name: "Febrero" },
  ],
  resourceEstimate: {
    processingPath: "inMemory",
    estimatedMaterializationRamBytes: 256 * 1024 * 1024 + 8192,
    estimatedTemporaryDiskBytes: 2048,
  },
};

describe("loadModel", () => {
  it("mantiene la carga pendiente hasta revisar ambas interpretaciones de encabezado", () => {
    const source: DatasetSourceInspection = {
      ...workbook,
      fileName: "ventas.csv",
      format: "csv",
      fileSizeBytes: 128,
      sheets: [],
      defaultSheetId: null,
      isCompressedContainer: false,
    };
    const pending = delimitedHeaderInspection(source);
    expect(pending).toMatchObject({
      kind: "sheet",
      selectedSheetId: "",
      headerMode: "firstRow",
      headerReviewLoading: true,
      headerReview: null,
    });
    if (pending.kind !== "sheet") throw new Error("Se esperaba una revisión delimitada.");

    const ready = completeDelimitedHeaderReview(pending, {
      delimiter: ";",
      firstRow: {
        headerMode: "firstRow",
        columns: [{ name: "id", dataType: "String" }],
        rows: [["1"]],
        includesFirstRow: false,
        sampleTruncated: false,
      },
      generated: {
        headerMode: "generated",
        columns: [{ name: "column_1", dataType: "String" }],
        rows: [["id"], ["1"]],
        includesFirstRow: true,
        sampleTruncated: false,
      },
    });
    const selected = updateSheetSelection(ready, {
      kind: "header_mode_changed",
      headerMode: "generated",
    });
    expect(selected).toMatchObject({
      kind: "sheet",
      headerMode: "generated",
      headerReviewLoading: false,
      headerReview: { delimiter: ";", generated: { includesFirstRow: true } },
    });
  });

  it("retiene y restaura el dataset activo si falla o se cancela un reemplazo", () => {
    const previous = createReadyDatasetStatus(dataset);
    const loading = beginDatasetLoad(previous);

    expect(loading).toMatchObject({ kind: "loading", previous });
    expect(restoreDatasetAfterLoadFailure(loading)).toBe(previous);
    expect(restoreDatasetAfterLoadFailure(beginDatasetLoad({ kind: "empty" }))).toEqual({ kind: "empty" });
  });

  it("actualiza progreso y solicitud de cancelación solo durante una carga", () => {
    const loading = beginDatasetLoad({ kind: "empty" });
    const progressed = updateDatasetLoadProgress(loading, {
      operation: "load",
      stage: "Leyendo filas",
      percent: 40,
    });

    expect(progressed).toMatchObject({
      kind: "loading",
      progress: { stage: "Leyendo filas", percent: 40 },
    });
    expect(requestDatasetLoadCancellation(progressed)).toMatchObject({
      kind: "loading",
      cancelRequested: true,
    });
    expect(requestDatasetLoadCancellation({ kind: "empty" })).toEqual({ kind: "empty" });
  });

  it("modela selección de hoja, encabezados y error sin perder el libro inspeccionado", () => {
    const selected = workbookInspection(workbook);
    expect(selected).toMatchObject({
      kind: "sheet",
      selectedSheetId: "sheet-2",
      headerMode: "firstRow",
    });

    const changed = updateSheetSelection(selected, {
      kind: "header_mode_changed",
      headerMode: "generated",
    });
    const failed = setLoadInspectionError(changed, "No se pudo leer la hoja");
    expect(failed).toMatchObject({
      kind: "sheet",
      source: workbook,
      headerMode: "generated",
      error: "No se pudo leer la hoja",
    });
  });

  it("pide revisar recursos antes de un archivo grande o una ruta source-backed", () => {
    expect(needsResourcePreflight(workbook)).toBe(true);
    expect(needsResourcePreflight({
      ...workbook,
      fileSizeBytes: 12,
      isCompressedContainer: false,
      resourceEstimate: { ...workbook.resourceEstimate, processingPath: "inMemory" },
    })).toBe(false);
    expect(needsResourcePreflight({
      ...workbook,
      fileSizeBytes: 12,
      isCompressedContainer: false,
      resourceEstimate: { ...workbook.resourceEstimate, processingPath: "sourceBacked" },
    })).toBe(true);
  });

  it("reutiliza solo la hoja exacta y el modo de encabezados guardado", () => {
    const profile: ImportProfile = {
      version: 1,
      format: "excel",
      sheetName: "Enero",
      headerMode: "generated",
      schema: [{ name: "column_1", dataType: "String" }],
    };
    const selected = workbookInspection(workbook, profile);
    expect(selected).toMatchObject({
      kind: "sheet",
      selectedSheetId: "sheet-1",
      headerMode: "generated",
      useSavedProfile: true,
      profileCanBeApplied: true,
    });

    const changed = updateSheetSelection(selected, { kind: "sheet_changed", sheetId: "sheet-2" });
    expect(changed).toMatchObject({ kind: "sheet", selectedSheetId: "sheet-2", useSavedProfile: false });
    const reused = updateSheetSelection(changed, { kind: "profile_toggled", useProfile: true });
    expect(reused).toMatchObject({ kind: "sheet", selectedSheetId: "sheet-1", headerMode: "generated", useSavedProfile: true });
  });

  it("no aplica convenciones guardadas a CSV source-backed", () => {
    const source: DatasetSourceInspection = {
      ...workbook,
      fileName: "ventas.csv",
      format: "csv",
      sheets: [],
      defaultSheetId: null,
      isCompressedContainer: false,
      resourceEstimate: { ...workbook.resourceEstimate, processingPath: "sourceBacked" },
    };
    const profile: ImportProfile = {
      version: 1,
      format: "csv",
      headerMode: "firstRow",
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
      schema: [
        { name: "fecha", dataType: "Date" },
        { name: "importe", dataType: "Float64" },
      ],
    };

    expect(delimitedHeaderInspection(source, profile)).toMatchObject({
      kind: "sheet",
      dateConvention: "unresolved",
      numberConvention: "unresolved",
      useSavedProfile: true,
    });

    const inMemorySource = {
      ...source,
      resourceEstimate: { ...source.resourceEstimate, processingPath: "inMemory" as const },
    };
    expect(delimitedHeaderInspection(inMemorySource, profile)).toMatchObject({
      kind: "sheet",
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
    });
  });

  it("no sustituye una hoja guardada que ya no existe por la hoja predeterminada", () => {
    const profile: ImportProfile = {
      version: 1,
      format: "excel",
      sheetName: "Marzo",
      headerMode: "generated",
      schema: [{ name: "column_1", dataType: "String" }],
    };
    expect(workbookInspection(workbook, profile)).toMatchObject({
      kind: "sheet",
      selectedSheetId: "sheet-2",
      headerMode: "firstRow",
      profileCanBeApplied: false,
      useSavedProfile: false,
      suggestedProfile: profile,
    });
  });
});

describe("modelo de Cargar sin cubrir (QA-38)", () => {
  it("comparte con Rust el prefijo de la codificación heredada", () => {
    const rust = readFileSync(resolve("src-tauri/src/dataset.rs"), "utf8");
    const declared = /const LEGACY_ENCODING_PREFIX: &str = "([^"]+)";/.exec(rust)?.[1];
    expect(declared).toBe(LEGACY_ENCODING_PREFIX);
    expect(legacyEncodingExample(`${LEGACY_ENCODING_PREFIX}Año`)).toBe("Año");
    expect(legacyEncodingExample("otro error")).toBeNull();
    expect(legacyEncodingExample(null)).toBeNull();
  });

  it("abre, completa y falla la vista previa de esquema solo con una hoja elegida", () => {
    const selected = workbookInspection(workbook);
    const loading = beginSchemaPreview(selected);
    expect(loading).toMatchObject({ schemaPreview: null, schemaPreviewLoading: true, schemaPreviewError: null });
    const preview = { rowCount: 1, columns: [], schemaMismatch: null };
    expect(completeSchemaPreview(loading, preview)).toMatchObject({ schemaPreview: preview, schemaPreviewLoading: false });
    expect(failSchemaPreview(loading, "sin acceso")).toMatchObject({ schemaPreview: null, schemaPreviewError: "sin acceso" });
    const idle = { kind: "idle" } as const;
    expect(beginSchemaPreview(idle)).toBe(idle);
    expect(completeSchemaPreview(idle, preview)).toBe(idle);
    expect(failSchemaPreview(idle, "x")).toBe(idle);
  });

  it("solo revisa encabezados de archivos delimitados", () => {
    const workbookSelection = workbookInspection(workbook);
    expect(beginDelimitedHeaderReview(workbookSelection)).toBe(workbookSelection);
    const delimited = workbookInspection({ ...workbook, format: "csv", fileName: "ventas.csv" });
    expect(beginDelimitedHeaderReview(delimited)).toMatchObject({ headerReview: null, headerReviewLoading: true, error: null });
  });

  it("guarda la discrepancia de esquema y el fallo al cancelar una carga", () => {
    const profile = { version: 1, format: "csv", schema: [] } as unknown as ImportProfile;
    const mismatch = { code: "importProfileSchemaMismatch" as const, missingColumns: ["id"], addedColumns: [], changedTypes: [] };
    expect(schemaMismatchInspection(workbook, profile, mismatch, "s1", "firstRow")).toEqual({
      kind: "schema_mismatch", source: workbook, profile, mismatch, sheetId: "s1", headerMode: "firstRow",
    });
    const loading = beginDatasetLoad({ kind: "ready", dataset } as never);
    expect(recoverDatasetLoadCancellationFailure(loading, "no respondió")).toMatchObject({ cancelRequested: false, cancellationError: "no respondió" });
    const idle = { kind: "idle" } as never;
    expect(recoverDatasetLoadCancellationFailure(idle, "x")).toBe(idle);
  });
});
