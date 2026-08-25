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
  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Revisar · Dataset activo</p>
          <h2>{datasetStatus.dataset.fileName}</h2>
          <p>Comprueba la estructura, la calidad y una muestra de los datos antes de modificarlos.</p>
        </div>
      </header>
      <ReviewTabList activeTab={reviewTab} onTabChange={onTabChange} />
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

      {reviewTab === "diagnosis" ? (
        <div id="review-diagnosis-panel" role="tabpanel" aria-labelledby="review-diagnosis-tab">
          <QualitySection
            dataset={datasetStatus.dataset}
            status={profileStatus}
            onAnalyze={onAnalyzeQuality}
            onCancel={onCancelProfile}
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
          <p className="step">Paridad de fuentes</p>
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
}: {
  dataset: DatasetPreview;
  status: ProfileStatus;
  onAnalyze: () => void;
  onCancel: () => void;
}) {
  return (
    <section className="phase-section" aria-labelledby="quality-title">
      <div className="section-heading">
        <div>
          <p className="step">Calidad inicial</p>
          <h3 id="quality-title">Perfil por columna</h3>
        </div>
        {status.kind !== "loading" && (
          <button type="button" onClick={onAnalyze}>
            {status.kind === "ready" ? "Analizar de nuevo" : "Analizar calidad"}
          </button>
        )}
      </div>
      <DatasetMetrics dataset={dataset} />
      <LocalQueryPanel />
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
    </section>
  );
}

function LocalQueryPanel() {
  const [query, setQuery] = useState("SELECT * FROM dataset LIMIT 50");
  const [state, setState] = useState<
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ready"; result: DatasetQueryResult }
    | { kind: "error"; message: string }
  >({ kind: "idle" });

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
    <section className="local-query" aria-labelledby="local-query-title">
      <div className="local-query__heading">
        <div>
          <p className="step">Consulta segura</p>
          <h4 id="local-query-title">Explorar con SQL local</h4>
          <p>Solo se acepta SELECT sobre <code>dataset</code>, columnas existentes, filtros simples, GROUP BY y COUNT/SUM/AVG/MIN/MAX; LIMIT/OFFSET queda acotado a 200 filas.</p>
        </div>
        <button type="button" onClick={() => void runQuery()} disabled={state.kind === "loading" || !query.trim()}>
          {state.kind === "loading" ? "Consultando…" : "Ejecutar consulta"}
        </button>
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
      {state.kind === "error" && <p className="notice notice--error" role="alert">No se pudo ejecutar la consulta: {state.message}</p>}
      {state.kind === "ready" && <LocalQueryResult result={state.result} />}
    </section>
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
  const distributionColumns = numericColumns.filter((column) =>
    [column.minimum, column.maximum, column.firstQuartile, column.median, column.thirdQuartile]
      .every((value) => value !== null && Number.isFinite(Number(value))),
  );
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
      </div>
    </section>
  );
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
