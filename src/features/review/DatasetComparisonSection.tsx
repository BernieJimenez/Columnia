import { useEffect, useState } from "react";

import { cancelOperation } from "../../bridge";
import type {
  ComparisonOptions,
  ConflictResolution,
  ConflictSource,
  DatasetColumn,
  DatasetJoinType,
} from "../../bridge";
import type { ComparisonStatus } from "./compareModel";
import type { JoinStatus, ReviewMutationStatus } from "./joinModel";
import { formatDataType } from "../../format";
import { errorMessage } from "../../bridge/errors";

const CONFLICT_PAGE_SIZE = 50;
const EXACT_COMPARISON: ComparisonOptions = { numericTolerance: false, ignoreCase: false, trimSpaces: false };

/** PROD-21: the options as the result reads them. */
function comparisonOptionsSummary(options: ComparisonOptions | undefined): string | null {
  if (!options) return null;
  const parts = [
    options.numericTolerance ? "tolerando el redondeo de los decimales" : null,
    options.ignoreCase ? "ignorando mayúsculas" : null,
    options.trimSpaces ? "ignorando los espacios al principio y al final" : null,
  ].filter((part): part is string => part !== null);
  if (parts.length === 0) return null;
  const last = parts.pop() as string;
  return `Se comparó ${parts.length > 0 ? `${parts.join(", ")} e ${last}` : last}.`;
}

export function DatasetComparisonSection({
  status,
  datasetColumns,
  datasetRevision,
  keyColumns,
  onKeyColumnsChange,
  options = EXACT_COMPARISON,
  onOptionsChange = () => undefined,
  onCompare,
  onCancelComparison,
  comparisonCancellationPending,
  onClear,
  onConsolidate,
  onResolveConflicts,
  onConflictPageChange,
  joinStatus,
  reviewMutationStatus,
  reviewMutationCancellationPending,
  joinType,
  onJoinTypeChange,
  onJoin,
  onCancelReviewMutation,
}: {
  status: ComparisonStatus;
  datasetColumns: DatasetColumn[];
  datasetRevision: number;
  keyColumns: string[];
  onKeyColumnsChange: (columns: string[]) => void;
  options?: ComparisonOptions;
  onOptionsChange?: (options: ComparisonOptions) => void;
  onCompare: () => void;
  onCancelComparison: () => void;
  comparisonCancellationPending: boolean;
  onClear: () => void;
  onConsolidate: () => void;
  onResolveConflicts: (decisions: ConflictResolution[]) => void;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  joinStatus: JoinStatus;
  reviewMutationStatus: ReviewMutationStatus;
  reviewMutationCancellationPending: boolean;
  joinType: DatasetJoinType;
  onJoinTypeChange: (joinType: DatasetJoinType) => void;
  onJoin: (joinType: DatasetJoinType) => void;
  onCancelReviewMutation: () => void;
}) {
  const [conflictChoices, setConflictChoices] = useState<Record<string, ConflictSource>>({});
  const [excludedConflictIndexes, setExcludedConflictIndexes] = useState<Record<number, true>>({});
  const [conflictPageLoading, setConflictPageLoading] = useState(false);
  const [conflictPageCancellationPending, setConflictPageCancellationPending] = useState(false);
  const [conflictPageCancellationError, setConflictPageCancellationError] = useState<string | null>(null);
  const comparisonKeyColumnsKey = JSON.stringify(keyColumns);
  const comparedFileName = status.kind === "ready" ? status.comparison.comparedFileName : null;
  // UX-20: the source for every conflict not decided one by one, also on
  // pages not opened yet.
  const [remainingConflictsSource, setRemainingConflictsSource] = useState<ConflictSource | null>(null);
  useEffect(() => {
    setConflictChoices({});
    setExcludedConflictIndexes({});
    setRemainingConflictsSource(null);
    // Any of these makes the choices on screen belong to another comparison.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status.kind, comparedFileName, comparisonKeyColumnsKey, datasetRevision]);

  function conflictChoiceKey(conflictIndex: number, column: string): string {
    return `${conflictIndex}:${column}`;
  }

  const visibleConflictResolutions = status.kind === "ready"
    ? status.comparison.conflicts.map((conflict, conflictIndex) => {
        const globalConflictIndex = status.comparison.conflictOffset + conflictIndex;
        return excludedConflictIndexes[globalConflictIndex] === true
          || conflict.cells.every((cell) => conflictChoices[conflictChoiceKey(globalConflictIndex, cell.column)] !== undefined);
      })
    : [];
  const excludedConflictCount = Object.keys(excludedConflictIndexes).length;
  const reviewMutationBusy = reviewMutationCancellationPending
    || reviewMutationStatus.kind === "running"
    || reviewMutationStatus.kind === "finalizing";
  const activeReviewMutation = reviewMutationStatus.kind === "running"
    || reviewMutationStatus.kind === "finalizing"
    ? reviewMutationStatus.mutation
    : null;
  const reviewMutationFinalizing = reviewMutationStatus.kind === "finalizing";
  const reviewMutationCancellationRequested = reviewMutationStatus.kind === "running"
    && reviewMutationStatus.cancellation === "requested";
  const visibleConflictPageComplete = visibleConflictResolutions.length > 0
    && visibleConflictResolutions.every(Boolean);

  function chooseConflictSource(conflictIndex: number, column: string, source: ConflictSource) {
    setExcludedConflictIndexes((current) => {
      if (current[conflictIndex] !== true) return current;
      const next = { ...current };
      delete next[conflictIndex];
      return next;
    });
    setConflictChoices((current) => ({
      ...current,
      [conflictChoiceKey(conflictIndex, column)]: source,
    }));
  }

  /** UX-20: one source for every cell of the conflicts on this page. */
  function chooseSourceForVisiblePage(source: ConflictSource) {
    if (status.kind !== "ready") return;
    const { conflicts, conflictOffset } = status.comparison;
    setConflictChoices((current) => {
      const next = { ...current };
      conflicts.forEach((conflict, conflictIndex) => {
        const globalConflictIndex = conflictOffset + conflictIndex;
        if (excludedConflictIndexes[globalConflictIndex] === true) return;
        for (const cell of conflict.cells) next[conflictChoiceKey(globalConflictIndex, cell.column)] = source;
      });
      return next;
    });
  }

  function chooseConflictExclusion(conflictIndex: number, excluded: boolean) {
    setExcludedConflictIndexes((current) => {
      if (excluded) return { ...current, [conflictIndex]: true };
      if (current[conflictIndex] !== true) return current;
      const next = { ...current };
      delete next[conflictIndex];
      return next;
    });
    if (excluded) {
      const prefix = `${conflictIndex}:`;
      setConflictChoices((current) => Object.fromEntries(
        Object.entries(current).filter(([key]) => !key.startsWith(prefix)),
      ));
    }
  }

  function selectedConflictDecisions(): ConflictResolution[] {
    const exclusions: ConflictResolution[] = Object.keys(excludedConflictIndexes).map((index) => ({
      action: "exclude",
      conflictIndex: Number(index),
    }));
    const sourceChoices: ConflictResolution[] = Object.entries(conflictChoices).map(([choiceKey, source]) => {
      const separator = choiceKey.indexOf(":");
      const conflictIndex = Number(choiceKey.slice(0, separator));
      const column = choiceKey.slice(separator + 1);
      return { action: "useSource", conflictIndex, column, source };
    });
    if (remainingConflictsSource === null || status.kind !== "ready") return [...exclusions, ...sourceChoices];
    // UX-20: a cell left open on this page takes the chosen source, and every
    // conflict without decisions is resolved as a whole row.
    const { conflicts, conflictOffset, conflictingKeyCount } = status.comparison;
    conflicts.forEach((conflict, conflictIndex) => {
      const globalConflictIndex = conflictOffset + conflictIndex;
      const decided = conflict.cells.some((cell) => conflictChoices[conflictChoiceKey(globalConflictIndex, cell.column)] !== undefined);
      if (!decided || excludedConflictIndexes[globalConflictIndex] === true) return;
      for (const cell of conflict.cells) {
        if (conflictChoices[conflictChoiceKey(globalConflictIndex, cell.column)] === undefined) {
          sourceChoices.push({ action: "useSource", conflictIndex: globalConflictIndex, column: cell.column, source: remainingConflictsSource });
        }
      }
    });
    const withDecisions = new Set(sourceChoices.map((choice) => choice.conflictIndex));
    const wholeRows: ConflictResolution[] = [];
    for (let conflictIndex = 0; conflictIndex < conflictingKeyCount; conflictIndex += 1) {
      if (excludedConflictIndexes[conflictIndex] === true || withDecisions.has(conflictIndex)) continue;
      wholeRows.push({ action: "useSource", conflictIndex, source: remainingConflictsSource });
    }
    return [...exclusions, ...sourceChoices, ...wholeRows];
  }

  async function requestConflictPage(offset: number) {
    setConflictPageLoading(true);
    setConflictPageCancellationPending(false);
    setConflictPageCancellationError(null);
    try {
      await onConflictPageChange(offset);
    } finally {
      setConflictPageLoading(false);
      setConflictPageCancellationPending(false);
    }
  }

  async function cancelConflictPage() {
    if (!conflictPageLoading || conflictPageCancellationPending) return;
    setConflictPageCancellationPending(true);
    setConflictPageCancellationError(null);
    try {
      await cancelOperation("datasetComparison");
    } catch (error: unknown) {
      setConflictPageCancellationError(errorMessage(error));
      setConflictPageCancellationPending(false);
    }
  }

  return (
    <section className="phase-section comparison-section" aria-labelledby="comparison-title">
      <div className="section-heading">
        <div>
          <p className="step">Comparar archivos</p>
          <h3 id="comparison-title">Comparar datasets</h3>
        </div>
        <button
          type="button"
          onClick={status.kind === "loading" ? onCancelComparison : onCompare}
          disabled={reviewMutationBusy || (status.kind === "loading" && comparisonCancellationPending)}
        >
          {status.kind === "loading"
            ? comparisonCancellationPending ? "Esperando cancelación…" : "Cancelar comparación"
            : "Elegir dataset para comparar"}
        </button>
      </div>
      <p className="profile-note">
        Compara las filas de ambos archivos. Tu dataset actual no cambia hasta que decidas consolidar.
      </p>
      <fieldset className="comparison-key-selector">
        <legend>Claves explícitas (opcional)</legend>
        <p>
          Selecciona una o varias columnas para detectar claves nuevas, duplicadas y conflictos.
          Sin claves, se comparan las filas completas.
        </p>
        <div className="comparison-key-options">
          {datasetColumns.map((column) => (
            <label key={column.name}>
              <input
                type="checkbox"
                checked={keyColumns.includes(column.name)}
                disabled={reviewMutationBusy || status.kind === "loading" || status.kind === "ready"}
                onChange={() => {
                  onKeyColumnsChange(
                    keyColumns.includes(column.name)
                      ? keyColumns.filter((name) => name !== column.name)
                      : [...keyColumns, column.name],
                  );
                }}
              />
              <span>
                <strong>{column.name}</strong>
                <small>{formatDataType(column.dataType)}</small>
              </span>
            </label>
          ))}
        </div>
        {/* UX-05: the key of a visible result cannot change under it. */}
        {status.kind === "ready" && (
          <p className="comparison-key-status">
            Para cambiar la clave, descarta antes la comparación: el resultado y las decisiones de conflictos son de la clave actual.
          </p>
        )}
        {keyColumns.length > 0 && (
          <p className="comparison-key-status" role="status">
            Se comparará por: <strong>{keyColumns.join(", ")}</strong>
          </p>
        )}
      </fieldset>
      <fieldset className="comparison-key-selector">
        <legend>Qué cuenta como igual</legend>
        <p>Sin opciones, dos valores son iguales solo si coinciden exactamente. Se aplican a las claves y a los valores.</p>
        <div className="comparison-key-options">
          {([
            ["numericTolerance", "Tolerar el redondeo de los decimales", "0,1 + 0,2 = 0,3 y 1 = 1,0"],
            ["ignoreCase", "Ignorar mayúsculas", "Hola = hola"],
            ["trimSpaces", "Ignorar espacios al principio y al final", "«Ana » = «Ana»"],
          ] as const).map(([option, label, example]) => (
            <label key={option}>
              <input
                type="checkbox"
                checked={options[option]}
                disabled={reviewMutationBusy || status.kind === "loading" || status.kind === "ready"}
                onChange={() => onOptionsChange({ ...options, [option]: !options[option] })}
              />
              <span>
                <strong>{label}</strong>
                <small>{example}</small>
              </span>
            </label>
          ))}
        </div>
        {status.kind === "ready" && comparisonOptionsSummary(status.comparison.options) && (
          <p className="comparison-key-status" role="status">{comparisonOptionsSummary(status.comparison.options)}</p>
        )}
      </fieldset>
      {keyColumns.length > 0 && (
        <fieldset className="join-selector">
          <legend>Unir datasets por clave</legend>
          <p>Elige la relación y después selecciona la segunda fuente local.</p>
          <div className="join-options">
            {([
              ["inner", "Inner", "Solo filas con clave en ambos datasets."],
              ["left", "Left", "Conserva todas las filas del dataset activo."],
              ["full", "Full", "Conserva las filas de ambos datasets."],
            ] as const).map(([value, label, description]) => (
              <label key={value}>
                <input
                  type="radio"
                  name="dataset-join-type"
                  value={value}
                  checked={joinType === value}
                  onChange={() => onJoinTypeChange(value)}
                  disabled={reviewMutationBusy}
                />
                <span>
                  <strong>{label}</strong>
                  <small>{description}</small>
                </span>
              </label>
            ))}
          </div>
          <button
            type="button"
            className="primary-action"
            onClick={() => activeReviewMutation === "join" ? onCancelReviewMutation() : onJoin(joinType)}
            disabled={
              // UX-16: a join during a comparison in progress did nothing.
              (status.kind === "loading" && activeReviewMutation !== "join")
              || (reviewMutationBusy
                && (activeReviewMutation !== "join"
                  || reviewMutationFinalizing
                  || reviewMutationCancellationRequested))
            }
          >
          {reviewMutationCancellationPending
            ? "Esperando cancelación…"
            : activeReviewMutation === "join"
              ? reviewMutationStatus.kind === "finalizing"
                ? "Finalizando unión…"
                : reviewMutationStatus.kind === "running" && reviewMutationStatus.cancellation === "requested"
                  ? "Cancelando unión…"
                  : "Cancelar unión"
              : activeReviewMutation === "consolidate"
                ? "Esperando consolidación…"
                : activeReviewMutation === "resolveConflicts"
                  ? "Esperando resolución…"
                  : "Elegir fuente y unir"}
          </button>
        </fieldset>
      )}
      {joinStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudieron unir los datasets: {joinStatus.message}
        </p>
      )}
      {status.kind === "loading" && (
        <>
          {status.cancellationError && (
            <p className="notice notice--error" role="alert">
              No se pudo solicitar la cancelación: {status.cancellationError}
            </p>
          )}
          <p className="notice" role="status">
            {comparisonCancellationPending
              ? "Cancelación solicitada. Si el selector de archivos sigue abierto, ciérralo para terminar."
              : "Leyendo la segunda fuente local…"}
          </p>
        </>
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo comparar la fuente: {status.message}
        </p>
      )}
      {reviewMutationStatus.kind === "running" && reviewMutationStatus.cancellationError && (
        <p className="notice notice--error" role="alert">
          No se pudo solicitar la cancelación: {reviewMutationStatus.cancellationError}
        </p>
      )}
      {reviewMutationStatus.kind === "error" && reviewMutationStatus.mutation === "consolidate" && (
        <p className="notice notice--error" role="alert">
          No se pudieron consolidar las filas: {reviewMutationStatus.message}
        </p>
      )}
      {reviewMutationStatus.kind === "error" && reviewMutationStatus.mutation === "resolveConflicts" && (
        <p className="notice notice--error" role="alert">
          No se pudieron resolver los conflictos: {reviewMutationStatus.message}
        </p>
      )}
      {status.kind === "ready" && (
        <>
          <p className="comparison-source" aria-live="polite">
            <strong>{status.comparison.currentFileName}</strong>
            <span aria-hidden="true"> ↔ </span>
            <strong>{status.comparison.comparedFileName}</strong>
          </p>
          {status.comparison.comparedSourceNote && (
            <p className="comparison-key-status">{status.comparison.comparedSourceNote}</p>
          )}
          <dl className="quality-summary" aria-label="Resumen de comparación">
            <div><dt>Filas compartidas</dt><dd>{status.comparison.commonRowCount.toLocaleString()}</dd></div>
            <div><dt>Solo en el activo</dt><dd>{status.comparison.currentOnlyRowCount.toLocaleString()}</dd></div>
            <div><dt>Solo en el comparado</dt><dd>{status.comparison.comparedOnlyRowCount.toLocaleString()}</dd></div>
          </dl>
          <div className="comparison-columns" aria-label="Resultado de columnas">
            <div>
              <h4>Columnas compartidas ({status.comparison.sharedColumns.length})</h4>
              <p>{status.comparison.sharedColumns.join(", ") || "Ninguna"}</p>
            </div>
            <div>
              <h4>Solo en el activo ({status.comparison.currentOnlyColumns.length})</h4>
              <p>{status.comparison.currentOnlyColumns.join(", ") || "Ninguna"}</p>
            </div>
            <div>
              <h4>Solo en el comparado ({status.comparison.comparedOnlyColumns.length})</h4>
              <p>{status.comparison.comparedOnlyColumns.join(", ") || "Ninguna"}</p>
            </div>
          </div>
          {status.comparison.keyColumns.length > 0 && (
            <div className="comparison-key-summary" aria-label="Resultado de comparación por clave">
              <h4>Resultado por clave</h4>
              <p className="comparison-key-summary__columns">
                Claves: <strong>{status.comparison.keyColumns.join(", ")}</strong>
              </p>
              <dl className="quality-summary">
                <div><dt>Claves coincidentes</dt><dd>{status.comparison.matchedKeyCount.toLocaleString()}</dd></div>
                <div><dt>Solo en el activo</dt><dd>{status.comparison.currentOnlyKeyCount.toLocaleString()}</dd></div>
                <div><dt>Solo en el comparado</dt><dd>{status.comparison.comparedOnlyKeyCount.toLocaleString()}</dd></div>
                <div><dt>Conflictos</dt><dd>{status.comparison.conflictingKeyCount.toLocaleString()}</dd></div>
                <div><dt>Claves duplicadas</dt><dd>{status.comparison.duplicateKeyCount.toLocaleString()}</dd></div>
              </dl>
              {(status.comparison.duplicateKeyRowCount ?? 0) > 0 && (
                <p className="notice" role="note">
                  {(status.comparison.duplicateKeyRowCount ?? 0).toLocaleString()} filas tienen una clave repetida en alguno de los dos datasets y no se comparan: no aparecen como conflicto aunque sus valores difieran. Quita los duplicados o elige más columnas clave.
                </p>
              )}
            </div>
          )}
          {status.comparison.conflicts.length > 0 && (
            <section className="conflict-resolution" aria-labelledby="conflict-resolution-title">
              <div className="conflict-resolution__heading">
                <div>
                  <p className="step">Decisión explícita</p>
                  <h4 id="conflict-resolution-title">Resolver conflictos por clave</h4>
                  <p>Elige el origen de cada celda divergente o excluye una clave del resultado. No se modifica nada hasta confirmar todas las decisiones.</p>
                  <div className="conflict-resolution__bulk" role="group" aria-label="Decidir varios conflictos a la vez">
                    <button type="button" onClick={() => chooseSourceForVisiblePage("current")} disabled={reviewMutationBusy}>
                      Conservar el activo en esta página
                    </button>
                    <button type="button" onClick={() => chooseSourceForVisiblePage("compared")} disabled={reviewMutationBusy}>
                      Usar el comparado en esta página
                    </button>
                    <button
                      type="button"
                      aria-pressed={remainingConflictsSource === "current"}
                      onClick={() => setRemainingConflictsSource((current) => current === "current" ? null : "current")}
                      disabled={reviewMutationBusy}
                    >
                      Conservar el activo en los demás conflictos
                    </button>
                    <button
                      type="button"
                      aria-pressed={remainingConflictsSource === "compared"}
                      onClick={() => setRemainingConflictsSource((current) => current === "compared" ? null : "compared")}
                      disabled={reviewMutationBusy}
                    >
                      Usar el comparado en los demás conflictos
                    </button>
                  </div>
                  {remainingConflictsSource !== null && (
                    <p role="status">
                      Los {status.comparison.conflictingKeyCount.toLocaleString()} conflictos que no decidas uno a uno usarán
                      {remainingConflictsSource === "current" ? " el valor del activo" : " el valor del comparado"} en todas sus columnas.
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  className="primary-action"
                  onClick={() => activeReviewMutation === "resolveConflicts"
                    ? onCancelReviewMutation()
                    : onResolveConflicts(selectedConflictDecisions())}
                  disabled={reviewMutationBusy
                    ? activeReviewMutation !== "resolveConflicts"
                      || reviewMutationFinalizing
                      || reviewMutationCancellationRequested
                    : conflictPageLoading
                      || (remainingConflictsSource === null && (status.comparison.conflictsTruncated || !visibleConflictPageComplete))}
                >
                  {reviewMutationCancellationPending
                    ? "Esperando cancelación…"
                    : activeReviewMutation === "resolveConflicts"
                      ? reviewMutationFinalizing
                        ? "Finalizando resolución…"
                        : reviewMutationCancellationRequested
                          ? "Cancelando resolución…"
                          : "Cancelar resolución"
                      : activeReviewMutation === "join"
                        ? "Esperando unión…"
                        : activeReviewMutation === "consolidate"
                          ? "Esperando consolidación…"
                          : "Resolver conflictos"}
                </button>
              </div>
              {excludedConflictCount > 0 && (
                <p className="notice" role="status">
                  {excludedConflictCount} {excludedConflictCount === 1 ? "fila activa se excluirá" : "filas activas se excluirán"} del resultado.
                </p>
              )}
              {activeReviewMutation === "resolveConflicts" && (
                <p className="notice" role="status" aria-live="polite">
                  {reviewMutationCancellationPending
                    ? "Enviando la solicitud de cancelación de la resolución."
                    : reviewMutationCancellationRequested
                      ? "Cancelación solicitada. Esperando a que la resolución se detenga sin cambiar el dataset."
                      : reviewMutationFinalizing
                        ? "La resolución terminó; se está publicando el resultado validado."
                        : "Resolviendo conflictos. Puedes cancelar mientras se calcula el resultado."}
                </p>
              )}
              {status.comparison.conflicts.map((conflict, conflictIndex) => {
                const globalConflictIndex = status.comparison.conflictOffset + conflictIndex;
                return (
                <fieldset className="conflict-resolution__item" key={globalConflictIndex}>
                  <legend>
                    Conflicto {globalConflictIndex + 1} · clave {conflict.key.map(cellText).join(" · ")}
                  </legend>
                  <div className="conflict-resolution__choices">
                    <label>
                      <input
                        type="checkbox"
                        aria-label={`Excluir la fila activa de la clave ${conflict.key.map(cellText).join(" · ")}`}
                        checked={excludedConflictIndexes[globalConflictIndex] === true}
                        disabled={reviewMutationBusy}
                        onChange={(event) => chooseConflictExclusion(globalConflictIndex, event.currentTarget.checked)}
                      />
                      Excluir la fila activa de esta clave del resultado
                    </label>
                  </div>
                  <p>La fila de esta clave se quitará del dataset activo; no se agrega la versión comparada.</p>
                  <ul>
                    {conflict.cells.map((cell) => {
                      const choiceKey = conflictChoiceKey(globalConflictIndex, cell.column);
                      return (
                        <li key={cell.column}>
                          <strong>{cell.column}</strong>
                          <span>Activo: <ConflictValue value={cell.current} /></span>
                          <span>Comparado: <ConflictValue value={cell.compared} /></span>
                          <div className="conflict-resolution__choices">
                            <label>
                              <input
                                type="radio"
                                name={`conflict-${conflictIndex}-${cell.column}`}
                                checked={conflictChoices[choiceKey] === "current"}
                                disabled={reviewMutationBusy || excludedConflictIndexes[globalConflictIndex] === true}
                                onChange={() => chooseConflictSource(globalConflictIndex, cell.column, "current")}
                              />
                              Conservar activo en {cell.column}
                            </label>
                            <label>
                              <input
                                type="radio"
                                name={`conflict-${conflictIndex}-${cell.column}`}
                                checked={conflictChoices[choiceKey] === "compared"}
                                disabled={reviewMutationBusy || excludedConflictIndexes[globalConflictIndex] === true}
                                onChange={() => chooseConflictSource(globalConflictIndex, cell.column, "compared")}
                              />
                              Usar comparado en {cell.column}
                            </label>
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                </fieldset>
                );
              })}
              {status.comparison.conflictsTruncated && (
                <p className="notice" role="status">
                  {visibleConflictPageComplete
                    ? "Esta página está completa. Avanza para revisar los siguientes conflictos antes de resolverlos."
                    : "Faltan decisiones en esta página. Elige una fuente para cada celda y avanza para revisar los siguientes conflictos."}
                </p>
              )}
              {status.comparison.conflicts.length > 0 && (
                <nav className="conflict-resolution__pager" aria-label="Paginación de conflictos">
                  <button
                    type="button"
                    onClick={() => void requestConflictPage(Math.max(0, status.comparison.conflictOffset - CONFLICT_PAGE_SIZE))}
                    disabled={reviewMutationBusy || conflictPageLoading || status.comparison.conflictOffset === 0}
                  >
                    Conflictos anteriores
                  </button>
                  <span>
                    Conflictos {status.comparison.conflictOffset + 1}–{status.comparison.conflictOffset + status.comparison.conflicts.length}
                    {" de "}{status.comparison.conflictingKeyCount.toLocaleString()}
                  </span>
                  <button
                    type="button"
                    onClick={() => void requestConflictPage(status.comparison.conflictOffset + status.comparison.conflicts.length)}
                    disabled={reviewMutationBusy || conflictPageLoading || !status.comparison.conflictsTruncated || !visibleConflictPageComplete}
                  >
                    {conflictPageLoading ? "Cargando conflictos…" : "Siguientes conflictos"}
                  </button>
                  {conflictPageLoading && (
                    <button
                      type="button"
                      className="secondary-action"
                      onClick={() => void cancelConflictPage()}
                      disabled={conflictPageCancellationPending}
                    >
                      {conflictPageCancellationPending ? "Esperando cancelación…" : "Cancelar carga"}
                    </button>
                  )}
                </nav>
              )}
              {conflictPageCancellationError && (
                <p className="notice" role="alert">No se pudo cancelar la carga de conflictos: {conflictPageCancellationError}</p>
              )}
            </section>
          )}
          <div className="comparison-actions">
            <button
              type="button"
              className="secondary-action"
              onClick={onClear}
              disabled={reviewMutationBusy}
            >
              Descartar comparación
            </button>
            <button
              type="button"
              className="primary-action"
              onClick={() => activeReviewMutation === "consolidate" ? onCancelReviewMutation() : onConsolidate()}
              disabled={
                !status.comparison.canConsolidate
                || (reviewMutationBusy
                  && (activeReviewMutation !== "consolidate"
                    || reviewMutationFinalizing
                    || reviewMutationCancellationRequested))
              }
            >
              {reviewMutationCancellationPending
                ? "Esperando cancelación…"
                : activeReviewMutation === "consolidate"
                ? reviewMutationStatus.kind === "finalizing"
                  ? "Finalizando consolidación…"
                  : reviewMutationStatus.kind === "running" && reviewMutationStatus.cancellation === "requested"
                    ? "Cancelando consolidación…"
                    : "Cancelar consolidación"
                : activeReviewMutation === "join"
                  ? "Esperando unión…"
                  : activeReviewMutation === "resolveConflicts"
                    ? "Esperando resolución…"
                  : "Consolidar filas"}
            </button>
          </div>
          {!status.comparison.canConsolidate && (
            <p className="notice" role="note">
              {status.comparison.keyColumns.length > 0 && status.comparison.conflictingKeyCount > 0
                ? "La consolidación por clave está bloqueada porque existen conflictos de valores."
                : status.comparison.keyColumns.length > 0 && status.comparison.duplicateKeyCount > 0
                  ? "La consolidación por clave está bloqueada porque existen claves duplicadas."
                  : "La consolidación requiere las mismas columnas en el mismo orden y con los mismos tipos."}
            </p>
          )}
        </>
      )}
    </section>
  );
}

/** TXT-12: an empty cell and a missing one read differently from the text «null». */
function cellText(value: string | null): string {
  if (value === null) return "(nulo)";
  return value === "" ? "(vacío)" : value;
}

function ConflictValue({ value }: { value: string | null }) {
  return value === null || value === ""
    ? <em className="conflict-resolution__missing">{cellText(value)}</em>
    : <code>{value}</code>;
}
