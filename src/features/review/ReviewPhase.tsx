import { useEffect, useState } from "react";

import { OperationProgressView } from "../../components/OperationProgressView";
import { ReviewTabList, type ReviewTab } from "../../components/ReviewTabList";
import { queryDataset } from "../../bridge";
import type {
  DatasetColumn,
  DatasetJoinType,
  DatasetPreview,
  DatasetProfile,
  DatasetQueryResult,
  ConflictResolution,
  ConflictSource,
  ColumnProfile,
  NumericCorrelationMatrix,
  CategoricalGroupSummary,
  TemporalSeriesSummary,
} from "../../bridge";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type { ReadyDatasetStatus } from "../load/loadModel";
import {
  nextPageOffset,
  pageRange,
  previousPageOffset,
  type ProfileStatus,
} from "./reviewModel";
import type { ComparisonStatus } from "./compareModel";
import type { JoinStatus } from "./joinModel";

const CONFLICT_PAGE_SIZE = 50;

interface ReviewPhaseProps {
  datasetStatus: ReadyDatasetStatus;
  profileStatus: ProfileStatus;
  reviewTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
  onPageChange: (offset: number) => void;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
  comparisonStatus: ComparisonStatus;
  datasetColumns: DatasetColumn[];
  comparisonKeyColumns: string[];
  onComparisonKeyColumnsChange: (columns: string[]) => void;
  onCompare: () => void;
  onClearComparison: () => void;
  onConsolidate: () => void;
  onResolveConflicts: (decisions: ConflictResolution[]) => void;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  joinStatus: JoinStatus;
  joinType: DatasetJoinType;
  onJoinTypeChange: (joinType: DatasetJoinType) => void;
  onJoin: (joinType: DatasetJoinType) => void;
}

export function ReviewPhase({
  datasetStatus,
  profileStatus,
  reviewTab,
  onTabChange,
  onPageChange,
  onAnalyzeQuality,
  onCancelProfile,
  comparisonStatus,
  datasetColumns,
  comparisonKeyColumns,
  onComparisonKeyColumnsChange,
  onCompare,
  onClearComparison,
  onConsolidate,
  onResolveConflicts,
  onConflictPageChange,
  joinStatus,
  joinType,
  onJoinTypeChange,
  onJoin,
}: ReviewPhaseProps) {
  const comparisonActive = comparisonStatus.kind !== "idle" || joinStatus.kind !== "idle";
  const [comparisonOpen, setComparisonOpen] = useState(comparisonActive);

  useEffect(() => {
    if (comparisonActive) setComparisonOpen(true);
  }, [comparisonActive]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Revisar · Dataset activo</p>
          <h2>Revisa antes de modificar</h2>
          <h3 className="phase-file">{datasetStatus.dataset.fileName}</h3>
          <p>Comprueba la estructura, la calidad y una muestra de los datos antes de modificarlos.</p>
        </div>
      </header>
      <ReviewTabList activeTab={reviewTab} onTabChange={onTabChange} />

      {reviewTab === "diagnosis" ? (
        <div id="review-diagnosis-panel" role="tabpanel" aria-labelledby="review-diagnosis-tab">
          <QualitySection
            dataset={datasetStatus.dataset}
            status={profileStatus}
            onAnalyze={onAnalyzeQuality}
            onCancel={onCancelProfile}
            comparisonAvailable={comparisonStatus.kind === "ready"}
          />
        </div>
      ) : (
        <div id="review-preview-panel" role="tabpanel" aria-labelledby="review-preview-tab">
          <DataPreview
            dataset={datasetStatus.dataset}
            pageOffset={datasetStatus.pageOffset}
            pageLoading={datasetStatus.pageLoading}
            pageError={datasetStatus.pageError}
            onPageChange={onPageChange}
          />
        </div>
      )}
      <details
        className="review-tool"
        open={comparisonOpen}
        onToggle={(event) => setComparisonOpen(event.currentTarget.open)}
      >
        <summary>
          <span>Comparar con otro dataset</span>
          <small>Opcional · detecta diferencias, conflictos y claves nuevas</small>
        </summary>
        <DatasetComparisonSection
          status={comparisonStatus}
          datasetColumns={datasetColumns}
          keyColumns={comparisonKeyColumns}
          onKeyColumnsChange={onComparisonKeyColumnsChange}
          onCompare={onCompare}
          onClear={onClearComparison}
          onConsolidate={onConsolidate}
          onResolveConflicts={onResolveConflicts}
          onConflictPageChange={onConflictPageChange}
          joinStatus={joinStatus}
          joinType={joinType}
          onJoinTypeChange={onJoinTypeChange}
          onJoin={onJoin}
        />
      </details>
    </>
  );
}

function DatasetComparisonSection({
  status,
  datasetColumns,
  keyColumns,
  onKeyColumnsChange,
  onCompare,
  onClear,
  onConsolidate,
  onResolveConflicts,
  onConflictPageChange,
  joinStatus,
  joinType,
  onJoinTypeChange,
  onJoin,
}: {
  status: ComparisonStatus;
  datasetColumns: DatasetColumn[];
  keyColumns: string[];
  onKeyColumnsChange: (columns: string[]) => void;
  onCompare: () => void;
  onClear: () => void;
  onConsolidate: () => void;
  onResolveConflicts: (decisions: ConflictResolution[]) => void;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  joinStatus: JoinStatus;
  joinType: DatasetJoinType;
  onJoinTypeChange: (joinType: DatasetJoinType) => void;
  onJoin: (joinType: DatasetJoinType) => void;
}) {
  const [conflictChoices, setConflictChoices] = useState<Record<string, ConflictSource>>({});
  const [conflictPageLoading, setConflictPageLoading] = useState(false);
  useEffect(() => {
    setConflictChoices({});
  }, [status.kind, status.kind === "ready" ? status.comparison.comparedFileName : null]);

  function conflictChoiceKey(conflictIndex: number, column: string): string {
    return `${conflictIndex}:${column}`;
  }

  const visibleConflictCellCount = status.kind === "ready"
    ? status.comparison.conflicts.reduce((total, conflict) => total + conflict.cells.length, 0)
    : 0;
  const visibleConflictChoiceKeys = status.kind === "ready"
    ? new Set(status.comparison.conflicts.flatMap((conflict, conflictIndex) =>
        conflict.cells.map((cell) => conflictChoiceKey(status.comparison.conflictOffset + conflictIndex, cell.column))))
    : new Set<string>();
  const selectedVisibleConflictCellCount = Object.keys(conflictChoices)
    .filter((key) => visibleConflictChoiceKeys.has(key)).length;
  const visibleConflictPageComplete = visibleConflictCellCount > 0 &&
    selectedVisibleConflictCellCount === visibleConflictCellCount;

  async function requestConflictPage(offset: number) {
    setConflictPageLoading(true);
    try {
      await onConflictPageChange(offset);
    } finally {
      setConflictPageLoading(false);
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
          onClick={onCompare}
          disabled={status.kind === "loading" || joinStatus.kind === "loading"}
        >
          {status.kind === "loading" ? "Comparando…" : "Elegir dataset para comparar"}
        </button>
      </div>
      <p className="profile-note">
        Contrasta filas como conjunto multivaluado y conserva el dataset activo hasta que decidas consolidar.
      </p>
      <fieldset className="comparison-key-selector">
        <legend>Claves explícitas (opcional)</legend>
        <p>
          Selecciona una o varias columnas para detectar claves nuevas, duplicadas y conflictos.
          Sin selección se mantiene la comparación multivaluada por fila.
        </p>
        <div className="comparison-key-options">
          {datasetColumns.map((column) => (
            <label key={column.name}>
              <input
                type="checkbox"
                checked={keyColumns.includes(column.name)}
                disabled={joinStatus.kind === "loading"}
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
                <small>{column.dataType}</small>
              </span>
            </label>
          ))}
        </div>
        {keyColumns.length > 0 && (
          <p className="comparison-key-status" role="status">
            Se comparará por: <strong>{keyColumns.join(", ")}</strong>
          </p>
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
                  disabled={joinStatus.kind === "loading"}
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
            onClick={() => onJoin(joinType)}
            disabled={joinStatus.kind === "loading"}
          >
            {joinStatus.kind === "loading" ? "Uniendo datasets…" : "Elegir fuente y unir"}
          </button>
        </fieldset>
      )}
      {joinStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudieron unir los datasets: {joinStatus.message}
        </p>
      )}
      {status.kind === "loading" && (
        <p className="notice" role="status">Leyendo la segunda fuente local…</p>
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo comparar la fuente: {status.message}
        </p>
      )}
      {status.kind === "ready" && (
        <>
          <p className="comparison-source" aria-live="polite">
            <strong>{status.comparison.currentFileName}</strong>
            <span aria-hidden="true"> ↔ </span>
            <strong>{status.comparison.comparedFileName}</strong>
          </p>
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
            </div>
          )}
          {status.comparison.conflicts.length > 0 && (
            <section className="conflict-resolution" aria-labelledby="conflict-resolution-title">
              <div className="conflict-resolution__heading">
                <div>
                  <p className="step">Decisión explícita</p>
                  <h4 id="conflict-resolution-title">Resolver conflictos por clave</h4>
                  <p>Elige el origen de cada celda divergente. No se modifica nada hasta confirmar todas las decisiones.</p>
                </div>
                <button
                  type="button"
                  className="primary-action"
                  onClick={() => onResolveConflicts(Object.entries(conflictChoices).map(([choiceKey, source]) => {
                    const separator = choiceKey.indexOf(":");
                    const conflictIndex = Number(choiceKey.slice(0, separator));
                    const column = choiceKey.slice(separator + 1);
                    return { conflictIndex, column, source };
                  }))}
                  disabled={conflictPageLoading || status.comparison.conflictsTruncated || !visibleConflictPageComplete}
                >
                  Resolver conflictos
                </button>
              </div>
              {status.comparison.conflicts.map((conflict, conflictIndex) => {
                const globalConflictIndex = status.comparison.conflictOffset + conflictIndex;
                return (
                <fieldset className="conflict-resolution__item" key={globalConflictIndex}>
                  <legend>
                    Conflicto {globalConflictIndex + 1} · clave {conflict.key.map((value) => value ?? "null").join(" · ")}
                  </legend>
                  <ul>
                    {conflict.cells.map((cell) => {
                      const choiceKey = conflictChoiceKey(globalConflictIndex, cell.column);
                      return (
                        <li key={cell.column}>
                          <strong>{cell.column}</strong>
                          <span>Activo: <code>{cell.current ?? "null"}</code></span>
                          <span>Comparado: <code>{cell.compared ?? "null"}</code></span>
                          <div className="conflict-resolution__choices">
                            <label>
                              <input
                                type="radio"
                                name={`conflict-${conflictIndex}-${cell.column}`}
                                checked={conflictChoices[choiceKey] === "current"}
                                onChange={() => setConflictChoices((current) => ({ ...current, [choiceKey]: "current" }))}
                              />
                              Conservar activo en {cell.column}
                            </label>
                            <label>
                              <input
                                type="radio"
                                name={`conflict-${conflictIndex}-${cell.column}`}
                                checked={conflictChoices[choiceKey] === "compared"}
                                onChange={() => setConflictChoices((current) => ({ ...current, [choiceKey]: "compared" }))}
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
                  Esta página está completa. Avanza para revisar los siguientes conflictos antes de resolverlos.
                </p>
              )}
              {status.comparison.conflicts.length > 0 && (
                <nav className="conflict-resolution__pager" aria-label="Paginación de conflictos">
                  <button
                    type="button"
                    onClick={() => void requestConflictPage(Math.max(0, status.comparison.conflictOffset - CONFLICT_PAGE_SIZE))}
                    disabled={conflictPageLoading || status.comparison.conflictOffset === 0}
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
                    disabled={conflictPageLoading || !status.comparison.conflictsTruncated || !visibleConflictPageComplete}
                  >
                    {conflictPageLoading ? "Cargando conflictos…" : "Siguientes conflictos"}
                  </button>
                </nav>
              )}
            </section>
          )}
          <div className="comparison-actions">
            <button type="button" className="secondary-action" onClick={onClear}>Descartar comparación</button>
            <button type="button" className="primary-action" onClick={onConsolidate} disabled={!status.comparison.canConsolidate}>
              Consolidar filas
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

function QualitySection({
  dataset,
  status,
  onAnalyze,
  onCancel,
  comparisonAvailable,
}: {
  dataset: DatasetPreview;
  status: ProfileStatus;
  onAnalyze: () => void;
  onCancel: () => void;
  comparisonAvailable: boolean;
}) {
  return (
    <section className="phase-section" aria-labelledby="quality-title">
      <div className="section-heading">
        <div>
          <p className="step">Calidad inicial</p>
          <h3 id="quality-title">Perfil por columna</h3>
        </div>
        {status.kind !== "loading" && (
          <button className="primary-action" type="button" onClick={onAnalyze}>
            {status.kind === "ready" ? "Analizar de nuevo" : "Analizar calidad"}
          </button>
        )}
      </div>
      <DatasetMetrics dataset={dataset} />
      {status.kind === "loading" && (
        <OperationProgressView
          progress={status.progress}
          cancellation={status.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel }}
        />
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {status.message}
        </p>
      )}
      {status.kind === "ready" && <QualityProfile profile={status.profile} />}
      <LocalQueryPanel comparisonAvailable={comparisonAvailable} />
    </section>
  );
}

function LocalQueryPanel({ comparisonAvailable }: { comparisonAvailable: boolean }) {
  const [query, setQuery] = useState("SELECT * FROM dataset LIMIT 50");
  const [state, setState] = useState<
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ready"; result: DatasetQueryResult }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [queryOpen, setQueryOpen] = useState(false);

  useEffect(() => {
    if (state.kind !== "idle") setQueryOpen(true);
  }, [state.kind]);

  async function runQuery() {
    setState({ kind: "loading" });
    try {
      setState({ kind: "ready", result: await queryDataset(query) });
    } catch (error: unknown) {
      setState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  return (
    <details
      className="review-tool review-tool--nested"
      open={queryOpen}
      onToggle={(event) => setQueryOpen(event.currentTarget.open)}
    >
      <summary>
        <span>Explorar con SQL local</span>
        <small>Opcional · consulta segura y de solo lectura</small>
      </summary>
      <section className="local-query" aria-labelledby="local-query-title">
        <div className="local-query__heading">
          <div>
            <p className="step">Consulta segura</p>
            <h4 id="local-query-title">Consulta SQL de solo lectura</h4>
            <p>
              Solo se acepta SELECT sobre <code>dataset</code>, columnas existentes, filtros simples,
              GROUP BY y COUNT/SUM/AVG/MIN/MAX; LIMIT/OFFSET queda acotado a 200 filas.
              {comparisonAvailable
                ? " La comparación cargada también está disponible como compared para JOIN INNER, LEFT o FULL."
                : " Carga una comparación para habilitar JOIN con la tabla compared."}
            </p>
            {comparisonAvailable && (
              <p className="local-query__example">
                Ejemplo: <code>SELECT id, segmento FROM dataset LEFT JOIN compared ON dataset.id = compared.codigo LIMIT 50</code>
              </p>
            )}
          </div>
        </div>
        <label className="local-query__field">
          Consulta SQL de solo lectura
          <textarea
            aria-label="Consulta SQL de solo lectura"
            rows={2}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
          />
        </label>
        <div className="local-query__actions">
          <button type="button" onClick={() => void runQuery()} disabled={state.kind === "loading" || !query.trim()}>
            {state.kind === "loading" ? "Consultando…" : "Ejecutar consulta"}
          </button>
        </div>
        {state.kind === "error" && <p className="notice notice--error" role="alert">No se pudo ejecutar la consulta: {state.message}</p>}
        {state.kind === "ready" && <LocalQueryResult result={state.result} />}
      </section>
    </details>
  );
}

function LocalQueryResult({ result }: { result: DatasetQueryResult }) {
  return (
    <div className="local-query__result" role="status" aria-live="polite">
      <p>{result.rowCount.toLocaleString()} filas disponibles · mostrando desde {result.offset + 1}{result.truncated ? " · resultado truncado por LIMIT" : ""}</p>
      <div className="profile-region" role="region" tabIndex={0} aria-label="Resultado de consulta SQL">
        <table>
          <thead><tr>{result.columns.map((column) => <th key={column.name} scope="col"><span>{column.name}</span><small>{column.dataType}</small></th>)}</tr></thead>
          <tbody>{result.rows.map((row, rowIndex) => <tr key={result.offset + rowIndex}>{row.map((value, columnIndex) => <td key={columnIndex}>{value ?? <span className="null-value">null</span>}</td>)}</tr>)}</tbody>
        </table>
      </div>
    </div>
  );
}

interface DataPreviewProps {
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
  onPageChange: (offset: number) => void;
}

export function DataPreview({
  dataset,
  pageOffset,
  pageLoading,
  pageError,
  onPageChange,
}: DataPreviewProps) {
  const { end: pageEnd, hasPrevious, hasNext } = pageRange(dataset, pageOffset);

  return (
    <>
      <div className="table-region" tabIndex={0} aria-label="Vista previa del dataset">
        <table>
          <thead>
            <tr>
              {dataset.columns.map((column) => (
                <th key={column.name} scope="col">
                  <span>{column.name}</span>
                  <small>{column.dataType}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {dataset.rows.map((row, rowIndex) => (
              <tr key={pageOffset + rowIndex}>
                {row.map((value, columnIndex) => (
                  <td key={columnIndex}>{value ?? <span className="null-value">null</span>}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="pagination" aria-label="Paginación de la vista previa">
        <p className="preview-note" aria-live="polite">
          {dataset.rowCount === 0
            ? "El dataset no contiene filas."
            : `Filas ${pageOffset + 1}–${pageEnd} de ${dataset.rowCount.toLocaleString()}`}
        </p>
        <div>
          <button
            type="button"
            onClick={() => onPageChange(previousPageOffset(pageOffset))}
            disabled={!hasPrevious || pageLoading}
          >
            Anterior
          </button>
          <button
            type="button"
            onClick={() => onPageChange(nextPageOffset(pageOffset))}
            disabled={!hasNext || pageLoading}
          >
            {pageLoading ? "Cargando…" : "Siguiente"}
          </button>
        </div>
      </div>
      {pageError && (
        <p className="notice notice--error" role="alert">
          No se pudo cambiar de página: {pageError}
        </p>
      )}
    </>
  );
}

function QualityProfile({ profile }: { profile: DatasetProfile }) {
  const textColumns = profile.columns.filter((column) => column.emptyCount !== null);
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);

  return (
    <>
      <dl className="quality-summary" aria-label="Resumen de calidad del dataset">
        <div>
          <dt>Filas duplicadas adicionales</dt>
          <dd>
            {profile.duplicateRowCount.toLocaleString()} ({profile.duplicatePercentage.toFixed(1)}%)
          </dd>
        </div>
        <div>
          <dt>Filas analizadas</dt>
          <dd>{profile.rowCount.toLocaleString()}</dd>
        </div>
      </dl>
      <QualityVisuals profile={profile} />
      <div
        className="profile-region"
        role="region"
        tabIndex={0}
        aria-label="Perfil de calidad por columna"
      >
        <table>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              <th scope="col">Completitud</th>
              <th scope="col">Nulos</th>
              <th scope="col">Únicos</th>
              <th scope="col">Mínimo</th>
              <th scope="col">Máximo</th>
              <th scope="col">Promedio</th>
            </tr>
          </thead>
          <tbody>
            {profile.columns.map((column) => (
              <tr key={column.name}>
                <th scope="row">
                  <span>{column.name}</span>
                  <small>{column.dataType}</small>
                </th>
                <td>{column.completenessPercentage.toFixed(1)}%</td>
                <td>{column.nullCount.toLocaleString()}</td>
                <td>{column.uniqueCount.toLocaleString()}</td>
                <td>{column.minimum ?? "—"}</td>
                <td>{column.maximum ?? "—"}</td>
                <td>
                  {column.mean === null
                    ? "—"
                    : column.mean.toLocaleString(undefined, { maximumFractionDigits: 3 })}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">El conteo de valores únicos excluye los nulos.</p>
      {numericColumns.length > 0 && (
        <>
          <h4 className="text-profile-title">Detalle de columnas numéricas</h4>
          <div
            className="profile-region profile-region--detail"
            role="region"
            tabIndex={0}
            aria-label="Perfil de columnas numéricas"
          >
            <table>
              <thead>
                <tr>
                  <th scope="col">Columna</th>
                  <th scope="col">Desv. estándar</th>
                  <th scope="col">Q1</th>
                  <th scope="col">Mediana</th>
                  <th scope="col">Q3</th>
                  <th scope="col">Posibles outliers</th>
                </tr>
              </thead>
              <tbody>
                {numericColumns.map((column) => (
                  <tr key={column.name}>
                    <th scope="row">{column.name}</th>
                    <td>{formatStatistic(column.standardDeviation)}</td>
                    <td>{formatStatistic(column.firstQuartile)}</td>
                    <td>{formatStatistic(column.median)}</td>
                    <td>{formatStatistic(column.thirdQuartile)}</td>
                    <td>{column.outlierCount?.toLocaleString() ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            Posibles outliers usa la regla IQR de 1.5× y requiere al menos cuatro valores. La
            desviación estándar es muestral.
          </p>
        </>
      )}
      {textColumns.length > 0 && (
        <>
          <h4 className="text-profile-title">Detalle de columnas de texto</h4>
          <div
            className="profile-region profile-region--detail"
            role="region"
            tabIndex={0}
            aria-label="Perfil de columnas de texto"
          >
            <table>
              <thead>
                <tr>
                  <th scope="col">Columna</th>
                  <th scope="col">Vacíos</th>
                  <th scope="col">Longitud mínima</th>
                  <th scope="col">Longitud máxima</th>
                  <th scope="col">Longitud promedio</th>
                  <th scope="col">Tipo sugerido</th>
                  <th scope="col">Coincidencia</th>
                  <th scope="col">No coinciden</th>
                </tr>
              </thead>
              <tbody>
                {textColumns.map((column) => (
                  <tr key={column.name}>
                    <th scope="row">{column.name}</th>
                    <td>{column.emptyCount?.toLocaleString()}</td>
                    <td>{column.minimumLength?.toLocaleString() ?? "—"}</td>
                    <td>{column.maximumLength?.toLocaleString() ?? "—"}</td>
                    <td>
                      {column.averageLength?.toLocaleString(undefined, {
                        maximumFractionDigits: 1,
                      }) ?? "—"}
                    </td>
                    <td>{suggestedTypeLabel(column.suggestedType)}</td>
                    <td>
                      {column.typeMatchPercentage === null
                        ? "—"
                        : `${column.typeMatchPercentage.toFixed(1)}%`}
                    </td>
                    <td>{column.invalidTypeCount?.toLocaleString() ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            “Vacíos” incluye cadenas sin caracteres o compuestas solamente por espacios. Las
            sugerencias requieren al menos tres valores y una coincidencia del 90%.
          </p>
        </>
      )}
    </>
  );
}

function QualityVisuals({ profile }: { profile: DatasetProfile }) {
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);
  const formatColumns = profile.columns.filter((column) => column.typeMatchPercentage !== null);
  const nullPatternColumns = profile.columns
    .filter((column) => column.nullCount > 0)
    .sort((left, right) => {
      const byNullCount = right.nullCount - left.nullCount;
      return byNullCount || left.name.localeCompare(right.name, "es", { sensitivity: "base" });
    });
  const distributionColumns = numericColumns.filter((column) =>
    [column.minimum, column.maximum, column.firstQuartile, column.median, column.thirdQuartile]
      .every((value) => value !== null && Number.isFinite(Number(value))),
  );
  const histogramColumns = numericColumns.filter((column) => (column.histogram?.length ?? 0) > 0);
  const maxOutlierCount = Math.max(
    1,
    ...numericColumns.map((column) => Math.max(0, column.outlierCount ?? 0)),
  );

  if (profile.columns.length === 0) return null;

  return (
    <section className="quality-visuals" aria-labelledby="quality-visuals-title">
      <div className="quality-visuals__heading">
        <div>
          <p className="step">Lectura rápida</p>
          <h4 id="quality-visuals-title">Señales del perfil</h4>
        </div>
        <p>
          Las barras ayudan a detectar patrones; las tablas de abajo conservan los valores exactos
          y el equivalente para lector de pantalla.
        </p>
      </div>
      <div className="quality-chart-grid">
        <div className="quality-chart" role="group" aria-labelledby="quality-completeness-title">
          <h5 id="quality-completeness-title">Completitud por columna</h5>
          <p className="quality-chart__note">Porcentaje de filas con un valor no nulo.</p>
          <div className="quality-chart__bars" role="list" aria-label="Completitud por columna">
            {profile.columns.map((column) => {
              const percentage = clampPercentage(column.completenessPercentage);

              return (
                <div className="quality-chart__item" role="listitem" key={column.name}>
                  <div className="quality-chart__label">
                    <span title={column.name}>{column.name}</span>
                    <strong>{percentage.toFixed(1)}%</strong>
                  </div>
                  <div className="quality-chart__track" aria-hidden="true">
                    <span style={{ width: `${percentage}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
        {numericColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-outliers-title">
            <h5 id="quality-outliers-title">Posibles outliers</h5>
            <p className="quality-chart__note">Filas fuera del rango IQR de 1.5×.</p>
            <div className="quality-chart__bars" role="list" aria-label="Posibles outliers por columna">
              {numericColumns.map((column) => {
                const count = Math.max(0, column.outlierCount ?? 0);
                const percentage = (count / maxOutlierCount) * 100;

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{count.toLocaleString()}</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {nullPatternColumns.length > 0 && (
          <div className="quality-chart quality-chart--wide" role="group" aria-labelledby="quality-null-patterns-title">
            <h5 id="quality-null-patterns-title">Patrones de nulos</h5>
            <p className="quality-chart__note">
              Prioriza columnas con más valores ausentes antes de transformar o exportar.
            </p>
            <div className="quality-chart__bars" role="list" aria-label="Patrones de nulos por columna">
              {nullPatternColumns.map((column) => {
                const percentage = clampPercentage(100 - column.completenessPercentage);

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{column.nullCount.toLocaleString()} nulos · {percentage.toFixed(1)}%</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span className="quality-chart__track-fill--warning" style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="quality-chart__table">
              <table aria-label="Tabla de patrones de nulos">
                <caption className="visually-hidden">Tabla de patrones de nulos por columna</caption>
                <thead>
                  <tr>
                    <th scope="col">Columna</th>
                    <th scope="col">Nulos</th>
                    <th scope="col">Porcentaje nulo</th>
                  </tr>
                </thead>
                <tbody>
                  {nullPatternColumns.map((column) => (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>{column.nullCount.toLocaleString()}</td>
                      <td>{clampPercentage(100 - column.completenessPercentage).toFixed(1)}%</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
        {formatColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-format-title">
            <h5 id="quality-format-title">Validación de formato</h5>
            <p className="quality-chart__note">
              Comprueba qué proporción coincide con el tipo sugerido antes de convertir columnas.
            </p>
            <div className="quality-chart__bars" role="list" aria-label="Validación de formato por columna">
              {formatColumns.map((column) => {
                const percentage = clampPercentage(column.typeMatchPercentage ?? 0);
                const invalidCount = Math.max(0, column.invalidTypeCount ?? 0);

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{percentage.toFixed(1)}% · {invalidCount.toLocaleString()} inválidos</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="quality-chart__table">
              <table aria-label="Tabla de validación de formato">
                <caption className="visually-hidden">Tabla de validación de formato por columna</caption>
                <thead>
                  <tr>
                    <th scope="col">Columna</th>
                    <th scope="col">Tipo sugerido</th>
                    <th scope="col">Coincidencia</th>
                    <th scope="col">Inválidos</th>
                  </tr>
                </thead>
                <tbody>
                  {formatColumns.map((column) => (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>
                        <span aria-hidden="true">{suggestedTypeLabel(column.suggestedType)}</span>
                        <span className="visually-hidden">
                          Tipo sugerido: {suggestedTypeLabel(column.suggestedType)}
                        </span>
                      </td>
                      <td>
                        <span aria-hidden="true">
                          {clampPercentage(column.typeMatchPercentage ?? 0).toFixed(1)}%
                        </span>
                        <span className="visually-hidden">
                          Coincidencia: {clampPercentage(column.typeMatchPercentage ?? 0).toFixed(1)}%
                        </span>
                      </td>
                      <td>{Math.max(0, column.invalidTypeCount ?? 0).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
        {profile.columns.some(isTemporalColumn) && (
          <TemporalCoverageChart
            columns={profile.columns.filter(isTemporalColumn)}
            rowCount={profile.rowCount}
          />
        )}
        {profile.temporalSeries?.map((summary, summaryIndex) => (
          <TemporalTrendChart
            key={summary.column}
            summary={summary}
            summaryIndex={summaryIndex}
          />
        ))}
        {distributionColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-distribution-title">
            <h5 id="quality-distribution-title">Distribución numérica</h5>
            <p className="quality-chart__note">Rango mínimo–máximo y caja entre Q1 y Q3; la marca central es la mediana.</p>
            <div className="quality-chart__bars" role="list" aria-label="Distribución numérica por columna">
              {distributionColumns.map((column) => {
                const minimum = Number(column.minimum);
                const maximum = Number(column.maximum);
                const firstQuartile = Number(column.firstQuartile);
                const median = Number(column.median);
                const thirdQuartile = Number(column.thirdQuartile);
                const span = Math.max(maximum - minimum, Number.EPSILON);
                const left = ((firstQuartile - minimum) / span) * 100;
                const width = ((thirdQuartile - firstQuartile) / span) * 100;
                const medianPosition = ((median - firstQuartile) / Math.max(thirdQuartile - firstQuartile, Number.EPSILON)) * 100;

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>Q1 {formatStatistic(firstQuartile)} · Mediana {formatStatistic(median)} · Q3 {formatStatistic(thirdQuartile)}</strong>
                    </div>
                    <div className="quality-boxplot" aria-hidden="true">
                      <span className="quality-boxplot__whisker" />
                      <span className="quality-boxplot__box" style={{ left: `${clampPercentage(left)}%`, width: `${clampPercentage(width)}%` }}>
                        <span className="quality-boxplot__median" style={{ left: `${clampPercentage(medianPosition)}%` }} />
                      </span>
                    </div>
                    <small className="quality-chart__range">Mín. {formatStatistic(minimum)} · Máx. {formatStatistic(maximum)}</small>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {histogramColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-histogram-title">
            <h5 id="quality-histogram-title">Histograma numérico</h5>
            <p className="quality-chart__note">
              Frecuencia de valores por intervalo. El último intervalo incluye su límite máximo.
            </p>
            <div className="quality-histograms">
              {histogramColumns.map((column, columnIndex) => {
                const buckets = column.histogram ?? [];
                const maximumCount = Math.max(1, ...buckets.map((bucket) => bucket.count));
                const titleId = `quality-histogram-column-${columnIndex}`;

                return (
                  <div className="quality-histogram" role="group" aria-labelledby={titleId} key={column.name}>
                    <h6 id={titleId}>{column.name}</h6>
                    <div className="quality-histogram__bars" aria-hidden="true">
                      {buckets.map((bucket, bucketIndex) => {
                        const percentage = (bucket.count / maximumCount) * 100;
                        const interval = histogramIntervalLabel(bucket.lower, bucket.upper, bucketIndex === buckets.length - 1);

                        return (
                          <div
                            className="quality-histogram__bar"
                            key={`${bucket.lower}-${bucket.upper}-${bucketIndex}`}
                            style={{ height: `${percentage}%` }}
                            title={`${interval}: ${bucket.count.toLocaleString()} filas`}
                          >
                            <span />
                          </div>
                        );
                      })}
                    </div>
                    <div className="quality-histogram__axis" aria-hidden="true">
                      <span>{formatStatistic(buckets[0]?.lower ?? null)}</span>
                      <span>{formatStatistic(buckets[buckets.length - 1]?.upper ?? null)}</span>
                    </div>
                    <div className="quality-histogram__table">
                      <table aria-label={`Tabla de frecuencias para ${column.name}`}>
                        <caption className="visually-hidden">Tabla de frecuencias para {column.name}</caption>
                        <thead>
                          <tr>
                            <th scope="col">Intervalo</th>
                            <th scope="col">Filas</th>
                          </tr>
                        </thead>
                        <tbody>
                          {buckets.map((bucket, bucketIndex) => (
                            <tr key={`${bucket.lower}-${bucket.upper}-${bucketIndex}`}>
                              <th scope="row">
                                {histogramIntervalLabel(bucket.lower, bucket.upper, bucketIndex === buckets.length - 1)}
                              </th>
                              <td>{bucket.count.toLocaleString()}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {profile.categoricalGroupSummaries?.map((summary, summaryIndex) => (
          <CategoricalGroupChart
            key={summary.column}
            summary={summary}
            summaryIndex={summaryIndex}
          />
        ))}
        {profile.numericCorrelations && profile.numericCorrelations.columns.length > 1 && (
          <NumericCorrelationChart matrix={profile.numericCorrelations} />
        )}
      </div>
    </section>
  );
}

function TemporalCoverageChart({
  columns,
  rowCount,
}: {
  columns: ColumnProfile[];
  rowCount: number;
}) {
  return (
    <div className="quality-chart quality-chart--wide quality-temporal" role="group" aria-labelledby="quality-temporal-title">
      <h5 id="quality-temporal-title">Cobertura temporal</h5>
      <p className="quality-chart__note">
        Rango mínimo–máximo y filas con valor para columnas de fecha o fecha-hora. Se calcula con
        el perfil, sin leer ni mostrar celdas.
      </p>
      <div className="quality-temporal__table">
        <table aria-label="Tabla de cobertura temporal">
          <caption className="visually-hidden">Rango y cobertura de columnas temporales</caption>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              <th scope="col">Tipo</th>
              <th scope="col">Rango detectado</th>
              <th scope="col">Filas con valor</th>
              <th scope="col">Cobertura</th>
            </tr>
          </thead>
          <tbody>
            {columns.map((column) => {
              const availableRows = Math.max(0, rowCount - column.nullCount);
              const coverage = clampPercentage(column.completenessPercentage);

              return (
                <tr key={column.name}>
                  <th scope="row">{column.name}</th>
                  <td>{temporalTypeLabel(column)}</td>
                  <td className="quality-temporal__range">
                    <span className="quality-temporal__range-value">
                      {formatTemporalValue(column.minimum)}
                      <span aria-hidden="true"> → </span>
                      <span className="visually-hidden"> hasta </span>
                      {formatTemporalValue(column.maximum)}
                    </span>
                    {(column.minimum === null || column.maximum === null) && (
                      <span className="visually-hidden">Rango parcial o no disponible</span>
                    )}
                  </td>
                  <td>{availableRows.toLocaleString()} de {Math.max(0, rowCount).toLocaleString()}</td>
                  <td>{coverage.toFixed(1)}%</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        La cobertura representa valores no nulos; un rango ausente significa que el perfil no pudo
        calcular uno de sus límites.
      </p>
    </div>
  );
}

function TemporalTrendChart({
  summary,
  summaryIndex,
}: {
  summary: TemporalSeriesSummary;
  summaryIndex: number;
}) {
  const titleId = `quality-temporal-trend-title-${summaryIndex}`;
  const tableLabel = `Tendencia temporal para ${summary.column}`;
  const [metric, setMetric] = useState<TemporalMetric>("rows");
  const metricId = `quality-temporal-metric-${summaryIndex}`;

  return (
    <div
      className="quality-chart quality-chart--wide quality-temporal-trend"
      role="group"
      aria-labelledby={titleId}
    >
      <h5 id={titleId}>Tendencia temporal · {summary.column}</h5>
      <p className="quality-chart__note">
        Conteo de filas por {summary.granularity === "day" ? "día" : summary.granularity === "month" ? "mes" : "año"}; solo se muestran
        agregados del perfil, nunca valores de celdas. Se incluyen {summary.parsedRowCount.toLocaleString()}
        de {(summary.parsedRowCount + summary.unparsedRowCount).toLocaleString()} filas interpretables.
      </p>
      {summary.periods.length > 0 ? (
        <>
          <div className="quality-temporal-trend__toolbar">
            <div>
              <span className="quality-temporal-trend__metric-caption">Lectura visible</span>
              <strong>{temporalMetricLabel(metric)}</strong>
            </div>
            <label htmlFor={metricId}>
              Medir por
              <select
                id={metricId}
                value={metric}
                aria-label={`Métrica temporal para ${summary.column}`}
                onChange={(event) => setMetric(event.target.value as TemporalMetric)}
              >
                <option value="rows">Filas</option>
                <option value="percentage">Porcentaje</option>
              </select>
            </label>
          </div>
          <TemporalLineChart
            summary={summary}
            metric={metric}
            titleId={`${titleId}-chart`}
          />
          {summary.granularity === "day" && <DailyTemporalCalendar summary={summary} />}
        </>
      ) : summary.granularity === "day" ? (
        <DailyTemporalCalendar summary={summary} />
      ) : (
        <p className="quality-temporal-empty" role="status">
          No hay periodos interpretables para mostrar en esta columna.
        </p>
      )}
      <div className="quality-temporal-trend__table">
        <table aria-label={tableLabel}>
          <caption className="visually-hidden">{tableLabel}</caption>
          <thead>
            <tr>
              <th scope="col">Periodo</th>
              <th scope="col">Filas</th>
              <th scope="col">Porcentaje de valores interpretables</th>
            </tr>
          </thead>
          <tbody>
            {summary.periods.map((period) => (
              <tr key={`table-${period.period}`}>
                <th scope="row">{period.period}</th>
                <td>{period.rowCount.toLocaleString()}</td>
                <td>{clampPercentage(period.percentage).toFixed(1)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        {summary.unparsedRowCount.toLocaleString()} filas sin periodo interpretable.
        {summary.truncated ? " Los periodos más antiguos se agruparon para mantener la lectura rápida." : ""}
      </p>
    </div>
  );
}

type TemporalMetric = "rows" | "percentage";

function TemporalLineChart({
  summary,
  metric,
  titleId,
}: {
  summary: TemporalSeriesSummary;
  metric: TemporalMetric;
  titleId: string;
}) {
  const chartHeight = 188;
  const chartWidth = Math.max(560, summary.periods.length * 72);
  const padding = { top: 18, right: 18, bottom: 34, left: 56 };
  const plotWidth = Math.max(1, chartWidth - padding.left - padding.right);
  const plotHeight = chartHeight - padding.top - padding.bottom;
  const values = summary.periods.map((period) => temporalMetricValue(period, metric));
  const maximumValue = metric === "percentage"
    ? 100
    : Math.max(1, ...values);
  const points = summary.periods.map((period, index) => {
    const x = summary.periods.length === 1
      ? padding.left + plotWidth / 2
      : padding.left + (index / (summary.periods.length - 1)) * plotWidth;
    const y = padding.top + plotHeight - (values[index] / maximumValue) * plotHeight;
    return { x, y, period };
  });
  const pointList = points.map(({ x, y }) => `${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
  const baseline = padding.top + plotHeight;
  const areaPath = points.length > 0
    ? `M ${points[0].x.toFixed(2)} ${baseline.toFixed(2)} L ${points.map(({ x, y }) => `${x.toFixed(2)} ${y.toFixed(2)}`).join(" L ")} L ${points.at(-1)!.x.toFixed(2)} ${baseline.toFixed(2)} Z`
    : "";
  const labelIndexes = temporalAxisIndexes(summary.periods.length);
  const descriptionId = `${titleId}-description`;

  return (
    <div className="quality-temporal-line" role="group" aria-labelledby={titleId}>
      <div className="quality-temporal-line__legend">
        <span><i aria-hidden="true" /> {temporalMetricLabel(metric)}</span>
        <span>Escala máxima: {temporalMetricDisplay(maximumValue, metric)}</span>
      </div>
      <div className="quality-temporal-line__viewport">
        <svg
          className="quality-temporal-line__chart"
          width={chartWidth}
          height={chartHeight}
          style={{ width: `max(100%, ${chartWidth}px)` }}
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          role="img"
          aria-labelledby={`${titleId} ${descriptionId}`}
        >
          <title id={titleId}>
            Serie temporal de {summary.column} por {temporalMetricLabel(metric).toLowerCase()}
          </title>
          <desc id={descriptionId}>
            Línea con {summary.periods.length.toLocaleString()} periodos. La tabla inferior contiene los mismos datos.
          </desc>
          {[0, 0.5, 1].map((ratio) => {
            const y = padding.top + plotHeight * ratio;
            return (
              <line
                className="quality-temporal-line__grid"
                key={ratio}
                x1={padding.left}
                x2={chartWidth - padding.right}
                y1={y}
                y2={y}
              />
            );
          })}
          <path className="quality-temporal-line__area" d={areaPath} aria-hidden="true" />
          <polyline className="quality-temporal-line__path" points={pointList} aria-hidden="true" />
          <text className="quality-temporal-line__scale" x={padding.left - 10} y={padding.top + 4} textAnchor="end">
            {temporalMetricDisplay(maximumValue, metric)}
          </text>
          <text
            className="quality-temporal-line__scale"
            x={padding.left - 10}
            y={padding.top + plotHeight / 2 + 4}
            textAnchor="end"
          >
            {temporalMetricDisplay(maximumValue / 2, metric)}
          </text>
          <text className="quality-temporal-line__scale" x={padding.left - 10} y={baseline + 4} textAnchor="end">
            {temporalMetricDisplay(0, metric)}
          </text>
          {points.map(({ x, y, period }) => (
            <circle className="quality-temporal-line__point" key={period.period} cx={x} cy={y} r="4">
              <title>{`${period.period}: ${temporalMetricDisplay(temporalMetricValue(period, metric), metric)}`}</title>
            </circle>
          ))}
          {labelIndexes.map((index) => (
            <text
              className="quality-temporal-line__label"
              key={summary.periods[index].period}
              x={points[index].x}
              y={chartHeight - 8}
              textAnchor={index === 0 ? "start" : index === summary.periods.length - 1 ? "end" : "middle"}
            >
              {formatTemporalAxisLabel(summary.periods[index].period, summary.granularity)}
            </text>
          ))}
        </svg>
      </div>
    </div>
  );
}

function DailyTemporalCalendar({ summary }: { summary: TemporalSeriesSummary }) {
  const calendarLabel = `Calendario diario para ${summary.column}`;
  const firstWeekday = temporalDayWeekday(summary.periods[0]?.period);
  const leadingEmptyDays = firstWeekday === null ? 0 : firstWeekday;
  const maximumCount = Math.max(1, ...summary.periods.map((period) => period.rowCount));

  if (summary.periods.length === 0) {
    return (
      <div className="quality-temporal-calendar quality-temporal-calendar--empty" role="group" aria-label={calendarLabel}>
        <h6>Calendario diario</h6>
        <p className="quality-temporal-empty" role="status">
          No hay días interpretables para mostrar en esta columna.
        </p>
      </div>
    );
  }

  return (
    <div className="quality-temporal-calendar" role="group" aria-label={calendarLabel}>
      <div className="quality-temporal-calendar__heading">
        <h6>Calendario diario</h6>
        <p>La intensidad resume la cantidad de filas; los días sin filas permanecen visibles.</p>
      </div>
      <div className="quality-temporal-calendar__weekdays" aria-hidden="true">
        {[
          "Lun",
          "Mar",
          "Mié",
          "Jue",
          "Vie",
          "Sáb",
          "Dom",
        ].map((day) => <span key={day}>{day}</span>)}
      </div>
      <ol className="quality-temporal-calendar__days" aria-label={calendarLabel}>
        {Array.from({ length: leadingEmptyDays }, (_, index) => (
          <li className="quality-temporal-calendar__empty-day" aria-hidden="true" key={`empty-${index}`} />
        ))}
        {summary.periods.map((period) => {
          const level = period.rowCount === 0
            ? 0
            : Math.max(1, Math.ceil((period.rowCount / maximumCount) * 4));
          const rowLabel = `${period.rowCount.toLocaleString()} ${period.rowCount === 1 ? "fila" : "filas"}`;

          return (
            <li
              className={`quality-temporal-calendar__day quality-temporal-calendar__day--level-${level}`}
              key={period.period}
              aria-label={`${period.period}: ${rowLabel}, ${clampPercentage(period.percentage).toFixed(1)}% de los valores interpretables`}
            >
              <time dateTime={period.period}>{formatCalendarDay(period.period)}</time>
              <strong>{period.rowCount.toLocaleString()}</strong>
              <small>filas</small>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function CategoricalGroupChart({
  summary,
  summaryIndex,
}: {
  summary: CategoricalGroupSummary;
  summaryIndex: number;
}) {
  const titleId = `quality-groups-title-${summaryIndex}`;
  const tableLabel = `Resumen de grupos para ${summary.column}`;

  return (
    <div className="quality-chart quality-chart--wide quality-groups" role="group" aria-labelledby={titleId}>
      <h5 id={titleId}>Distribución por categoría</h5>
      <p className="quality-chart__note">
        Principales categorías de <strong>{summary.column}</strong>. Se muestran como máximo ocho
        grupos; los valores restantes se reúnen en “Resto” para mantener la lectura rápida y
        reducir la exposición de valores poco frecuentes.
      </p>
      <div className="quality-chart__bars" role="list" aria-label={`Distribución de grupos para ${summary.column}`}>
        {summary.groups.map((group) => {
          const percentage = clampPercentage(group.percentage);

          return (
            <div className="quality-chart__item" role="listitem" key={`${group.label}-${group.isOther}`}>
              <div className="quality-chart__label">
                <span title={group.label}>{group.label}</span>
                <strong>{group.rowCount.toLocaleString()} filas · {percentage.toFixed(1)}%</strong>
              </div>
              <div className="quality-chart__track" aria-hidden="true">
                <span
                  className={group.isOther ? "quality-chart__track-fill--other" : undefined}
                  style={{ width: `${percentage}%` }}
                />
              </div>
            </div>
          );
        })}
      </div>
      <div className="quality-chart__table">
        <table aria-label={tableLabel}>
          <caption className="visually-hidden">{tableLabel}</caption>
          <thead>
            <tr>
              <th scope="col">Grupo</th>
              <th scope="col">Filas</th>
              <th scope="col">Porcentaje</th>
            </tr>
          </thead>
          <tbody>
            {summary.groups.map((group) => (
              <tr key={`table-${group.label}-${group.isOther}`}>
                <th scope="row">
                  {group.label}
                  {group.isOther && <span className="visually-hidden">, categorías restantes</span>}
                </th>
                <td>{group.rowCount.toLocaleString()}</td>
                <td>{clampPercentage(group.percentage).toFixed(1)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        {summary.distinctCount.toLocaleString()} valores distintos detectados
        {summary.truncated ? "; el resto está agrupado para evitar ruido y preservar privacidad." : "."}
      </p>
    </div>
  );
}

function NumericCorrelationChart({ matrix }: { matrix: NumericCorrelationMatrix }) {
  return (
    <div className="quality-chart quality-chart--wide quality-correlation" role="group" aria-labelledby="quality-correlation-title">
      <h5 id="quality-correlation-title">Correlaciones numéricas</h5>
      <p className="quality-chart__note">
        Pearson entre pares disponibles. La lectura usa {matrix.sampledRowCount.toLocaleString()} filas
        {matrix.truncated ? " y muestra las primeras 12 columnas numéricas" : ""}.
      </p>
      <div className="quality-correlation__table" role="region" tabIndex={0} aria-label="Matriz de correlaciones numéricas">
        <table>
          <caption className="visually-hidden">Matriz de correlaciones numéricas de Pearson</caption>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              {matrix.columns.map((column) => (
                <th scope="col" key={column} title={column}>{column}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {matrix.columns.map((rowColumn, rowIndex) => (
              <tr key={rowColumn}>
                <th scope="row" title={rowColumn}>{rowColumn}</th>
                {matrix.columns.map((column, columnIndex) => {
                  const coefficient = correlationAt(matrix, rowIndex, columnIndex);
                  const label = correlationLabel(coefficient);
                  const isDiagonal = rowIndex === columnIndex;

                  return (
                    <td
                      className={correlationClass(coefficient, isDiagonal)}
                      key={column}
                      aria-label={`${rowColumn} con ${column}: ${label}`}
                    >
                      {label}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        Un valor cercano a 1 o -1 indica una relación lineal fuerte; “—” significa que no hubo
        suficientes valores o variación para calcularla.
      </p>
    </div>
  );
}

function correlationAt(matrix: NumericCorrelationMatrix, rowIndex: number, columnIndex: number): number | null {
  if (rowIndex === columnIndex) return 1;
  const firstIndex = Math.min(rowIndex, columnIndex);
  const secondIndex = Math.max(rowIndex, columnIndex);
  const firstColumn = matrix.columns[firstIndex];
  const secondColumn = matrix.columns[secondIndex];
  return (
    matrix.pairs.find(
      (pair) => pair.firstColumn === firstColumn && pair.secondColumn === secondColumn,
    )?.coefficient ?? null
  );
}

function correlationLabel(coefficient: number | null): string {
  return coefficient === null || !Number.isFinite(coefficient) ? "—" : coefficient.toFixed(2);
}

function correlationClass(coefficient: number | null, isDiagonal: boolean): string {
  if (isDiagonal) return "quality-correlation__cell quality-correlation__cell--diagonal";
  if (coefficient === null || !Number.isFinite(coefficient)) return "quality-correlation__cell";
  if (coefficient >= 0.7 || coefficient <= -0.7) return "quality-correlation__cell quality-correlation__cell--strong";
  if (coefficient >= 0.3 || coefficient <= -0.3) return "quality-correlation__cell quality-correlation__cell--moderate";
  return "quality-correlation__cell quality-correlation__cell--weak";
}

function isTemporalColumn(column: ColumnProfile): boolean {
  const dataType = column.dataType.trim().toLowerCase();
  return dataType === "date"
    || dataType.includes("datetime")
    || dataType.includes("timestamp")
    || column.suggestedType === "date";
}

function temporalTypeLabel(column: ColumnProfile): string {
  const dataType = column.dataType.trim().toLowerCase();
  if (dataType === "date") return "Fecha";
  if (dataType.includes("datetime") || dataType.includes("timestamp")) return "Fecha y hora";
  return "Fecha detectada";
}

function formatTemporalValue(value: string | null): string {
  if (!value) return "No disponible";
  return value.replace("T", " ").replace(/\+00:00$/, " UTC");
}

function temporalMetricLabel(metric: TemporalMetric): string {
  return metric === "rows" ? "Filas" : "Porcentaje de valores";
}

function temporalMetricValue(period: { rowCount: number; percentage: number }, metric: TemporalMetric): number {
  return metric === "rows" ? Math.max(0, period.rowCount) : clampPercentage(period.percentage);
}

function temporalMetricDisplay(value: number, metric: TemporalMetric): string {
  return metric === "rows" ? Math.round(value).toLocaleString() : `${clampPercentage(value).toFixed(0)}%`;
}

function temporalAxisIndexes(periodCount: number): number[] {
  if (periodCount <= 3) return Array.from({ length: periodCount }, (_, index) => index);
  const indexes = [0, Math.floor((periodCount - 1) / 2), periodCount - 1];
  return [...new Set(indexes)];
}

function formatTemporalAxisLabel(period: string, granularity: TemporalSeriesSummary["granularity"]): string {
  if (granularity === "day") return formatCalendarDay(period);
  return period;
}

function parseTemporalDay(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  if (
    date.getUTCFullYear() !== Number(match[1])
    || date.getUTCMonth() !== Number(match[2]) - 1
    || date.getUTCDate() !== Number(match[3])
  ) {
    return null;
  }
  return date;
}

function temporalDayWeekday(value: string | undefined): number | null {
  if (!value) return null;
  const date = parseTemporalDay(value);
  return date ? (date.getUTCDay() + 6) % 7 : null;
}

function formatCalendarDay(value: string): string {
  const date = parseTemporalDay(value);
  return date
    ? new Intl.DateTimeFormat("es", {
        day: "numeric",
        month: "short",
        timeZone: "UTC",
      }).format(date)
    : value;
}

function clampPercentage(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

function suggestedTypeLabel(type: string | null): string {
  switch (type) {
    case "boolean":
      return "Booleano";
    case "integer":
      return "Entero";
    case "decimal":
      return "Decimal";
    case "date":
      return "Fecha";
    default:
      return "—";
  }
}

function formatStatistic(value: number | null): string {
  return value?.toLocaleString(undefined, { maximumFractionDigits: 3 }) ?? "—";
}

function histogramIntervalLabel(lower: number, upper: number, includesMaximum: boolean): string {
  return `${formatStatistic(lower)} – ${formatStatistic(upper)}${includesMaximum ? " (incluye máximo)" : ""}`;
}
