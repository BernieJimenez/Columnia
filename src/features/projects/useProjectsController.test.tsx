import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview, ProjectOpenResult, ProjectSummary, ProjectWorkspace } from "../../bridge";
import { useProjectsController } from "./useProjectsController";

const bridge = vi.hoisted(() => ({
  deleteProject: vi.fn(),
  getRecoveryCandidate: vi.fn(),
  importDataprepSessionProject: vi.fn(),
  listProjects: vi.fn(),
  openProject: vi.fn(),
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
  bridge.listProjects.mockResolvedValue([summary]);
  bridge.getRecoveryCandidate.mockResolvedValue(summary);
  bridge.saveProject.mockResolvedValue(summary);
  bridge.openProject.mockResolvedValue({ project: summary, dataset, workspace, profile: null } satisfies ProjectOpenResult);
  bridge.deleteProject.mockResolvedValue(undefined);
  bridge.importDataprepSessionProject.mockResolvedValue(summary);
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

  it("importa una sesión DataPrep y abre el proyecto resultante sin recibir una ruta", async () => {
    const onProjectOpened = vi.fn();
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened,
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));

    await act(async () => result.current.importSession());

    expect(bridge.importDataprepSessionProject).toHaveBeenCalledOnce();
    expect(bridge.importDataprepSessionProject).toHaveBeenCalledWith();
    expect(bridge.openProject).toHaveBeenCalledWith(summary.id);
    expect(onProjectOpened).toHaveBeenCalledWith({ project: summary, dataset, workspace, profile: null });
    expect(result.current.activeProject).toEqual(summary);
    expect(result.current.operation).toMatchObject({ kind: "success", message: expect.stringContaining("importada") });
  });

  it("sanitiza rutas locales en errores de operación", async () => {
    bridge.importDataprepSessionProject.mockRejectedValue(new Error("No se pudo leer C:\\Users\\private\\sesion.json"));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));

    await act(async () => result.current.importSession());

    expect(result.current.operation).toMatchObject({ kind: "error" });
    expect(result.current.operation).not.toHaveProperty("message", expect.stringContaining("C:\\Users\\private"));
    expect(result.current.operation).toHaveProperty("message", expect.stringContaining("archivo seleccionado"));
  });

  it("mantiene el estado en reposo cuando se cancela el selector de sesiones", async () => {
    bridge.importDataprepSessionProject.mockRejectedValue(new Error("No se seleccionó una sesión DataPrep."));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: false,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));

    await act(async () => result.current.importSession());

    expect(result.current.operation).toEqual({ kind: "idle" });
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
});
