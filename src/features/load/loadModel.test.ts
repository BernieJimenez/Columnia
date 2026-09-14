import { describe, expect, it } from "vitest";

import type { DatasetPreview, DatasetSourceInspection, ImportProfile } from "../../bridge";
import {
  beginDatasetLoad,
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
