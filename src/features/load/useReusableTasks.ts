import { useCallback, useEffect, useRef, useState } from "react";

import {
  checkReusableTaskSchema,
  deleteReusableTask,
  listReusableTasks,
  openReusableTask,
  saveReusableTask,
  type ReusableTask,
  type ReusableTaskSchema,
  type ReusableTaskSchemaCompatibility,
  type ReusableTaskSummary,
} from "../../bridge";

export type ReusableTaskCatalogState = "unavailable" | "loading" | "ready" | "error";
export type ReusableTaskOperation = "open" | "save" | "delete" | "check_schema";

export interface OpenedReusableTask {
  id: string;
  task: ReusableTask;
}

export interface ReusableTaskSchemaCheck {
  taskId: string;
  compatibility: ReusableTaskSchemaCompatibility;
}

interface UseReusableTasksOptions {
  connected: boolean;
  blocked?: boolean;
}

export interface ReusableTasksController {
  catalogState: ReusableTaskCatalogState;
  tasks: ReusableTaskSummary[];
  workingAction: ReusableTaskOperation | null;
  isBusy: boolean;
  error: string | null;
  openedTask: OpenedReusableTask | null;
  schemaCheck: ReusableTaskSchemaCheck | null;
  refresh: () => Promise<void>;
  open: (taskId: string) => Promise<ReusableTask | null>;
  save: (taskId: string | null, task: ReusableTask) => Promise<ReusableTaskSummary | null>;
  remove: (taskId: string) => Promise<boolean>;
  checkSchema: (
    taskId: string,
    schema: ReusableTaskSchema,
  ) => Promise<ReusableTaskSchemaCompatibility | null>;
  clearError: () => void;
}

function errorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const sanitized = raw
    .replace(/[A-Za-z]:[\\/][^\r\n"'`<>]*/g, "archivo seleccionado")
    .replace(/(?:^|\s)(?:\/[^\s"'\x60<>]+)+/g, " archivo seleccionado")
    .trim();
  return (sanitized || "No se pudo completar la operación.").slice(0, 240);
}

function newestFirst(tasks: ReusableTaskSummary[]): ReusableTaskSummary[] {
  return [...tasks].sort((first, second) =>
    second.updatedAt.localeCompare(first.updatedAt) || first.id.localeCompare(second.id));
}

export function useReusableTasks({
  connected,
  blocked = false,
}: UseReusableTasksOptions): ReusableTasksController {
  const initialCatalogState: ReusableTaskCatalogState = connected ? "loading" : "unavailable";
  const [catalogState, setCatalogState] = useState<ReusableTaskCatalogState>(initialCatalogState);
  const [tasks, setTasks] = useState<ReusableTaskSummary[]>([]);
  const [workingAction, setWorkingAction] = useState<ReusableTaskOperation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openedTask, setOpenedTask] = useState<OpenedReusableTask | null>(null);
  const [schemaCheck, setSchemaCheck] = useState<ReusableTaskSchemaCheck | null>(null);

  const optionsRef = useRef({ connected, blocked });
  optionsRef.current = { connected, blocked };
  const mountedRef = useRef(true);
  const operationLock = useRef(false);
  const operationGeneration = useRef(0);
  const catalogGeneration = useRef(0);
  const catalogStateRef = useRef<ReusableTaskCatalogState>(initialCatalogState);

  const changeCatalogState = useCallback((next: ReusableTaskCatalogState) => {
    catalogStateRef.current = next;
    setCatalogState(next);
  }, []);

  const refresh = useCallback(async () => {
    if (!optionsRef.current.connected || operationLock.current) return;
    const request = ++catalogGeneration.current;
    changeCatalogState("loading");
    setError(null);
    try {
      const nextTasks = await listReusableTasks();
      if (!mountedRef.current
        || request !== catalogGeneration.current
        || !optionsRef.current.connected) return;
      setTasks(newestFirst(nextTasks));
      changeCatalogState("ready");
    } catch (cause: unknown) {
      if (!mountedRef.current
        || request !== catalogGeneration.current
        || !optionsRef.current.connected) return;
      changeCatalogState("error");
      setError(errorMessage(cause));
    }
  }, [changeCatalogState]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      catalogGeneration.current += 1;
      operationGeneration.current += 1;
      operationLock.current = false;
    };
  }, []);

  useEffect(() => {
    if (!connected) {
      catalogGeneration.current += 1;
      operationGeneration.current += 1;
      operationLock.current = false;
      setWorkingAction(null);
      if (!connected) {
        setTasks([]);
        setOpenedTask(null);
        setSchemaCheck(null);
        setError(null);
        changeCatalogState("unavailable");
      }
      return;
    }
    void refresh();
  }, [changeCatalogState, connected, refresh]);

  const runExclusive = useCallback(async <T,>(
    action: ReusableTaskOperation,
    operation: () => Promise<T>,
    onSuccess: (result: T) => void,
  ): Promise<T | null> => {
    if (!optionsRef.current.connected || optionsRef.current.blocked || operationLock.current) return null;
    operationLock.current = true;
    const request = ++operationGeneration.current;
    const refreshCatalogAfter = catalogStateRef.current !== "ready";
    catalogGeneration.current += 1;
    setWorkingAction(action);
    setError(null);
    if (action === "check_schema") setSchemaCheck(null);

    const isCurrent = () => mountedRef.current
      && request === operationGeneration.current
      && optionsRef.current.connected;

    try {
      const result = await operation();
      if (!isCurrent()) return null;
      onSuccess(result);
      return result;
    } catch (cause: unknown) {
      if (isCurrent()) setError(errorMessage(cause));
      return null;
    } finally {
      if (isCurrent()) {
        operationLock.current = false;
        setWorkingAction(null);
        if (refreshCatalogAfter) void refresh();
      }
    }
  }, [refresh]);

  const open = useCallback((taskId: string) => runExclusive(
    "open",
    () => openReusableTask(taskId),
    (task) => {
      setOpenedTask({ id: taskId, task });
      setSchemaCheck(null);
    },
  ), [runExclusive]);

  const save = useCallback((taskId: string | null, task: ReusableTask) => runExclusive(
    "save",
    () => saveReusableTask(taskId, task),
    (saved) => {
      if (catalogStateRef.current === "ready") {
        setTasks((current) => newestFirst([
          ...current.filter((candidate) => candidate.id !== saved.id),
          saved,
        ]));
      }
      setOpenedTask({ id: saved.id, task });
      setSchemaCheck(null);
    },
  ), [runExclusive]);

  const remove = useCallback(async (taskId: string): Promise<boolean> => {
    const removed = await runExclusive(
      "delete",
      async () => {
        await deleteReusableTask(taskId);
        return true;
      },
      () => {
        if (catalogStateRef.current === "ready") {
          setTasks((current) => current.filter((candidate) => candidate.id !== taskId));
        }
        setOpenedTask((current) => current?.id === taskId ? null : current);
        setSchemaCheck((current) => current?.taskId === taskId ? null : current);
      },
    );
    return removed === true;
  }, [runExclusive]);

  const checkSchema = useCallback((taskId: string, schema: ReusableTaskSchema) => runExclusive(
    "check_schema",
    () => checkReusableTaskSchema(taskId, schema),
    (compatibility) => setSchemaCheck({ taskId, compatibility }),
  ), [runExclusive]);

  return {
    catalogState,
    tasks,
    workingAction,
    isBusy: workingAction !== null,
    error,
    openedTask,
    schemaCheck,
    refresh,
    open,
    save,
    remove,
    checkSchema,
    clearError: () => setError(null),
  };
}
