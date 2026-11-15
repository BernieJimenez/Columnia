import { useState } from "react";

import { busyDescribedBy } from "../../busyReasons";
import type {
  ConflictResolution,
  ConflictSource,
  DatasetColumn,
  DatasetComparison,
  HistoryState,
} from "../../bridge";
import { plural } from "../../plural";
import {
  ComparisonSetup,
  ConflictPager,
  ConflictValue,
  cellText,
  comparisonMutationBusy,
} from "../review/DatasetComparisonSection";
import {
  JOIN_TYPE_OPTIONS,
  consolidationEstimate,
  joinEstimate,
  joinTypeLabel,
  type ReviewMutationKind,
} from "../review/joinModel";
import type { ReviewComparison } from "../review/useReviewController";

const MUTATION_NOUN: Record<ReviewMutationKind, string> = {
  join: "unión",
  consolidate: "consolidación",
  resolveConflicts: "resolución",
};

const MUTATION_PROGRESS: Record<ReviewMutationKind, string> = {
  join: "Uniendo los dos archivos. Puedes cancelar mientras se calcula el resultado.",
  consolidate: "Añadiendo las filas nuevas. Puedes cancelar mientras se calcula el resultado.",
  resolveConflicts: "Resolviendo los conflictos. Puedes cancelar mientras se calcula el resultado.",
};

/** What every combination sets aside: it belonged to the table before. */
const COMBINE_DISCARDED = "El contrato de Entregar, la receta de Transformaciones, el perfil de importación, el origen de la importación, el historial de Consulta SQL y el vínculo con el proyecto guardado.";

/** What «Deshacer» does after combining, as App restores it. */
function combineUndoText(snapshotsEnabled: boolean): string {
  return snapshotsEnabled
    ? "Cada acción queda como un paso del historial. «Deshacer» vuelve a las filas de antes y recupera lo descartado que no hayas vuelto a configurar después; el vínculo con el proyecto, si el proyecto sigue guardado."
    : "El historial está desactivado: no se podrá deshacer ni recuperar lo descartado. Guarda el proyecto antes si quieres conservar la versión actual.";
}

function sizeText(rowCount: number | null, columnCount: number): string {
  const columns = plural(columnCount, "columna", "columnas");
  return rowCount === null
    ? `hay claves repetidas, así que el número de filas se sabrá al unir; quedarán ${columns}`
    : `quedarán ${plural(rowCount, "fila", "filas")} y ${columns}`;
}

/**
 * Preparar: join with the file compared in Revisar, add its new rows or
 * resolve the conflicts. The section says once what any of them sets aside
 * and what «Deshacer» brings back; each action says beforehand, with figures,
 * what it will leave. The result is a step of the history like any other.
 */
export function CombineWithFileSection({
  comparison,
  datasetColumns,
  datasetRevision,
  historyStatus,
  busy,
}: {
  comparison: ReviewComparison;
  datasetColumns: DatasetColumn[];
  datasetRevision: number;
  historyStatus: HistoryState;
  /** Another Preparar change is running. */
  busy: boolean;
}) {
  const { status } = comparison;
  const mutationBusy = comparisonMutationBusy(comparison);
  const mutationStatus = comparison.mutationStatus ?? { kind: "idle" };
  const activeMutation = mutationStatus.kind === "running" || mutationStatus.kind === "finalizing"
    ? mutationStatus.mutation
    : null;
  const finalizing = mutationStatus.kind === "finalizing";
  const cancellationRequested = mutationStatus.kind === "running" && mutationStatus.cancellation === "requested";
  const cancellationPending = comparison.mutationCancellationPending ?? false;
  const anyBusy = busy || mutationBusy;

  /** Busy for this action: the button stays focusable and ignores the click. */
  function blockedFor(kind: ReviewMutationKind): boolean {
    if (busy || status.kind === "loading") return true;
    if (!mutationBusy) return false;
    return activeMutation !== kind || finalizing || cancellationRequested || cancellationPending;
  }

  function buttonLabel(kind: ReviewMutationKind, idleLabel: string): string {
    if (cancellationPending && activeMutation !== null) return "Esperando cancelación…";
    if (activeMutation === kind) {
      if (finalizing) return `Finalizando ${MUTATION_NOUN[kind]}…`;
      if (cancellationRequested) return `Cancelando ${MUTATION_NOUN[kind]}…`;
      return `Cancelar ${MUTATION_NOUN[kind]}`;
    }
    if (activeMutation !== null) return `Esperando ${MUTATION_NOUN[activeMutation]}…`;
    return idleLabel;
  }

  function run(kind: ReviewMutationKind, start: () => void) {
    if (blockedFor(kind)) return;
    if (activeMutation === kind) comparison.onCancelMutation?.();
    else start();
  }

  const progressText = activeMutation === null
    ? ""
    : cancellationPending
      ? "Enviando la solicitud de cancelación."
      : cancellationRequested
        ? "Cancelación solicitada. Esperando a que se detenga sin cambiar el dataset."
        : finalizing
          ? "Terminado; se está publicando el resultado."
          : MUTATION_PROGRESS[activeMutation];

  const errorText = comparison.joinStatus.kind === "error"
    ? `No se pudo unir: ${comparison.joinStatus.message}`
    : mutationStatus.kind === "error" && mutationStatus.mutation === "consolidate"
      ? `No se pudieron añadir las filas: ${mutationStatus.message}`
      : mutationStatus.kind === "error" && mutationStatus.mutation === "resolveConflicts"
        ? `No se pudieron resolver los conflictos: ${mutationStatus.message}`
        : mutationStatus.kind === "running" && mutationStatus.cancellationError
          ? `No se pudo solicitar la cancelación: ${mutationStatus.cancellationError}`
          : null;

  // The summary of the details already names the section: no repeated heading.
  return (
    <section className="combine-section" aria-label="Combinar con otro archivo">
      {status.kind !== "ready" ? (
        <>
          {comparison.notice && <p className="notice" role="note">{comparison.notice}</p>}
          <p className="combine-section__lead">
            Une tu dataset con otro archivo por una clave, añade sus filas nuevas o resuelve los valores que no
            coinciden. Primero se comparan los dos archivos; nada cambia hasta que confirmes una acción.
          </p>
          <ComparisonSetup comparison={comparison} datasetColumns={datasetColumns} primary={false} busy={anyBusy} />
        </>
      ) : (
        <>
          <div className="combine-section__source">
            <p>
              Comparado con <strong>{status.comparison.comparedFileName}</strong>{" "}
              {status.comparison.keyColumns.length > 0
                ? <>por <strong>{status.comparison.keyColumns.join(", ")}</strong></>
                : "por filas completas"}
              {": "}
              {plural(status.comparison.keyColumns.length > 0 ? status.comparison.comparedOnlyKeyCount : status.comparison.comparedOnlyRowCount, "fila nueva", "filas nuevas")},{" "}
              {plural(status.comparison.keyColumns.length > 0 ? status.comparison.currentOnlyKeyCount : status.comparison.currentOnlyRowCount, "fila que falta", "filas que faltan")}
              {status.comparison.keyColumns.length > 0 && <> y {plural(status.comparison.conflictingKeyCount, "conflicto", "conflictos")}</>}.
            </p>
            <button
              type="button"
              className="secondary-action"
              onClick={() => { if (!anyBusy) comparison.onClear(); }}
              aria-disabled={anyBusy || undefined}
              aria-describedby={busyDescribedBy(anyBusy)}
            >
              Descartar comparación
            </button>
          </div>
          {/* Said once for the three actions: any of them sets aside the same. */}
          <dl className="combine-summary" aria-label="Al combinar">
            <div><dt>Al combinar se descartan</dt><dd>{COMBINE_DISCARDED}</dd></div>
            <div><dt>Deshacer</dt><dd>{combineUndoText(historyStatus.snapshotsEnabled)}</dd></div>
          </dl>
          {errorText && <p className="notice notice--error" role="alert">{errorText}</p>}
          <p className="combine-status" role="status">{progressText}</p>
          <JoinAction
            comparison={status.comparison}
            joinType={comparison.joinType}
            onJoinTypeChange={comparison.onJoinTypeChange}
            locked={anyBusy}
            blocked={blockedFor("join")}
            label={buttonLabel("join", `Unir con ${status.comparison.comparedFileName}`)}
            onJoin={() => run("join", () => comparison.onJoin(comparison.joinType))}
          />
          <ConsolidateAction
            comparison={status.comparison}
            locked={anyBusy}
            blocked={blockedFor("consolidate")}
            label={buttonLabel(
              "consolidate",
              `Añadir ${plural(consolidationEstimate(status.comparison).addedRowCount, "fila nueva", "filas nuevas")}`,
            )}
            onConsolidate={() => run("consolidate", comparison.onConsolidate)}
          />
          {status.comparison.conflictingKeyCount > 0 && (
            <ConflictResolutionAction
              // Another file, key or revision makes the choices belong to
              // another comparison: they start again.
              key={`${status.comparison.comparedFileName}|${status.comparison.keyColumns.join(",")}|${datasetRevision}`}
              comparison={status.comparison}
              locked={anyBusy}
              blocked={blockedFor("resolveConflicts")}
              label={buttonLabel("resolveConflicts", "Resolver conflictos")}
              onResolve={(decisions) => run("resolveConflicts", () => comparison.onResolveConflicts(decisions))}
              onConflictPageChange={comparison.onConflictPageChange}
            />
          )}
        </>
      )}
    </section>
  );
}

/** «Qué pasará», with the figures of this action only. */
function ActionSummary({ what }: { what: string }) {
  return (
    <dl className="combine-summary">
      <div><dt>Qué pasará</dt><dd>{what}</dd></div>
    </dl>
  );
}

function ActionButton({ label, blocked, locked, onClick }: { label: string; blocked: boolean; locked: boolean; onClick: () => void }) {
  return (
    <div className="combine-action__footer">
      <button
        type="button"
        className="secondary-action"
        onClick={onClick}
        aria-disabled={blocked || undefined}
        aria-describedby={busyDescribedBy(locked)}
      >
        {label}
      </button>
    </div>
  );
}

function JoinAction({
  comparison,
  joinType,
  onJoinTypeChange,
  locked,
  blocked,
  label,
  onJoin,
}: {
  comparison: DatasetComparison;
  joinType: ReviewComparison["joinType"];
  onJoinTypeChange: ReviewComparison["onJoinTypeChange"];
  locked: boolean;
  blocked: boolean;
  label: string;
  onJoin: () => void;
}) {
  if (comparison.keyColumns.length === 0) {
    return (
      <section className="combine-action" aria-labelledby="combine-join-title">
        <h4 id="combine-join-title">Unir por clave</h4>
        <p>Para unir, descarta la comparación y compara de nuevo eligiendo al menos una columna clave.</p>
      </section>
    );
  }
  const estimate = joinEstimate(comparison, joinType);
  const what = `Unir con ${comparison.comparedFileName} por ${comparison.keyColumns.join(", ")}, ${joinTypeLabel(joinType).toLowerCase()}: ${sizeText(estimate.rowCount, estimate.columnCount)}.`;
  return (
    <section className="combine-action" aria-labelledby="combine-join-title">
      <h4 id="combine-join-title">Unir por clave</h4>
      <p>Añade a cada fila las columnas del otro archivo con la misma clave.</p>
      <fieldset className="join-selector">
        <legend>Qué filas quedan</legend>
        <div className="join-options">
          {JOIN_TYPE_OPTIONS.map((option) => (
            <label key={option.value}>
              <input
                type="radio"
                name="dataset-join-type"
                value={option.value}
                checked={joinType === option.value}
                onChange={() => onJoinTypeChange(option.value)}
                disabled={locked}
                aria-describedby={busyDescribedBy(locked)}
              />
              <span>
                <strong>{option.label}</strong>
                <small>{option.description}</small>
              </span>
            </label>
          ))}
        </div>
      </fieldset>
      <ActionSummary what={what} />
      <ActionButton label={label} blocked={blocked} locked={locked} onClick={onJoin} />
    </section>
  );
}

function ConsolidateAction({
  comparison,
  locked,
  blocked,
  label,
  onConsolidate,
}: {
  comparison: DatasetComparison;
  locked: boolean;
  blocked: boolean;
  label: string;
  onConsolidate: () => void;
}) {
  const keyed = comparison.keyColumns.length > 0;
  if (!comparison.canConsolidate) {
    return (
      <section className="combine-action" aria-labelledby="combine-consolidate-title">
        <h4 id="combine-consolidate-title">Añadir las filas nuevas</h4>
        <p className="notice" role="note">
          {keyed && comparison.conflictingKeyCount > 0
            ? "No se pueden añadir las filas mientras haya conflictos: resuélvelos antes."
            : keyed && comparison.duplicateKeyCount > 0
              ? "No se pueden añadir las filas porque hay claves repetidas: quítalas o elige más columnas clave."
              : "Para añadir las filas, los dos archivos necesitan las mismas columnas, en el mismo orden y con los mismos tipos."}
        </p>
      </section>
    );
  }
  const estimate = consolidationEstimate(comparison);
  const added = plural(estimate.addedRowCount, "fila", "filas");
  const what = keyed
    ? `Se añaden al final las ${added} de ${comparison.comparedFileName} cuya clave no está en tu dataset: ${sizeText(estimate.rowCount, estimate.columnCount)}.`
    : `Se añaden al final las ${added} de ${comparison.comparedFileName}, también las que ya tienes: ${sizeText(estimate.rowCount, estimate.columnCount)}.`;
  return (
    <section className="combine-action" aria-labelledby="combine-consolidate-title">
      <h4 id="combine-consolidate-title">Añadir las filas nuevas</h4>
      <ActionSummary what={what} />
      <ActionButton label={label} blocked={blocked} locked={locked} onClick={onConsolidate} />
    </section>
  );
}

function conflictChoiceKey(conflictIndex: number, column: string): string {
  return `${conflictIndex}:${column}`;
}

function ConflictResolutionAction({
  comparison,
  locked,
  blocked,
  label,
  onResolve,
  onConflictPageChange,
}: {
  comparison: DatasetComparison;
  locked: boolean;
  blocked: boolean;
  label: string;
  onResolve: (decisions: ConflictResolution[]) => void;
  onConflictPageChange: (offset: number) => void | Promise<void>;
}) {
  const [choices, setChoices] = useState<Record<string, ConflictSource>>({});
  const [excluded, setExcluded] = useState<Record<number, true>>({});
  // UX-20: the source for every conflict not decided one by one, also on
  // pages not opened yet.
  const [remainingSource, setRemainingSource] = useState<ConflictSource | null>(null);
  const [missingDecisions, setMissingDecisions] = useState<string | null>(null);

  const { conflicts, conflictOffset, conflictingKeyCount, conflictsTruncated } = comparison;
  const pageComplete = conflicts.length > 0 && conflicts.every((conflict, index) => {
    const globalIndex = conflictOffset + index;
    return excluded[globalIndex] === true
      || conflict.cells.every((cell) => choices[conflictChoiceKey(globalIndex, cell.column)] !== undefined);
  });
  const excludedCount = Object.keys(excluded).length;

  function choose(conflictIndex: number, column: string, source: ConflictSource) {
    setMissingDecisions(null);
    setChoices((current) => ({ ...current, [conflictChoiceKey(conflictIndex, column)]: source }));
  }

  function choosePage(source: ConflictSource) {
    setMissingDecisions(null);
    setChoices((current) => {
      const next = { ...current };
      conflicts.forEach((conflict, index) => {
        const globalIndex = conflictOffset + index;
        if (excluded[globalIndex] === true) return;
        for (const cell of conflict.cells) next[conflictChoiceKey(globalIndex, cell.column)] = source;
      });
      return next;
    });
  }

  function exclude(conflictIndex: number, value: boolean) {
    setMissingDecisions(null);
    setExcluded((current) => {
      if (value) return { ...current, [conflictIndex]: true };
      const next = { ...current };
      delete next[conflictIndex];
      return next;
    });
    if (value) {
      const prefix = `${conflictIndex}:`;
      setChoices((current) => Object.fromEntries(Object.entries(current).filter(([key]) => !key.startsWith(prefix))));
    }
  }

  function decisions(): ConflictResolution[] {
    const exclusions: ConflictResolution[] = Object.keys(excluded).map((index) => ({
      action: "exclude",
      conflictIndex: Number(index),
    }));
    const sourceChoices: ConflictResolution[] = Object.entries(choices).map(([choiceKey, source]) => {
      const separator = choiceKey.indexOf(":");
      return {
        action: "useSource",
        conflictIndex: Number(choiceKey.slice(0, separator)),
        column: choiceKey.slice(separator + 1),
        source,
      };
    });
    if (remainingSource === null) return [...exclusions, ...sourceChoices];
    // UX-20: a cell left open on this page takes the chosen source, and every
    // conflict without decisions is resolved as a whole row.
    conflicts.forEach((conflict, index) => {
      const globalIndex = conflictOffset + index;
      const decided = conflict.cells.some((cell) => choices[conflictChoiceKey(globalIndex, cell.column)] !== undefined);
      if (!decided || excluded[globalIndex] === true) return;
      for (const cell of conflict.cells) {
        if (choices[conflictChoiceKey(globalIndex, cell.column)] === undefined) {
          sourceChoices.push({ action: "useSource", conflictIndex: globalIndex, column: cell.column, source: remainingSource });
        }
      }
    });
    const withDecisions = new Set(sourceChoices.map((choice) => choice.conflictIndex));
    const wholeRows: ConflictResolution[] = [];
    for (let index = 0; index < conflictingKeyCount; index += 1) {
      if (excluded[index] === true || withDecisions.has(index)) continue;
      wholeRows.push({ action: "useSource", conflictIndex: index, source: remainingSource });
    }
    return [...exclusions, ...sourceChoices, ...wholeRows];
  }

  function resolve() {
    if (blocked) return;
    if (!locked && remainingSource === null && (conflictsTruncated || !pageComplete)) {
      setMissingDecisions(conflictsTruncated && pageComplete
        ? "Quedan conflictos en las páginas siguientes: avanza para decidirlos o elige qué hacer con los demás conflictos."
        : "Faltan decisiones: elige un valor para cada celda, quita la fila o elige qué hacer con los demás conflictos.");
      return;
    }
    onResolve(decisions());
  }

  const what = `Cada celda en conflicto toma el valor que elijas${excludedCount > 0 ? ` y se ${excludedCount === 1 ? "quita 1 fila" : `quitan ${excludedCount.toLocaleString("es-ES")} filas`}` : ""}: ${sizeText(comparison.currentRowCount - excludedCount, comparison.sharedColumns.length + comparison.currentOnlyColumns.length)}.`;

  return (
    <section className="combine-action conflict-resolution" aria-labelledby="conflict-resolution-title">
      <h4 id="conflict-resolution-title">Resolver conflictos</h4>
      <p>
        Hay {plural(conflictingKeyCount, "clave", "claves")} con valores distintos en los dos archivos. Elige qué valor
        se queda en cada celda o quita la fila.
      </p>
      <div className="conflict-resolution__bulk" role="group" aria-label="Decidir varios conflictos a la vez">
        <button type="button" className="secondary-action" onClick={() => { if (!locked) choosePage("current"); }} aria-disabled={locked || undefined} aria-describedby={busyDescribedBy(locked)}>
          Conservar tu dataset en esta página
        </button>
        <button type="button" className="secondary-action" onClick={() => { if (!locked) choosePage("compared"); }} aria-disabled={locked || undefined} aria-describedby={busyDescribedBy(locked)}>
          Usar el otro archivo en esta página
        </button>
        <button
          type="button"
          className="secondary-action"
          aria-pressed={remainingSource === "current"}
          onClick={() => { if (locked) return; setMissingDecisions(null); setRemainingSource((current) => current === "current" ? null : "current"); }}
          aria-disabled={locked || undefined}
          aria-describedby={busyDescribedBy(locked)}
        >
          Conservar tu dataset en los demás conflictos
        </button>
        <button
          type="button"
          className="secondary-action"
          aria-pressed={remainingSource === "compared"}
          onClick={() => { if (locked) return; setMissingDecisions(null); setRemainingSource((current) => current === "compared" ? null : "compared"); }}
          aria-disabled={locked || undefined}
          aria-describedby={busyDescribedBy(locked)}
        >
          Usar el otro archivo en los demás conflictos
        </button>
      </div>
      {remainingSource !== null && (
        <p>
          Los conflictos que no decidas uno a uno usarán
          {remainingSource === "current" ? " el valor de tu dataset" : " el valor del otro archivo"} en todas sus columnas.
        </p>
      )}
      {conflicts.map((conflict, index) => {
        const globalIndex = conflictOffset + index;
        const key = conflict.key.map(cellText).join(" · ");
        const isExcluded = excluded[globalIndex] === true;
        return (
          <fieldset className="conflict-resolution__item combine-conflict" key={globalIndex}>
            <legend>Conflicto {globalIndex + 1} · clave {key}</legend>
            <div className="combine-conflict__cells">
              {conflict.cells.map((cell) => {
                const choiceKey = conflictChoiceKey(globalIndex, cell.column);
                const name = `conflict-${globalIndex}-${cell.column}`;
                return (
                  <div key={cell.column} className="combine-conflict__cell" role="radiogroup" aria-label={cell.column}>
                    <span className="combine-conflict__column" aria-hidden="true">{cell.column}</span>
                    <label>
                      <input
                        type="radio"
                        name={name}
                        checked={choices[choiceKey] === "current"}
                        disabled={locked || isExcluded}
                        aria-describedby={busyDescribedBy(locked)}
                        onChange={() => choose(globalIndex, cell.column, "current")}
                      />
                      <span>Tu dataset: <ConflictValue value={cell.current} /></span>
                    </label>
                    <label>
                      <input
                        type="radio"
                        name={name}
                        checked={choices[choiceKey] === "compared"}
                        disabled={locked || isExcluded}
                        aria-describedby={busyDescribedBy(locked)}
                        onChange={() => choose(globalIndex, cell.column, "compared")}
                      />
                      <span>Otro archivo: <ConflictValue value={cell.compared} /></span>
                    </label>
                  </div>
                );
              })}
            </div>
            <label className="combine-conflict__exclude">
              <input
                type="checkbox"
                checked={isExcluded}
                disabled={locked}
                aria-describedby={busyDescribedBy(locked)}
                onChange={(event) => exclude(globalIndex, event.currentTarget.checked)}
              />
              Quitar esta fila del resultado
            </label>
            {isExcluded && (
              <p className="combine-conflict__excluded">
                La fila con esta clave se quitará de tu dataset; no se añade la del otro archivo.
              </p>
            )}
          </fieldset>
        );
      })}
      {conflictsTruncated && !pageComplete && remainingSource === null && (
        <p>Decide esta página para pasar a los siguientes conflictos.</p>
      )}
      <ConflictPager
        comparison={comparison}
        onConflictPageChange={onConflictPageChange}
        busy={locked}
        nextBlocked={!pageComplete}
      />
      <ActionSummary what={what} />
      {missingDecisions && <p className="notice" role="alert">{missingDecisions}</p>}
      <ActionButton label={label} blocked={blocked} locked={locked} onClick={resolve} />
    </section>
  );
}
