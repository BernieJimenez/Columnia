import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import type { ReviewTab } from "./components/ReviewTabList";
import {
  INITIAL_DELIVERY_CONTRACT,
  deliveryContractFromRules,
  deliveryRules,
  invalidateDeliveryContract,
  reduceDeliveryContract,
  isDatabaseExportFormat,
  type DeliveryContractAction,
  type DeliveryContractState,
  type DeliveryExportRequest,
  type DeliveryExportState,
} from "./features/delivery/deliveryModel";
import { LoadPhase, type LoadRuntimeState } from "./features/load/LoadPhase";
import { ReusableTaskPanel } from "./features/load/ReusableTaskPanel";
import {
  createReusableTaskExceptionPolicy,
  exceptionPolicyMatchesSchema,
  refreshReusableTaskExceptionPolicy,
} from "./features/load/reusableTaskExceptions";
import { ModalDialog } from "./components/ModalDialog";
import {
  beginDatasetLoad,
  createReadyDatasetStatus,
  requestDatasetLoadCancellation,
  restoreDatasetAfterLoadFailure,
  setLoadInspectionError,
  beginDelimitedHeaderReview,
  completeDelimitedHeaderReview,
  delimitedHeaderInspection,
  updateDatasetLoadProgress,
  updateSheetSelection,
  schemaMismatchInspection,
  workbookInspection,
  needsResourcePreflight,
  type DatasetStatus,
  type LoadInspectionState,
  type ProfileReviewAction,
  type ResourcePreflightAction,
  type SchemaMismatchAction,
  type SheetSelectionAction,
} from "./features/load/loadModel";
import {
  createImportProfile,
  importProfileApplicability,
  parseImportProfileMismatch,
} from "./features/load/importProfile";
import {
  readRecentDatasets,
  rememberRecentDataset,
  removeRecentDataset,
  writeRecentDatasets,
  type RecentDataset,
} from "./features/load/recentFilesModel";
import { usePrepareController } from "./features/prepare/usePrepareController";
import { ProjectsPanel } from "./features/projects/ProjectsPanel";
import { useProjectsController } from "./features/projects/useProjectsController";
import {
  beginComparison,
  clearComparison,
  completeComparison,
  failComparison,
  type ComparisonStatus,
} from "./features/review/compareModel";
import {
  beginJoin,
  clearJoin,
  failJoin,
  type JoinStatus,
  type ReviewMutationKind,
  type ReviewMutationStatus,
} from "./features/review/joinModel";
import {
  PAGE_SIZE,
  beginPageLoad,
  beginProfileAnalysis,
  completePageLoad,
  failPageLoad,
  normalizePageOffset,
  readAnalysisSampleRowsPreference,
  readQueryEnginePreference,
  requestProfileCancellation,
  updateProfileProgress,
  writeAnalysisSampleRowsPreference,
  writeQueryEnginePreference,
  type AnalysisSampleRows,
  type ProfileStatus,
} from "./features/review/reviewModel";
import type { QualityActionTarget } from "./features/review/qualityActionPlan";
import { ResourceMonitor } from "./components/ResourceMonitor";
import { ThemeSwitcher } from "./components/ThemeSwitcher";
import { UpdatePanel } from "./components/UpdatePanel";
import { DiagnosticsDialog } from "./features/diagnostics/DiagnosticsDialog";
import type { DatasetMetricInput } from "./features/diagnostics/diagnosticsModel";
import { WorkspaceNav } from "./features/workspaces/WorkspaceNav";
import { workflowPhases, type WorkflowPhase } from "./features/workspaces/workspaceModel";

import {
  cancelOperation,
  clearDatasetComparison,
  compareDataset,
  discardDatasetSelection,
  exportDataset,
  exportDatasetToDatabase,
  getAppInfo,
  getDatasetConflictPage,
  getDatasetPage,
  getDatasetProfile,
  inspectDroppedDataset as inspectDroppedDatasetSource,
  inspectSampleDataset,
  inspectWorkbookSheets,
  joinDataset,
  listSampleDatasets,
  loadDatasetSelection,
  pickDatasetSource,
  previewDelimitedHeaderReview,
  resolveDatasetConflicts,
  useConsolidatedDataset,
  type AppInfo,
  type CancellableOperation,
  type DatasetJoinType,
  type DatasetPreview,
  type ConflictResolution,
  type DatasetSourceInspection,
  type ImportProfile,
  type DatasetQueryEngine,
  type ExportFormat,
  type OperationProgress,
  type PerformanceProfile,
  type PrivacyMode,
  type ReusableTask,
  type ReusableTaskExceptionPolicy,
  type ReusableTaskOutputFormat,
  type ReusableTaskSchema,
  type SavedRecipe,
  type SampleDatasetDescriptor,
  type SqlQueryHistoryEntry,
  type SpreadsheetHeaderMode,
} from "./bridge";
import {
  readPerformanceProfile,
  writePerformanceProfile,
} from "./features/settings/performanceModel";

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo | null }
  | { kind: "browser" }
  | { kind: "error"; message: string };

interface QueuedReusableTask {
  id: string;
  task: ReusableTask;
}

interface ReusableTaskApplicationReview {
  task: ReusableTask;
  importProfileUsed: boolean;
  schemaMismatchConfirmed: boolean;
}

const CONFLICT_PAGE_SIZE = 50;

const loadDeliveryPhase = () => import("./features/delivery/DeliveryPhase");
const loadPreparePhase = () => import("./features/prepare/PreparePhase");
const loadReviewPhase = () => import("./features/review/ReviewPhase");

const DeliveryPhase = lazy(async () => {
  const module = await loadDeliveryPhase();
  return { default: module.DeliveryPhase };
});

const PreparePhase = lazy(async () => {
  const module = await loadPreparePhase();
  return { default: module.PreparePhase };
});

const ReviewPhase = lazy(async () => {
  const module = await loadReviewPhase();
  return { default: module.ReviewPhase };
});

function preloadPhase(phase: WorkflowPhase): void {
  if (phase === "review") void loadReviewPhase();
  if (phase === "prepare") void loadPreparePhase();
  if (phase === "deliver") void loadDeliveryPhase();
}

function initialAppStatus(): AppStatus {
  return isTauriRuntime()
    ? { kind: "ready", info: null }
    : { kind: "browser" };
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function isCancellationError(error: unknown): boolean {
  return String(error).includes("cancelada por el usuario");
}

function reusableOutputFormat(format: ExportFormat): ReusableTaskOutputFormat {
  switch (format) {
    case "postgresql":
    case "mysql":
    case "sqlserver":
      return "csv";
    default:
      return format;
  }
}

export function App() {
  const [status, setStatus] = useState<AppStatus>(initialAppStatus);
  const [datasetStatus, setDatasetStatus] = useState<DatasetStatus>({ kind: "empty" });
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });
  const [analysisSampleRows, setAnalysisSampleRows] = useState<AnalysisSampleRows>(readAnalysisSampleRowsPreference);
  const [queryEngine, setQueryEngine] = useState<DatasetQueryEngine>(readQueryEnginePreference);
  const [performanceProfile, setPerformanceProfile] = useState<PerformanceProfile>(readPerformanceProfile);
  const [comparisonStatus, setComparisonStatus] = useState<ComparisonStatus>({ kind: "idle" });
  const [comparisonCancellationPending, setComparisonCancellationPending] = useState(false);
  const [comparisonKeyColumns, setComparisonKeyColumns] = useState<string[]>([]);
  const [joinStatus, setJoinStatus] = useState<JoinStatus>({ kind: "idle" });
  const [reviewMutationStatus, setReviewMutationStatus] = useState<ReviewMutationStatus>({ kind: "idle" });
  const [reviewMutationCancellationPending, setReviewMutationCancellationPending] = useState(false);
  const [joinType, setJoinType] = useState<DatasetJoinType>("inner");
  const [exportFormat, setExportFormat] = useState<ExportFormat>("csv");
  const [privacyMode, setPrivacyMode] = useState<PrivacyMode>("none");
  const [exportStatus, setExportStatus] = useState<DeliveryExportState>({ kind: "idle" });
  const [deliveryContract, setDeliveryContract] = useState<DeliveryContractState>(INITIAL_DELIVERY_CONTRACT);
  const [activePhase, setActivePhase] = useState<WorkflowPhase>("load");
  const [prepareFocusTarget, setPrepareFocusTarget] = useState<QualityActionTarget | null>(null);
  const [reviewTab, setReviewTab] = useState<ReviewTab>("diagnosis");
  const [loadInspection, setLoadInspection] = useState<LoadInspectionState>({ kind: "idle" });
  const [workbookInspectionCancellationPending, setWorkbookInspectionCancellationPending] = useState(false);
  const [selectionFinalizing, setSelectionFinalizing] = useState(false);
  const headerPreviewRequestRef = useRef(0);
  const inspectionRequestRef = useRef(0);
  const inspectionInFlightRef = useRef(false);
  const loadRequestRef = useRef(0);
  const loadInFlightRef = useRef(false);
  const comparisonRequestRef = useRef(0);
  const comparisonOperationInFlightRef = useRef(false);
  const reviewMutationCancellationPendingRef = useRef(false);
  const comparisonPageRequestRef = useRef(0);
  const joinRequestRef = useRef(0);
  const [pageCancellationPending, setPageCancellationPending] = useState(false);
  const pageCancellationRequestRef = useRef<number | null>(null);
  const reviewMutationRef = useRef<{
    mutation: ReviewMutationKind;
    requestId: number;
    datasetRevision: number;
    cancellationRequested: boolean;
  } | null>(null);
  const exportRequestRef = useRef(0);
  const exportInFlightRef = useRef(false);
  const [activeImportProfile, setActiveImportProfile] = useState<ImportProfile | null>(null);
  const [queuedReusableTask, setQueuedReusableTask] = useState<QueuedReusableTask | null>(null);
  const [reusableTaskApplicationReview, setReusableTaskApplicationReview] = useState<ReusableTaskApplicationReview | null>(null);
  const [recentDatasets, setRecentDatasets] = useState<RecentDataset[]>(readRecentDatasets);
  const [sampleDatasets, setSampleDatasets] = useState<SampleDatasetDescriptor[]>([]);
  const [recipeDraft, setRecipeDraft] = useState<SavedRecipe | null>(null);
  const [activeExceptionPolicy, setActiveExceptionPolicy] = useState<ReusableTaskExceptionPolicy | null>(null);
  const [sqlHistory, setSqlHistory] = useState<SqlQueryHistoryEntry[]>([]);
  const [datasetRevision, setDatasetRevision] = useState(0);
  const datasetRevisionRef = useRef(0);
  const autoProfileRevisionRef = useRef<number | null>(null);
  const profileRequestSequenceRef = useRef(0);
  const activeProfileRequestRef = useRef<{ revision: number; sequence: number } | null>(null);
  const pageRequestRef = useRef(0);
  const operationBusyRef = useRef(false);
  const [completedPhases, setCompletedPhases] = useState<Set<WorkflowPhase>>(() => new Set());
  const [recipeSession, setRecipeSession] = useState(0);
  const stageRef = useRef<HTMLDivElement>(null);
  const previousPhaseRef = useRef(activePhase);

  useEffect(() => {
    if (previousPhaseRef.current === activePhase) return;
    previousPhaseRef.current = activePhase;
    stageRef.current?.focus({ preventScroll: true });
  }, [activePhase]);

  function bumpDatasetRevision() {
    datasetRevisionRef.current += 1;
    profileRequestSequenceRef.current += 1;
    pageRequestRef.current += 1;
    pageCancellationRequestRef.current = null;
    setPageCancellationPending(false);
    comparisonRequestRef.current += 1;
    comparisonPageRequestRef.current += 1;
    joinRequestRef.current += 1;
    exportRequestRef.current += 1;
    setDatasetRevision(datasetRevisionRef.current);
  }

  function resetCompletedPhases() {
    setCompletedPhases(new Set(["load"]));
  }
  const [sidebarUtilitiesOpen, setSidebarUtilitiesOpen] = useState(false);
  const [sidebarLegalOpen, setSidebarLegalOpen] = useState(false);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const prepare = usePrepareController({
    activeDataset: datasetStatus.kind === "ready" ? datasetStatus.dataset : null,
    exceptionPolicy: activeExceptionPolicy,
    onDatasetChanged: (dataset) => {
      bumpDatasetRevision();
      setActiveExceptionPolicy((current) => current && exceptionPolicyMatchesSchema(current, dataset.columns)
        ? current
        : null);
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
      setSqlHistory([]);
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setReviewMutationStatus({ kind: "idle" });
      setJoinType("inner");
      void clearDatasetComparison().catch(() => undefined);
    },
    onProfileInvalidated: () => setProfileStatus({ kind: "idle" }),
    onDeliveryInvalidated: invalidateDeliveryGate,
  });
  const coreOperationBusy =
    datasetStatus.kind === "loading" ||
    (datasetStatus.kind === "ready" && datasetStatus.pageLoading) ||
    profileStatus.kind === "loading" ||
    prepare.changeStatus.kind === "working" ||
    deliveryContract.gate.kind === "loading" ||
    exportStatus.kind === "loading" ||
    comparisonStatus.kind === "loading" ||
    joinStatus.kind === "loading" ||
    reviewMutationStatus.kind === "running" ||
    reviewMutationStatus.kind === "finalizing";
  const loadSelectionBusy = loadInspection.kind === "inspecting" ||
    loadInspection.kind === "workbook_inspecting" ||
    loadInspection.kind === "sheet" ||
    loadInspection.kind === "profile_review" ||
    loadInspection.kind === "resource_preflight" ||
    loadInspection.kind === "schema_mismatch" ||
    selectionFinalizing;
  const projects = useProjectsController({
    connected: status.kind === "ready",
    blocked: coreOperationBusy || loadSelectionBusy,
    hasDataset: datasetStatus.kind === "ready",
    datasetRevision,
    workspace: {
      qualityRules: deliveryRules(deliveryContract),
      recipeDraft,
      ...(sqlHistory.length > 0 ? { sqlHistory } : {}),
      reviewTab,
      previewOffset: datasetStatus.kind === "ready" ? datasetStatus.pageOffset : 0,
      activePhase,
      queryEngine,
      analysisSampleRows,
      performanceProfile,
      exportFormat,
      privacyMode,
      comparisonKeyColumns,
      joinType,
      importProfile: activeImportProfile ?? undefined,
    },
    onActiveProjectDeleted: () => {
      setSqlHistory([]);
      setPerformanceProfile(readPerformanceProfile());
      setComparisonKeyColumns([]);
      setJoinType("inner");
      setExportFormat("csv");
      setPrivacyMode("none");
    },
    onActiveProjectUnlinked: () => {
      setPerformanceProfile(readPerformanceProfile());
      setComparisonKeyColumns([]);
      setJoinType("inner");
      setExportFormat("csv");
      setPrivacyMode("none");
    },
    onProjectOpened: async ({ dataset, workspace, profile }) => {
      bumpDatasetRevision();
      resetCompletedPhases();
      const initialDataset = createReadyDatasetStatus(dataset);
      setDatasetStatus(initialDataset);
      const previewOffset = normalizePageOffset(workspace.previewOffset ?? 0, dataset.rowCount);
      if (previewOffset > 0) {
        const requestedRevision = datasetRevisionRef.current;
        const requestId = ++pageRequestRef.current;
        try {
          const page = await getDatasetPage(previewOffset, PAGE_SIZE);
          if (datasetRevisionRef.current === requestedRevision && pageRequestRef.current === requestId) {
            setDatasetStatus(completePageLoad(initialDataset, page));
          }
        } catch {
          // El snapshot sigue siendo válido; la muestra vuelve a su primera página.
        }
      }
      setLoadInspection({ kind: "idle" });
      setQueuedReusableTask(null);
      setReusableTaskApplicationReview(null);
      setActiveImportProfile(workspace.importProfile ?? null);
      setProfileStatus(profile ? { kind: "ready", profile } : { kind: "idle" });
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      setDeliveryContract(deliveryContractFromRules(workspace.qualityRules));
      setExportStatus({ kind: "idle" });
      setComparisonStatus(clearComparison());
      const availableColumns = new Set(dataset.columns.map((column) => column.name));
      const restoredKeyColumns = (workspace.comparisonKeyColumns ?? []).filter(
        (column, index, columns) => availableColumns.has(column) && columns.indexOf(column) === index,
      );
      setComparisonKeyColumns(restoredKeyColumns);
      setJoinStatus(clearJoin());
      setReviewMutationStatus({ kind: "idle" });
      setJoinType(workspace.joinType ?? "inner");
      setExportFormat(workspace.exportFormat ?? "csv");
      setPrivacyMode(workspace.privacyMode ?? "none");
      await clearDatasetComparison().catch(() => undefined);
      setRecipeDraft(workspace.recipeDraft);
      setActiveExceptionPolicy(null);
      setSqlHistory(workspace.sqlHistory ?? []);
      setRecipeSession((current) => current + 1);
      setReviewTab(workspace.reviewTab ?? "diagnosis");
      setActivePhase(workspace.activePhase ?? "review");
      const selectedQueryEngine = workspace.queryEngine ?? readQueryEnginePreference();
      setQueryEngine(selectedQueryEngine);
      writeQueryEnginePreference(selectedQueryEngine);
      const sampleRows = workspace.analysisSampleRows ?? readAnalysisSampleRowsPreference();
      setAnalysisSampleRows(sampleRows);
      writeAnalysisSampleRowsPreference(sampleRows);
      const selectedPerformanceProfile = workspace.performanceProfile ?? readPerformanceProfile();
      setPerformanceProfile(selectedPerformanceProfile);
    },
  });
  const deliveryDatasetFingerprint = datasetStatus.kind === "ready"
    ? JSON.stringify({
        fileName: datasetStatus.dataset.fileName,
        rowCount: datasetStatus.dataset.rowCount,
        columns: datasetStatus.dataset.columns,
        rows: datasetStatus.dataset.rows,
      })
    : null;
  const previousDeliveryFingerprint = useRef<string | null>(null);

  useEffect(() => {
    if (deliveryDatasetFingerprint === null) return;
    if (previousDeliveryFingerprint.current !== null &&
        previousDeliveryFingerprint.current !== deliveryDatasetFingerprint) {
      invalidateDeliveryGate();
    }
    previousDeliveryFingerprint.current = deliveryDatasetFingerprint;
  }, [deliveryDatasetFingerprint]);

  useEffect(() => {
    if (typeof performance !== "undefined") {
      performance.mark("columnia:app-render");
    }
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let active = true;
    getAppInfo()
      .then((info) => active && setStatus({ kind: "ready", info }))
      .catch((error: unknown) => {
        if (active) {
          const message = error instanceof Error ? error.message : String(error);
          setStatus({ kind: "error", message });
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (status.kind !== "ready") return;
    let active = true;
    listSampleDatasets()
      .then((samples) => active && setSampleDatasets(samples))
      .catch(() => active && setSampleDatasets([]));
    return () => {
      active = false;
    };
  }, [status.kind]);

  useEffect(() => {
    if (status.kind !== "ready") return;

    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("columnia://dataset-drop", () => {
      if (operationBusyRef.current) return;
      void inspectDatasetSource(inspectDroppedDatasetSource());
    }).then((cleanup) => {
      if (disposed) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    }).catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [status.kind]);

  useEffect(() => {
    writeRecentDatasets(recentDatasets);
  }, [recentDatasets]);

  function invalidateDeliveryGate() {
    setDeliveryContract(invalidateDeliveryContract);
    setExportStatus({ kind: "idle" });
  }

  function updateDeliveryContract(action: DeliveryContractAction) {
    setDeliveryContract((current) => reduceDeliveryContract(current, action));
    if (action.kind === "rules_changed") setExportStatus({ kind: "idle" });
  }

  async function requestDelimitedHeaderReview(source: DatasetSourceInspection) {
    const requestId = ++headerPreviewRequestRef.current;
    try {
      const preview = await previewDelimitedHeaderReview(source.selectionId);
      if (headerPreviewRequestRef.current !== requestId) return;
      setLoadInspection((current) =>
        current.kind === "sheet" && current.source.selectionId === source.selectionId
          ? completeDelimitedHeaderReview(current, preview)
          : current,
      );
    } catch (error: unknown) {
      if (headerPreviewRequestRef.current !== requestId) return;
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) =>
        current.kind === "sheet" && current.source.selectionId === source.selectionId
          ? setLoadInspectionError(current, message)
          : current,
      );
    }
  }

  function retryDelimitedHeaderReview() {
    if (loadInspection.kind !== "sheet" || loadInspection.source.format === "excel") return;
    const source = loadInspection.source;
    setLoadInspection((current) => beginDelimitedHeaderReview(current));
    void requestDelimitedHeaderReview(source);
  }

  async function loadSelection(
    source: DatasetSourceInspection,
    sheetId: string | null,
    headerMode: SpreadsheetHeaderMode | null = null,
    expectedProfile: ImportProfile | null = null,
    profileSeed: ImportProfile | null = expectedProfile,
    taskForReview: ReusableTask | null = null,
    schemaMismatchConfirmed = false,
    conventions: Pick<ImportProfile, "dateConvention" | "numberConvention"> | null = profileSeed,
  ) {
    if (loadInFlightRef.current) return;
    const delimitedConventions = source.format === "csv" || source.format === "tsv"
      ? conventions
      : null;
    loadInFlightRef.current = true;
    const requestId = ++loadRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const isCurrentRequest = () =>
      loadRequestRef.current === requestId && datasetRevisionRef.current === requestedRevision;
    setSelectionFinalizing(true);
    setDatasetStatus((current) => beginDatasetLoad(current));
    setLoadInspection({ kind: "idle" });
    try {
      const dataset = await loadDatasetSelection(source.selectionId, sheetId, headerMode, (progress) => {
        if (!isCurrentRequest()) return;
        setDatasetStatus((current) => updateDatasetLoadProgress(current, progress));
      }, expectedProfile,
      delimitedConventions?.dateConvention && delimitedConventions.dateConvention !== "unresolved"
        ? delimitedConventions.dateConvention
        : null,
      delimitedConventions?.numberConvention && delimitedConventions.numberConvention !== "unresolved"
        ? delimitedConventions.numberConvention
        : null);
      if (!isCurrentRequest()) return;
      setActiveImportProfile(createImportProfile(
        source,
        dataset,
        { sheetId, headerMode },
        delimitedConventions,
      ));
      const applyQueuedTaskAutomatically = taskForReview !== null
        && expectedProfile !== null
        && !schemaMismatchConfirmed;
      if (taskForReview) {
        setQueuedReusableTask(null);
        if (applyQueuedTaskAutomatically) {
          setReusableTaskApplicationReview(null);
        } else {
          setReusableTaskApplicationReview({
            task: taskForReview,
            importProfileUsed: expectedProfile !== null,
            schemaMismatchConfirmed,
          });
        }
      }
      setRecentDatasets((current) => rememberRecentDataset(current, {
        fileName: source.fileName,
        format: source.format,
      }));
      setDatasetStatus(createReadyDatasetStatus(dataset));
      bumpDatasetRevision();
      resetCompletedPhases();
      projects.unlinkActiveProject();
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      setLoadInspection({ kind: "idle" });
      setProfileStatus({ kind: "idle" });
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setReviewMutationStatus({ kind: "idle" });
      setJoinType("inner");
      setExportFormat("csv");
      setPrivacyMode("none");
      await clearDatasetComparison().catch(() => undefined);
      if (loadRequestRef.current !== requestId) return;
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      if (loadRequestRef.current !== requestId) return;
      setExportStatus({ kind: "idle" });
      setReviewTab("diagnosis");
      if (applyQueuedTaskAutomatically && taskForReview) {
        applyReusableTask(taskForReview, true);
      } else {
        setActivePhase("review");
      }
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      setDatasetStatus(restoreDatasetAfterLoadFailure);
      if (isCancellationError(error)) {
        return;
      }
      const mismatch = parseImportProfileMismatch(error);
      if (mismatch && expectedProfile) {
        setLoadInspection(schemaMismatchInspection(
          source,
          expectedProfile,
          mismatch,
          sheetId,
          headerMode,
        ));
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) => setLoadInspectionError(current, message));
    } finally {
      if (loadRequestRef.current === requestId) setSelectionFinalizing(false);
      loadInFlightRef.current = false;
    }
  }

  async function inspectDatasetSource(sourcePromise: Promise<DatasetSourceInspection | null>) {
    if (inspectionInFlightRef.current || loadInFlightRef.current) return;
    inspectionInFlightRef.current = true;
    const requestId = ++inspectionRequestRef.current;
    const isCurrentRequest = () => inspectionRequestRef.current === requestId;
    setActivePhase("load");
    setLoadInspection({ kind: "inspecting" });
    try {
      const source = await sourcePromise;
      if (!isCurrentRequest()) return;
      if (!source) {
        setLoadInspection({ kind: "idle" });
        return;
      }
      const selectedImportProfile = queuedReusableTask?.task.importProfile ?? activeImportProfile;
      if (source.format === "excel") {
        if (!isCurrentRequest()) return;
        setLoadInspection({ kind: "workbook_inspecting", source });
        const sheets = await inspectWorkbookSheets(source.selectionId);
        if (!isCurrentRequest()) return;
        setLoadInspection(workbookInspection({ ...source, sheets }, selectedImportProfile));
        return;
      }
      if (source.format === "csv" || source.format === "tsv") {
        if (!isCurrentRequest()) return;
        setLoadInspection(delimitedHeaderInspection(source, selectedImportProfile));
        void requestDelimitedHeaderReview(source);
        return;
      }
      const applicability = selectedImportProfile
        ? importProfileApplicability(selectedImportProfile, source)
        : null;
      if (selectedImportProfile && applicability?.kind === "applicable") {
        if (!isCurrentRequest()) return;
        setLoadInspection({
          kind: "profile_review",
          source,
          profile: selectedImportProfile,
        });
        return;
      }
      if (needsResourcePreflight(source)) {
        if (!isCurrentRequest()) return;
        setLoadInspection({ kind: "resource_preflight", source });
        return;
      }
      await loadSelection(source, source.sheets[0]?.id ?? null);
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      if (isCancellationError(error)) {
        setLoadInspection({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) => setLoadInspectionError(current, message));
    } finally {
      if (isCurrentRequest()) {
        setLoadInspection((current) => current.kind === "inspecting" ? { kind: "idle" } : current);
        inspectionInFlightRef.current = false;
      }
    }
  }

  async function cancelWorkbookInspection() {
    if (loadInspection.kind !== "workbook_inspecting" || workbookInspectionCancellationPending) return;
    const selectionId = loadInspection.source.selectionId;
    setWorkbookInspectionCancellationPending(true);
    inspectionRequestRef.current += 1;
    let cancellationError: string | null = null;
    try {
      await cancelOperation("load");
    } catch (error: unknown) {
      cancellationError = error instanceof Error ? error.message : String(error);
    }
    try {
      await discardDatasetSelection(selectionId);
    } catch (error: unknown) {
      cancellationError ??= error instanceof Error ? error.message : String(error);
    }
    inspectionInFlightRef.current = false;
    setLoadInspection(cancellationError === null
      ? { kind: "idle" }
      : { kind: "error", message: cancellationError });
    setWorkbookInspectionCancellationPending(false);
  }

  function selectDataset() {
    return inspectDatasetSource(pickDatasetSource());
  }

  function selectSampleDataset(sampleId: string) {
    return inspectDatasetSource(inspectSampleDataset(sampleId));
  }

  function selectRecentDataset(_item: RecentDataset) {
    // Recent entries never contain a path or reusable native selection. Reopen the picker.
    void selectDataset();
  }

  async function cancelPendingSelection() {
    inspectionRequestRef.current += 1;
    loadRequestRef.current += 1;
    headerPreviewRequestRef.current += 1;
    const source = loadInspection.kind === "sheet" || loadInspection.kind === "profile_review" || loadInspection.kind === "resource_preflight" || loadInspection.kind === "schema_mismatch"
      ? loadInspection.source
      : undefined;
    setLoadInspection({ kind: "idle" });
    if (source) {
      try {
        await discardDatasetSelection(source.selectionId);
      } catch (error: unknown) {
        setLoadInspection({
          kind: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }

  function handleSheetSelection(action: SheetSelectionAction) {
    if (action.kind === "cancelled") {
      void cancelPendingSelection();
      return;
    }
    if (action.kind === "confirmed") {
      if (loadInspection.kind === "sheet") {
        if (loadInspection.source.format !== "excel" && !loadInspection.headerReview) return;
        const queuedProfile = queuedReusableTask?.task.importProfile;
        const queuedProfileIsApplicable = queuedProfile !== undefined &&
          importProfileApplicability(queuedProfile, loadInspection.source).kind === "applicable";
        const selectedProfile = queuedReusableTask
          ? queuedProfileIsApplicable ? queuedProfile : null
          : loadInspection.useSavedProfile ? loadInspection.savedProfile : null;
        const conventions = {
          dateConvention: loadInspection.dateConvention,
          numberConvention: loadInspection.numberConvention,
        };
        const profileWithConventions = selectedProfile
          ? { ...selectedProfile, ...conventions }
          : null;
        void loadSelection(
          loadInspection.source,
          loadInspection.source.format === "excel" ? loadInspection.selectedSheetId : null,
          loadInspection.headerMode,
          profileWithConventions,
          profileWithConventions,
          queuedReusableTask?.task ?? null,
          false,
          conventions,
        );
      }
      return;
    }
    setLoadInspection((current) => updateSheetSelection(current, action));
  }

  function handleProfileReviewAction(action: ProfileReviewAction) {
    if (loadInspection.kind !== "profile_review") return;
    if (action.kind === "cancelled") {
      void cancelPendingSelection();
      return;
    }
    const { source, profile } = loadInspection;
    if (action.kind === "use_defaults") {
      void loadSelection(
        source,
        null,
        null,
        null,
        null,
        queuedReusableTask?.task ?? null,
      );
      return;
    }
    if (action.kind === "use_profile") {
      void loadSelection(
        source,
        null,
        null,
        profile,
        profile,
        queuedReusableTask?.task ?? null,
      );
      return;
    }
  }

  function handleResourcePreflightAction(action: ResourcePreflightAction) {
    if (loadInspection.kind !== "resource_preflight") return;
    if (action.kind === "cancelled") {
      void cancelPendingSelection();
      return;
    }
    const { source } = loadInspection;
    const selectedProfile = queuedReusableTask?.task.importProfile ?? activeImportProfile;
    const applicableProfile = selectedProfile &&
      importProfileApplicability(selectedProfile, source).kind === "applicable"
      ? selectedProfile
      : null;
    void loadSelection(
      source,
      source.sheets[0]?.id ?? null,
      applicableProfile?.headerMode ?? null,
      applicableProfile,
      applicableProfile,
      queuedReusableTask?.task ?? null,
    );
  }

  function handleSchemaMismatchAction(action: SchemaMismatchAction) {
    if (loadInspection.kind !== "schema_mismatch") return;
    if (action.kind === "cancelled") {
      void cancelPendingSelection();
      return;
    }
    const { source, profile, sheetId, headerMode } = loadInspection;
    // The user explicitly approved the changed schema. Import remains lexical;
    // no casts, column aliases, or sample values are applied.
    void loadSelection(source, sheetId, headerMode, null, profile, queuedReusableTask?.task ?? null, true);
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
      const message = error instanceof Error ? error.message : String(error);
      setProfileStatus({ kind: "error", message });
    } finally {
      if (activeProfileRequestRef.current?.sequence === sequence) {
        activeProfileRequestRef.current = null;
      }
    }
  }

  useEffect(() => {
    if (datasetStatus.kind !== "ready" || profileStatus.kind !== "idle") return;
    if (autoProfileRevisionRef.current === datasetRevision) return;

    // Cada revisión se analiza una vez de forma automática. Tras cancelar o fallar,
    // el avance principal permite reintentarlo sin crear un bucle de reintentos.
    autoProfileRevisionRef.current = datasetRevision;
    void analyzeQuality();
  }, [datasetRevision, datasetStatus.kind, profileStatus.kind]);

  async function compareActiveDataset() {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    comparisonPageRequestRef.current += 1;
    const requestedRevision = datasetRevisionRef.current;
    const previousComparisonStatus = comparisonStatus;
    setReviewMutationStatus({ kind: "idle" });
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
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    } finally {
      comparisonOperationInFlightRef.current = false;
      if (comparisonRequestRef.current === requestId) {
        setComparisonCancellationPending(false);
      }
    }
  }

  async function clearActiveComparison() {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    comparisonPageRequestRef.current += 1;
    setReviewMutationStatus({ kind: "idle" });
    try {
      await cancelOperation("datasetComparison");
      await clearDatasetComparison();
      if (comparisonRequestRef.current !== requestId) return;
      setComparisonStatus(clearComparison());
    } catch (error: unknown) {
      if (comparisonRequestRef.current !== requestId) return;
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
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
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function consolidateComparedDataset() {
    if (
      comparisonOperationInFlightRef.current
      || comparisonStatus.kind !== "ready"
      || !comparisonStatus.comparison.canConsolidate
    ) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const mutation = {
      mutation: "consolidate" as const,
      requestId,
      datasetRevision: requestedRevision,
      cancellationRequested: false,
    };
    reviewMutationRef.current = mutation;
    setReviewMutationStatus({ kind: "running", mutation: "consolidate", cancellation: "available" });
    try {
      const dataset = await useConsolidatedDataset();
      if (
        reviewMutationRef.current !== mutation
        || comparisonRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      setReviewMutationStatus({ kind: "finalizing", mutation: "consolidate" });
      setDatasetStatus(createReadyDatasetStatus(dataset));
      bumpDatasetRevision();
      resetCompletedPhases();
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setJoinType("inner");
      setExportFormat("csv");
      setPrivacyMode("none");
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setActiveImportProfile(null);
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
    } catch (error: unknown) {
      if (
        reviewMutationRef.current !== mutation
        || comparisonRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      if (isCancellationError(error)) {
        setReviewMutationStatus({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setReviewMutationStatus({ kind: "error", mutation: "consolidate", message });
    } finally {
      if (reviewMutationRef.current === mutation) {
        reviewMutationRef.current = null;
        setReviewMutationStatus((current) =>
          current.kind === "running" || current.kind === "finalizing"
            ? { kind: "idle" }
            : current,
        );
      }
      if (!reviewMutationCancellationPendingRef.current) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  async function resolveComparedConflicts(decisions: ConflictResolution[]) {
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++comparisonRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const mutation = {
      mutation: "resolveConflicts" as const,
      requestId,
      datasetRevision: requestedRevision,
      cancellationRequested: false,
    };
    reviewMutationRef.current = mutation;
    setReviewMutationStatus({ kind: "running", mutation: "resolveConflicts", cancellation: "available" });
    try {
      const dataset = await resolveDatasetConflicts(decisions);
      if (
        reviewMutationRef.current !== mutation
        || comparisonRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      setReviewMutationStatus({ kind: "finalizing", mutation: "resolveConflicts" });
      setDatasetStatus(createReadyDatasetStatus(dataset));
      bumpDatasetRevision();
      resetCompletedPhases();
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setJoinType("inner");
      setExportFormat("csv");
      setPrivacyMode("none");
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setActiveImportProfile(null);
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
    } catch (error: unknown) {
      if (
        reviewMutationRef.current !== mutation
        || comparisonRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      if (isCancellationError(error)) {
        setReviewMutationStatus({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setReviewMutationStatus({ kind: "error", mutation: "resolveConflicts", message });
    } finally {
      if (reviewMutationRef.current === mutation) {
        reviewMutationRef.current = null;
        setReviewMutationStatus((current) =>
          current.kind === "running" || current.kind === "finalizing"
            ? { kind: "idle" }
            : current,
        );
      }
      if (!reviewMutationCancellationPendingRef.current) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  async function joinActiveDataset(requestedJoinType: DatasetJoinType) {
    if (comparisonKeyColumns.length === 0) {
      setJoinStatus(failJoin("Selecciona al menos una columna clave para unir datasets."));
      return;
    }
    if (comparisonOperationInFlightRef.current) return;
    comparisonOperationInFlightRef.current = true;
    const requestId = ++joinRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const mutation = {
      mutation: "join" as const,
      requestId,
      datasetRevision: requestedRevision,
      cancellationRequested: false,
    };
    reviewMutationRef.current = mutation;
    setReviewMutationStatus({ kind: "running", mutation: "join", cancellation: "available" });
    setJoinStatus(beginJoin(requestedJoinType));
    try {
      const dataset = await joinDataset(comparisonKeyColumns, requestedJoinType);
      if (
        reviewMutationRef.current !== mutation
        || joinRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      if (!dataset) {
        setJoinStatus(clearJoin());
        setReviewMutationStatus({ kind: "idle" });
        return;
      }
      setReviewMutationStatus({ kind: "finalizing", mutation: "join" });
      setDatasetStatus(createReadyDatasetStatus(dataset));
      bumpDatasetRevision();
      resetCompletedPhases();
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setActiveImportProfile(null);
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await clearDatasetComparison().catch(() => undefined);
      await prepare.refreshHistory();
      setReviewTab("diagnosis");
      setActivePhase("review");
    } catch (error: unknown) {
      if (
        reviewMutationRef.current !== mutation
        || joinRequestRef.current !== requestId
        || datasetRevisionRef.current !== requestedRevision
      ) return;
      if (isCancellationError(error)) {
        setJoinStatus(clearJoin());
        setReviewMutationStatus({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setJoinStatus(failJoin(message));
    } finally {
      if (reviewMutationRef.current === mutation) {
        reviewMutationRef.current = null;
        setReviewMutationStatus((current) =>
          current.kind === "running" || current.kind === "finalizing"
            ? { kind: "idle" }
            : current,
        );
      }
      if (!reviewMutationCancellationPendingRef.current) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  async function cancelActiveReviewMutation() {
    const mutation = reviewMutationRef.current;
    if (!mutation || mutation.cancellationRequested) return;
    if (reviewMutationStatus.kind !== "running" || reviewMutationStatus.mutation !== mutation.mutation) return;

    mutation.cancellationRequested = true;
    reviewMutationCancellationPendingRef.current = true;
    setReviewMutationCancellationPending(true);
    setReviewMutationStatus({
      kind: "running",
      mutation: mutation.mutation,
      cancellation: "requested",
    });
    try {
      await cancelOperation("reviewMutation");
    } catch (error: unknown) {
      if (reviewMutationRef.current !== mutation) return;
      mutation.cancellationRequested = false;
      setReviewMutationStatus({
        kind: "running",
        mutation: mutation.mutation,
        cancellation: "available",
        cancellationError: error instanceof Error ? error.message : String(error),
      });
    } finally {
      reviewMutationCancellationPendingRef.current = false;
      setReviewMutationCancellationPending(false);
      if (reviewMutationRef.current === null) {
        comparisonOperationInFlightRef.current = false;
      }
    }
  }

  async function cancelActiveOperation(operation: CancellableOperation) {
    if (operation === "load") {
      setDatasetStatus(requestDatasetLoadCancellation);
    } else if (operation === "profile") {
      setProfileStatus(requestProfileCancellation);
    } else if (operation === "export") {
      setExportStatus((current) =>
        current.kind === "loading" ? { ...current, cancellation: "requested" } : current,
      );
    } else if (operation === "datasetComparison") {
      setComparisonCancellationPending(true);
    }

    try {
      await cancelOperation(operation);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      if (operation === "load") {
        setDatasetStatus({ kind: "error", message });
      } else if (operation === "profile") {
        setProfileStatus({ kind: "error", message });
      } else if (operation === "export") {
        setExportStatus({ kind: "error", message });
      } else if (operation === "datasetComparison") {
        setComparisonCancellationPending(false);
        setComparisonStatus(failComparison(message));
      }
    }
  }

  async function exportActiveDataset(request: DeliveryExportRequest) {
    if (datasetStatus.kind !== "ready") return;
    if (exportInFlightRef.current) return;
    exportInFlightRef.current = true;
    const requestId = ++exportRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const isCurrentRequest = () =>
      exportRequestRef.current === requestId && datasetRevisionRef.current === requestedRevision;
    const rules = request.validation.kind === "contract" ? request.validation.rules : [];
    const allowUnvalidated = request.validation.kind === "explicitly_unvalidated";
    setExportStatus({
      kind: "loading",
      format: request.format,
      progress: { operation: "export", stage: "Esperando destino", percent: 0 },
      cancellation: "available",
    });
    try {
      const onProgress = (progress: OperationProgress) => {
        if (!isCurrentRequest()) return;
        setExportStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      };
      let result;
      if (isDatabaseExportFormat(request.format)) {
        if (!request.databaseTarget) throw new Error("Falta configurar el destino de base de datos.");
        result = await exportDatasetToDatabase(
          request.databaseTarget,
          rules,
          allowUnvalidated,
          onProgress,
          request.privacyMode,
        );
      } else {
        result = recipeDraft && request.format === "bundle"
          ? await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode, recipeDraft)
          : await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode);
      }
      if (!isCurrentRequest()) return;
      setExportStatus(result ? { kind: "success", result } : { kind: "idle" });
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      if (isCancellationError(error)) {
        setExportStatus({ kind: "cancelled" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setExportStatus({ kind: "error", message });
    } finally {
      exportInFlightRef.current = false;
    }
  }

  async function changePage(offset: number) {
    if (datasetStatus.kind !== "ready" || datasetStatus.pageLoading) return;

    const previous = datasetStatus;
    const requestedRevision = datasetRevisionRef.current;
    const requestId = ++pageRequestRef.current;
    setDatasetStatus(beginPageLoad(previous));

    try {
      const page = await getDatasetPage(offset, PAGE_SIZE);
      if (datasetRevisionRef.current !== requestedRevision || pageRequestRef.current !== requestId) return;
      if (pageCancellationRequestRef.current === requestId) {
        setDatasetStatus(previous);
        return;
      }
      setDatasetStatus(completePageLoad(previous, page));
    } catch (error: unknown) {
      if (datasetRevisionRef.current !== requestedRevision || pageRequestRef.current !== requestId) return;
      if (isCancellationError(error)) {
        setDatasetStatus(previous);
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus(failPageLoad(previous, message));
    } finally {
      if (pageRequestRef.current === requestId) {
        if (pageCancellationRequestRef.current === requestId) {
          pageCancellationRequestRef.current = null;
          setPageCancellationPending(false);
        }
      }
    }
  }

  async function cancelPageChange() {
    if (datasetStatus.kind !== "ready" || !datasetStatus.pageLoading) return;
    const requestId = pageRequestRef.current;
    if (pageCancellationRequestRef.current === requestId) return;
    pageCancellationRequestRef.current = requestId;
    setPageCancellationPending(true);
    try {
      await cancelOperation("datasetPage");
    } catch {
      if (pageCancellationRequestRef.current === requestId) {
        pageCancellationRequestRef.current = null;
        setPageCancellationPending(false);
      }
    }
  }

  const readyDataset = datasetStatus.kind === "ready" ? datasetStatus : undefined;
  const retainedDataset =
    datasetStatus.kind === "loading" ? datasetStatus.previous : undefined;
  const activeDataset = readyDataset ?? retainedDataset;
  const reusableTaskSchema: ReusableTaskSchema | null = activeDataset
    ? activeImportProfile?.schema
      ?? activeDataset.dataset.columns.map(({ name, dataType }) => ({ name, dataType }))
    : null;
  const reusableTaskDraft = activeDataset && activeImportProfile
    ? {
        version: 1 as const,
        importProfile: activeImportProfile,
        recipe: recipeDraft,
        exceptionPolicy: createReusableTaskExceptionPolicy(activeImportProfile.schema, recipeDraft),
        qualityRules: deliveryRules(deliveryContract),
        outputFormat: reusableOutputFormat(exportFormat),
        privacyMode,
      }
    : null;
  const operationBusy = coreOperationBusy || loadSelectionBusy || projects.isBusy;
  operationBusyRef.current = operationBusy;
  const activePhaseIndex = Math.max(0, workflowPhases.findIndex((phase) => phase.id === activePhase));
  const activePhaseMeta = workflowPhases[activePhaseIndex];
  const previousPhase = workflowPhases[activePhaseIndex - 1];
  const nextPhase = workflowPhases[activePhaseIndex + 1];
  const progressValue = activePhaseIndex + 1;
  const profileGatedPhase = activePhase === "review" || activePhase === "prepare";
  const primaryNextLabel = profileGatedPhase && profileStatus.kind !== "ready"
    ? profileStatus.kind === "error" || profileStatus.kind === "cancelled"
      ? "Reintentar análisis"
      : profileStatus.kind === "loading"
        ? "Analizando calidad…"
        : "Analizar calidad"
    : activePhase === "review"
      ? "Ver plan de preparación"
      : activePhase === "prepare"
        ? "Revisar opciones de entrega"
        : nextPhase ? `Continuar a ${nextPhase.label}` : "";
  const primaryNextDescription = profileGatedPhase && profileStatus.kind !== "ready"
    ? profileStatus.kind === "error"
      ? "El análisis tuvo un problema; puedes intentarlo de nuevo."
      : profileStatus.kind === "cancelled"
        ? "El análisis se canceló y no publicó resultados parciales; puedes volver a intentarlo."
      : "El diagnóstico se inicia automáticamente al cargar y puede tardar según el tamaño."
    : profileGatedPhase
      ? activePhase === "review"
        ? "El diagnóstico está actualizado. Revisa las señales antes de cambiar los datos."
        : "Prepara los datos; el diagnóstico se actualizará después de cada corrección."
      : activeDataset ? nextPhase?.description : "Carga un dataset para continuar con la revisión.";

  function handleNextPhase() {
    if (!nextPhase) return;
    if (profileGatedPhase && profileStatus.kind !== "ready") {
      if (
        profileStatus.kind === "idle" ||
        profileStatus.kind === "error" ||
        profileStatus.kind === "cancelled"
      ) {
        void analyzeQuality();
      }
      return;
    }
    setActivePhase(nextPhase.id);
  }

  function isPhaseComplete(phase: WorkflowPhase): boolean {
    switch (phase) {
      case "load":
      case "review":
        return completedPhases.has(phase);
      case "prepare":
        return prepare.changeStatus.kind === "applied";
      case "deliver":
        return exportStatus.kind === "success";
    }
  }

  function applyReusableTask(task: ReusableTask, preserveActiveImportProfile = false) {
    setQueuedReusableTask(null);
    setReusableTaskApplicationReview(null);
    if (!preserveActiveImportProfile) setActiveImportProfile(task.importProfile);
    setRecipeDraft(task.recipe);
    setActiveExceptionPolicy(task.exceptionPolicy ?? null);
    setDeliveryContract(deliveryContractFromRules(task.qualityRules));
    setExportFormat(task.outputFormat);
    setPrivacyMode(task.privacyMode);
    setExportStatus({ kind: "idle" });
    setRecipeSession((current) => current + 1);
    setCompletedPhases(new Set(["load"]));
    setActivePhase(task.recipe ? "prepare" : "review");
  }

  function handleRecipeDraftChange(draft: SavedRecipe) {
    setRecipeDraft(draft);
    setActiveExceptionPolicy((current) => current
      ? refreshReusableTaskExceptionPolicy(current, draft) ?? null
      : null);
  }

  function prepareReusableTaskImport(taskId: string, task: ReusableTask) {
    setReusableTaskApplicationReview(null);
    setQueuedReusableTask({ id: taskId, task });
  }

  function applyReviewedReusableTask() {
    const review = reusableTaskApplicationReview;
    if (!review) return;
    applyReusableTask(review.task, true);
  }

  function dismissReusableTaskApplicationReview() {
    setReusableTaskApplicationReview(null);
  }

  const reviewHasContextualContinue = activePhase === "review"
    && reviewTab === "diagnosis"
    && profileStatus.kind === "ready";
  const loadRuntime: LoadRuntimeState = status.kind === "ready"
    ? { kind: "connected" }
    : status.kind === "browser"
      ? { kind: "browser" }
      : { kind: "unavailable" };

  return (
    <div className="shell">
      <a className="skip-link" href="#main-content">Saltar al contenido principal</a>
      <p className="visually-hidden" aria-live="polite" aria-atomic="true">
        Etapa activa: {activePhaseMeta.label}.
      </p>
      <p id="dataset-required-hint" className="visually-hidden">
        Carga un dataset para habilitar las etapas Revisar, Preparar y Entregar.
      </p>
      <aside className="sidebar" aria-label="Navegación principal">
        <div className="brand">
          <p className="eyebrow">Estación local de datos</p>
          <h1 id="app-title"><svg className="brand__mark" viewBox="0 0 32 32" fill="none" aria-hidden="true"><rect x="2" y="4" width="7" height="24" rx="2" fill="currentColor" /><rect x="12" y="10" width="7" height="18" rx="2" fill="currentColor" opacity=".7" /><rect x="22" y="16" width="7" height="12" rx="2" fill="currentColor" opacity=".45" /></svg>Columnia</h1>
        </div>

        <WorkspaceNav activePhase={activePhase} />

        <nav className="side-nav" aria-label="Flujo de preparación de datos">
          {workflowPhases.map((phase, phaseIndex) => {
            const available = phase.id === "load" || Boolean(activeDataset);
            const phaseComplete = isPhaseComplete(phase.id);
            const phaseState = phaseIndex === activePhaseIndex
              ? "current"
              : phaseComplete ? "complete" : "upcoming";
            return (
              <button
                key={phase.id}
                type="button"
                aria-label={phase.label}
                aria-description={phaseComplete ? "Completada" : undefined}
                className={`side-nav__item side-nav__item--${phaseState}${activePhase === phase.id ? " side-nav__active" : ""}`}
                aria-current={activePhase === phase.id ? "step" : undefined}
                aria-disabled={!available || undefined}
                aria-describedby={!available ? "dataset-required-hint" : undefined}
                onMouseEnter={() => preloadPhase(phase.id)}
                onFocus={() => preloadPhase(phase.id)}
                onClick={() => available && setActivePhase(phase.id)}
                disabled={operationBusy}
                title={!available ? "Carga un dataset para habilitar esta etapa" : undefined}
              >
                <span className="side-nav__marker" aria-hidden="true">
                  {phaseComplete ? "✓" : phase.number}
                </span>
                <span className="side-nav__copy">
                  <span className="side-nav__label-row">
                    <strong>{phase.label}</strong>
                    <small className="side-nav__state" aria-hidden="true">
                      {phaseComplete ? "Hecho" : phaseState === "current" ? "Ahora" : "Después"}
                    </small>
                  </span>
                  <small aria-hidden="true">{phase.description}</small>
                </span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar__dataset">
          <span>Dataset activo</span>
          <strong>
            {activeDataset ? activeDataset.dataset.fileName : "Sin dataset"}
          </strong>
        </div>

        <div className="sidebar__tools">
          <details
            className="sidebar__utilities"
            open={sidebarUtilitiesOpen}
          >
          <summary
            onClick={(event) => {
              event.preventDefault();
              const details = event.currentTarget.parentElement;
              const willOpen = !(details instanceof HTMLDetailsElement && details.open);
              setSidebarUtilitiesOpen(willOpen);
              if (willOpen) setSidebarLegalOpen(false);
            }}
          >
            Preferencias y recursos
          </summary>
          <div className="sidebar__utilities-content">
            <ResourceMonitor
              enabled={status.kind === "ready"}
              visible={sidebarUtilitiesOpen}
              observeUsage={sidebarUtilitiesOpen}
              performanceProfile={performanceProfile}
              onPerformanceProfileChange={(profile) => {
                setPerformanceProfile(profile);
                if (!projects.activeProject) writePerformanceProfile(profile);
              }}
            />
            <ThemeSwitcher />
            <UpdatePanel
              enabled={status.kind === "ready" && status.info?.updaterConfigured === true}
              currentVersion={status.kind === "ready" && status.info ? status.info.version : null}
            />
            <button
              type="button"
              className="sidebar__diagnostics-trigger"
              onClick={() => {
                setSidebarUtilitiesOpen(false);
                setDiagnosticsOpen(true);
              }}
            >
              Preparar diagnóstico local
            </button>
          </div>
          </details>

          <details className="sidebar__legal" open={sidebarLegalOpen}>
          <summary
            onClick={(event) => {
              event.preventDefault();
              const details = event.currentTarget.parentElement;
              const willOpen = !(details instanceof HTMLDetailsElement && details.open);
              setSidebarLegalOpen(willOpen);
              if (willOpen) setSidebarUtilitiesOpen(false);
            }}
          >
            Licencia y privacidad
          </summary>
          <div
            className="sidebar__legal-content"
            role="region"
            aria-labelledby="legal-panel-title"
            aria-describedby="legal-panel-summary"
          >
            <h2 id="legal-panel-title">Licencia y privacidad de Columnia</h2>
            <p id="legal-panel-summary"><strong>Columnia</strong> procesa los datos localmente y no inicia conexiones de red por sí sola. Solo una exportación ODBC explícita envía las filas elegidas al destino que el usuario configura.</p>
            <p><strong>Versión:</strong> {status.kind === "ready" && status.info ? status.info.version : "no disponible"}</p>
            <h2>Licencia</h2>
            <p>El producto se distribuye bajo MIT. Las dependencias conservan sus avisos en <code>THIRD_PARTY_NOTICES.md</code>.</p>
            <h2>Privacidad local</h2>
            <p>Los archivos seleccionados, proyectos, perfiles y artefactos temporales permanecen en el dispositivo, salvo las filas que el usuario elija enviar mediante una exportación ODBC explícita. La cadena de conexión y la contraseña solo viven durante esa sesión. No se usan telemetría, cuentas ni analítica remota.</p>
            <h2>Retención y borrado</h2>
            <p>El usuario controla la carpeta de proyectos y puede eliminar proyectos desde la aplicación o borrar sus archivos locales. Los snapshots huérfanos se limpian de forma oportunista después de una hora de gracia.</p>
            <p className="sidebar__legal-note">Responsable y canal de contacto: deben definirse para la jurisdicción de publicación antes de distribuir.</p>
          </div>
          </details>
        </div>
      </aside>

      <main id="main-content" className="main-content" tabIndex={-1}>
        <header className="topbar">
          <div className="flow-overview">
            <div className="flow-overview__copy">
              <p className="flow-overview__step">Paso {progressValue} de {workflowPhases.length}</p>
              <p className="page-title">{activePhaseMeta.label}</p>
              <p className="flow-overview__next">
                {nextPhase ? `Después: ${nextPhase.label}` : "Última etapa del flujo"}
              </p>
            </div>
            <div
              className="flow-progress"
              role="progressbar"
              aria-label="Progreso del flujo"
              aria-valuemin={1}
              aria-valuemax={workflowPhases.length}
              aria-valuenow={progressValue}
              aria-valuetext={`Paso ${progressValue} de ${workflowPhases.length}: ${activePhaseMeta.label}`}
            >
              <span style={{ width: `${(progressValue / workflowPhases.length) * 100}%` }} />
            </div>
          </div>
          <div
            className={`runtime runtime--${status.kind}`}
            role={status.kind === "error" ? "alert" : "status"}
            aria-live={status.kind === "error" ? "assertive" : "polite"}
            aria-atomic="true"
          >
            {status.kind === "loading" && "Conectando con Rust…"}
            {status.kind === "browser" && "Vista web · motor no conectado"}
            {status.kind === "ready" && "Motor local listo"}
            {status.kind === "error" && `Error del motor: ${status.message}`}
          </div>
        </header>

        <section
          className={`workspace workspace--${activePhase}`}
          aria-label={`Etapa ${activePhaseMeta.label}`}
          aria-busy={operationBusy}
        >
          <div
            ref={stageRef}
            className="workspace__stage"
            key={activePhase}
            tabIndex={-1}
            aria-label={`Contenido de la etapa ${activePhaseMeta.label}`}
          >
          <Suspense fallback={<div className="phase-loading" role="status">Cargando etapa…</div>}>
            {activePhase === "load" && (
              <LoadPhase
                runtime={loadRuntime}
                reusableTaskPanel={(
                  <ReusableTaskPanel
                    connected={loadRuntime.kind === "connected"}
                    blocked={operationBusy}
                    schema={reusableTaskSchema}
                    draft={reusableTaskDraft}
                    onApply={applyReusableTask}
                    onPrepareImport={prepareReusableTaskImport}
                    pendingTaskId={queuedReusableTask?.id ?? null}
                    pendingTaskName={queuedReusableTask?.task.name ?? null}
                    onClearPendingImport={() => setQueuedReusableTask(null)}
                  />
                )}
                pendingTaskName={queuedReusableTask?.task.name ?? null}
                disabled={operationBusy}
                datasetStatus={datasetStatus}
                inspection={loadInspection}
                recentDatasets={recentDatasets}
                sampleDatasets={sampleDatasets}
                onSelect={selectDataset}
                onSelectSample={selectSampleDataset}
                onSelectRecent={selectRecentDataset}
                onClearRecent={() => setRecentDatasets([])}
                onRemoveRecent={(id) => setRecentDatasets((current) => removeRecentDataset(current, id))}
                onSheetAction={handleSheetSelection}
                onRetryHeaderPreview={retryDelimitedHeaderReview}
                onProfileReviewAction={handleProfileReviewAction}
                onResourcePreflightAction={handleResourcePreflightAction}
                onSchemaMismatchAction={handleSchemaMismatchAction}
                onCancelLoad={() => cancelActiveOperation("load")}
                workbookInspectionCancellationPending={workbookInspectionCancellationPending}
                onCancelWorkbookInspection={() => void cancelWorkbookInspection()}
              >
                <ProjectsPanel
                  catalog={projects.catalog}
                  operation={projects.operation}
                  deletion={projects.deletion}
                  activeProject={projects.activeProject}
                  versions={projects.versions}
                  autoSave={projects.autoSave}
                  autoSaveEnabled={projects.autoSaveEnabled}
                  datasetFileName={activeDataset?.dataset.fileName ?? null}
                  disabled={operationBusy}
                  onSave={(name) => void projects.save(name)}
                  onOpen={(projectId) => void projects.open(projectId)}
                  onRestore={(projectId, versionId) => void projects.restore(projectId, versionId)}
                  onAutoSaveChange={projects.setAutoSaveEnabled}
                  onDeleteRequest={projects.requestDelete}
                  onDeleteCancel={projects.cancelDelete}
                  onDeleteConfirm={() => void projects.confirmDelete()}
                  onRetry={() => void projects.refresh()}
                  onClearFeedback={projects.clearFeedback}
                />
              </LoadPhase>
            )}

            {activePhase === "review" && readyDataset && (
              <ReviewPhase
                datasetStatus={readyDataset}
                profileStatus={profileStatus}
                reviewTab={reviewTab}
                onTabChange={setReviewTab}
                onPageChange={changePage}
                onCancelPageChange={() => void cancelPageChange()}
                pageCancellationPending={pageCancellationPending}
                onCancelProfile={() => cancelActiveOperation("profile")}
                onContinueToPrepare={(target) => {
                  setPrepareFocusTarget(target ?? null);
                  setCompletedPhases((current) => new Set(current).add("review"));
                  setActivePhase("prepare");
                }}
                comparisonStatus={comparisonStatus}
                comparisonCancellationPending={comparisonCancellationPending}
                comparisonKeyColumns={comparisonKeyColumns}
                onComparisonKeyColumnsChange={setComparisonKeyColumns}
                datasetColumns={readyDataset.dataset.columns}
                joinStatus={joinStatus}
                reviewMutationStatus={reviewMutationStatus}
                reviewMutationCancellationPending={reviewMutationCancellationPending}
                joinType={joinType}
                onJoinTypeChange={setJoinType}
                onCompare={() => void compareActiveDataset()}
                onCancelComparison={() => void cancelActiveOperation("datasetComparison")}
                onClearComparison={() => void clearActiveComparison()}
                onConsolidate={() => void consolidateComparedDataset()}
                onResolveConflicts={(decisions) => void resolveComparedConflicts(decisions)}
                onConflictPageChange={(offset) => changeConflictPage(offset)}
                onJoin={(requestedJoinType) => void joinActiveDataset(requestedJoinType)}
                onCancelReviewMutation={() => void cancelActiveReviewMutation()}
                sqlHistory={sqlHistory}
                onSqlHistoryChange={setSqlHistory}
                datasetRevision={datasetRevision}
                queryEngine={queryEngine}
                onQueryEngineChange={setQueryEngine}
                analysisSampleRows={analysisSampleRows}
                onAnalysisSampleRowsChange={setAnalysisSampleRows}
              />
            )}

            {activePhase === "prepare" && readyDataset && (
              <PreparePhase
                dataset={readyDataset.dataset}
                datasetRevision={datasetRevision}
                initialQualityFocus={prepareFocusTarget}
                onQualityFocusHandled={() => setPrepareFocusTarget(null)}
                profileStatus={profileStatus}
                changeStatus={prepare.changeStatus}
                historyStatus={prepare.historyStatus}
                qualityRules={deliveryRules(deliveryContract)}
                recipeDraft={recipeDraft}
                recipeSession={recipeSession}
                onCancelProfile={() => cancelActiveOperation("profile")}
                onCancelPrepare={prepare.cancelCurrent}
                onRemoveDuplicates={prepare.applyDuplicateRemoval}
                onRemoveNearDuplicates={prepare.applyNearDuplicateRemoval}
                onRemoveEmptyRows={prepare.applyEmptyRowRemoval}
                onRemoveConstantColumns={prepare.applyConstantColumnRemoval}
                onRemoveEmptyColumns={prepare.applyEmptyColumnRemoval}
                onRemoveHighNullColumns={prepare.applyHighNullColumnRemoval}
                onRemoveIdentifierColumns={prepare.applyIdentifierColumnRemoval}
                onRemovePersonalColumns={prepare.applyPersonalColumnRemoval}
                onMaskPersonalValues={prepare.applyPersonalValueMasking}
                onNormalizeBooleans={prepare.applyBooleanNormalization}
                onParseDates={prepare.applyDateParsing}
                onCastNumeric={prepare.applyNumericCast}
                onFixEncoding={prepare.applyEncodingFix}
                onNullifyInvalidTypes={prepare.applyInvalidTypeCleanup}
                onImputeMissingValues={prepare.applyMissingValueImputation}
                onImputeCategoricalValues={prepare.applyCategoricalImputation}
                onImputeOutliers={prepare.applyOutlierImputation}
                onCapOutliers={prepare.applyOutlierCapping}
                onDropOutliers={prepare.applyOutlierRemoval}
                onEnableRowAudit={prepare.applyRowAudit}
                onNormalizeColumns={prepare.applyColumnNormalization}
                onApplyRecommended={prepare.applyRecommendedCorrections}
                onTrimText={prepare.trimText}
                onNormalizeText={prepare.normalizeText}
                onApplyTransforms={prepare.applyStructuralTransforms}
                onRecipeDraftChange={handleRecipeDraftChange}
                onUndo={prepare.undoChange}
                onRedo={prepare.redoChange}
              />
            )}

            {activePhase === "deliver" && readyDataset && (
              <DeliveryPhase
                dataset={readyDataset.dataset}
                recipeDraft={recipeDraft}
                preparationChanges={prepare.historyStatus.entries
                  .filter((entry) => entry.index > 0 && entry.index <= prepare.historyStatus.currentIndex)
                  .map((entry) => entry.label)}
                contract={deliveryContract}
                exportState={exportStatus}
                exportFormat={exportFormat}
                onExportFormatChange={setExportFormat}
                privacyMode={privacyMode}
                onPrivacyModeChange={setPrivacyMode}
                onContractAction={updateDeliveryContract}
                onExport={exportActiveDataset}
                onCancelExport={() => cancelActiveOperation("export")}
              />
            )}
          </Suspense>
          </div>
          <footer className={`flow-footer${nextPhase ? "" : " flow-footer--terminal"}`} aria-label="Navegación entre etapas">
            {nextPhase && !reviewHasContextualContinue && (
              <div className="flow-footer__copy">
                <p className="step">Siguiente paso</p>
                <strong>{activePhase === "review" && profileStatus.kind === "ready" ? "Plan de preparación" : nextPhase.label}</strong>
                <p>{primaryNextDescription}</p>
              </div>
            )}
            <div className="flow-footer__actions">
              {previousPhase && (
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => setActivePhase(previousPhase.id)}
                  disabled={operationBusy}
                >
                  Volver a {previousPhase.label}
                </button>
              )}
              {nextPhase && activeDataset && !reviewHasContextualContinue && (
                <button
                  type="button"
                  className="primary-action flow-footer__next-action"
                  onMouseEnter={() => preloadPhase(nextPhase.id)}
                  onFocus={() => preloadPhase(nextPhase.id)}
                  onClick={handleNextPhase}
                  disabled={!activeDataset || operationBusy}
                >
                  {primaryNextLabel}
                </button>
              )}
            </div>
          </footer>
        </section>
      </main>

      {reusableTaskApplicationReview && (
        <ModalDialog
          role="dialog"
          labelledBy="reusable-task-apply-title"
          describedBy="reusable-task-apply-description"
          onDismiss={dismissReusableTaskApplicationReview}
        >
          <p className="eyebrow">Tarea reutilizable</p>
          <h2 id="reusable-task-apply-title">Revisa la configuración guardada</h2>
          <p id="reusable-task-apply-description">
            Se importó el archivo con {reusableTaskApplicationReview.importProfileUsed
              ? `el perfil de “${reusableTaskApplicationReview.task.name}”`
              : "la interpretación predeterminada"}.
            Elige si aplicas los ajustes restantes al espacio de trabajo.
          </p>
          <div className="sheet-import-summary">
            <dl>
              <div>
                <dt>Preparación</dt>
                <dd>{reusableTaskApplicationReview.task.recipe?.name ?? "Sin receta"}</dd>
              </div>
              <div>
                <dt>Reglas de calidad</dt>
                <dd>{reusableTaskApplicationReview.task.qualityRules.length}</dd>
              </div>
              <div>
                <dt>Salida y privacidad</dt>
                <dd>{reusableTaskApplicationReview.task.outputFormat.toUpperCase()} · {reusableTaskApplicationReview.task.privacyMode}</dd>
              </div>
            </dl>
          </div>
          {reusableTaskApplicationReview.schemaMismatchConfirmed && (
            <p className="notice" role="note">
              Confirmaste un esquema distinto. Las decisiones de conversión ligadas al esquema guardado quedan invalidadas. Revisa las columnas de la receta y las reglas antes de continuar; ninguna transformación se ejecutará ahora.
            </p>
          )}
          <p className="notice" role="note">
            La receta se cargará como borrador editable y no cambiará filas hasta que la revises y la ejecutes. No se guardaron credenciales ni autorización de sobrescritura.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={dismissReusableTaskApplicationReview}>
              Seguir sin esos ajustes
            </button>
            <button type="button" className="primary-action" onClick={applyReviewedReusableTask}>
              Aplicar tarea guardada
            </button>
          </div>
        </ModalDialog>
      )}

      {diagnosticsOpen && (
        <DiagnosticsDialog
          appVersion={status.kind === "ready" ? status.info?.version ?? null : null}
          activePhase={activePhase}
          datasetMetrics={activeDataset
            ? {
              rowCount: activeDataset.dataset.rowCount,
              columnCount: activeDataset.dataset.columnCount,
              fileSizeBytes: activeDataset.dataset.fileSizeBytes,
            } satisfies DatasetMetricInput
            : null}
          onDismiss={() => setDiagnosticsOpen(false)}
        />
      )}
    </div>
  );
}
