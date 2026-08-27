import { describe, expect, it } from "vitest";

import type { DatasetPreview } from "../../bridge";
import { createReadyDatasetStatus } from "../load/loadModel";
import {
  PAGE_SIZE,
  beginPageLoad,
  beginProfileAnalysis,
  completePageLoad,
  failPageLoad,
  nextPageOffset,
  pageRange,
  previousPageOffset,
  requestProfileCancellation,
  updateProfileProgress,
} from "./reviewModel";

const dataset: DatasetPreview = {
  fileName: "datos.csv",
  fileSizeBytes: 100,
  rowCount: 120,
  columnCount: 1,
  columns: [{ name: "id", dataType: "String" }],
  rows: [["1"], ["2"]],
};

describe("reviewModel", () => {
  it("mantiene la paginación contractual de 50 filas", () => {
    expect(PAGE_SIZE).toBe(50);
    expect(previousPageOffset(50)).toBe(0);
    expect(previousPageOffset(25)).toBe(0);
    expect(nextPageOffset(50)).toBe(100);
    expect(pageRange(dataset, 50)).toEqual({ end: 52, hasPrevious: true, hasNext: true });
  });

  it("publica una página completa o recupera la anterior con un error", () => {
    const previous = { ...createReadyDatasetStatus(dataset), pageOffset: 50 };
    expect(beginPageLoad(previous)).toMatchObject({ pageLoading: true, pageError: undefined });
    expect(completePageLoad(previous, { offset: 100, rows: [["101"]] })).toMatchObject({
      pageOffset: 100,
      pageLoading: false,
      dataset: { rows: [["101"]] },
    });
    expect(failPageLoad(previous, "falló")).toMatchObject({
      pageOffset: 50,
      pageLoading: false,
      pageError: "falló",
      dataset,
    });
  });

  it("actualiza y cancela únicamente un perfil activo", () => {
    const loading = beginProfileAnalysis();
    const progressed = updateProfileProgress(loading, {
      operation: "profile",
      stage: "Calculando métricas",
      percent: 70,
    });
    expect(progressed).toMatchObject({ kind: "loading", progress: { percent: 70 } });
    expect(
      updateProfileProgress(progressed, {
        operation: "profile",
        stage: "Contando valores únicos",
        percent: 25,
      }),
    ).toBe(progressed);
    expect(requestProfileCancellation(progressed)).toMatchObject({
      kind: "loading",
      cancelRequested: true,
    });
    expect(requestProfileCancellation({ kind: "idle" })).toEqual({ kind: "idle" });
  });
});
