import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { ExportResult, QualityRule, SavedRecipe } from "../../bridge";
import { INITIAL_DELIVERY_CONTRACT } from "./deliveryModel";
import { useDeliveryController } from "./useDeliveryController";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const rule = { id: "r1", kind: "not_null", column: "id" } as unknown as QualityRule;
const result = { fileName: "ventas.csv", fileSizeBytes: 10, format: "CSV" } as unknown as ExportResult;
const recipe = { name: "Receta" } as unknown as SavedRecipe;

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
  datasetReady = true,
  recipeDraft = null,
}: { datasetReady?: boolean; recipeDraft?: SavedRecipe | null } = {}) {
  const datasetRevisionRef = { current: 1 };
  const hook = renderHook(
    ({ fingerprint }: { fingerprint: string | null }) => useDeliveryController({
      datasetRevisionRef,
      datasetReady,
      datasetFingerprint: fingerprint,
      recipeDraft,
    }),
    { initialProps: { fingerprint: "a" as string | null } },
  );
  return { ...hook, datasetRevisionRef };
}

const unvalidatedCsv = {
  format: "csv" as const,
  privacyMode: "none" as const,
  validation: { kind: "explicitly_unvalidated" as const },
};

describe("useDeliveryController", () => {
  it("exports a file with the contract rules and publishes the result", async () => {
    const exportFile = vi.spyOn(bridge, "exportDataset").mockResolvedValue(result);
    const { result: hook } = setup();

    await act(async () => hook.current.exportActiveDataset({
      format: "csv",
      privacyMode: "mask",
      validation: { kind: "contract", rules: [rule] },
    } as never));

    expect(exportFile).toHaveBeenCalledWith("csv", [rule], false, expect.any(Function), "mask");
    expect(hook.current.exportStatus).toEqual({ kind: "success", result });
    expect(hook.current.busy).toBe(false);
  });

  it("sends the recipe with a bundle and the target with a database delivery", async () => {
    const exportFile = vi.spyOn(bridge, "exportDataset").mockResolvedValue(result);
    const exportRemote = vi.spyOn(bridge, "exportDatasetToDatabase").mockResolvedValue(result);
    const { result: hook } = setup({ recipeDraft: recipe });

    await act(async () => hook.current.exportActiveDataset({ ...unvalidatedCsv, format: "bundle" } as never));
    expect(exportFile).toHaveBeenCalledWith("bundle", [], true, expect.any(Function), "none", recipe);

    const target = { kind: "sqlserver", table: "ventas" };
    await act(async () => hook.current.exportActiveDataset({
      ...unvalidatedCsv,
      format: "sqlserver",
      databaseTarget: target,
    } as never));
    expect(exportRemote).toHaveBeenCalledWith(target, [], true, expect.any(Function), "none");

    await act(async () => hook.current.exportActiveDataset({ ...unvalidatedCsv, format: "sqlserver" } as never));
    expect(hook.current.exportStatus).toEqual({
      kind: "error",
      message: "Falta configurar el destino de base de datos.",
    });
  });

  it("ignores exports without a dataset and drops a response made stale by a new revision", async () => {
    const pending = deferred<ExportResult | null>();
    const exportFile = vi.spyOn(bridge, "exportDataset").mockReturnValue(pending.promise);
    const idle = setup({ datasetReady: false });
    await act(async () => idle.result.current.exportActiveDataset(unvalidatedCsv as never));
    expect(exportFile).not.toHaveBeenCalled();
    idle.unmount();

    const { result: hook } = setup();
    act(() => void hook.current.exportActiveDataset(unvalidatedCsv as never));
    expect(hook.current.exportStatus.kind).toBe("loading");
    act(() => hook.current.invalidateRequests());
    await act(async () => pending.resolve(result));
    expect(hook.current.exportStatus).toEqual({ kind: "idle" });
  });

  it("requests cancellation, recovers a failed request and reports the cancelled export", async () => {
    const pending = deferred<ExportResult | null>();
    vi.spyOn(bridge, "exportDataset").mockReturnValue(pending.promise);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockRejectedValueOnce(new Error("sin respuesta"));
    const { result: hook } = setup();

    act(() => void hook.current.exportActiveDataset(unvalidatedCsv as never));
    await act(async () => hook.current.cancelExport());
    expect(cancel).toHaveBeenCalledWith("export");
    expect(hook.current.exportStatus).toMatchObject({
      kind: "loading",
      cancellation: "available",
      cancellationError: "sin respuesta",
    });

    await act(async () => pending.reject(new Error("Exportación cancelada por el usuario")));
    expect(hook.current.exportStatus).toEqual({ kind: "cancelled" });
  });

  it("applies saved settings, resets them and invalidates the gate when the data changes", async () => {
    vi.spyOn(bridge, "exportDataset").mockResolvedValue(result);
    const { result: hook, rerender } = setup();

    act(() => hook.current.applySettings({ qualityRules: [rule], exportFormat: "parquet", privacyMode: "hash" }));
    expect(hook.current.workspace).toEqual({ qualityRules: [rule], exportFormat: "parquet", privacyMode: "hash" });
    expect(hook.current.contract).toMatchObject({ kind: "with_contract", rules: [rule] });

    await act(async () => hook.current.exportActiveDataset(unvalidatedCsv as never));
    expect(hook.current.exportStatus.kind).toBe("success");
    rerender({ fingerprint: "b" });
    expect(hook.current.exportStatus).toEqual({ kind: "idle" });

    act(() => hook.current.updateContract({ kind: "rules_changed", rules: [] } as never));
    act(() => {
      hook.current.resetOutput();
      hook.current.resetContract();
    });
    expect(hook.current.workspace).toEqual({ qualityRules: [], exportFormat: "csv", privacyMode: "none" });
    expect(hook.current.contract).toEqual(INITIAL_DELIVERY_CONTRACT);

    act(() => hook.current.applySettings({}));
    expect(hook.current.exportFormat).toBe("csv");
  });
});
