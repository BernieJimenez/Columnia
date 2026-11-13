import { useState } from "react";

import { busyDescribedBy } from "../../busyReasons";
import { cancelOperation } from "../../bridge";
import type { ComparisonOptions, DatasetColumn, DatasetComparison } from "../../bridge";
import { MissingValue } from "../../components/MissingValue";
import { formatDataType } from "../../format";
import { errorMessage } from "../../bridge/errors";
import type { ReviewComparison } from "./useReviewController";

export const CONFLICT_PAGE_SIZE = 50;
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

/** A mutation of Preparar is running or publishing its result. */
export function comparisonMutationBusy(comparison: ReviewComparison): boolean {
  const status = comparison.mutationStatus ?? { kind: "idle" };
  return comparison.mutationCancellationPending === true
    || status.kind === "running"
    || status.kind === "finalizing";
}

/**
 * Revisar: compare the active dataset with another file and look at the
 * result. Nothing here changes the dataset; joining, adding the new rows and
 * resolving conflicts happen in Preparar, as steps of its history.
 */
export function DatasetComparisonSection({
  comparison,
  datasetColumns,
  onCombineInPrepare,
}: {
  comparison: ReviewComparison;
  datasetColumns: DatasetColumn[];
  /** «Combinar en Preparar»: goes to Preparar with this comparison. */
  onCombineInPrepare?: () => void;
}) {
  const { status } = comparison;
  const busy = comparisonMutationBusy(comparison);
  return (
    <section className="phase-section comparison-section" aria-labelledby="comparison-title">
      <h3 id="comparison-title" className="comparison-step-title">
        {status.kind === "ready" ? "Resultado de la comparación" : "Elige la clave y el otro archivo"}
      </h3>
      {status.kind === "ready" ? (
        <>
          <ComparisonResult comparison={status.comparison} onConflictPageChange={comparison.onConflictPageChange} busy={busy} />
          <div className="comparison-next">
            <p>
              Tu dataset no ha cambiado. Para unir los dos archivos, añadir sus filas nuevas o resolver los
              valores distintos, sigue en Preparar: allí cada cambio queda en el historial.
            </p>
            <div className="comparison-actions">
              {onCombineInPrepare && (
                <button type="button" className="secondary-action" onClick={onCombineInPrepare} disabled={busy}>
                  Combinar en Preparar
                </button>
              )}
              <button
                type="button"
                className="secondary-action"
                onClick={comparison.onClear}
                disabled={busy}
                aria-describedby={busyDescribedBy(busy)}
              >
                Descartar comparación
              </button>
            </div>
          </div>
        </>
      ) : (
        <>
          <p className="profile-note">
            Compara tu dataset con otro archivo para ver las filas nuevas, las que faltan y los valores que no
            coinciden. Tu dataset no cambia.
          </p>
          <ComparisonSetup comparison={comparison} datasetColumns={datasetColumns} primary busy={busy} />
        </>
      )}
    </section>
  );
}

/**
 * The first step: key, what counts as equal and the file. `primary`: the
 * compare button is the main action of its view (Revisar), not of Preparar.
 */
export function ComparisonSetup({
  comparison,
  datasetColumns,
  primary,
  busy,
}: {
  comparison: ReviewComparison;
  datasetColumns: DatasetColumn[];
  primary: boolean;
  busy: boolean;
}) {
  const { status, keyColumns } = comparison;
  const options = comparison.options ?? EXACT_COMPARISON;
  const onOptionsChange = comparison.onOptionsChange ?? (() => undefined);
  const loading = status.kind === "loading";
  const cancellationPending = comparison.cancellationPending ?? false;
  const locked = busy || loading;
  return (
    <div className="comparison-setup">
      <fieldset className="comparison-key-selector">
        <legend>Columnas clave (opcional)</legend>
        <p>
          Con una clave se ven las filas nuevas, las que faltan y los valores distintos de cada fila.
          Sin clave se comparan las filas completas.
        </p>
        <div className="comparison-key-options">
          {datasetColumns.map((column) => (
            <label key={column.name}>
              <input
                type="checkbox"
                checked={keyColumns.includes(column.name)}
                disabled={locked}
                aria-describedby={busyDescribedBy(busy)}
                onChange={() => {
                  comparison.onKeyColumnsChange(
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
        {keyColumns.length > 0 && (
          <p className="comparison-key-status">
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
                disabled={locked}
                aria-describedby={busyDescribedBy(busy)}
                onChange={() => onOptionsChange({ ...options, [option]: !options[option] })}
              />
              <span>
                <strong>{label}</strong>
                <small>{example}</small>
              </span>
            </label>
          ))}
        </div>
      </fieldset>
      <div className="comparison-actions">
        <button
          type="button"
          className={primary && !loading ? "primary-action" : undefined}
          onClick={loading ? comparison.onCancelComparison : comparison.onCompare}
          disabled={busy || (loading && cancellationPending)}
          aria-describedby={busyDescribedBy(busy)}
        >
          {loading
            ? cancellationPending ? "Esperando cancelación…" : "Cancelar comparación"
            : "Comparar con otro archivo"}
        </button>
      </div>
      {status.kind === "loading" && status.cancellationError && (
        <p className="notice notice--error" role="alert">
          No se pudo solicitar la cancelación: {status.cancellationError}
        </p>
      )}
      {loading && (
        <p className="notice" role="status">
          {cancellationPending
            ? "Cancelación solicitada. Si el selector de archivos sigue abierto, ciérralo para terminar."
            : "Leyendo el otro archivo…"}
        </p>
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo comparar el archivo: {status.message}
        </p>
      )}
    </div>
  );
}

/** The second step: what the comparison found, read-only. */
export function ComparisonResult({
  comparison,
  onConflictPageChange,
  busy,
}: {
  comparison: DatasetComparison;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  busy: boolean;
}) {
  const keyed = comparison.keyColumns.length > 0;
  const optionsSummary = comparisonOptionsSummary(comparison.options);
  return (
    <div className="comparison-result">
      <p className="comparison-source">
        <strong>{comparison.currentFileName}</strong>
        <span aria-hidden="true"> ↔ </span>
        <span className="visually-hidden"> comparado con </span>
        <strong>{comparison.comparedFileName}</strong>
        {" · "}
        {keyed ? <>por <strong>{comparison.keyColumns.join(", ")}</strong></> : "por filas completas"}
      </p>
      {comparison.comparedSourceNote && <p className="comparison-key-status">{comparison.comparedSourceNote}</p>}
      {optionsSummary && <p className="comparison-key-status">{optionsSummary}</p>}
      <p className="comparison-key-status">Para cambiar la clave, descarta antes la comparación.</p>
      {keyed ? (
        <dl className="quality-summary comparison-counts" aria-label="Resultado por clave">
          <div><dt>Filas nuevas (solo en el otro archivo)</dt><dd>{comparison.comparedOnlyKeyCount.toLocaleString()}</dd></div>
          <div><dt>Filas que faltan (solo en tu dataset)</dt><dd>{comparison.currentOnlyKeyCount.toLocaleString()}</dd></div>
          <div><dt>Conflictos (misma clave, valores distintos)</dt><dd>{comparison.conflictingKeyCount.toLocaleString()}</dd></div>
          <div><dt>Coinciden</dt><dd>{comparison.matchedKeyCount.toLocaleString()}</dd></div>
          <div><dt>Claves repetidas</dt><dd>{comparison.duplicateKeyCount.toLocaleString()}</dd></div>
        </dl>
      ) : (
        <dl className="quality-summary comparison-counts" aria-label="Resultado por filas completas">
          <div><dt>Filas en los dos</dt><dd>{comparison.commonRowCount.toLocaleString()}</dd></div>
          <div><dt>Solo en tu dataset</dt><dd>{comparison.currentOnlyRowCount.toLocaleString()}</dd></div>
          <div><dt>Solo en el otro archivo</dt><dd>{comparison.comparedOnlyRowCount.toLocaleString()}</dd></div>
        </dl>
      )}
      {(comparison.duplicateKeyRowCount ?? 0) > 0 && (
        <p className="notice" role="note">
          {(comparison.duplicateKeyRowCount ?? 0).toLocaleString()} filas tienen una clave repetida en alguno de los dos archivos y no se comparan: no aparecen como conflicto aunque sus valores difieran. Quita los duplicados o elige más columnas clave.
        </p>
      )}
      <div className="comparison-columns" aria-label="Resultado de columnas">
        <div>
          <h4>Columnas en los dos ({comparison.sharedColumns.length})</h4>
          <p>{comparison.sharedColumns.join(", ") || "Ninguna"}</p>
        </div>
        <div>
          <h4>Solo en tu dataset ({comparison.currentOnlyColumns.length})</h4>
          <p>{comparison.currentOnlyColumns.join(", ") || "Ninguna"}</p>
        </div>
        <div>
          <h4>Solo en el otro archivo ({comparison.comparedOnlyColumns.length})</h4>
          <p>{comparison.comparedOnlyColumns.join(", ") || "Ninguna"}</p>
        </div>
      </div>
      {comparison.conflicts.length > 0 && (
        <section className="comparison-conflicts" aria-labelledby="comparison-conflicts-title">
          <h4 id="comparison-conflicts-title">Conflictos</h4>
          <ul>
            {comparison.conflicts.map((conflict, index) => (
              <li key={comparison.conflictOffset + index}>
                <strong>Clave {conflict.key.map(cellText).join(" · ")}</strong>
                {conflict.cells.map((cell) => (
                  <span key={cell.column} className="comparison-conflicts__cell">
                    {cell.column}: <ConflictValue value={cell.current} /> en tu dataset,{" "}
                    <ConflictValue value={cell.compared} /> en el otro archivo
                  </span>
                ))}
              </li>
            ))}
          </ul>
          <ConflictPager comparison={comparison} onConflictPageChange={onConflictPageChange} busy={busy} />
        </section>
      )}
    </div>
  );
}

/**
 * Pages through the conflicts of the comparison. `nextBlocked`: Preparar asks
 * to decide a page before the next one.
 */
export function ConflictPager({
  comparison,
  onConflictPageChange,
  busy,
  nextBlocked = false,
}: {
  comparison: DatasetComparison;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  busy: boolean;
  nextBlocked?: boolean;
}) {
  const [loading, setLoading] = useState(false);
  const [cancellationPending, setCancellationPending] = useState(false);
  const [cancellationError, setCancellationError] = useState<string | null>(null);

  async function requestPage(offset: number) {
    setLoading(true);
    setCancellationPending(false);
    setCancellationError(null);
    try {
      await onConflictPageChange(offset);
    } finally {
      setLoading(false);
      setCancellationPending(false);
    }
  }

  async function cancelPage() {
    if (!loading || cancellationPending) return;
    setCancellationPending(true);
    setCancellationError(null);
    try {
      await cancelOperation("datasetComparison");
    } catch (error: unknown) {
      setCancellationError(errorMessage(error));
      setCancellationPending(false);
    }
  }

  if (comparison.conflicts.length === 0) return null;
  const first = comparison.conflictOffset + 1;
  const last = comparison.conflictOffset + comparison.conflicts.length;
  return (
    <>
      <nav className="conflict-resolution__pager" aria-label="Paginación de conflictos">
        <button
          type="button"
          onClick={() => void requestPage(Math.max(0, comparison.conflictOffset - CONFLICT_PAGE_SIZE))}
          disabled={busy || loading || comparison.conflictOffset === 0}
          aria-describedby={busyDescribedBy(busy)}
        >
          Conflictos anteriores
        </button>
        <span>
          Conflictos {first}–{last} de {comparison.conflictingKeyCount.toLocaleString()}
        </span>
        <button
          type="button"
          onClick={() => void requestPage(last)}
          disabled={busy || loading || !comparison.conflictsTruncated || nextBlocked}
          aria-describedby={busyDescribedBy(busy)}
        >
          {loading ? "Cargando conflictos…" : "Siguientes conflictos"}
        </button>
        {loading && (
          <button type="button" className="secondary-action" onClick={() => void cancelPage()} disabled={cancellationPending}>
            {cancellationPending ? "Esperando cancelación…" : "Cancelar carga"}
          </button>
        )}
      </nav>
      {cancellationError && (
        <p className="notice" role="alert">No se pudo cancelar la carga de conflictos: {cancellationError}</p>
      )}
    </>
  );
}

/** TXT-12: an empty cell and a missing one read differently from the text «null». */
export function cellText(value: string | null): string {
  if (value === null) return "(nulo)";
  return value === "" ? "(vacío)" : value;
}

/** A conflicting value: a null reads «∅ nulo» as in the data tables. */
export function ConflictValue({ value }: { value: string | null }) {
  if (value === null) return <MissingValue />;
  return value === ""
    ? <em className="conflict-resolution__missing">{cellText(value)}</em>
    : <code>{value}</code>;
}
