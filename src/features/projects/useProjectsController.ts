import { useCallback, useEffect, useRef, useState } from "react";

import {
  deleteProject,
  getRecoveryCandidate,
  importDataprepSessionProject,
  listProjects,
  openProject,
  saveProject,
  type ProjectOpenResult,
  type ProjectSummary,
  type ProjectWorkspace,
} from "../../bridge";
import {
  sortProjects,
  validateProjectName,
  type ProjectCatalogState,
  type ProjectDeletionState,
  type ProjectOperationState,
} from "./projectModel";

interface ProjectsControllerOptions {
  connected: boolean;
  blocked: boolean;
  hasDataset: boolean;
  workspace: ProjectWorkspace;
  onProjectOpened: (result: ProjectOpenResult) => Promise<void> | void;
  onActiveProjectDeleted?: () => void;
}

function errorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const sanitized = raw
    .replace(/[A-Za-z]:[\\/][^\r\n"'`<>]*/g, "archivo seleccionado")
    .replace(/(?:^|\s)(?:\/[^\s"'`<>]+)+/g, " archivo seleccionado")
    .trim();
  return (sanitized || "No se pudo completar la operación.").slice(0, 240);
}

function isUserCancellation(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  const normalized = message.toLocaleLowerCase("es");
  return normalized.includes("no se seleccionó") || normalized.includes("cancelada por el usuario");
}

export function useProjectsController({
  connected,
  blocked,
  hasDataset,
  workspace,
  onProjectOpened,
  onActiveProjectDeleted,
}: ProjectsControllerOptions) {
  const [catalog, setCatalog] = useState<ProjectCatalogState>({ kind: "unavailable" });
  const [operation, setOperation] = useState<ProjectOperationState>({ kind: "idle" });
  const [deletion, setDeletion] = useState<ProjectDeletionState>({ kind: "idle" });
  const [activeProject, setActiveProject] = useState<ProjectSummary | null>(null);
  const operationLock = useRef(false);

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
    await runExclusive(
      { kind: "working", operation: "save", projectId: activeProject?.id ?? null },
      async () => {
        const saved = await saveProject(activeProject?.id ?? null, validation.name, workspace);
        setActiveProject(saved);
        setOperation({ kind: "success", message: activeProject
          ? `Proyecto “${saved.name}” actualizado.`
          : `Proyecto “${saved.name}” guardado.` });
        await refresh();
      },
    );
  }, [activeProject, hasDataset, refresh, runExclusive, workspace]);

  const open = useCallback(async (projectId: string) => {
    await runExclusive(
      { kind: "working", operation: "open", projectId },
      async () => {
        const result = await openProject(projectId);
        await onProjectOpened(result);
        setActiveProject(result.project);
        setOperation({ kind: "success", message: `Proyecto “${result.project.name}” abierto.` });
        await refresh();
      },
    );
  }, [onProjectOpened, refresh, runExclusive]);

  const importSession = useCallback(async () => {
    await runExclusive(
      { kind: "working", operation: "import", projectId: null },
      async () => {
        try {
          const imported = await importDataprepSessionProject();
          const result = await openProject(imported.id);
          await onProjectOpened(result);
          setActiveProject(result.project);
          setOperation({ kind: "success", message: `Sesión DataPrep importada como “${result.project.name}”.` });
          await refresh();
        } catch (error: unknown) {
          if (isUserCancellation(error)) {
            setOperation({ kind: "idle" });
            return;
          }
          throw error;
        }
      },
    );
  }, [onProjectOpened, refresh, runExclusive]);

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
    isBusy: operation.kind === "working",
    refresh,
    save,
    open,
    importSession,
    requestDelete: (project: ProjectSummary) => setDeletion({ kind: "confirming", project }),
    cancelDelete: () => setDeletion({ kind: "idle" }),
    confirmDelete,
    unlinkActiveProject: () => setActiveProject(null),
    clearFeedback: () => setOperation({ kind: "idle" }),
  };
}
