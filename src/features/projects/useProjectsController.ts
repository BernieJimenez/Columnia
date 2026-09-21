import { useCallback, useEffect, useRef, useState } from "react";

import {
  autosaveProject,
  cancelOperation,
  deleteProject,
  getRecoveryCandidate,
  listProjectVersions,
  listProjects,
  openProject,
  restoreProjectVersion,
  saveProject,
  type ProjectOpenResult,
  type ProjectSummary,
  type ProjectWorkspace,
} from "../../bridge";
import {
  sortProjects,
  validateProjectName,
  type ProjectCatalogState,
  type ProjectAutoSaveState,
  type ProjectDeletionState,
  type ProjectOperationState,
  type ProjectVersionsState,
} from "./projectModel";

const AUTO_SAVE_PREFERENCE_PREFIX = "columnia.project.auto-save.";

function readAutoSavePreference(projectId: string): boolean {
  try {
    return window.localStorage.getItem(`${AUTO_SAVE_PREFERENCE_PREFIX}${projectId}`) === "enabled";
  } catch {
    return false;
  }
}

function writeAutoSavePreference(projectId: string, enabled: boolean): void {
  try {
    window.localStorage.setItem(
      `${AUTO_SAVE_PREFERENCE_PREFIX}${projectId}`,
      enabled ? "enabled" : "disabled",
    );
  } catch {
    // The setting still applies for this session when browser storage is unavailable.
  }
}

interface ProjectsControllerOptions {
  connected: boolean;
  blocked: boolean;
  hasDataset: boolean;
  workspace: ProjectWorkspace;
  datasetRevision?: number;
  onProjectOpened: (result: ProjectOpenResult) => Promise<void> | void;
  onActiveProjectDeleted?: () => void;
  onActiveProjectUnlinked?: () => void;
}

function errorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const sanitized = raw
    .replace(/[A-Za-z]:[\\/][^\r\n"'`<>]*/g, "archivo seleccionado")
    .replace(/(?:^|\s)(?:\/[^\s"'`<>]+)+/g, " archivo seleccionado")
    .trim();
  return (sanitized || "No se pudo completar la operación.").slice(0, 240);
}

function isCancellationError(error: unknown): boolean {
  return String(error).includes("cancelada por el usuario");
}

export function useProjectsController({
  connected,
  blocked,
  hasDataset,
  workspace,
  datasetRevision = 0,
  onProjectOpened,
  onActiveProjectDeleted,
  onActiveProjectUnlinked,
}: ProjectsControllerOptions) {
  const [catalog, setCatalog] = useState<ProjectCatalogState>({ kind: "unavailable" });
  const [operation, setOperation] = useState<ProjectOperationState>({ kind: "idle" });
  const [deletion, setDeletion] = useState<ProjectDeletionState>({ kind: "idle" });
  const [versions, setVersions] = useState<ProjectVersionsState>({ kind: "ready", versions: [] });
  const [autoSavePreference, setAutoSavePreference] = useState<{
    projectId: string | null;
    enabled: boolean;
  }>({ projectId: null, enabled: false });
  const [autoSave, setAutoSave] = useState<ProjectAutoSaveState>({ kind: "disabled" });
  const [activeProject, setActiveProject] = useState<ProjectSummary | null>(null);
  const [openCancellationPending, setOpenCancellationPending] = useState(false);
  const [restoreCancellationPending, setRestoreCancellationPending] = useState(false);
  const [saveCancellationPending, setSaveCancellationPending] = useState(false);
  const [autoSaveCancellationPending, setAutoSaveCancellationPending] = useState(false);
  const operationLock = useRef(false);
  const autoSaveInProgress = useRef(false);
  const lastAutoSaveSignature = useRef<string | null>(null);
  const failedAutoSaveSignature = useRef<string | null>(null);
  const skipAutoSaveForProject = useRef<string | null>(null);
  const workspaceRef = useRef(workspace);
  workspaceRef.current = workspace;
  const workspaceSignature = JSON.stringify(workspace);
  const autoSaveEnabled = Boolean(
    activeProject && autoSavePreference.projectId === activeProject.id && autoSavePreference.enabled,
  );

  const refresh = useCallback(async () => {
    if (!connected) {
      setCatalog({ kind: "unavailable" });
      return;
    }
    setCatalog({ kind: "loading" });
    try {
      const [projects, recoveryCandidate] = await Promise.all([
        listProjects(),
        getRecoveryCandidate(),
      ]);
      setCatalog({ kind: "ready", projects: sortProjects(projects), recoveryCandidate });
    } catch (error: unknown) {
      setCatalog({ kind: "error", message: errorMessage(error) });
    }
  }, [connected]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const refreshVersions = useCallback(async (projectId: string) => {
    setVersions({ kind: "loading" });
    try {
      setVersions({ kind: "ready", versions: await listProjectVersions(projectId) });
    } catch (error: unknown) {
      setVersions({ kind: "error", message: errorMessage(error) });
    }
  }, []);

  useEffect(() => {
    const projectId = activeProject?.id ?? null;
    if (!connected || !projectId) {
      setVersions({ kind: "ready", versions: [] });
      return;
    }
    void refreshVersions(projectId);
  }, [activeProject?.id, activeProject?.updatedAt, connected, refreshVersions]);

  useEffect(() => {
    const projectId = activeProject?.id ?? null;
    setAutoSavePreference({
      projectId,
      enabled: projectId ? readAutoSavePreference(projectId) : false,
    });
    setAutoSave(projectId && readAutoSavePreference(projectId)
      ? { kind: "idle" }
      : { kind: "disabled" });
    lastAutoSaveSignature.current = null;
    failedAutoSaveSignature.current = null;
  }, [activeProject?.id]);

  const runExclusive = useCallback(async (
    next: Extract<ProjectOperationState, { kind: "working" }>,
    task: () => Promise<void>,
  ) => {
    if (operationLock.current || blocked) return;
    operationLock.current = true;
    setOperation(next);
    try {
      await task();
    } catch (error: unknown) {
      setOperation({ kind: "error", message: errorMessage(error) });
    } finally {
      operationLock.current = false;
    }
  }, [blocked]);

  const save = useCallback(async (rawName: string) => {
    const validation = validateProjectName(rawName);
    if (!validation.valid) {
      setOperation({ kind: "error", message: validation.message });
      return;
    }
    if (!hasDataset) {
      setOperation({ kind: "error", message: "Carga un dataset antes de guardar un proyecto." });
      return;
    }
    setSaveCancellationPending(false);
    await runExclusive(
      { kind: "working", operation: "save", projectId: activeProject?.id ?? null },
      async () => {
        try {
          const saved = await saveProject(activeProject?.id ?? null, validation.name, workspace);
          setActiveProject(saved);
          lastAutoSaveSignature.current = `${saved.id}:${datasetRevision}:${JSON.stringify(workspace)}`;
          failedAutoSaveSignature.current = null;
          setOperation({ kind: "success", message: activeProject
            ? `Proyecto “${saved.name}” actualizado.`
            : `Proyecto “${saved.name}” guardado.` });
          await refresh();
        } catch (error: unknown) {
          if (isCancellationError(error)) {
            setOperation({ kind: "idle" });
            return;
          }
          throw error;
        }
      },
    );
    setSaveCancellationPending(false);
  }, [activeProject, datasetRevision, hasDataset, refresh, runExclusive, workspace]);

  const cancelSave = useCallback(async () => {
    if (
      !operationLock.current
      || operation.kind !== "working"
      || operation.operation !== "save"
      || saveCancellationPending
    ) {
      return;
    }
    setSaveCancellationPending(true);
    try {
      await cancelOperation("projectSave");
    } catch (error: unknown) {
      setSaveCancellationPending(false);
      setOperation({ kind: "error", message: errorMessage(error) });
    }
  }, [operation, saveCancellationPending]);

  const open = useCallback(async (projectId: string) => {
    setOpenCancellationPending(false);
    await runExclusive(
      { kind: "working", operation: "open", projectId },
      async () => {
        try {
          const result = await openProject(projectId);
          skipAutoSaveForProject.current = readAutoSavePreference(projectId) ? projectId : null;
          await onProjectOpened(result);
          setActiveProject(result.project);
          setOperation({ kind: "success", message: `Proyecto “${result.project.name}” abierto.` });
          await refresh();
        } catch (error: unknown) {
          if (isCancellationError(error)) {
            setOperation({ kind: "idle" });
            return;
          }
          throw error;
        }
      },
    );
    setOpenCancellationPending(false);
  }, [onProjectOpened, refresh, runExclusive]);

  const cancelOpen = useCallback(async () => {
    if (
      !operationLock.current
      || operation.kind !== "working"
      || operation.operation !== "open"
      || openCancellationPending
    ) {
      return;
    }
    setOpenCancellationPending(true);
    try {
      await cancelOperation("projectOpen");
    } catch (error: unknown) {
      setOpenCancellationPending(false);
      setOperation({ kind: "error", message: errorMessage(error) });
    }
  }, [openCancellationPending, operation]);

  const restore = useCallback(async (projectId: string, versionId: number) => {
    setRestoreCancellationPending(false);
    await runExclusive(
      { kind: "working", operation: "restore", projectId },
      async () => {
        try {
          const result = await restoreProjectVersion(projectId, versionId);
          skipAutoSaveForProject.current = readAutoSavePreference(projectId) ? projectId : null;
          await onProjectOpened(result);
          setActiveProject(result.project);
          setOperation({ kind: "success", message: `Versión restaurada: “${result.project.name}”.` });
          await refresh();
        } catch (error: unknown) {
          if (isCancellationError(error)) {
            setOperation({ kind: "idle" });
            return;
          }
          throw error;
        }
      },
    );
    setRestoreCancellationPending(false);
  }, [onProjectOpened, refresh, runExclusive]);

  const cancelRestore = useCallback(async () => {
    if (
      !operationLock.current
      || operation.kind !== "working"
      || operation.operation !== "restore"
      || restoreCancellationPending
    ) {
      return;
    }
    setRestoreCancellationPending(true);
    try {
      await cancelOperation("projectOpen");
    } catch (error: unknown) {
      setRestoreCancellationPending(false);
      setOperation({ kind: "error", message: errorMessage(error) });
    }
  }, [operation, restoreCancellationPending]);

  const setAutoSaveEnabled = useCallback((enabled: boolean) => {
    if (!activeProject) return;
    writeAutoSavePreference(activeProject.id, enabled);
    setAutoSavePreference({ projectId: activeProject.id, enabled });
    setAutoSave(enabled ? { kind: "idle" } : { kind: "disabled" });
    lastAutoSaveSignature.current = null;
    failedAutoSaveSignature.current = null;
  }, [activeProject]);

  useEffect(() => {
    if (!activeProject || !autoSaveEnabled || !connected || !hasDataset || blocked) return;
    if (operationLock.current || autoSaveInProgress.current || autoSave.kind === "saving") return;
    const signature = `${activeProject.id}:${datasetRevision}:${workspaceSignature}`;
    if (lastAutoSaveSignature.current === signature || failedAutoSaveSignature.current === signature) return;
    if (skipAutoSaveForProject.current === activeProject.id) {
      skipAutoSaveForProject.current = null;
      lastAutoSaveSignature.current = signature;
      setAutoSave({ kind: "idle" });
      return;
    }
    const timeout = window.setTimeout(() => {
      if (operationLock.current || autoSaveInProgress.current) return;
      autoSaveInProgress.current = true;
      operationLock.current = true;
      setAutoSaveCancellationPending(false);
      setAutoSave({ kind: "saving" });
      void (async () => {
        try {
          const saved = await autosaveProject(activeProject.id, activeProject.name, workspaceRef.current);
          lastAutoSaveSignature.current = signature;
          failedAutoSaveSignature.current = null;
          setActiveProject(saved);
          setAutoSave({ kind: "saved", savedAt: new Date().toISOString() });
          await refresh();
        } catch (error: unknown) {
          failedAutoSaveSignature.current = signature;
          if (isCancellationError(error)) {
            setAutoSave({ kind: "idle" });
            return;
          }
          setAutoSave({
            kind: "error",
            message: `${errorMessage(error)} La última versión válida se conserva.`,
          });
        } finally {
          operationLock.current = false;
          autoSaveInProgress.current = false;
          setAutoSaveCancellationPending(false);
        }
      })();
    }, 900);
    return () => window.clearTimeout(timeout);
  }, [
    activeProject,
    autoSave.kind,
    autoSaveEnabled,
    blocked,
    connected,
    datasetRevision,
    hasDataset,
    operation.kind,
    refresh,
    workspaceSignature,
  ]);

  const cancelAutoSave = useCallback(async () => {
    if (
      !autoSaveInProgress.current
      || autoSave.kind !== "saving"
      || autoSaveCancellationPending
    ) {
      return;
    }
    setAutoSaveCancellationPending(true);
    try {
      await cancelOperation("projectSave");
    } catch (error: unknown) {
      setAutoSaveCancellationPending(false);
      setAutoSave({ kind: "error", message: errorMessage(error) });
    }
  }, [autoSave.kind, autoSaveCancellationPending]);

  const confirmDelete = useCallback(async () => {
    if (deletion.kind !== "confirming") return;
    const target = deletion.project;
    await runExclusive(
      { kind: "working", operation: "delete", projectId: target.id },
      async () => {
        await deleteProject(target.id);
        setDeletion({ kind: "idle" });
        if (activeProject?.id === target.id) {
          setActiveProject(null);
          onActiveProjectDeleted?.();
        }
        setOperation({ kind: "success", message: `Proyecto “${target.name}” eliminado. El dataset abierto se conserva.` });
        await refresh();
      },
    );
  }, [activeProject, deletion, onActiveProjectDeleted, refresh, runExclusive]);

  return {
    catalog,
    operation,
    deletion,
    activeProject,
    isBusy: operation.kind === "working" || autoSave.kind === "saving",
    refresh,
    save,
    open,
    cancelSave,
    saveCancellationPending,
    cancelOpen,
    openCancellationPending,
    versions,
    refreshVersions,
    restore,
    cancelRestore,
    restoreCancellationPending,
    autoSave,
    cancelAutoSave,
    autoSaveCancellationPending,
    autoSaveEnabled,
    setAutoSaveEnabled,
    requestDelete: (project: ProjectSummary) => setDeletion({ kind: "confirming", project }),
    cancelDelete: () => setDeletion({ kind: "idle" }),
    confirmDelete,
    unlinkActiveProject: () => {
      setActiveProject(null);
      onActiveProjectUnlinked?.();
    },
    clearFeedback: () => setOperation({ kind: "idle" }),
  };
}
