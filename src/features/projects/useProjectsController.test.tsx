import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview, ProjectOpenResult, ProjectSummary, ProjectWorkspace } from "../../bridge";
import { useProjectsController } from "./useProjectsController";

const bridge = vi.hoisted(() => ({
  autosaveProject: vi.fn(),
  cancelOperation: vi.fn(),
  deleteProject: vi.fn(),
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
  bridge.cancelOperation.mockResolvedValue(undefined);
  bridge.listProjects.mockResolvedValue({ projects: [summary], recoveryCandidate: summary });
  bridge.listProjectVersions.mockResolvedValue([]);
  bridge.restoreProjectVersion.mockResolvedValue({ project: summary, dataset, workspace, profile: null });
  bridge.saveProject.mockResolvedValue(summary);
  bridge.openProject.mockResolvedValue({ project: summary, dataset, workspace, profile: null } satisfies ProjectOpenResult);
  bridge.deleteProject.mockResolvedValue(undefined);
});

describe("useProjectsController", () => {
  it("carga catálogo y recuperación en una llamada al conectar", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    expect(bridge.listProjects).toHaveBeenCalledOnce();
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

  it("cancela la carga del catálogo y conserva el resultado previo", async () => {
    let rejectCatalog!: (error: Error) => void;
    bridge.listProjects.mockReturnValueOnce(new Promise((_, reject) => { rejectCatalog = reject; }));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("loading"));
    await act(async () => result.current.cancelCatalogLoad());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectCatalog");
    rejectCatalog(new Error("Operación cancelada por el usuario."));
    await waitFor(() => expect(result.current.catalog.kind).toBe("cancelled"));
  });

  it("expone el error al fallar la cancelación del catálogo", async () => {
    let resolveCatalog!: (value: { projects: ProjectSummary[]; recoveryCandidate: ProjectSummary | null }) => void;
    bridge.listProjects.mockReturnValueOnce(new Promise((resolve) => { resolveCatalog = resolve; }));
    bridge.cancelOperation.mockRejectedValueOnce(new Error("C:\\tmp\\catalogo"));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("loading"));
    await act(async () => result.current.cancelCatalogLoad());
    expect(result.current.catalog).toMatchObject({ kind: "error", message: expect.not.stringContaining("C:\\") });
    resolveCatalog({ projects: [summary], recoveryCandidate: summary });
  });

  it("carga, cancela y reporta errores del historial de versiones", async () => {
    const version = { id: 12, createdAt: "2026-08-22T10:00:00Z", label: "Versión" };
    bridge.listProjectVersions.mockResolvedValueOnce([version]);
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.open(summary.id));
    await waitFor(() => expect(result.current.versions).toMatchObject({ kind: "ready", versions: [version] }));

    let rejectVersions!: (error: Error) => void;
    bridge.listProjectVersions.mockReturnValueOnce(new Promise((_, reject) => { rejectVersions = reject; }));
    act(() => { void result.current.refreshVersions(summary.id); });
    await waitFor(() => expect(result.current.versions.kind).toBe("loading"));
    await act(async () => result.current.cancelVersionsLoad());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectVersions");
    rejectVersions(new Error("Operación cancelada por el usuario."));
    await waitFor(() => expect(result.current.versions.kind).toBe("cancelled"));

    bridge.listProjectVersions.mockRejectedValueOnce(new Error("C:\\tmp\\versions"));
    await act(async () => result.current.refreshVersions(summary.id));
    expect(result.current.versions).toMatchObject({ kind: "error", message: expect.not.stringContaining("C:\\") });
  });

  it("maneja cancelación y fallo de guardado, apertura, restauración y borrado", async () => {
    let rejectSave!: (error: Error) => void;
    bridge.saveProject.mockReturnValueOnce(new Promise((_, reject) => { rejectSave = reject; }));
    const onProjectOpened = vi.fn();
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened,
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    let save!: Promise<void>;
    act(() => { save = result.current.save("Guardado cancelable"); });
    await waitFor(() => expect(result.current.operation.kind).toBe("working"));
    await act(async () => result.current.cancelSave());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectSave");
    rejectSave(new Error("Operación cancelada por el usuario."));
    await act(async () => save);
    expect(result.current.operation).toEqual({ kind: "idle" });

    bridge.openProject.mockRejectedValueOnce(new Error("C:\\tmp\\open"));
    await act(async () => result.current.open(summary.id));
    expect(result.current.operation).toMatchObject({ kind: "error", message: expect.not.stringContaining("C:\\") });

    bridge.restoreProjectVersion.mockRejectedValueOnce(new Error("C:\\tmp\\restore"));
    await act(async () => result.current.restore(summary.id, 12));
    expect(result.current.operation).toMatchObject({ kind: "error", message: expect.not.stringContaining("C:\\") });

    act(() => result.current.requestDelete(summary));
    bridge.deleteProject.mockRejectedValueOnce(new Error("C:\\tmp\\delete"));
    await act(async () => result.current.confirmDelete());
    expect(result.current.operation).toMatchObject({ kind: "error", message: expect.not.stringContaining("C:\\") });
    expect(result.current.deletion.kind).toBe("idle");
    expect(onProjectOpened).not.toHaveBeenCalled();
  });

  it("cancela una operación de apertura y notifica el desvinculado del proyecto activo", async () => {
    let rejectOpen!: (error: Error) => void;
    bridge.openProject.mockReturnValueOnce(new Promise((_, reject) => { rejectOpen = reject; }));
    const onActiveProjectUnlinked = vi.fn();
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
      onActiveProjectUnlinked,
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    let open!: Promise<void>;
    act(() => { open = result.current.open(summary.id); });
    await waitFor(() => expect(result.current.operation.kind).toBe("working"));
    await act(async () => result.current.cancelOpen());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectOpen");
    rejectOpen(new Error("Operación cancelada por el usuario."));
    await act(async () => open);
    act(() => result.current.unlinkActiveProject());
    expect(onActiveProjectUnlinked).toHaveBeenCalledOnce();
  });

  it("cancela una restauración pendiente y conserva el estado inactivo", async () => {
    let rejectRestore!: (error: Error) => void;
    bridge.restoreProjectVersion.mockReturnValueOnce(new Promise((_, reject) => {
      rejectRestore = reject;
    }));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    let restore!: Promise<void>;
    act(() => { restore = result.current.restore(summary.id, 12); });
    await waitFor(() => expect(result.current.operation.kind).toBe("working"));
    await act(async () => result.current.cancelRestore());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectOpen");
    rejectRestore(new Error("Operación cancelada por el usuario."));
    await act(async () => restore);
    expect(result.current.operation).toEqual({ kind: "idle" });
  });

  it("cancela un borrado pendiente y limpia la confirmación", async () => {
    let rejectDelete!: (error: Error) => void;
    bridge.deleteProject.mockReturnValueOnce(new Promise((_, reject) => {
      rejectDelete = reject;
    }));
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    act(() => result.current.requestDelete(summary));
    let deletion!: Promise<void>;
    act(() => { deletion = result.current.confirmDelete(); });
    await waitFor(() => expect(result.current.operation.kind).toBe("working"));
    await act(async () => result.current.cancelProjectDelete());
    expect(bridge.cancelOperation).toHaveBeenCalledWith("projectDelete");
    rejectDelete(new Error("Operación cancelada por el usuario."));
    await act(async () => deletion);
    expect(result.current.operation).toEqual({ kind: "idle" });
    expect(result.current.deletion).toEqual({ kind: "idle" });
  });

  it("rechaza nombres demasiado largos y permite limpiar el feedback", async () => {
    const { result } = renderHook(() => useProjectsController({
      connected: true,
      blocked: false,
      hasDataset: true,
      workspace,
      onProjectOpened: vi.fn(),
    }));
    await waitFor(() => expect(result.current.catalog.kind).toBe("ready"));
    await act(async () => result.current.save("x".repeat(129)));
    expect(result.current.operation).toMatchObject({ kind: "error", message: expect.stringContaining("128") });
    act(() => result.current.clearFeedback());
    expect(result.current.operation).toEqual({ kind: "idle" });
    act(() => result.current.cancelDelete());
    expect(result.current.deletion).toEqual({ kind: "idle" });
  });
});
