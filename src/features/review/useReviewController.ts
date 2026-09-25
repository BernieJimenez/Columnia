import { useEffect, useRef, useState, type MutableRefObject } from "react";

import type { ReviewTab } from "../../components/ReviewTabList";
import {
  adoptConsolidatedDataset,
  cancelOperation,
  clearDatasetComparison,
  compareDataset,
  getDatasetConflictPage,
  getDatasetProfile,
  joinDataset,
  resolveDatasetConflicts,
  type ConflictResolution,
  type DatasetColumn,
  type DatasetJoinType,
  type DatasetPreview,
  type DatasetProfile,
  type DatasetQueryEngine,
  type SqlQueryHistoryEntry,
} from "../../bridge";
import {
  beginComparison,
  clearComparison,
  completeComparison,
  failComparison,
  type ComparisonStatus,
} from "./compareModel";
import {
  beginJoin,
  clearJoin,
  failJoin,
  type JoinStatus,
  type ReviewMutationKind,
  type ReviewMutationStatus,
} from "./joinModel";
import {
  beginProfileAnalysis,
  readAnalysisSampleRowsPreference,
  readQueryEnginePreference,
  recoverProfileCancellationFailure,
  requestProfileCancellation,
  updateProfileProgress,
  writeAnalysisSampleRowsPreference,
  writeQueryEnginePreference,
  type AnalysisSampleRows,
  type ProfileStatus,
} from "./reviewModel";

const CONFLICT_PAGE_SIZE = 50;

/** Everything the comparison and join tools of Revisar show and do. */
export interface ReviewComparison {
  status: ComparisonStatus;
  cancellationPending?: boolean;
  keyColumns: string[];
  onKeyColumnsChange: (columns: string[]) => void;
  onCompare: () => void;
  onCancelComparison?: () => void;
  onClear: () => void;
  onConsolidate: () => void;
  onResolveConflicts: (decisions: ConflictResolution[]) => void;
  onConflictPageChange: (offset: number) => void | Promise<void>;
  joinStatus: JoinStatus;
  mutationStatus?: ReviewMutationStatus;
  mutationCancellationPending?: boolean;
  joinType: DatasetJoinType;
  onJoinTypeChange: (joinType: DatasetJoinType) => void;
  onJoin: (joinType: DatasetJoinType) => void;
  onCancelMutation?: () => void;
}

/** Review settings a project stores and restores. */
export interface ReviewWorkspace {
  reviewTab?: ReviewTab;
  sqlHistory?: SqlQueryHistoryEntry[];
  queryEngine?: DatasetQueryEngine;
  analysisSampleRows?: AnalysisSampleRows;
  comparisonKeyColumns?: string[];
  joinType?: DatasetJoinType;
}

interface ReviewControllerOptions {
  /** Revision of the active dataset; stale responses are dropped against it. */
  datasetRevisionRef: MutableRefObject<number>;
  datasetRevision: number;
  datasetReady: boolean;
  /**
   * A comparison or join replaced the active dataset. The review state is
   * already reset; the owner publishes the dataset and resets the rest.
   */
  onDatasetReplaced: (dataset: DatasetPreview, mutation: ReviewMutationKind) => Promise<void>;
}

function isCancellationError(error: unknown): boolean {
  return String(error).includes("cancelada por el usuario");
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useReviewController({
  datasetRevisionRef,
  datasetRevision,
  datasetReady,
  onDatasetReplaced,
}: ReviewControllerOptions) {
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });
  const [analysisSampleRows, setAnalysisSampleRows] = useState<AnalysisSampleRows>(readAnalysisSampleRowsPreference);
  const [queryEngine, setQueryEngine] = useState<DatasetQueryEngine>(readQueryEnginePreference);
  const [reviewTab, setReviewTab] = useState<ReviewTab>("diagnosis");
  const [sqlHistory, setSqlHistory] = useState<SqlQueryHistoryEntry[]>([]);
  const [comparisonStatus, setComparisonStatus] = useState<ComparisonStatus>({ kind: "idle" });
  const [comparisonCancellationPending, setComparisonCancellationPending] = useState(false);
  const [comparisonKeyColumns, setComparisonKeyColumns] = useState<string[]>([]);
  const [joinStatus, setJoinStatus] = useState<JoinStatus>({ kind: "idle" });
  const [joinType, setJoinType] = useState<DatasetJoinType>("inner");
  const [mutationStatus, setMutationStatus] = useState<ReviewMutationStatus>({ kind: "idle" });
  const [mutationCancellationPending, setMutationCancellationPending] = useState(false);
  const autoProfileRevisionRef = useRef<number | null>(null);
  const profileRequestSequenceRef = useRef(0);
  const activeProfileRequestRef = useRef<{ revision: number; sequence: number } | null>(null);
  const comparisonRequestRef = useRef(0);
  const comparisonOperationInFlightRef = useRef(false);
  const comparisonPageRequestRef = useRef(0);
  const joinRequestRef = useRef(0);
  const mutationCancellationPendingRef = useRef(false);
  const mutationRef = useRef<{
    mutation: ReviewMutationKind;
    requestId: number;
    datasetRevision: number;
    cancellationRequested: boolean;
  } | null>(null);

  /** A new dataset revision makes every pending review response stale. */
  function invalidateRequests() {
    profileRequestSequenceRef.current += 1;
    comparisonRequestRef.current += 1;
    comparisonPageRequestRef.current += 1;
    joinRequestRef.current += 1;
  }

  function resetForDataset() {
    setSqlHistory([]);
    setComparisonStatus(clearComparison());
    setComparisonKeyColumns([]);
    setJoinStatus(clearJoin());
    setMutationStatus({ kind: "idle" });
    setJoinType("inner");
  }

  function forgetProjectSettings({ clearSqlHistory }: { clearSqlHistory: boolean }) {
    if (clearSqlHistory) setSqlHistory([]);
    setComparisonKeyColumns([]);
    setJoinType("inner");
  }

  function restoreWorkspace(
    workspace: ReviewWorkspace,
    columns: DatasetColumn[],
    profile: DatasetProfile | null | undefined,
  ) {
    setProfileStatus(profile ? { kind: "ready", profile } : { kind: "idle" });
    setComparisonStatus(clearComparison());
    const availableColumns = new Set(columns.map((column) => column.name));
    setComparisonKeyColumns((workspace.comparisonKeyColumns ?? []).filter(
      (column, index, keyColumns) => availableColumns.has(column) && keyColumns.indexOf(column) === index,
    ));
    setJoinStatus(clearJoin());
    setMutationStatus({ kind: "idle" });
    setJoinType(workspace.joinType ?? "inner");
    setSqlHistory(workspace.sqlHistory ?? []);
    setReviewTab(workspace.reviewTab ?? "diagnosis");
    const selectedQueryEngine = workspace.queryEngine ?? readQueryEnginePreference();
    setQueryEngine(selectedQueryEngine);
    writeQueryEnginePreference(selectedQueryEngine);
    const sampleRows = workspace.analysisSampleRows ?? readAnalysisSampleRowsPreference();
    setAnalysisSampleRows(sampleRows);
    writeAnalysisSampleRowsPreference(sampleRows);
  }

  function invalidateProfile() {
    setProfileStatus({ kind: "idle" });
  }

  async function analyzeQuality() {
    const requestedRevision = datasetRevisionRef.current;
    if (activeProfileRequestRef.current?.revision === requestedRevision) return;
    const sequence = ++profileRequestSequenceRef.current;
    activeProfileRequestRef.current = { revision: requestedRevision, sequence };
    const isCurrentRequest = () =>
      datasetRevisionRef.current === requestedRevision &&
      activeProfileRequestRef.current?.sequence === sequence;
    setProfileStatus(beginProfileAnalysis());
    try {
      const profile = await getDatasetProfile((progress) => {
        if (!isCurrentRequest()) return;
        setProfileStatus((current) => updateProfileProgress(current, progress));
      }, analysisSampleRows);
      if (!isCurrentRequest()) return;
      setProfileStatus({ kind: "ready", profile });
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      if (isCancellationError(error)) {
        setProfileStatus({ kind: "cancelled" });
        return;
      }
      setProfileStatus({ kind: "error", message: errorMessage(error) });
    } finally {
      if (activeProfileRequestRef.current?.sequence === sequence) {
        activeProfileRequestRef.current = null;
      }
    }
  }

  useEffect(() => {
    if (!datasetReady || profileStatus.kind !== "idle") return;
    if (autoProfileRevisionRef.current === datasetRevision) return;

    // Cada revisión se analiza una vez de forma automática. Tras cancelar o fallar,
    // el avance principal permite reintentarlo sin crear un bucle de reintentos.
    autoProfileRevisionRef.current = datasetRevision;
    void analyzeQuality();
  }, [datasetRevision, datasetReady, profileStatus.kind]);

  async function cancelProfile() {
    setProfileStatus(requestProfileCancellation);
    try {
      await cancelOperation("profile");
    } catch (error: unknown) {
      setProfileStatus((current) => recoverProfileCancellationFailure(current, errorMessage(error)));
    }
  }

  async function compare() {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    comparisonPageRequestRef.current += 1;
    const requestedRevision = datasetRevisionRef.current;
    const previousComparisonStatus = comparisonStatus;
    setMutationStatus({ kind: "idle" });
    const isCurrentRequest = () =>
      comparisonRequestRef.current === requestId && datasetRevisionRef.current === requestedRevision;
    setComparisonCancellationPending(false);
    setComparisonStatus(beginComparison());
    try {
      const comparison = await compareDataset(comparisonKeyColumns);
      if (!isCurrentRequest()) return;
      setComparisonStatus(comparison ? completeComparison(comparison) : clearComparison());
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      if (isCancellationError(error)) {
        setComparisonStatus(previousComparisonStatus);
        return;
      }
      setComparisonStatus(failComparison(errorMessage(error)));
    } finally {
      comparisonOperationInFlightRef.current = false;
      if (comparisonRequestRef.current === requestId) {
        setComparisonCancellationPending(false);
      }
    }
  }

  async function cancelComparison() {
    setComparisonCancellationPending(true);
    setComparisonStatus((current) =>
      current.kind === "loading" ? { ...current, cancellationError: undefined } : current,
    );
    try {
      await cancelOperation("datasetComparison");
    } catch (error: unknown) {
      setComparisonCancellationPending(false);
      setComparisonStatus((current) => current.kind === "loading"
        ? { ...current, cancellationError: errorMessage(error) }
        : current);
    }
  }

  async function clearActiveComparison() {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    comparisonPageRequestRef.current += 1;
    setMutationStatus({ kind: "idle" });
    try {
      await cancelOperation("datasetComparison");
      await clearDatasetComparison();
      if (comparisonRequestRef.current !== requestId) return;
      setComparisonStatus(clearComparison());
    } catch (error: unknown) {
      if (comparisonRequestRef.current !== requestId) return;
      setComparisonStatus(failComparison(errorMessage(error)));
    } finally {
      comparisonOperationInFlightRef.current = false;
    }
  }

  async function changeConflictPage(offset: number) {
    if (comparisonStatus.kind !== "ready") return;
    const requestId = ++comparisonPageRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const previousComparisonStatus = comparisonStatus;
    try {
      const page = await getDatasetConflictPage(offset, CONFLICT_PAGE_SIZE);
      if (comparisonPageRequestRef.current !== requestId || datasetRevisionRef.current !== requestedRevision) return;
      if (!page) return;
      setComparisonStatus(completeComparison({
        ...comparisonStatus.comparison,
        conflicts: page.conflicts,
        conflictOffset: page.offset,
        conflictsTruncated: page.hasNext,
      }));
    } catch (error: unknown) {
      if (comparisonPageRequestRef.current !== requestId || datasetRevisionRef.current !== requestedRevision) return;
      if (isCancellationError(error)) {
        setComparisonStatus(previousComparisonStatus);
        return;
      }
      setComparisonStatus(failComparison(errorMessage(error)));
    }
  }

  /**
   * Runs one dataset-replacing mutation (consolidate, resolve conflicts or
   * join) with cancellation and stale-response protection.
   */
  async function runMutation(
    mutationKind: ReviewMutationKind,
    requestRef: MutableRefObject<number>,
    execute: () => Promise<DatasetPreview | null>,
    handlers: { onEmpty?: () => void; onCancelled?: () => void; onError: (message: string) => void },
  ) {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++requestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const mutation = {
      mutation: mutationKind,
      requestId,
      datasetRevision: requestedRevision,
      cancellationRequested: false,
    };
    const isCurrent = () =>
      mutationRef.current === mutation
      && requestRef.current === requestId
      && datasetRevisionRef.current === requestedRevision;
    mutationRef.current = mutation;
    setMutationStatus({ kind: "running", mutation: mutationKind, cancellation: "available" });
    try {
      const dataset = await execute();
      if (!isCurrent()) return;
      if (!dataset) {
        handlers.onEmpty?.();
        setMutationStatus({ kind: "idle" });
        return;
      }
      setMutationStatus({ kind: "finalizing", mutation: mutationKind });
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setSqlHistory([]);
      if (mutationKind === "join") {
        setReviewTab("diagnosis");
      } else {
        setJoinType("inner");
      }
      await onDatasetReplaced(dataset, mutationKind);
    } catch (error: unknown) {
      if (!isCurrent()) return;
      if (isCancellationError(error)) {
        handlers.onCancelled?.();
        setMutationStatus({ kind: "idle" });
        return;
      }
      handlers.onError(errorMessage(error));
    } finally {
      if (mutationRef.current === mutation) {
        mutationRef.current = null;
        setMutationStatus((current) =>
          current.kind === "running" || current.kind === "finalizing"
            ? { kind: "idle" }
            : current,
        );
      }
      if (!mutationCancellationPendingRef.current) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  function consolidate() {
    if (comparisonStatus.kind !== "ready" || !comparisonStatus.comparison.canConsolidate) return;
    return runMutation("consolidate", comparisonRequestRef, adoptConsolidatedDataset, {
      onError: (message) => setMutationStatus({ kind: "error", mutation: "consolidate", message }),
    });
  }

  function resolveConflicts(decisions: ConflictResolution[]) {
    return runMutation("resolveConflicts", comparisonRequestRef, () => resolveDatasetConflicts(decisions), {
      onError: (message) => setMutationStatus({ kind: "error", mutation: "resolveConflicts", message }),
    });
  }

  function join(requestedJoinType: DatasetJoinType) {
    if (comparisonKeyColumns.length === 0) {
      setJoinStatus(failJoin("Selecciona al menos una columna clave para unir datasets."));
      return;
    }
    if (comparisonOperationInFlightRef.current) return;
    setJoinStatus(beginJoin(requestedJoinType));
    return runMutation("join", joinRequestRef, () => joinDataset(comparisonKeyColumns, requestedJoinType), {
      onEmpty: () => setJoinStatus(clearJoin()),
      onCancelled: () => setJoinStatus(clearJoin()),
      onError: (message) => setJoinStatus(failJoin(message)),
    });
  }

  async function cancelMutation() {
    const mutation = mutationRef.current;
    if (!mutation || mutation.cancellationRequested) return;
    if (mutationStatus.kind !== "running" || mutationStatus.mutation !== mutation.mutation) return;

    mutation.cancellationRequested = true;
    mutationCancellationPendingRef.current = true;
    setMutationCancellationPending(true);
    setMutationStatus({
      kind: "running",
      mutation: mutation.mutation,
      cancellation: "requested",
    });
    try {
      await cancelOperation("reviewMutation");
    } catch (error: unknown) {
      if (mutationRef.current !== mutation) return;
      mutation.cancellationRequested = false;
      setMutationStatus({
        kind: "running",
        mutation: mutation.mutation,
        cancellation: "available",
        cancellationError: errorMessage(error),
      });
    } finally {
      mutationCancellationPendingRef.current = false;
      setMutationCancellationPending(false);
      if (mutationRef.current === null) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  const comparison: ReviewComparison = {
    status: comparisonStatus,
    cancellationPending: comparisonCancellationPending,
    keyColumns: comparisonKeyColumns,
    onKeyColumnsChange: setComparisonKeyColumns,
    onCompare: () => void compare(),
    onCancelComparison: () => void cancelComparison(),
    onClear: () => void clearActiveComparison(),
    onConsolidate: () => void consolidate(),
    onResolveConflicts: (decisions) => void resolveConflicts(decisions),
    onConflictPageChange: changeConflictPage,
    joinStatus,
    mutationStatus,
    mutationCancellationPending,
    joinType,
    onJoinTypeChange: setJoinType,
    onJoin: (requestedJoinType) => void join(requestedJoinType),
    onCancelMutation: () => void cancelMutation(),
  };

  const busy =
    comparisonStatus.kind === "loading" ||
    joinStatus.kind === "loading" ||
    mutationStatus.kind === "running" ||
    mutationStatus.kind === "finalizing";

  return {
    profileStatus,
    analysisSampleRows,
    setAnalysisSampleRows,
    queryEngine,
    setQueryEngine,
    reviewTab,
    setReviewTab,
    sqlHistory,
    setSqlHistory,
    comparison,
    busy,
    workspace: {
      reviewTab,
      ...(sqlHistory.length > 0 ? { sqlHistory } : {}),
      queryEngine,
      analysisSampleRows,
      comparisonKeyColumns,
      joinType,
    },
    analyzeQuality,
    cancelProfile,
    invalidateProfile,
    invalidateRequests,
    resetForDataset,
    forgetProjectSettings,
    restoreWorkspace,
  };
}

export type ReviewController = ReturnType<typeof useReviewController>;
