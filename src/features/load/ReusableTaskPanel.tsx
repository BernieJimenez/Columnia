import { useMemo, useState, type FormEvent } from "react";

import type { ReusableTask, ReusableTaskSchema } from "../../bridge";
import { useReusableTasks } from "./useReusableTasks";

interface ReusableTaskPanelProps {
  connected: boolean;
  blocked: boolean;
  schema: ReusableTaskSchema | null;
  draft: Omit<ReusableTask, "name"> | null;
  onApply: (task: ReusableTask) => void;
}

export function ReusableTaskPanel({
  connected,
  blocked,
  schema,
  draft,
  onApply,
}: ReusableTaskPanelProps) {
  const reusableTasks = useReusableTasks({ connected, blocked });
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [taskName, setTaskName] = useState("");
  const [reviewedSchema, setReviewedSchema] = useState<{ taskId: string; fingerprint: string } | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const schemaFingerprint = useMemo(() => schema === null ? null : JSON.stringify(schema), [schema]);
  const openedTask = reusableTasks.openedTask?.id === selectedTaskId
    ? reusableTasks.openedTask.task
    : null;
  const checkedCompatibility = reusableTasks.schemaCheck?.taskId === selectedTaskId
    ? reusableTasks.schemaCheck.compatibility
    : null;
  const reviewIsCurrent = schemaFingerprint !== null
    && reviewedSchema?.taskId === selectedTaskId
    && reviewedSchema.fingerprint === schemaFingerprint;
  const canApply = !blocked
    && !reusableTasks.isBusy
    && openedTask !== null
    && reviewIsCurrent
    && checkedCompatibility?.status === "ready";

  async function openAndReviewTask(taskId: string) {
    if (!taskId || reusableTasks.isBusy) return;
    reusableTasks.clearError();
    setSaveMessage(null);
    setReviewedSchema(null);
    const task = await reusableTasks.open(taskId);
    if (!task || schema === null || schemaFingerprint === null) return;
    const compatibility = await reusableTasks.checkSchema(taskId, schema);
    if (compatibility) {
      setReviewedSchema({ taskId, fingerprint: schemaFingerprint });
    }
  }

  async function saveCurrentTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedName = taskName.trim();
    if (!draft || !trimmedName || reusableTasks.isBusy) return;
    reusableTasks.clearError();
    setSaveMessage(null);
    const saved = await reusableTasks.save(null, { ...draft, name: trimmedName });
    if (saved) {
      setTaskName("");
      setSelectedTaskId(saved.id);
      setReviewedSchema(null);
      setSaveMessage(`Tarea “${saved.name}” guardada en este equipo.`);
    }
  }

  function applySelectedTask() {
    if (canApply && openedTask) onApply(openedTask);
  }

  return (
    <details className="load-secondary reusable-task-panel">
      <summary>
        <span>Reutilizar una tarea</span>
        <small>Importación, preparación y validación guardadas</small>
      </summary>
      <div className="load-secondary__content">
        {reusableTasks.catalogState === "loading" && (
          <p className="recipe-hint" role="status">Cargando tareas guardadas…</p>
        )}
        {reusableTasks.catalogState === "unavailable" && (
          <p className="recipe-hint" role="status">Las tareas reutilizables están disponibles en la aplicación de escritorio.</p>
        )}
        {reusableTasks.catalogState === "error" && (
          <p className="recipe-error" role="alert">No se pudieron cargar las tareas guardadas.</p>
        )}
        {reusableTasks.error && <p className="recipe-error" role="alert">{reusableTasks.error}</p>}

        {reusableTasks.catalogState === "ready" && (
          <section aria-label="Tareas guardadas">
            {reusableTasks.tasks.length === 0 ? (
              <p className="recipe-hint">Todavía no hay tareas guardadas en este equipo.</p>
            ) : (
              <div className="reusable-task-panel__select-row">
                <label htmlFor="reusable-task-select">Tarea guardada</label>
                <select
                  id="reusable-task-select"
                  value={selectedTaskId}
                  onChange={(event) => {
                    const taskId = event.target.value;
                    setSelectedTaskId(taskId);
                    setReviewedSchema(null);
                    setSaveMessage(null);
                    reusableTasks.clearError();
                    if (taskId) void openAndReviewTask(taskId);
                  }}
                  disabled={blocked || reusableTasks.isBusy}
                >
                  <option value="">Selecciona una tarea</option>
                  {reusableTasks.tasks.map((task) => (
                    <option key={task.id} value={task.id}>{task.name}</option>
                  ))}
                </select>
              </div>
            )}

            {selectedTaskId && reusableTasks.workingAction === "open" && (
              <p className="recipe-hint" role="status">Abriendo y revisando la tarea guardada…</p>
            )}
            {selectedTaskId && reusableTasks.workingAction === "check_schema" && (
              <p className="recipe-hint" role="status">Comparando el esquema de entrada…</p>
            )}

            {selectedTaskId && schema === null && (
              <p className="recipe-hint" role="status">
                Abre un archivo para comprobar su esquema antes de usar esta tarea.
              </p>
            )}
            {selectedTaskId && schema !== null && !reviewIsCurrent && (
              <p className="recipe-hint" role="status">
                {openedTask
                  ? "El esquema cambió desde la última revisión. Vuelve a revisar la tarea."
                  : "Revisa la tarea para comparar su esquema con el archivo actual."}
              </p>
            )}
            {reviewIsCurrent && checkedCompatibility?.status === "ready" && (
              <p className="notice notice--success" role="status">
                El esquema es compatible. Puedes aplicar la configuración guardada.
              </p>
            )}
            {reviewIsCurrent && checkedCompatibility?.status === "review_required" && (
              <div className="notice notice--error" role="alert">
                <strong>El esquema cambió; revisa las diferencias antes de continuar.</strong>
                <ul className="reusable-task-panel__differences">
                  {checkedCompatibility.missingColumns.map((column) => (
                    <li key={`missing-${column}`}>Falta la columna “{column}”.</li>
                  ))}
                  {checkedCompatibility.addedColumns.map((column) => (
                    <li key={`added-${column}`}>Se agregó la columna “{column}”.</li>
                  ))}
                  {checkedCompatibility.changedTypes.map((change) => (
                    <li key={`type-${change.column}`}>
                      “{change.column}” cambió de {change.expected} a {change.actual}.
                    </li>
                  ))}
                  {checkedCompatibility.orderChanged && <li>Cambió el orden de las columnas.</li>}
                </ul>
              </div>
            )}

            {selectedTaskId && (
              <button
                type="button"
                className="primary-action"
                onClick={applySelectedTask}
                disabled={!canApply}
              >
                Usar esta configuración
              </button>
            )}

            <form className="project-save reusable-task-panel__save" onSubmit={(event) => void saveCurrentTask(event)}>
              <label htmlFor="reusable-task-name">Guardar configuración actual</label>
              <div>
                <input
                  id="reusable-task-name"
                  value={taskName}
                  maxLength={128}
                  onChange={(event) => {
                    setTaskName(event.target.value);
                    setSaveMessage(null);
                    reusableTasks.clearError();
                  }}
                  placeholder="Nombre de la tarea"
                  disabled={!draft || blocked || reusableTasks.isBusy}
                  aria-describedby="reusable-task-name-help"
                />
                <button
                  type="submit"
                  className="secondary-action"
                  disabled={!draft || !taskName.trim() || blocked || reusableTasks.isBusy}
                >
                  {reusableTasks.workingAction === "save" ? "Guardando…" : "Guardar"}
                </button>
              </div>
              <small id="reusable-task-name-help">
                {draft
                  ? "Se guardan las opciones de importación, preparación, calidad y salida."
                  : "Carga un archivo y configura su flujo para guardar una tarea."}
              </small>
            </form>
            {saveMessage && <p className="notice notice--success" role="status">{saveMessage}</p>}
          </section>
        )}
      </div>
    </details>
  );
}
