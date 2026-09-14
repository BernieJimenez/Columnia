import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview, ProjectOpenResult, ProjectSummary, ProjectWorkspace } from "../../bridge";
import { useProjectsController } from "./useProjectsController";

const bridge = vi.hoisted(() => ({
  autosaveProject: vi.fn(),
  deleteProject: vi.fn(),
  getRecoveryCandidate: vi.fn(),
  listProjectVersions: vi.fn(),
  listProjects: vi.fn(),
  openProject: vi.fn(),
  restoreProjectVersion: vi.fn(),
  saveProject: vi.fn(),
}));

vi.mock("../../bridge", () => bridge);

const summary: ProjectSummary = {
  id: "project-1",
  name: "Proyecto uno",
  datasetFileName: "snapshot.parquet",
  rowCount: 2,
  columnCount: 1,
  createdAt: "2026-08-20T10:00:00Z",
  updatedAt: "2026-08-21T10:00:00Z",
};
const dataset: DatasetPreview = {
  fileName: "snapshot.parquet",
  fileSizeBytes: 20,
  rowCount: 2,
  columnCount: 1,
  columns: [{ name: "id", dataType: "Int64" }],
  rows: [["1"], ["2"]],
};
const workspace: ProjectWorkspace = { qualityRules: [], recipeDraft: null };

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
  bridge.autosaveProject.mockResolvedValue(summary);
  bridge.listProjects.mockResolvedValue([summary]);
  bridge.listProjectVersions.mockResolvedValue([]);
  bridge.getRecoveryCandidate.mockResolvedValue(summary);
  bridge.restoreProjectVersion.mockResolvedValue({ project: summary, dataset, workspace, profile: null });
  bridge.saveProject.mockResolvedValue(summary);
  bridge.openProject.mockResolvedValue({ project: summary, dataset, workspace, profile: null } satisfies ProjectOpenResult);
  bridge.deleteProject.mockResolvedValue(undefined);
});

describe("useProjectsController", () => {
  it("carga catálogo y recuperación en paralelo al conectar", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    expect(bridge.listProjects).toHaveBeenCalledOnce();
    expect(bridge.getRecoveryCandidate).toHaveBeenCalledOnce();
  });

  it("abre un snapshot, lo marca activo y publica el dataset una sola vez", async () => {
    const onProjectOpened = vi.fn();
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened,
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.open(summary.id));
    expect(onProjectOpened).toHaveBeenCalledWith({ project: summary, dataset, workspace, profile: null });
    expect(result.current.activeProject).toEqual(summary);
    expect(result.current.operation.kind).toBe("success");
  });

  it("impide doble guardado mientras la primera operación sigue activa", async () => {
    let resolveSave!: (value: ProjectSummary) => void;
    bridge.saveProject.mockImplementation(() => new Promise((resolve) => { resolveSave = resolve; }));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    let first!: Promise<void>;
    act(() => {
      first = result.current.save("Proyecto seguro");
      void result.current.save("Duplicado");
    });
    expect(bridge.saveProject).toHaveBeenCalledOnce();
    expect(bridge.saveProject).toHaveBeenCalledWith(null, "Proyecto seguro", workspace);
    await act(async () => { resolveSave(summary); await first; });
  });

  it("al borrar el activo solo desvincula el proyecto", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.open(summary.id));
    act(() => result.current.requestDelete(summary));
    await act(async () => result.current.confirmDelete());
    expect(bridge.deleteProject).toHaveBeenCalledWith(summary.id);
    expect(result.current.activeProject).toBeNull();
    expect(result.current.operation).toMatchObject({ kind: "success", message: expect.stringContaining("se conserva") });
  });

  it("restaura una versión y activa su snapshot en un solo flujo del controlador", async () => {
    const onProjectOpened = vi.fn();
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened,
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.restore(summary.id, 12));
    expect(bridge.restoreProjectVersion).toHaveBeenCalledWith(summary.id, 12);
    expect(bridge.openProject).not.toHaveBeenCalled();
    expect(onProjectOpened).toHaveBeenCalledWith({ project: summary, dataset, workspace, profile: null });
    expect(result.current.activeProject).toEqual(summary);
  });

  it("expone estados desconectado y error sin filtrar rutas del sistema", async () => {
    const disconnected = renderHook(() => useProjectsController({
      connected: false,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(disconnected.result.current.catalog.kind).toBe("unavailable"));

    bridge.listProjects.mockRejectedValueOnce(new Error("C:\\Users\\secret\\projects"));
    const connected = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(connected.result.current.catalog.kind).toBe("error"));
    expect(connected.result.current.catalog).toMatchObject({ message: expect.not.stringContaining("C:\\") });
  });

  it("rechaza nombres inválidos y guardado sin dataset antes de tocar el bridge", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.save("   "));
    expect(result.current.operation).toMatchObject({ kind: "error" });
    await act(async () => result.current.save("Proyecto sin datos"));
    expect(result.current.operation).toMatchObject({ kind: "error", message: expect.stringContaining("dataset") });
    expect(bridge.saveProject).not.toHaveBeenCalled();
  });

  it("respeta el bloqueo externo de operaciones", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: true,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.save("No debe guardar"));
    expect(bridge.saveProject).not.toHaveBeenCalled();
    expect(result.current.operation).toEqual({ kind: "idle" });
  });

  it("autoguarda solo tras opt-in y cambios de workspace, y publica estado guardado", async () => {
    const onProjectOpened = vi.fn();
    const { result, rerender } = renderHook(
      ({ currentWorkspace, revision }: { currentWorkspace: ProjectWorkspace; revision: number }) =>
        useProjectsController({
          connected: true,
          blocked: false,
          hasDataset: true,
          workspace: currentWorkspace,
          datasetRevision: revision,
          onProjectOpened,
        }),
      { initialProps: { currentWorkspace: workspace, revision: 1 } },
    );
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.open(summary.id));
    expect(bridge.autosaveProject).not.toHaveBeenCalled();

    act(() => result.current.setAutoSaveEnabled(true));
    rerender({
      currentWorkspace: { ...workspace, activePhase: "prepare" },
      revision: 2,
    });
    await waitFor(() => expect(bridge.autosaveProject).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(bridge.autosaveProject).toHaveBeenCalledWith(
      summary.id,
      summary.name,
      { ...workspace, activePhase: "prepare" },
    );
    expect(result.current.autoSave.kind).toBe("saved");
  });

  it("informa fallo de autoguardado sin perder la preferencia ni afirmar guardado", async () => {
    window.localStorage.setItem("columnia.project.auto-save.project-1", "enabled");
    bridge.autosaveProject.mockRejectedValueOnce(new Error("disco lleno"));
    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) => useProjectsController({
        connected: true,
        blocked: false,
        hasDataset: true,
        workspace: { ...workspace, activePhase: revision === 1 ? "review" : "prepare" },
        datasetRevision: revision,
        onProjectOpened: vi.fn(),
      }),
      { initialProps: { revision: 1 } },
    );
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.open(summary.id));
    rerender({ revision: 2 });
    await waitFor(() => expect(result.current.autoSave.kind).toBe("error"), { timeout: 3000 });
    expect(result.current.autoSave).toMatchObject({
      kind: "error",
      message: expect.stringContaining("La última versión válida se conserva"),
    });
  });
});
