import { useMemo, useState, type FormEvent } from "react";

import type { ReusableTask, ReusableTaskSchema } from "../../bridge";
import { exceptionPolicyMatchesSchema } from "./reusableTaskExceptions";
import { useReusableTasks } from "./useReusableTasks";

interface ReusableTaskPanelProps {
  connected: boolean;
  blocked: boolean;
  schema: ReusableTaskSchema | null;
  draft: Omit<ReusableTask, "name"> | null;
  onApply: (task: ReusableTask) => void;
  onPrepareImport?: (taskId: string, task: ReusableTask) => void;
  pendingTaskId?: string | null;
  pendingTaskName?: string | null;
  onClearPendingImport?: () => void;
}

export function ReusableTaskPanel({
  connected,
  blocked,
  schema,
  draft,
  onApply,
  onPrepareImport = () => undefined,
  pendingTaskId = null,
  pendingTaskName = null,
  onClearPendingImport = () => undefined,
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
  const hasUnsupportedConversionAction = openedTask?.exceptionPolicy?.conversions.some(
    ({ onInvalid }) => onInvalid !== "review",
  ) ?? false;
  const canApply = !blocked
    && !reusableTasks.isBusy
    && openedTask !== null
    && exceptionPolicyMatchesSchema(openedTask.exceptionPolicy, openedTask.importProfile.schema)
    && !hasUnsupportedConversionAction
    && reviewIsCurrent
    && checkedCompatibility?.status === "ready";
  const canPrepareImport = !blocked && !reusableTasks.isBusy && openedTask !== null && !hasUnsupportedConversionAction;

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

  function prepareSelectedTask() {
    if (canPrepareImport && openedTask) onPrepareImport(selectedTaskId, openedTask);
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

            {openedTask && (
              <div className="sheet-import-summary" aria-label="Resumen de la tarea guardada">
                <h4>Configuración que se reutilizará</h4>
                <dl>
                  <div>
                    <dt>Perfil de entrada</dt>
                    <dd>
                      {openedTask.importProfile.format.toUpperCase()} · {openedTask.importProfile.schema.length} columnas ·
                      {" "}{openedTask.importProfile.headerMode === "generated"
                        ? "encabezados generados"
                        : openedTask.importProfile.headerMode === "firstRow"
                          ? "encabezados de la primera fila"
                          : "sin selección de encabezados"}
                    </dd>
                  </div>
                  <div>
                    <dt>Preparación</dt>
                    <dd>{openedTask.recipe?.name ?? "Sin receta guardada"}</dd>
                  </div>
                  {openedTask.exceptionPolicy && (
                    <div>
                      <dt>Decisiones de conversión</dt>
                      <dd>
                        {openedTask.exceptionPolicy.conversions.length} decisión{openedTask.exceptionPolicy.conversions.length === 1 ? "" : "es"} · base léxica ·
                        {" "}valores no interpretables requieren revisión
                      </dd>
                    </div>
                  )}
                  <div>
                    <dt>Validación y salida</dt>
                    <dd>
                      {openedTask.qualityRules.length} reglas · {openedTask.outputFormat.toUpperCase()} · privacidad {openedTask.privacyMode}
                    </dd>
                  </div>
                </dl>
                {openedTask.importProfile.schema.length > 0 && (
                  <p>Columnas esperadas: {openedTask.importProfile.schema.map((column) => column.name).join(", ")}</p>
                )}
                {openedTask.exceptionPolicy && (
                  <p className="recipe-hint">
                    Las conversiones quedan preseleccionadas como borrador. El esquema debe coincidir exactamente y solo se ejecutan al usar la acción actual de aplicar transformaciones.
                  </p>
                )}
                {hasUnsupportedConversionAction && (
                  <p className="recipe-error" role="alert">
                    Esta tarea contiene una decisión para valores no interpretables que todavía no está disponible en este flujo. No se aplicará la tarea.
                  </p>
                )}
              </div>
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
              <div className="reusable-task-panel__actions">
                <button
                  type="button"
                  className="primary-action"
                  onClick={prepareSelectedTask}
                  disabled={!canPrepareImport}
                >
                  {pendingTaskId === selectedTaskId ? "Tarea lista para importar" : "Preparar próxima importación"}
                </button>
                {schema !== null && (
                  <button
                    type="button"
                    className="secondary-action"
                    onClick={applySelectedTask}
                    disabled={!canApply}
                  >
                    Aplicar al dataset actual
                  </button>
                )}
              </div>
            )}

            {pendingTaskId && (
              <div className="notice notice--success reusable-task-panel__pending" role="status">
                <span>
                  Tarea “{pendingTaskName ?? reusableTasks.tasks.find((task) => task.id === pendingTaskId)?.name ?? "guardada"}” preparada. Su perfil se usará al elegir otro archivo; los demás ajustes se pedirán antes de aplicarse.
                </span>
                <button type="button" className="inline-action" onClick={onClearPendingImport} disabled={blocked}>
                  Quitar
                </button>
              </div>
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
