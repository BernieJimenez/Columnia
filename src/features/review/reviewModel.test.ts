import { describe, expect, it } from "vitest";

import type { DatasetPreview } from "../../bridge";
import { createReadyDatasetStatus } from "../load/loadModel";
import {
  ANALYSIS_SAMPLE_ROW_OPTIONS,
  ANALYSIS_SAMPLE_ROWS_STORAGE_KEY,
  DEFAULT_ANALYSIS_SAMPLE_ROWS,
  PAGE_SIZE,
  QUERY_ENGINE_STORAGE_KEY,
  beginPageLoad,
  beginProfileAnalysis,
  completePageLoad,
  failPageLoad,
  isAnalysisSampleRows,
  isDatasetQueryEngine,
  nextPageOffset,
  normalizePageOffset,
  pageRange,
  previousPageOffset,
  readAnalysisSampleRowsPreference,
  readQueryEnginePreference,
  requestProfileCancellation,
  updateProfileProgress,
  writeAnalysisSampleRowsPreference,
  writeQueryEnginePreference,
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
  it("valida y conserva el motor SQL elegido con Polars como fallback", () => {
    const values = new Map<string, string>();
    const storage: Storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => { values.set(key, value); },
      removeItem: (key: string) => { values.delete(key); },
      clear: () => { values.clear(); },
      key: (index: number) => [...values.keys()][index] ?? null,
      get length() { return values.size; },
    };

    expect(isDatasetQueryEngine("polars")).toBe(true);
    expect(isDatasetQueryEngine("duckdb")).toBe(true);
    expect(isDatasetQueryEngine("sqlite")).toBe(false);
    expect(readQueryEnginePreference(storage)).toBe("polars");

    writeQueryEnginePreference("duckdb", storage);

    expect(values.get(QUERY_ENGINE_STORAGE_KEY)).toBe("duckdb");
    expect(readQueryEnginePreference(storage)).toBe("duckdb");
    values.set(QUERY_ENGINE_STORAGE_KEY, "sqlite");
    expect(readQueryEnginePreference(storage)).toBe("polars");
  });

  it("valida y conserva el límite de muestra de correlaciones", () => {
    const values = new Map<string, string>();
    const storage: Storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => { values.set(key, value); },
      removeItem: (key: string) => { values.delete(key); },
      clear: () => { values.clear(); },
      key: (index: number) => [...values.keys()][index] ?? null,
      get length() { return values.size; },
    };

    expect(ANALYSIS_SAMPLE_ROW_OPTIONS).toEqual([10_000, 50_000, 100_000]);
    expect(isAnalysisSampleRows(50_000)).toBe(true);
    expect(isAnalysisSampleRows(999)).toBe(false);
    expect(readAnalysisSampleRowsPreference(storage)).toBe(DEFAULT_ANALYSIS_SAMPLE_ROWS);

    writeAnalysisSampleRowsPreference(50_000, storage);

    expect(values.get(ANALYSIS_SAMPLE_ROWS_STORAGE_KEY)).toBe("50000");
    expect(readAnalysisSampleRowsPreference(storage)).toBe(50_000);
    values.set(ANALYSIS_SAMPLE_ROWS_STORAGE_KEY, "999");
    expect(readAnalysisSampleRowsPreference(storage)).toBe(DEFAULT_ANALYSIS_SAMPLE_ROWS);
  });

  it("mantiene la paginación contractual de 50 filas", () => {
    expect(PAGE_SIZE).toBe(50);
    expect(previousPageOffset(50)).toBe(0);
    expect(previousPageOffset(25)).toBe(0);
    expect(nextPageOffset(50)).toBe(100);
    expect(normalizePageOffset(75, 120)).toBe(50);
    expect(normalizePageOffset(999, 120)).toBe(100);
    expect(normalizePageOffset(-1, 120)).toBe(0);
    expect(normalizePageOffset(50, 50)).toBe(0);
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
