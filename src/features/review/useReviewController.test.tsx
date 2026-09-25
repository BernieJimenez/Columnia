import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetComparison, DatasetPreview, DatasetProfile } from "../../bridge";
import { useReviewController } from "./useReviewController";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

const dataset: DatasetPreview = {
  fileName: "ventas.csv",
  fileSizeBytes: 10,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "id", dataType: "i64" },
    { name: "nombre", dataType: "str" },
  ],
  rows: [["1", "Ana"], ["2", "Luis"]],
};

const profile = { rowCount: 2, columns: [] } as unknown as DatasetProfile;

const comparison = {
  canConsolidate: true,
  conflicts: [],
  conflictOffset: 0,
  conflictsTruncated: false,
} as unknown as DatasetComparison;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function setup({
  datasetReady = false,
  onDatasetReplaced = vi.fn(async () => undefined),
}: {
  datasetReady?: boolean;
  onDatasetReplaced?: (next: DatasetPreview, mutation: string) => Promise<void>;
} = {}) {
  const datasetRevisionRef = { current: 1 };
  const hook = renderHook(
    ({ revision }: { revision: number }) => useReviewController({
      datasetRevisionRef,
      datasetRevision: revision,
      datasetReady,
      onDatasetReplaced,
    }),
    { initialProps: { revision: 1 } },
  );
  return { ...hook, datasetRevisionRef, onDatasetReplaced };
}

describe("useReviewController", () => {
  it("analyzes each ready revision once and drops a stale profile", async () => {
    const pending = deferred<DatasetProfile>();
    const getProfile = vi.spyOn(bridge, "getDatasetProfile").mockReturnValue(pending.promise);
    const { result, datasetRevisionRef, rerender } = setup({ datasetReady: true });

    await waitFor(() => expect(result.current.profileStatus.kind).toBe("loading"));
    expect(getProfile).toHaveBeenCalledTimes(1);

    datasetRevisionRef.current = 2;
    await act(async () => pending.resolve(profile));
    expect(result.current.profileStatus.kind).toBe("loading");

    getProfile.mockResolvedValue(profile);
    act(() => result.current.invalidateProfile());
    rerender({ revision: 2 });
    await waitFor(() => expect(result.current.profileStatus).toEqual({ kind: "ready", profile }));
    expect(getProfile).toHaveBeenCalledTimes(2);
  });

  it("reports a cancelled or failed analysis and recovers a failed cancellation", async () => {
    const pending = deferred<DatasetProfile>();
    vi.spyOn(bridge, "getDatasetProfile").mockReturnValue(pending.promise);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockRejectedValueOnce(new Error("sin respuesta"));
    const { result } = setup();

    act(() => void result.current.analyzeQuality());
    await act(async () => result.current.cancelProfile());
    expect(cancel).toHaveBeenCalledWith("profile");
    expect(result.current.profileStatus).toMatchObject({
      kind: "loading",
      cancelRequested: false,
      cancellationError: "sin respuesta",
    });

    await act(async () => pending.reject(new Error("Operación cancelada por el usuario")));
    expect(result.current.profileStatus).toEqual({ kind: "cancelled" });

    vi.spyOn(bridge, "getDatasetProfile").mockRejectedValueOnce(new Error("disco lleno"));
    await act(async () => result.current.analyzeQuality());
    expect(result.current.profileStatus).toEqual({ kind: "error", message: "disco lleno" });
  });

  it("compares, pages conflicts, keeps the previous result on cancel and clears", async () => {
    const compare = vi.spyOn(bridge, "compareDataset").mockResolvedValueOnce(comparison);
    vi.spyOn(bridge, "getDatasetConflictPage").mockResolvedValueOnce({
      conflicts: [],
      offset: 50,
      hasNext: true,
    } as unknown as Awaited<ReturnType<typeof bridge.getDatasetConflictPage>>);
    vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    const clear = vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    const { result } = setup();

    act(() => result.current.comparison.onKeyColumnsChange(["id"]));
    await act(async () => result.current.comparison.onCompare());
    await waitFor(() => expect(result.current.comparison.status.kind).toBe("ready"));
    expect(compare).toHaveBeenCalledWith(["id"]);

    await act(async () => result.current.comparison.onConflictPageChange(50));
    expect(result.current.comparison.status).toMatchObject({
      kind: "ready",
      comparison: { conflictOffset: 50, conflictsTruncated: true },
    });

    compare.mockRejectedValueOnce(new Error("Comparación cancelada por el usuario"));
    await act(async () => result.current.comparison.onCompare());
    await waitFor(() => expect(result.current.comparison.status.kind).toBe("ready"));

    compare.mockRejectedValueOnce(new Error("formato distinto"));
    await act(async () => result.current.comparison.onCompare());
    await waitFor(() => expect(result.current.comparison.status).toEqual({ kind: "error", message: "formato distinto" }));

    await act(async () => result.current.comparison.onClear());
    await waitFor(() => expect(result.current.comparison.status).toEqual({ kind: "idle" }));
    expect(clear).toHaveBeenCalledTimes(1);
  });

  it("reports empty comparisons, failed pages, failed clears and empty joins", async () => {
    const compare = vi.spyOn(bridge, "compareDataset").mockResolvedValueOnce(null);
    vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    vi.spyOn(bridge, "clearDatasetComparison").mockRejectedValueOnce(new Error("snapshot ocupado"));
    vi.spyOn(bridge, "getDatasetConflictPage").mockRejectedValueOnce(new Error("página perdida"));
    vi.spyOn(bridge, "joinDataset").mockResolvedValueOnce(null);
    const { result, onDatasetReplaced } = setup();

    await act(async () => result.current.comparison.onCompare());
    expect(result.current.comparison.status).toEqual({ kind: "idle" });

    compare.mockResolvedValueOnce(comparison);
    await act(async () => result.current.comparison.onCompare());
    await waitFor(() => expect(result.current.comparison.status.kind).toBe("ready"));
    await act(async () => result.current.comparison.onConflictPageChange(50));
    expect(result.current.comparison.status).toEqual({ kind: "error", message: "página perdida" });

    await act(async () => result.current.comparison.onClear());
    await waitFor(() => expect(result.current.comparison.status).toEqual({ kind: "error", message: "snapshot ocupado" }));

    act(() => result.current.comparison.onKeyColumnsChange(["id"]));
    await act(async () => result.current.comparison.onJoin("inner"));
    expect(result.current.comparison.joinStatus).toEqual({ kind: "idle" });
    expect(onDatasetReplaced).not.toHaveBeenCalled();
  });

  it("requires key columns to join and hands the joined dataset to the owner", async () => {
    vi.spyOn(bridge, "joinDataset").mockResolvedValueOnce(dataset);
    const { result, onDatasetReplaced } = setup();

    act(() => result.current.comparison.onJoin("left"));
    expect(result.current.comparison.joinStatus).toMatchObject({ kind: "error" });

    act(() => {
      result.current.comparison.onKeyColumnsChange(["id"]);
      result.current.comparison.onJoinTypeChange("left");
      result.current.setReviewTab("preview");
      result.current.setSqlHistory([{ id: "q", sql: "SELECT 1" } as never]);
    });
    await act(async () => result.current.comparison.onJoin("left"));

    expect(onDatasetReplaced).toHaveBeenCalledWith(dataset, "join");
    expect(result.current.comparison.keyColumns).toEqual([]);
    expect(result.current.comparison.joinType).toBe("left");
    expect(result.current.reviewTab).toBe("diagnosis");
    expect(result.current.sqlHistory).toEqual([]);
    expect(result.current.comparison.mutationStatus).toEqual({ kind: "idle" });
  });

  it("consolidates, reports mutation errors and requests cancellation", async () => {
    vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "adoptConsolidatedDataset").mockResolvedValueOnce(dataset);
    const { result, onDatasetReplaced } = setup();

    await act(async () => result.current.comparison.onCompare());
    await waitFor(() => expect(result.current.comparison.status.kind).toBe("ready"));
    act(() => result.current.comparison.onJoinTypeChange("full"));
    await act(async () => result.current.comparison.onConsolidate());
    expect(onDatasetReplaced).toHaveBeenCalledWith(dataset, "consolidate");
    expect(result.current.comparison.joinType).toBe("inner");

    vi.spyOn(bridge, "resolveDatasetConflicts").mockRejectedValueOnce(new Error("conflicto sin origen"));
    await act(async () => result.current.comparison.onResolveConflicts([]));
    await waitFor(() => expect(result.current.comparison.mutationStatus).toEqual({
      kind: "error",
      mutation: "resolveConflicts",
      message: "conflicto sin origen",
    }));

    const pending = deferred<DatasetPreview>();
    vi.spyOn(bridge, "resolveDatasetConflicts").mockReturnValueOnce(pending.promise);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    act(() => result.current.comparison.onResolveConflicts([]));
    await waitFor(() => expect(result.current.busy).toBe(true));
    await act(async () => result.current.comparison.onCancelMutation?.());
    expect(cancel).toHaveBeenCalledWith("reviewMutation");
    await act(async () => pending.reject(new Error("Resolución cancelada por el usuario")));
    expect(result.current.comparison.mutationStatus).toEqual({ kind: "idle" });
    expect(result.current.busy).toBe(false);
  });

  it("restores, resets and exposes the project workspace", () => {
    const { result } = setup();

    act(() => result.current.restoreWorkspace({
      reviewTab: "preview",
      sqlHistory: [{ id: "q", sql: "SELECT 1" } as never],
      queryEngine: "duckdb",
      analysisSampleRows: 50_000,
      comparisonKeyColumns: ["id", "ausente", "id"],
      joinType: "left",
    }, dataset.columns, profile));
    expect(result.current.profileStatus).toEqual({ kind: "ready", profile });
    expect(result.current.workspace).toMatchObject({
      reviewTab: "preview",
      queryEngine: "duckdb",
      analysisSampleRows: 50_000,
      comparisonKeyColumns: ["id"],
      joinType: "left",
    });
    expect(result.current.workspace.sqlHistory).toHaveLength(1);

    act(() => result.current.forgetProjectSettings({ clearSqlHistory: false }));
    expect(result.current.workspace.comparisonKeyColumns).toEqual([]);
    expect(result.current.workspace.sqlHistory).toHaveLength(1);

    act(() => result.current.resetForDataset());
    expect(result.current.workspace).not.toHaveProperty("sqlHistory");
    expect(result.current.workspace.joinType).toBe("inner");
  });
});
