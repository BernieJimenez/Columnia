import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ReusableTask,
  ReusableTaskSchemaCompatibility,
  ReusableTaskSummary,
} from "../../bridge";
import { useReusableTasks } from "./useReusableTasks";

const bridge = vi.hoisted(() => ({
  checkReusableTaskSchema: vi.fn(),
  deleteReusableTask: vi.fn(),
  listReusableTasks: vi.fn(),
  openReusableTask: vi.fn(),
  saveReusableTask: vi.fn(),
}));

vi.mock("../../bridge", () => bridge);

const reusableTask: ReusableTask = {
  version: 1,
  name: "Cierre mensual",
  importProfile: {
    version: 1,
    format: "csv",
    schema: [{ name: "id", dataType: "Int64" }],
  },
  recipe: null,
  qualityRules: [{ column: "id", kind: "not_null", maxInvalid: 0 }],
  outputFormat: "csv",
  privacyMode: "mask",
};

const firstTask: ReusableTaskSummary = {
  id: "task-1",
  name: reusableTask.name,
  createdAt: "2026-09-01T10:00:00Z",
  updatedAt: "2026-09-01T10:00:00Z",
  inputColumnCount: 1,
  hasRecipe: false,
  qualityRuleCount: 1,
  outputFormat: "csv",
};

const secondTask: ReusableTaskSummary = {
  ...firstTask,
  id: "task-2",
  name: "Cierre trimestral",
  createdAt: "2026-09-02T10:00:00Z",
  updatedAt: "2026-09-02T10:00:00Z",
};

const readyCompatibility: ReusableTaskSchemaCompatibility = {
  status: "ready",
  missingColumns: [],
  addedColumns: [],
  changedTypes: [],
  orderChanged: false,
};

beforeEach(() => {
  vi.clearAllMocks();
  bridge.listReusableTasks.mockResolvedValue([firstTask]);
  bridge.openReusableTask.mockResolvedValue(reusableTask);
  bridge.saveReusableTask.mockResolvedValue(secondTask);
  bridge.deleteReusableTask.mockResolvedValue(undefined);
  bridge.checkReusableTaskSchema.mockResolvedValue(readyCompatibility);
});

describe("useReusableTasks", () => {
  it("carga, abre, valida, guarda y elimina tareas locales", async () => {
    const { result } = renderHook(() => useReusableTasks({ connected: true }));
    await waitFor(() => expect(result.current.catalogState).toBe("ready"));
    expect(result.current.tasks).toEqual([firstTask]);

    await act(async () => result.current.open(firstTask.id));
    expect(bridge.openReusableTask).toHaveBeenCalledWith(firstTask.id);
    expect(result.current.openedTask).toEqual({ id: firstTask.id, task: reusableTask });

    const schema = [{ name: "id", dataType: "Int64" }];
    await act(async () => result.current.checkSchema(firstTask.id, schema));
    expect(bridge.checkReusableTaskSchema).toHaveBeenCalledWith(firstTask.id, schema);
    expect(result.current.schemaCheck).toEqual({ taskId: firstTask.id, compatibility: readyCompatibility });

    await act(async () => result.current.save(null, reusableTask));
    expect(bridge.saveReusableTask).toHaveBeenCalledWith(null, reusableTask);
    expect(result.current.tasks).toEqual([secondTask, firstTask]);

    await act(async () => expect(result.current.remove(firstTask.id)).resolves.toBe(true));
    expect(bridge.deleteReusableTask).toHaveBeenCalledWith(firstTask.id);
    expect(result.current.tasks).toEqual([secondTask]);
    expect(result.current.openedTask).toBeNull();
  });

  it("ignora ejecuciones dobles mientras una tarea está abriéndose", async () => {
    let resolveOpen!: (task: ReusableTask) => void;
    bridge.openReusableTask.mockImplementation(() => new Promise((resolve) => {
      resolveOpen = resolve;
    }));
    const { result } = renderHook(() => useReusableTasks({ connected: true }));
    await waitFor(() => expect(result.current.catalogState).toBe("ready"));

    let first!: Promise<ReusableTask | null>;
    let duplicate!: Promise<ReusableTask | null>;
    act(() => {
      first = result.current.open(firstTask.id);
      duplicate = result.current.open(firstTask.id);
    });
    expect(bridge.openReusableTask).toHaveBeenCalledOnce();
    expect(result.current.workingAction).toBe("open");

    await act(async () => {
      resolveOpen(reusableTask);
      await expect(first).resolves.toEqual(reusableTask);
      await expect(duplicate).resolves.toBeNull();
    });
    expect(result.current.isBusy).toBe(false);
  });

  it("descarta la respuesta de catálogo anterior cuando llega una actualización más reciente", async () => {
    let resolveStale!: (tasks: ReusableTaskSummary[]) => void;
    bridge.listReusableTasks
      .mockImplementationOnce(() => new Promise((resolve) => { resolveStale = resolve; }))
      .mockResolvedValueOnce([secondTask]);
    const { result } = renderHook(() => useReusableTasks({ connected: true }));
    await waitFor(() => expect(bridge.listReusableTasks).toHaveBeenCalledOnce());

    await act(async () => result.current.refresh());
    expect(result.current.tasks).toEqual([secondTask]);

    await act(async () => {
      resolveStale([firstTask]);
      await Promise.resolve();
    });
    expect(result.current.tasks).toEqual([secondTask]);
  });

  it("descarta una apertura pendiente si se desconecta el escritorio", async () => {
    let resolveOpen!: (task: ReusableTask) => void;
    bridge.openReusableTask.mockImplementation(() => new Promise((resolve) => {
      resolveOpen = resolve;
    }));
    const { result, rerender } = renderHook(
      ({ connected }) => useReusableTasks({ connected }),
      { initialProps: { connected: true } },
    );
    await waitFor(() => expect(result.current.catalogState).toBe("ready"));

    let pending!: Promise<ReusableTask | null>;
    act(() => { pending = result.current.open(firstTask.id); });
    rerender({ connected: false });
    await act(async () => {
      resolveOpen(reusableTask);
      await expect(pending).resolves.toBeNull();
    });

    expect(result.current.catalogState).toBe("unavailable");
    expect(result.current.openedTask).toBeNull();
    expect(result.current.isBusy).toBe(false);
  });

  it("respeta el bloqueo externo y sanitiza errores del catálogo", async () => {
    const { result, rerender } = renderHook(
      ({ blocked }) => useReusableTasks({ connected: true, blocked }),
      { initialProps: { blocked: true } },
    );
    await waitFor(() => expect(result.current.catalogState).toBe("ready"));
    await act(async () => result.current.open(firstTask.id));
    expect(bridge.openReusableTask).not.toHaveBeenCalled();

    rerender({ blocked: false });
    bridge.openReusableTask.mockRejectedValueOnce(new Error("C:\\Users\\private\\task.json"));
    await act(async () => result.current.open(firstTask.id));
    expect(result.current.error).not.toContain("C:\\Users\\private");
    expect(result.current.isBusy).toBe(false);
    expect(bridge.openReusableTask).toHaveBeenCalledOnce();
  });
});
