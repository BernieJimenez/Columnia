import { describe, expect, it } from "vitest";

import type { DatasetPreview, DatasetSourceInspection } from "../../bridge";
import {
  beginDatasetLoad,
  createReadyDatasetStatus,
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
};

describe("loadModel", () => {
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
});
