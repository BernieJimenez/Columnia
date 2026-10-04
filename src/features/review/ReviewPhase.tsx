import { useEffect, useRef, useState, type ReactNode } from "react";

import { renderCellValue } from "../../components/CellText";
import { OperationProgressView } from "../../components/OperationProgressView";
import { ReviewTabList, type ReviewTab } from "../../components/ReviewTabList";
import { cancelOperation, queryDataset } from "../../bridge";
import type {
  DatasetQueryEngine,
  DatasetPreview,
  DatasetProfile,
  DatasetQueryResult,
  SqlQueryHistoryEntry,
} from "../../bridge";
import type { ReadyDatasetStatus } from "../load/loadModel";
import {
  ANALYSIS_SAMPLE_ROW_OPTIONS,
  isAnalysisSampleRows,
  commaDecimalColumns,
  nextPageOffset,
  pageRange,
  readAnalysisSampleRowsPreference,
  previousPageOffset,
  readQueryEnginePreference,
  type AnalysisSampleRows,
  type ProfileStatus,
  writeAnalysisSampleRowsPreference,
  writeQueryEnginePreference,
} from "./reviewModel";
import { DatasetComparisonSection } from "./DatasetComparisonSection";
import { QualitySnapshot } from "./QualitySnapshot";
import { buildQualityActionPlan } from "./qualityActionPlan";
import {
  buildPrepareProposal,
  defaultProposalSelection,
  proposalItemTitle,
} from "../prepare/proposalModel";
import type { QualityActionTarget } from "./qualityActionPlan";
import type { ReviewComparison } from "./useReviewController";
import { formatBytes, formatDataType, formatDecimal, formatPercent } from "../../format";
import {
  QualityVisuals,
  formatStatistic,
  profileColumnTypeLabel,
  suggestedTypeLabel,
} from "./ReviewCharts";

interface ReviewPhaseProps {
  datasetStatus: ReadyDatasetStatus;
  profileStatus: ProfileStatus;
  reviewTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
  onPageChange: (offset: number) => void;
  onCancelPageChange?: () => void;
  pageCancellationPending?: boolean;
  onCancelProfile: () => void;
  onContinueToPrepare?: (target?: QualityActionTarget) => void;
  comparison: ReviewComparison;
  datasetRevision?: number;
  sqlHistory?: SqlQueryHistoryEntry[];
  onSqlHistoryChange?: (entries: SqlQueryHistoryEntry[]) => void;
  queryEngine?: DatasetQueryEngine;
  onQueryEngineChange?: (engine: DatasetQueryEngine) => void;
  analysisSampleRows?: AnalysisSampleRows;
  onAnalysisSampleRowsChange?: (sampleRows: AnalysisSampleRows) => void;
}

export function ReviewPhase({
  datasetStatus,
  profileStatus,
  reviewTab,
  onTabChange,
  onPageChange,
  onCancelPageChange = () => undefined,
  pageCancellationPending = false,
  onCancelProfile,
  onContinueToPrepare = () => undefined,
  comparison,
  datasetRevision = 0,
  sqlHistory = NO_SQL_HISTORY,
  onSqlHistoryChange = () => undefined,
  queryEngine,
  onQueryEngineChange = () => undefined,
  analysisSampleRows,
  onAnalysisSampleRowsChange = () => undefined,
}: ReviewPhaseProps) {
  const comparisonStatus = comparison.status;
  const comparisonActive = comparisonStatus.kind !== "idle" || comparison.joinStatus.kind !== "idle";
  const [comparisonOpen, setComparisonOpen] = useState(comparisonActive);
  const [localAnalysisSampleRows, setLocalAnalysisSampleRows] = useState(readAnalysisSampleRowsPreference);
  const [sqlDraft, setSqlDraft] = useState<SqlQueryDraft | null>(null);
  const selectedAnalysisSampleRows = analysisSampleRows ?? localAnalysisSampleRows;

  useEffect(() => {
    if (comparisonActive) setComparisonOpen(true);
  }, [comparisonActive]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Revisa antes de modificar</h2>
          <h3 className="phase-file">{datasetStatus.dataset.fileName}</h3>
          <p className="phase-meta">
            {datasetStatus.dataset.rowCount.toLocaleString()} filas · {datasetStatus.dataset.columnCount.toLocaleString()} columnas · {formatBytes(datasetStatus.dataset.fileSizeBytes)}
          </p>
        </div>
      </header>
      <ReviewTabList activeTab={reviewTab} onTabChange={onTabChange} />

      {reviewTab === "diagnosis" ? (
        <div id="review-diagnosis-panel" role="tabpanel" aria-labelledby="review-diagnosis-tab">
          <QualitySection
            status={profileStatus}
            dataset={datasetStatus.dataset}
            onCancel={onCancelProfile}
            onContinueToPrepare={onContinueToPrepare}
            comparisonAvailable={comparisonStatus.kind === "ready"}
            sqlHistory={sqlHistory}
            onSqlHistoryChange={onSqlHistoryChange}
            sqlDraft={sqlDraft}
            onSqlDraftChange={setSqlDraft}
            queryEngine={queryEngine}
            onQueryEngineChange={onQueryEngineChange}
            analysisSampleRows={selectedAnalysisSampleRows}
            onAnalysisSampleRowsChange={(sampleRows) => {
              setLocalAnalysisSampleRows(sampleRows);
              writeAnalysisSampleRowsPreference(sampleRows);
              onAnalysisSampleRowsChange(sampleRows);
            }}
            datasetRevision={datasetRevision}
          />
        </div>
      ) : (
        <div id="review-preview-panel" role="tabpanel" aria-labelledby="review-preview-tab">
          <DatasetPreviewPanel
            dataset={datasetStatus.dataset}
            pageOffset={datasetStatus.pageOffset}
            pageLoading={datasetStatus.pageLoading}
            pageCancellationPending={pageCancellationPending}
            pageError={datasetStatus.pageError}
            onPageChange={onPageChange}
            onCancelPageChange={onCancelPageChange}
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
          datasetColumns={datasetStatus.dataset.columns}
          datasetRevision={datasetRevision}
          keyColumns={comparison.keyColumns}
          onKeyColumnsChange={comparison.onKeyColumnsChange}
          onCompare={comparison.onCompare}
          onCancelComparison={comparison.onCancelComparison ?? (() => undefined)}
          comparisonCancellationPending={comparison.cancellationPending ?? false}
          onClear={comparison.onClear}
          onConsolidate={comparison.onConsolidate}
          onResolveConflicts={comparison.onResolveConflicts}
          onConflictPageChange={comparison.onConflictPageChange}
          joinStatus={comparison.joinStatus}
          reviewMutationStatus={comparison.mutationStatus ?? { kind: "idle" }}
          reviewMutationCancellationPending={comparison.mutationCancellationPending ?? false}
          joinType={comparison.joinType}
          onJoinTypeChange={comparison.onJoinTypeChange}
          onJoin={comparison.onJoin}
          onCancelReviewMutation={comparison.onCancelMutation ?? (() => undefined)}
        />
      </details>
    </>
  );
}

function QualitySection({
  status,
  onCancel,
  onContinueToPrepare,
  comparisonAvailable,
  sqlHistory,
  onSqlHistoryChange,
  sqlDraft,
  onSqlDraftChange,
  queryEngine,
  onQueryEngineChange,
  analysisSampleRows,
  onAnalysisSampleRowsChange,
  datasetRevision,
  dataset,
}: {
  status: ProfileStatus;
  dataset: DatasetPreview;
  onCancel: () => void;
  onContinueToPrepare: (target?: QualityActionTarget) => void;
  comparisonAvailable: boolean;
  sqlHistory: SqlQueryHistoryEntry[];
  onSqlHistoryChange: (entries: SqlQueryHistoryEntry[]) => void;
  sqlDraft: SqlQueryDraft | null;
  onSqlDraftChange: (draft: SqlQueryDraft) => void;
  queryEngine?: DatasetQueryEngine;
  onQueryEngineChange: (engine: DatasetQueryEngine) => void;
  analysisSampleRows: AnalysisSampleRows;
  onAnalysisSampleRowsChange: (sampleRows: AnalysisSampleRows) => void;
  datasetRevision: number;
}) {
  const tools = (
    <>
      <details className="review-tool review-tool--nested quality-settings">
        <summary>
          <span>Configuración del análisis</span>
          <small>Muestra usada para calcular correlaciones</small>
        </summary>
        <div className="quality-sample-control">
          <label>
            Filas de muestra para correlaciones
            <select
              aria-label="Filas de muestra para correlaciones"
              value={analysisSampleRows}
              onChange={(event) => {
                const nextSampleRows = Number(event.target.value);
                if (isAnalysisSampleRows(nextSampleRows)) onAnalysisSampleRowsChange(nextSampleRows);
              }}
            >
              {ANALYSIS_SAMPLE_ROW_OPTIONS.map((sampleRows) => (
                <option key={sampleRows} value={sampleRows}>
                  {sampleRows.toLocaleString()} filas
                </option>
              ))}
            </select>
          </label>
          <p>Se aplica al próximo análisis y solo limita las correlaciones; el resto del perfil conserva su cobertura.</p>
        </div>
      </details>
      <LocalQueryPanel
        comparisonAvailable={comparisonAvailable}
        queryHistory={sqlHistory}
        onQueryHistoryChange={onSqlHistoryChange}
        draft={sqlDraft}
        onDraftChange={onSqlDraftChange}
        queryEngine={queryEngine}
        onQueryEngineChange={onQueryEngineChange}
        datasetRevision={datasetRevision}
      />
    </>
  );
  return (
    <section className="phase-section" aria-labelledby="quality-title">
      <h3 id="quality-title" className="visually-hidden">Diagnóstico del dataset</h3>
      {status.kind === "loading" && (
        <OperationProgressView
          progress={status.progress}
          cancellation={status.cancelRequested
            ? { kind: "requested" }
            : {
                kind: "available",
                onCancel,
                ...(status.cancellationError ? { error: status.cancellationError } : {}),
              }}
        />
      )}
      {status.kind === "cancelled" && (
        <p className="notice" role="status">
          El análisis se canceló. No se publicó ningún resultado parcial; puedes
          reintentarlo desde la acción principal.
        </p>
      )}
      {status.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {status.message}
        </p>
      )}
      {status.kind === "ready" ? (
        <QualityProfile
          profile={status.profile}
          dataset={dataset}
          onContinueToPrepare={onContinueToPrepare}
          datasetRevision={datasetRevision}
          extraTools={tools}
        />
      ) : (
        <ReviewMoreTools>{tools}</ReviewMoreTools>
      )}
    </section>
  );
}

/** A stable default, so that a re-render does not look like a new history. */
const NO_SQL_HISTORY: SqlQueryHistoryEntry[] = [];

/** The SQL being written and its last result, kept while Review stays open (FUN-27). */
export type SqlQueryDraft = {
  text: string;
  open: boolean;
  result: DatasetQueryResult | null;
  revision: number;
};

function LocalQueryPanel({
  comparisonAvailable,
  queryHistory: persistedQueryHistory,
  onQueryHistoryChange,
  draft,
  onDraftChange,
  queryEngine,
  onQueryEngineChange,
  datasetRevision,
}: {
  comparisonAvailable: boolean;
  queryHistory: SqlQueryHistoryEntry[];
  onQueryHistoryChange: (entries: SqlQueryHistoryEntry[]) => void;
  draft: SqlQueryDraft | null;
  onDraftChange: (draft: SqlQueryDraft) => void;
  queryEngine?: DatasetQueryEngine;
  onQueryEngineChange: (engine: DatasetQueryEngine) => void;
  datasetRevision: number;
}) {
  // A result belongs to its revision; the text survives a new revision.
  const restoredResult = draft?.revision === datasetRevision ? draft.result : null;
  const [query, setQuery] = useState(draft?.text ?? "SELECT * FROM dataset LIMIT 50");
  const [localQueryEngine, setLocalQueryEngine] = useState<DatasetQueryEngine>(readQueryEnginePreference);
  const selectedQueryEngine = queryEngine ?? localQueryEngine;
  const [state, setState] = useState<
    | { kind: "idle" }
    | { kind: "loading"; cancelRequested: boolean }
    | { kind: "ready"; result: DatasetQueryResult }
    | { kind: "cancelled" }
    | { kind: "error"; message: string }
  >(restoredResult ? { kind: "ready", result: restoredResult } : { kind: "idle" });
  const [queryOpen, setQueryOpen] = useState(draft?.open ?? false);
  const [queryHistory, setQueryHistory] = useState<SqlQueryHistoryEntry[]>(persistedQueryHistory);
  const activeQueryRef = useRef(0);
  const cancelledQueryRef = useRef<number | null>(null);
  const queryStartedAtRef = useRef(new Map<number, number>());
  const recordedQueryIdsRef = useRef<number[]>([]);
  const queryHistoryRef = useRef(queryHistory);
  const queryRevisionRef = useRef(datasetRevision);

  useEffect(() => {
    if (queryRevisionRef.current === datasetRevision) return;
    queryRevisionRef.current = datasetRevision;
    activeQueryRef.current += 1;
    cancelledQueryRef.current = null;
    queryStartedAtRef.current.clear();
    recordedQueryIdsRef.current = [];
    queryHistoryRef.current = [];
    setQueryHistory([]);
    setState({ kind: "idle" });
    onQueryHistoryChange([]);
  }, [datasetRevision, onQueryHistoryChange]);

  useEffect(() => {
    if (queryHistoryRef.current === persistedQueryHistory) {
      activeQueryRef.current = Math.max(
        activeQueryRef.current,
        ...persistedQueryHistory.map((entry) => entry.id),
        0,
      );
      return;
    }
    queryHistoryRef.current = persistedQueryHistory;
    setQueryHistory(persistedQueryHistory);
    activeQueryRef.current = Math.max(
      activeQueryRef.current,
      ...persistedQueryHistory.map((entry) => entry.id),
      0,
    );
  }, [persistedQueryHistory]);

  useEffect(() => {
    if (state.kind !== "idle") setQueryOpen(true);
  }, [state.kind]);

  useEffect(() => {
    onDraftChange({
      text: query,
      open: queryOpen,
      result: state.kind === "ready" ? state.result : null,
      revision: datasetRevision,
    });
  }, [query, queryOpen, state, datasetRevision, onDraftChange]);

  // A query still running when the panel goes away is cancelled (FUN-27).
  const runningRef = useRef(false);
  runningRef.current = state.kind === "loading";
  useEffect(() => () => {
    if (runningRef.current) void cancelOperation("query").catch(() => undefined);
  }, []);

  function recordQueryHistory(
    requestId: number,
    outcome: SqlQueryHistoryEntry["outcome"],
    rowCount: number | null = null,
  ) {
    if (recordedQueryIdsRef.current.includes(requestId)) return;
    recordedQueryIdsRef.current = [...recordedQueryIdsRef.current.slice(-31), requestId];
    const startedAt = queryStartedAtRef.current.get(requestId) ?? Date.now();
    queryStartedAtRef.current.delete(requestId);
    const nextHistory = [
      { id: requestId, outcome, durationMs: Math.max(0, Date.now() - startedAt), rowCount },
      ...queryHistoryRef.current,
    ].slice(0, 5);
    queryHistoryRef.current = nextHistory;
    setQueryHistory(nextHistory);
    onQueryHistoryChange(nextHistory);
  }

  async function runQuery() {
    const requestId = activeQueryRef.current + 1;
    activeQueryRef.current = requestId;
    cancelledQueryRef.current = null;
    queryStartedAtRef.current.set(requestId, Date.now());
    setState({ kind: "loading", cancelRequested: false });
    try {
      const result = await queryDataset(query, selectedQueryEngine);
      if (activeQueryRef.current !== requestId) return;
      if (cancelledQueryRef.current === requestId) {
        recordQueryHistory(requestId, "cancelled");
        setState({ kind: "cancelled" });
        return;
      }
      recordQueryHistory(requestId, "success", result.rowCount);
      setState({ kind: "ready", result });
    } catch (error: unknown) {
      if (activeQueryRef.current !== requestId) return;
      if (cancelledQueryRef.current === requestId) {
        recordQueryHistory(requestId, "cancelled");
        setState({ kind: "cancelled" });
        return;
      }
      recordQueryHistory(requestId, "error");
      setState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function cancelQuery() {
    if (state.kind !== "loading" || state.cancelRequested) return;
    const requestId = activeQueryRef.current;
    cancelledQueryRef.current = requestId;
    setState({ kind: "loading", cancelRequested: true });
    try {
      await cancelOperation("query");
      if (activeQueryRef.current === requestId) {
        recordQueryHistory(requestId, "cancelled");
        setState({ kind: "cancelled" });
      }
    } catch (error: unknown) {
      if (activeQueryRef.current !== requestId) return;
      cancelledQueryRef.current = null;
      setState({
        kind: "error",
        message: `No se pudo cancelar la consulta: ${error instanceof Error ? error.message : String(error)}`,
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
        <label className="local-query__field">
          Motor de consulta
          <select
            aria-label="Motor de consulta"
            value={selectedQueryEngine}
            onChange={(event) => {
              const nextEngine = event.target.value as DatasetQueryEngine;
              setLocalQueryEngine(nextEngine);
              writeQueryEnginePreference(nextEngine);
              onQueryEngineChange(nextEngine);
            }}
          >
            <option value="polars">Polars · predeterminado</option>
            <option value="duckdb">DuckDB · SQL local</option>
          </select>
        </label>
        <p className="local-query__engine-note">
          DuckDB ejecuta la consulta sobre snapshots Parquet temporales y mantiene el mismo límite seguro de 200 filas.
        </p>
        <div className="local-query__actions">
          <button type="button" onClick={() => void runQuery()} disabled={state.kind === "loading" || !query.trim()}>
            {state.kind === "loading" ? "Consultando…" : "Ejecutar consulta"}
          </button>
          {state.kind === "loading" && (
            <button
              type="button"
              className="secondary-action"
              onClick={() => void cancelQuery()}
              disabled={state.cancelRequested}
              aria-describedby="local-query-status"
            >
              {state.cancelRequested ? "Cancelando…" : "Cancelar consulta"}
            </button>
          )}
        </div>
        {queryHistory.length > 0 && (
          <section className="local-query__history" aria-labelledby="local-query-history-title">
            <div className="local-query__history-heading">
              <div>
                <p className="step">Sesión actual</p>
                <h5 id="local-query-history-title">Actividad reciente</h5>
              </div>
              <span>Últimas {queryHistory.length} consultas</span>
            </div>
            <ol aria-label="Historial de consultas SQL">
              {queryHistory.map((entry) => (
                <li key={entry.id} className={`local-query__history-item local-query__history-item--${entry.outcome}`}>
                  <strong>
                    {entry.outcome === "success"
                      ? "Completada"
                      : entry.outcome === "cancelled"
                        ? "Cancelada"
                        : "Error"}
                  </strong>
                  <span>{formatQueryDuration(entry.durationMs)}</span>
                  <span>{entry.rowCount === null ? "Sin resultado" : `${entry.rowCount.toLocaleString()} filas`}</span>
                </li>
              ))}
            </ol>
          </section>
        )}
        {state.kind === "loading" && (
          <p id="local-query-status" className="local-query__status" role="status" aria-live="polite">
            {state.cancelRequested ? "Solicitando la cancelación…" : "La consulta sigue en ejecución."}
          </p>
        )}
        {state.kind === "cancelled" && (
          <p className="notice" role="status" aria-live="polite">
            Consulta cancelada. No se actualizó el resultado.
          </p>
        )}
        {state.kind === "error" && <p className="notice notice--error" role="alert">No se pudo ejecutar la consulta: {state.message}</p>}
        {state.kind === "ready" && <LocalQueryResult result={state.result} />}
      </section>
    </details>
  );
}

function formatQueryDuration(durationMs: number): string {
  return durationMs < 1000
    ? `${durationMs} ms`
    : `${formatDecimal(durationMs / 1000, 1)} s`;
}

function LocalQueryResult({ result }: { result: DatasetQueryResult }) {
  return (
    <div className="local-query__result" role="status" aria-live="polite">
      <p>
        {result.rowCount.toLocaleString()} filas disponibles · mostrando desde {result.offset + 1}
        {result.truncated ? " · resultado truncado por LIMIT" : ""}
        {result.engine ? ` · calculado con ${result.engine === "duckdb" ? "DuckDB" : "Polars"}` : ""}
      </p>
      <div className="profile-region" role="region" tabIndex={0} aria-label="Resultado de consulta SQL">
        <table>
          <thead><tr>{result.columns.map((column) => <th key={column.name} scope="col"><span>{column.name}</span><small>{formatDataType(column.dataType)}</small></th>)}</tr></thead>
          <tbody>{result.rows.map((row, rowIndex) => <tr key={result.offset + rowIndex}>{row.map((value, columnIndex) => <td key={columnIndex}>{renderCellValue(value)}</td>)}</tr>)}</tbody>
        </table>
      </div>
    </div>
  );
}

interface DatasetPreviewProps {
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageCancellationPending?: boolean;
  pageError?: string;
  onPageChange: (offset: number) => void;
  onCancelPageChange?: () => void;
}

export function DatasetPreviewPanel({
  dataset,
  pageOffset,
  pageLoading,
  pageCancellationPending = false,
  pageError,
  onPageChange,
  onCancelPageChange = () => undefined,
}: DatasetPreviewProps) {
  const { end: pageEnd, hasPrevious, hasNext } = pageRange(dataset, pageOffset);

  return (
    <>
      <div className="table-region" tabIndex={0} aria-label="Vista previa del dataset">
        <table>
          <thead>
            <tr>
              {dataset.columns.map((column) => (
                <th key={column.name} scope="col" aria-label={`${column.name} ${formatDataType(column.dataType)}`}>
                  <span>{column.name}</span>
                  <small>{formatDataType(column.dataType)}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {dataset.rows.map((row, rowIndex) => (
              <tr key={pageOffset + rowIndex}>
                {row.map((value, columnIndex) => (
                  <td key={columnIndex}>{renderCellValue(value)}</td>
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
          {pageLoading && (
            <button
              type="button"
              className="secondary-action"
              onClick={onCancelPageChange}
              disabled={pageCancellationPending}
            >
              {pageCancellationPending ? "Cancelando carga…" : "Cancelar carga"}
            </button>
          )}
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

/** One disclosure for everything beyond the summary, so Revisar shows one decision. */
function ReviewMoreTools({ children }: { children: ReactNode }) {
  return (
    <details className="review-tool review-more">
      <summary>
        <span>Más análisis y herramientas</span>
        <small>Detalle, perfil por columna, configuración y SQL</small>
      </summary>
      <div className="review-more__content">{children}</div>
    </details>
  );
}

function QualityProfile({
  profile,
  dataset,
  onContinueToPrepare,
  datasetRevision,
  extraTools,
}: {
  profile: DatasetProfile;
  dataset: DatasetPreview;
  onContinueToPrepare: (target?: QualityActionTarget) => void;
  datasetRevision: number;
  extraTools: ReactNode;
}) {
  const textColumns = profile.columns.filter((column) => column.emptyCount !== null);
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);
  const columnsWithNulls = profile.columns.filter((column) => column.nullCount > 0 && column.name !== "_cambios");
  const totalNullCount = columnsWithNulls.reduce((total, column) => total + column.nullCount, 0);
  const invalidTypeCount = profile.columns.reduce(
    (total, column) => total + (column.name === "_cambios" ? 0 : Math.max(0, column.invalidTypeCount ?? 0)),
    0,
  );
  const actionPlan = buildQualityActionPlan({
    nullCount: totalNullCount,
    nullColumnCount: columnsWithNulls.length,
    duplicateCount: profile.duplicateRowCount,
    invalidTypeCount,
  });
  // The summary lists exactly what Preparar will propose, so Revisar never
  // announces a problem the proposal does not address, or hides one it does.
  const proposal = buildPrepareProposal(profile, dataset);
  const proposalSelection = defaultProposalSelection(proposal);
  const priorityCount = proposal.length;
  const commaColumns = commaDecimalColumns(profile);
  return (
    <>
      <section className="quality-overview quality-overview--plain" aria-labelledby="quality-overview-title">
        <h4 id="quality-overview-title">
          {priorityCount === 0
            ? "No encontramos nada que arreglar"
            : `Encontramos ${priorityCount} ${priorityCount === 1 ? "cosa" : "cosas"} para arreglar`}
        </h4>
        {proposal.length > 0 && (
          <ul className="quality-issues" aria-label="Resumen de calidad del dataset">
            {proposal.map((item) => (
              <li key={item.id}>{proposalItemTitle(item, proposalSelection)}</li>
            ))}
          </ul>
        )}
        {/* No signal focus: the proposal opens on its own, without the advanced tools. */}
        <button
          className="primary-action quality-overview__continue"
          type="button"
          onClick={() => onContinueToPrepare()}
        >
          {proposal.length > 0 ? "Ver cambios propuestos" : "Continuar a Preparar"}
        </button>
        {commaColumns.length > 0 && (
          <p className="quality-overview__note" role="note">
            {commaColumns.length === 1 ? `«${commaColumns[0]}» parece` : `${commaColumns.map((name) => `«${name}»`).join(", ")} parecen`}{" "}
            usar coma decimal (1,5 o 1.234,56) y por eso no se analiza como número. Vuelve a cargar el archivo eligiendo
            «Decimal coma · miles punto» en Números.
          </p>
        )}
      </section>
      <ReviewMoreTools>
        <QualitySnapshot
          rowCount={profile.rowCount}
          columnCount={profile.columns.filter((column) => column.name !== "_cambios").length}
          nullCount={totalNullCount}
          duplicateCount={profile.duplicateRowCount}
          duplicatePercentage={profile.duplicatePercentage}
          invalidTypeCount={invalidTypeCount}
        />
        {actionPlan.length > 0 && (
          <ol className="quality-action-plan" aria-label="Prioridades de revisión">
            {actionPlan.map((action) => (
              <li key={action.target}>
                <div>
                  <h5>{action.title}</h5>
                  <p>{action.explanation}</p>
                  <p className="quality-action-plan__impact">{action.impact}</p>
                </div>
              </li>
            ))}
          </ol>
        )}
        <p className="quality-overview__meta">
          <span>Filas analizadas</span>
          <strong>{profile.rowCount.toLocaleString()}</strong>
        </p>
      <details className="review-tool quality-details">
        <summary>
          <span>Explorar análisis detallado</span>
          <small>Gráficos, distribuciones, fechas y correlaciones</small>
        </summary>
        <QualityVisuals profile={profile} datasetRevision={datasetRevision} />
      </details>
      <details className="review-tool quality-details">
        <summary>
          <span>Ver perfil por columna</span>
          <small>Valores exactos y estadísticas por tipo</small>
        </summary>
        <div className="quality-profile-tables">
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
                  <small>{profileColumnTypeLabel(column)}</small>
                </th>
                <td>{formatPercent(column.completenessPercentage, 1)}</td>
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
                        : `${formatPercent(column.typeMatchPercentage, 1)}`}
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
        </div>
      </details>
      {extraTools}
      </ReviewMoreTools>
    </>
  );
}
