import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { busyDescribedBy } from "../../busyReasons";
import { useLatest } from "../../useLatest";

import { renderCellValue } from "../../components/CellText";
import { ColumnProfileHeader } from "../../components/ColumnProfileHeader";
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
  nextPageOffset,
  profileProvenanceText,
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
import { allRows, buildUncorrectedIssues } from "./qualityActionPlan";
import { byImpact, proposalImpact } from "./proposalImpact";
import {
  buildPrepareProposal,
  defaultProposalSelection,
  proposalItemTitle,
} from "../prepare/proposalModel";
import type { QualityActionTarget } from "./qualityActionPlan";
import type { ReviewComparison } from "./useReviewController";
import { formatDataType, formatDecimal, formatNumber, formatPercent } from "../../format";
import { isRowAuditColumn } from "../../rowAudit";
import { isNumericType } from "../../dataTypes";
import {
  QualityVisuals,
  formatStatistic,
  readsAsCode,
  profileColumnTypeLabel,
  suggestedTypeLabel,
} from "./ReviewCharts";
import { plural } from "../../plural";
import { errorMessage } from "../../bridge/errors";

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
  /** «Volver a cargar con coma decimal»: goes back to Cargar. Without it the action is not offered. */
  onReturnToLoad?: () => void;
  /** Runs the quality analysis again; the profile error carries this button. */
  onRetryProfile?: () => void;
  comparison: ReviewComparison;
  /** «Combinar en Preparar»: Revisar compares; Preparar joins and consolidates. */
  onCombineInPrepare?: () => void;
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
  onReturnToLoad,
  onRetryProfile,
  comparison,
  onCombineInPrepare,
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
          <p className="phase-lead">Entiende la calidad y la estructura de los datos. Nada cambia hasta que apruebes una corrección.</p>
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
            onReturnToLoad={onReturnToLoad}
            onRetryProfile={onRetryProfile}
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
            profile={profileStatus.kind === "ready" ? profileStatus.profile : undefined}
            profilePending={profileStatus.kind === "idle" || profileStatus.kind === "loading"}
          />
        </div>
      )}
      <details
        className="review-tool"
        open={comparisonOpen}
        onToggle={(event) => setComparisonOpen(event.currentTarget.open)}
      >
        <summary>
          <span>Comparar con otro archivo</span>
          <small>Opcional · filas nuevas, filas que faltan y conflictos</small>
        </summary>
        <DatasetComparisonSection
          comparison={comparison}
          datasetColumns={datasetStatus.dataset.columns}
          onCombineInPrepare={onCombineInPrepare}
        />
      </details>
    </>
  );
}

function QualitySection({
  status,
  onCancel,
  onContinueToPrepare,
  onReturnToLoad,
  onRetryProfile,
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
  onReturnToLoad?: () => void;
  onRetryProfile?: () => void;
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
          <p>
            {analysisSampleRows >= dataset.rowCount
              ? `Con esta muestra las correlaciones usan todas las filas (${dataset.rowCount.toLocaleString()}). `
              : ""}
            Al cambiarla, el análisis se repite con esta muestra; solo limita las correlaciones y el resto del perfil conserva su cobertura.
          </p>
        </div>
      </details>
      <LocalQueryPanel
        columnNames={dataset.columns.map((column) => column.name).filter((name) => !isRowAuditColumn(name))}
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
          El análisis se canceló. No se publicó ningún resultado parcial; pulsa
          «Reintentar análisis», abajo, para volver a empezar.
        </p>
      )}
      {status.kind === "error" && <ProfileErrorNotice message={status.message} onRetry={onRetryProfile} />}
      {status.kind === "ready" ? (
        <QualityProfile
          profile={status.profile}
          dataset={dataset}
          onContinueToPrepare={onContinueToPrepare}
          onReturnToLoad={onReturnToLoad}
          datasetRevision={datasetRevision}
          extraTools={tools}
        />
      ) : (
        <ReviewMoreTools>{tools}</ReviewMoreTools>
      )}
    </section>
  );
}

/**
 * The profile failed: the notice names a likely cause and carries its own
 * retry. When it appears while focus is on the page body or on the flow footer
 * (where the analysis was started), focus moves here so it is heard.
 */
function ProfileErrorNotice({ message, onRetry }: { message: string; onRetry?: () => void }) {
  const noticeRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const active = document.activeElement;
    const lost = !active
      || active === document.body
      || (active instanceof HTMLElement && (active.closest(".flow-footer") !== null || active.matches(":disabled")));
    if (lost) noticeRef.current?.focus();
  }, []);
  return (
    <div ref={noticeRef} className="notice notice--error profile-error" role="alert" tabIndex={-1}>
      <p>No se pudo analizar la calidad: {message}</p>
      <p>
        Causa probable: el archivo cambió o se movió mientras se leía, o el equipo se quedó sin memoria.
        Comprueba que el archivo sigue donde estaba y vuelve a intentarlo; si falla otra vez, vuelve a Cargar el archivo.
      </p>
      {onRetry && (
        <button type="button" className="secondary-action" onClick={onRetry}>
          Reintentar análisis
        </button>
      )}
    </div>
  );
}

/** A stable default, so that a re-render does not look like a new history. */
const NO_SQL_HISTORY: SqlQueryHistoryEntry[] = [];

/**
 * PROD-12: a column name as both engines read it: plain when it is a simple
 * identifier, otherwise between double quotes with inner quotes doubled.
 */
export function sqlIdentifier(name: string): string {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) ? name : `"${name.replaceAll('"', '""')}"`;
}

/** The SQL being written and its last result, kept while Review stays open (FUN-27). */
export type SqlQueryDraft = {
  text: string;
  open: boolean;
  result: DatasetQueryResult | null;
  revision: number;
};

export function LocalQueryPanel({
  columnNames,
  comparisonAvailable,
  queryHistory: persistedQueryHistory,
  onQueryHistoryChange,
  draft,
  onDraftChange,
  queryEngine,
  onQueryEngineChange,
  datasetRevision,
  standalone = false,
}: {
  columnNames: string[];
  comparisonAvailable: boolean;
  queryHistory: SqlQueryHistoryEntry[];
  onQueryHistoryChange: (entries: SqlQueryHistoryEntry[]) => void;
  draft: SqlQueryDraft | null;
  onDraftChange: (draft: SqlQueryDraft) => void;
  queryEngine?: DatasetQueryEngine;
  onQueryEngineChange: (engine: DatasetQueryEngine) => void;
  datasetRevision: number;
  /** Shown open on its own (the SQL console), not as a disclosure of Revisar. */
  standalone?: boolean;
}) {
  // A result belongs to its revision; the text survives a new revision.
  const restoredResult = draft?.revision === datasetRevision ? draft.result : null;
  const [query, setQuery] = useState(draft?.text ?? "SELECT * FROM dataset LIMIT 50");
  const editorRef = useRef<HTMLTextAreaElement | null>(null);

  /** PROD-12: puts the quoted column where the cursor is and keeps it there. */
  function insertColumn(name: string) {
    const editor = editorRef.current;
    const identifier = sqlIdentifier(name);
    const start = editor?.selectionStart ?? query.length;
    const end = editor?.selectionEnd ?? query.length;
    setQuery(`${query.slice(0, start)}${identifier}${query.slice(end)}`);
    requestAnimationFrame(() => {
      editor?.focus();
      editor?.setSelectionRange(start + identifier.length, start + identifier.length);
    });
  }
  const [localQueryEngine, setLocalQueryEngine] = useState<DatasetQueryEngine>(readQueryEnginePreference);
  const selectedQueryEngine = queryEngine ?? localQueryEngine;
  const [state, setState] = useState<
    | { kind: "idle" }
    | { kind: "loading"; cancelRequested: boolean }
    | { kind: "ready"; result: DatasetQueryResult }
    | { kind: "cancelled" }
    | { kind: "error"; message: string }
  >(restoredResult ? { kind: "ready", result: restoredResult } : { kind: "idle" });
  const [queryOpen, setQueryOpen] = useState(standalone || (draft?.open ?? false));
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
  const runningRef = useLatest(state.kind === "loading");
  useEffect(() => () => {
    if (runningRef.current) void cancelOperation("query").catch(() => undefined);
  }, [runningRef]);

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
        message: errorMessage(error),
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
        message: `No se pudo cancelar la consulta: ${errorMessage(error)}`,
      });
    }
  }

  // AUD-053: under the drawer's h2 the panel starts at h3, so the levels never skip.
  const TitleHeading = standalone ? "h3" : "h4";
  const HistoryHeading = standalone ? "h4" : "h5";
  const panel = (
      <section className="local-query" aria-labelledby="local-query-title">
        <div className="local-query__heading">
          <div>
            <TitleHeading id="local-query-title">Consulta SQL</TitleHeading>
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
          Consulta SQL
          <textarea
            ref={editorRef}
            aria-label="Consulta SQL"
            rows={2}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
          />
        </label>
        {columnNames.length > 0 && (
          <details className="local-query__columns">
            <summary>Columnas del dataset ({columnNames.length.toLocaleString()})</summary>
            <p>Pulsa una para insertarla donde está el cursor; los nombres con espacios, comas o comillas se citan solos.</p>
            <ul>
              {columnNames.map((name) => (
                <li key={name}>
                  <button type="button" aria-label={`Insertar la columna ${name}`} onClick={() => insertColumn(name)}>
                    {name}
                  </button>
                </li>
              ))}
            </ul>
          </details>
        )}
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
            <option value="polars">Polars · el habitual</option>
            <option value="duckdb">DuckDB · SQL local</option>
          </select>
        </label>
        <p className="local-query__engine-note">
          DuckDB ejecuta la consulta sobre una copia temporal del dataset en tu equipo y mantiene el mismo límite seguro de 200 filas.
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
                <HistoryHeading id="local-query-history-title">Actividad reciente</HistoryHeading>
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
                  <span>{entry.rowCount === null ? "Sin resultado" : plural(entry.rowCount, "fila", "filas")}</span>
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
  );
  // In the SQL console drawer the query is the whole content: no disclosure.
  if (standalone) return panel;
  return (
    <details
      className="review-tool review-tool--nested"
      open={queryOpen}
      onToggle={(event) => setQueryOpen(event.currentTarget.open)}
    >
      <summary>
        <span>Consulta SQL</span>
        <small>Opcional · consulta segura y de solo lectura</small>
      </summary>
      {panel}
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
    <div className="local-query__result">
      {/* Only the summary is announced; the table is read by moving into it. */}
      <p role="status" aria-live="polite">
        {plural(result.rowCount, "fila", "filas")}
        <span className="visually-hidden"> en el resultado</span>
      </p>
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
  /** With the quality profile, each header summarises its whole column. */
  profile?: DatasetProfile;
  /** The profile has not arrived yet: headers and note keep the room it will take. */
  profilePending?: boolean;
}

export function DatasetPreviewPanel({
  dataset,
  pageOffset,
  pageLoading,
  pageCancellationPending = false,
  pageError,
  onPageChange,
  onCancelPageChange = () => undefined,
  profile,
  profilePending = false,
}: DatasetPreviewProps) {
  const { end: pageEnd, hasPrevious, hasNext } = pageRange(dataset, pageOffset);
  const profileIdBase = useId();
  const reserveProfile = !profile && profilePending;
  // COD-10: names may repeat, so the position decides when it agrees.
  const columnProfile = (name: string, index: number) => {
    if (!profile) return undefined;
    const atIndex = profile.columns[index];
    return atIndex?.name === name ? atIndex : profile.columns.find((column) => column.name === name);
  };

  return (
    <>
      {(profile || reserveProfile) && (
        // While pending the note is invisible but laid out, so the table does not drop when it appears.
        <p className={reserveProfile ? "preview-note preview-note--pending" : "preview-note"} aria-hidden={reserveProfile || undefined}>
          Bajo cada columna: cuántas celdas tienen valor, cuántos valores distintos hay y su rango, calculados sobre {dataset.rowCount === 1 ? "la única fila" : `todas las filas (${dataset.rowCount.toLocaleString()})`}.
        </p>
      )}
      <div className="table-region" tabIndex={0} aria-label="Vista previa del dataset">
        <table className={profile || reserveProfile ? "table--profiled" : undefined}>
          <thead>
            <tr>
              {dataset.columns.map((column, columnIndex) => {
                const columnProfileOfHeader = columnProfile(column.name, columnIndex);
                // The id follows the position, never the name: names may repeat or hold odd characters.
                const profileId = columnProfileOfHeader ? `${profileIdBase}-profile-${columnIndex}` : undefined;
                return (
                  // The name stays short (name and type), which is what screen readers repeat on every cell;
                  // the profile is the description, announced once per header.
                  <th
                    key={column.name}
                    scope="col"
                    className={isNumericType(column.dataType) ? "cell--number" : undefined}
                    aria-label={`${column.name} ${formatDataType(column.dataType)}`}
                    aria-describedby={profileId}
                  >
                    <ColumnProfileHeader column={column} profile={columnProfileOfHeader} profileId={profileId} pending={reserveProfile} rowCount={dataset.rowCount} />
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {dataset.rows.map((row, rowIndex) => (
              <tr key={pageOffset + rowIndex}>
                {row.map((value, columnIndex) => (
                  <td
                    key={columnIndex}
                    className={isNumericType(dataset.columns[columnIndex]?.dataType ?? "") ? "cell--number" : undefined}
                  >
                    {renderCellValue(value)}
                  </td>
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
            : dataset.rows.length === 0
              // FUN-50: a page past the end (the dataset shrank) has no range.
              ? `Esta página no tiene filas; el dataset tiene ${dataset.rowCount.toLocaleString()}.`
              : `Filas ${(pageOffset + 1).toLocaleString()}–${pageEnd.toLocaleString()} de ${dataset.rowCount.toLocaleString()}`}
        </p>
        <div>
          <button
            type="button"
            onClick={() => onPageChange(previousPageOffset(pageOffset))}
            disabled={!hasPrevious || pageLoading}
            aria-describedby={busyDescribedBy(pageLoading)}
          >
            Anterior
          </button>
          <button
            type="button"
            onClick={() => onPageChange(nextPageOffset(pageOffset))}
            disabled={!hasNext || pageLoading}
            aria-describedby={busyDescribedBy(pageLoading)}
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
  onReturnToLoad,
  datasetRevision,
  extraTools,
}: {
  profile: DatasetProfile;
  dataset: DatasetPreview;
  onContinueToPrepare: (target?: QualityActionTarget) => void;
  onReturnToLoad?: () => void;
  datasetRevision: number;
  extraTools: ReactNode;
}) {
  const userColumns = profile.columns.filter((column) => !isRowAuditColumn(column.name));
  const textColumns = userColumns.filter((column) => column.emptyCount !== null);
  const numericColumns = userColumns.filter((column) => column.outlierCount !== null);
  const columnsWithNulls = profile.columns.filter((column) => column.nullCount > 0 && !isRowAuditColumn(column.name));
  const totalNullCount = columnsWithNulls.reduce((total, column) => total + column.nullCount, 0);
  // The list lists exactly what Preparar will propose, so Revisar never
  // announces a problem the proposal does not address, or hides one it does;
  // what the proposal leaves alone goes in «No se corrige solo».
  const proposal = buildPrepareProposal(profile, dataset);
  const proposalSelection = defaultProposalSelection(proposal);
  const priorityCount = proposal.length;
  const uncorrected = buildUncorrectedIssues(profile);
  const provenance = profileProvenanceText(profile);
  return (
    <>
      <section className="quality-overview quality-overview--plain" aria-labelledby="quality-overview-title">
        <h4 id="quality-overview-title">
          {priorityCount > 0
            ? `${priorityCount} ${priorityCount === 1 ? "cosa se puede corregir" : "cosas se pueden corregir"} con la propuesta`
            : uncorrected.length > 0
              ? "No hay correcciones automáticas"
              : "No encontramos nada que arreglar"}
        </h4>
        {provenance && <p className="quality-overview__provenance">{provenance}</p>}
        {proposal.length > 0 && (
          <ul className="quality-issues" aria-label="Resumen de calidad del dataset">
            {byImpact(proposal).map((item) => {
              const impact = proposalImpact(item, profile);
              return (
                <li key={item.id} className={`quality-issue quality-issue--${impact.level}`}>
                  <span className="quality-issue__title">{proposalItemTitle(item, proposalSelection)}</span>
                  <span className="quality-issue__impact">
                    {impact.label}{impact.detail ? ` · ${impact.detail}` : ""}
                  </span>
                </li>
              );
            })}
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
        {uncorrected.length > 0 && (
          <section className="quality-uncorrected" aria-labelledby="quality-uncorrected-title">
            <h5 id="quality-uncorrected-title">No se corrige solo</h5>
            <p className="quality-uncorrected__lead">
              La propuesta de Preparar no toca esto; las cifras cuentan {allRows(profile.rowCount)}.
            </p>
            <ul className="quality-uncorrected__list" aria-label="Lo que no se corrige solo">
              {uncorrected.map((issue) => (
                <li key={issue.id} className="quality-uncorrected__item">
                  <span className="quality-uncorrected__title">{issue.title}</span>
                  <span className="quality-uncorrected__detail">{issue.detail}</span>
                  {issue.action?.kind === "prepare" && (
                    <button
                      type="button"
                      className="secondary-action"
                      onClick={() => onContinueToPrepare(issue.action?.kind === "prepare" ? issue.action.target : undefined)}
                    >
                      {issue.action.label}
                    </button>
                  )}
                  {issue.action?.kind === "load" && onReturnToLoad && (
                    <button type="button" className="secondary-action" onClick={onReturnToLoad}>
                      {issue.action.label}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        )}
      </section>
      <ReviewMoreTools>
        <QualitySnapshot
          rowCount={profile.rowCount}
          columnCount={userColumns.length}
          nullCount={totalNullCount}
        />
        <p className="quality-overview__meta">
          <span>Filas analizadas</span>
          <strong>{profile.rowCount.toLocaleString()}</strong>
        </p>
      <details className="review-tool quality-details">
        <summary>
          <span>Ver análisis detallado</span>
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
            {userColumns.map((column) => (
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
                    : formatNumber(column.mean, 3, !readsAsCode(column.name, Number(column.minimum), Number(column.maximum)))}
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
                  <th scope="col">Valores atípicos</th>
                </tr>
              </thead>
              <tbody>
                {numericColumns.map((column) => {
                  // UX-08: a year's quartiles read «2013», not «2,013».
                  const grouping = !readsAsCode(column.name, Number(column.minimum), Number(column.maximum));
                  return (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>{formatStatistic(column.standardDeviation)}</td>
                      <td>{formatStatistic(column.firstQuartile, grouping)}</td>
                      <td>{formatStatistic(column.median, grouping)}</td>
                      <td>{formatStatistic(column.thirdQuartile, grouping)}</td>
                      <td>{column.outlierCount?.toLocaleString() ?? "—"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            Los valores atípicos (regla IQR de 1.5×) requieren al menos cuatro valores. La
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
                      {column.averageLength == null ? "—" : formatNumber(column.averageLength, 1)}
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
