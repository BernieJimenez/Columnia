import { lazy, Suspense, useEffect, useEffectEvent, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { personalDataColumnNames, suggestQualityRules } from "./features/delivery/deliveryModel";
import { useDeliveryController } from "./features/delivery/useDeliveryController";
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
  recoverDatasetLoadCancellationFailure,
  restoreDatasetAfterLoadFailure,
  setLoadInspectionError,
  beginDelimitedHeaderReview,
  completeDelimitedHeaderReview,
  beginSchemaPreview,
  completeSchemaPreview,
  failSchemaPreview,
  delimitedHeaderInspection,
  updateDatasetLoadProgress,
  updateSheetSelection,
  schemaMismatchInspection,
  workbookInspection,
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
  PAGE_SIZE,
  beginPageLoad,
  completePageLoad,
  failPageLoad,
  normalizePageOffset,
} from "./features/review/reviewModel";
import { useReviewController } from "./features/review/useReviewController";
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
  discardDatasetSelection,
  getAppInfo,
  getDatasetPage,
  inspectDroppedDataset as inspectDroppedDatasetSource,
  inspectSampleDataset,
  inspectWorkbookSheets,
  listSampleDatasets,
  loadDatasetSelection,
  pickDatasetSource,
  previewDelimitedHeaderReview,
  previewDatasetSelection,
  type AppInfo,
  type DatasetSourceInspection,
  type ImportProfile,
  type ExportFormat,
  type PerformanceProfile,
  type ReusableTask,
  type ReusableTaskExceptionPolicy,
  type ReusableTaskOutputFormat,
  type ReusableTaskSchema,
  type SavedRecipe,
  type SampleDatasetDescriptor,
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
  const [performanceProfile, setPerformanceProfile] = useState<PerformanceProfile>(readPerformanceProfile);
  const [activePhase, setActivePhase] = useState<WorkflowPhase>("load");
  const [prepareFocusTarget, setPrepareFocusTarget] = useState<QualityActionTarget | null>(null);
  const [loadInspection, setLoadInspection] = useState<LoadInspectionState>({ kind: "idle" });
  const [workbookInspectionCancellationPending, setWorkbookInspectionCancellationPending] = useState(false);
  const [selectionFinalizing, setSelectionFinalizing] = useState(false);
  const headerPreviewRequestRef = useRef(0);
  const schemaPreviewRequestRef = useRef(0);
  const schemaPreviewActiveRequestRef = useRef<number | null>(null);
  const schemaPreviewInFlightRef = useRef(false);
  const inspectionRequestRef = useRef(0);
  const inspectionInFlightRef = useRef(false);
  const selectionCancellationInFlightRef = useRef(false);
  const loadRequestRef = useRef(0);
  const loadInFlightRef = useRef(false);
  const [pageCancellationPending, setPageCancellationPending] = useState(false);
  const pageCancellationRequestRef = useRef<number | null>(null);
  const [activeImportProfile, setActiveImportProfile] = useState<ImportProfile | null>(null);
  const [queuedReusableTask, setQueuedReusableTask] = useState<QueuedReusableTask | null>(null);
  const [reusableTaskApplicationReview, setReusableTaskApplicationReview] = useState<ReusableTaskApplicationReview | null>(null);
  const [recentDatasets, setRecentDatasets] = useState<RecentDataset[]>(readRecentDatasets);
  const [sampleDatasets, setSampleDatasets] = useState<SampleDatasetDescriptor[]>([]);
  const [recipeDraft, setRecipeDraft] = useState<SavedRecipe | null>(null);
  const [activeExceptionPolicy, setActiveExceptionPolicy] = useState<ReusableTaskExceptionPolicy | null>(null);
  const [datasetRevision, setDatasetRevision] = useState(0);
  const datasetRevisionRef = useRef(0);
  const pageRequestRef = useRef(0);
  const operationBusyRef = useRef(false);
  const [completedPhaseRevisions, setCompletedPhaseRevisions] = useState<Partial<Record<WorkflowPhase, number>>>({});
  const [recipeSession, setRecipeSession] = useState(0);
  const stageRef = useRef<HTMLDivElement>(null);
  const previousPhaseRef = useRef(activePhase);

  useEffect(() => {
    if (previousPhaseRef.current === activePhase) return;
    previousPhaseRef.current = activePhase;
    stageRef.current?.focus({ preventScroll: true });
  }, [activePhase]);

  function bumpDatasetRevision() {
    const nextRevision = datasetRevisionRef.current + 1;
    datasetRevisionRef.current = nextRevision;
    setCompletedPhaseRevisions({ load: nextRevision });
    review.invalidateRequests();
    pageRequestRef.current += 1;
    pageCancellationRequestRef.current = null;
    setPageCancellationPending(false);
    delivery.invalidateRequests();
    setDatasetRevision(nextRevision);
  }
  const [sidebarUtilitiesOpen, setSidebarUtilitiesOpen] = useState(false);
  const [sidebarLegalOpen, setSidebarLegalOpen] = useState(false);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const delivery = useDeliveryController({
    datasetRevisionRef,
    datasetReady: datasetStatus.kind === "ready",
    datasetFingerprint: datasetStatus.kind === "ready"
      ? JSON.stringify({
          fileName: datasetStatus.dataset.fileName,
          rowCount: datasetStatus.dataset.rowCount,
          columns: datasetStatus.dataset.columns,
          rows: datasetStatus.dataset.rows,
        })
      : null,
    recipeDraft,
  });
  const review = useReviewController({
    datasetRevisionRef,
    datasetRevision,
    datasetReady: datasetStatus.kind === "ready",
    onDatasetReplaced: async (dataset, mutation) => {
      setDatasetStatus(createReadyDatasetStatus(dataset));
      bumpDatasetRevision();
      if (mutation !== "join") delivery.resetOutput();
      review.invalidateProfile();
      projects.unlinkActiveProject();
      setActiveImportProfile(null);
      delivery.resetContract();
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      if (mutation === "join") await clearDatasetComparison().catch(() => undefined);
      await prepare.refreshHistory();
      if (mutation === "join") setActivePhase("review");
    },
  });
  const { profileStatus } = review;
  const prepare = usePrepareController({
    activeDataset: datasetStatus.kind === "ready" ? datasetStatus.dataset : null,
    exceptionPolicy: activeExceptionPolicy,
    onDatasetChanged: (dataset) => {
      bumpDatasetRevision();
      setActiveExceptionPolicy((current) => current && exceptionPolicyMatchesSchema(current, dataset.columns)
        ? current
        : null);
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
      review.resetForDataset();
      void clearDatasetComparison().catch(() => undefined);
    },
    onProfileInvalidated: review.invalidateProfile,
    onDeliveryInvalidated: delivery.invalidateGate,
  });
  // Work the user started in the current phase. The quality analysis and the
  // autosave run in the background and show their own progress, so they block
  // new operations but not moving between phases.
  const foregroundOperationBusy =
    datasetStatus.kind === "loading" ||
    (datasetStatus.kind === "ready" && datasetStatus.pageLoading) ||
    prepare.changeStatus.kind === "working" ||
    delivery.busy ||
    review.busy;
  const coreOperationBusy = foregroundOperationBusy || profileStatus.kind === "loading";
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
      ...delivery.workspace,
      recipeDraft,
      ...review.workspace,
      previewOffset: datasetStatus.kind === "ready" ? datasetStatus.pageOffset : 0,
      activePhase,
      performanceProfile,
      importProfile: activeImportProfile ?? undefined,
    },
    onActiveProjectDeleted: () => {
      review.forgetProjectSettings({ clearSqlHistory: true });
      setPerformanceProfile(readPerformanceProfile());
      delivery.resetOutput();
    },
    onActiveProjectUnlinked: () => {
      setPerformanceProfile(readPerformanceProfile());
      review.forgetProjectSettings({ clearSqlHistory: false });
      delivery.resetOutput();
    },
    onProjectOpened: async ({ dataset, workspace, profile }) => {
      bumpDatasetRevision();
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
      review.restoreWorkspace(workspace, dataset.columns, profile);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      delivery.applySettings(workspace);
      await clearDatasetComparison().catch(() => undefined);
      setRecipeDraft(workspace.recipeDraft);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      setActivePhase(workspace.activePhase ?? "review");
      const selectedPerformanceProfile = workspace.performanceProfile ?? readPerformanceProfile();
      setPerformanceProfile(selectedPerformanceProfile);
    },
  });
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

  const onDatasetDrop = useEffectEvent(() => {
    if (operationBusyRef.current) return;
    void inspectDatasetSource(inspectDroppedDatasetSource());
  });

  useEffect(() => {
    if (status.kind !== "ready") return;

    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("columnia://dataset-drop", () => onDatasetDrop()).then((cleanup) => {
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
    schemaPreviewRequestRef.current += 1;
    const source = loadInspection.source;
    setLoadInspection((current) => beginDelimitedHeaderReview(current));
    void requestDelimitedHeaderReview(source);
  }

  async function previewSelectedSchema(
    selection: Extract<LoadInspectionState, { kind: "sheet" }>,
    expectedProfile: ImportProfile | null,
    conventions: Required<Pick<ImportProfile, "dateConvention" | "numberConvention">>,
  ) {
    if (schemaPreviewInFlightRef.current) return;
    schemaPreviewInFlightRef.current = true;
    const requestId = ++schemaPreviewRequestRef.current;
    schemaPreviewActiveRequestRef.current = requestId;
    const selectionId = selection.source.selectionId;
    setLoadInspection((current) => beginSchemaPreview(current));
    try {
      const isDelimited = selection.source.format === "csv" || selection.source.format === "tsv";
      const preview = await previewDatasetSelection(
        selectionId,
        selection.source.format === "excel" ? selection.selectedSheetId : null,
        selection.source.format === "excel" || isDelimited ? selection.headerMode : null,
        expectedProfile,
        isDelimited ? conventions.dateConvention : null,
        isDelimited ? conventions.numberConvention : null,
      );
      if (schemaPreviewRequestRef.current !== requestId) return;
      setLoadInspection((current) =>
        current.kind === "sheet" && current.source.selectionId === selectionId
          ? completeSchemaPreview(current, preview)
          : current,
      );
    } catch (error: unknown) {
      if (schemaPreviewRequestRef.current !== requestId) return;
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) =>
        current.kind === "sheet" && current.source.selectionId === selectionId
          ? failSchemaPreview(current, message)
          : current,
      );
    } finally {
      if (schemaPreviewActiveRequestRef.current === requestId) {
        schemaPreviewActiveRequestRef.current = null;
        schemaPreviewInFlightRef.current = false;
      }
    }
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
      projects.unlinkActiveProject();
      delivery.resetContract();
      setRecipeDraft(null);
      setActiveExceptionPolicy(null);
      setRecipeSession((current) => current + 1);
      setLoadInspection({ kind: "idle" });
      review.invalidateProfile();
      review.resetForDataset();
      delivery.resetOutput();
      await clearDatasetComparison().catch(() => undefined);
      if (loadRequestRef.current !== requestId) return;
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      if (loadRequestRef.current !== requestId) return;
      review.setReviewTab("diagnosis");
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
    schemaPreviewRequestRef.current += 1;
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
      if (!isCurrentRequest()) return;
      setLoadInspection(workbookInspection(source, selectedImportProfile));
      return;
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
    const source = loadInspection.source;
    setWorkbookInspectionCancellationPending(true);
    try {
      await cancelAndDiscardSelection(source);
    } finally {
      setWorkbookInspectionCancellationPending(false);
    }
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

  function invalidateSelectionRequests() {
    inspectionRequestRef.current += 1;
    loadRequestRef.current += 1;
    headerPreviewRequestRef.current += 1;
    schemaPreviewRequestRef.current += 1;
    schemaPreviewActiveRequestRef.current = null;
    schemaPreviewInFlightRef.current = false;
  }

  async function cancelAndDiscardSelection(
    source: DatasetSourceInspection,
    pending: { cancelPending: boolean; discardPending: boolean } = {
      cancelPending: true,
      discardPending: true,
    },
  ) {
    if (selectionCancellationInFlightRef.current) return;
    selectionCancellationInFlightRef.current = true;
    inspectionInFlightRef.current = true;
    invalidateSelectionRequests();
    setLoadInspection({ kind: "selection_cancelling", source });

    let cancelPending = pending.cancelPending;
    let discardPending = pending.discardPending;
    const errors: string[] = [];
    if (cancelPending) {
      try {
        await cancelOperation("load");
        cancelPending = false;
      } catch (error: unknown) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    }
    if (discardPending) {
      try {
        await discardDatasetSelection(source.selectionId);
        discardPending = false;
      } catch (error: unknown) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    }

    if (cancelPending || discardPending) {
      setLoadInspection({
        kind: "selection_cancellation_failed",
        source,
        message: errors.join(" · "),
        cancelPending,
        discardPending,
      });
    } else {
      setLoadInspection({ kind: "idle" });
      inspectionInFlightRef.current = false;
    }
    selectionCancellationInFlightRef.current = false;
  }

  async function retrySelectionCancellation() {
    if (loadInspection.kind !== "selection_cancellation_failed") return;
    await cancelAndDiscardSelection(loadInspection.source, {
      cancelPending: loadInspection.cancelPending,
      discardPending: loadInspection.discardPending,
    });
  }

  async function cancelPendingSelection() {
    if (selectionCancellationInFlightRef.current) return;
    const source = loadInspection.kind === "sheet" || loadInspection.kind === "profile_review" || loadInspection.kind === "resource_preflight" || loadInspection.kind === "schema_mismatch"
      ? loadInspection.source
      : undefined;
    if (source) {
      await cancelAndDiscardSelection(source);
      return;
    }
    invalidateSelectionRequests();
    setLoadInspection({ kind: "idle" });
    inspectionInFlightRef.current = false;
  }

  function handleSheetSelection(action: SheetSelectionAction) {
    if (action.kind === "cancelled") {
      void cancelPendingSelection();
      return;
    }
    if (action.kind === "confirmed") {
      if (loadInspection.kind === "sheet") {
        const requiresDelimitedPreview = loadInspection.source.format === "csv" || loadInspection.source.format === "tsv";
        if (requiresDelimitedPreview && !loadInspection.headerReview) return;
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
        if (!loadInspection.schemaPreview) {
          void previewSelectedSchema(loadInspection, profileWithConventions, conventions);
          return;
        }
        const schemaMismatch = loadInspection.schemaPreview.schemaMismatch !== null;
        const profileToApply = schemaMismatch ? null : profileWithConventions;
        void loadSelection(
          loadInspection.source,
          loadInspection.source.format === "excel" ? loadInspection.selectedSheetId : null,
          loadInspection.source.format === "excel" || loadInspection.source.format === "csv" || loadInspection.source.format === "tsv"
            ? loadInspection.headerMode
            : null,
          profileToApply,
          profileToApply,
          queuedReusableTask?.task ?? null,
          schemaMismatch,
          conventions,
        );
      }
      return;
    }
    schemaPreviewRequestRef.current += 1;
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

  async function cancelDatasetLoad() {
    setDatasetStatus(requestDatasetLoadCancellation);
    try {
      await cancelOperation("load");
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus((current) => recoverDatasetLoadCancellationFailure(current, message));
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
        qualityRules: delivery.qualityRules,
        outputFormat: reusableOutputFormat(delivery.exportFormat),
        privacyMode: delivery.privacyMode,
      }
    : null;
  const operationBusy = coreOperationBusy || loadSelectionBusy || projects.isBusy;
  operationBusyRef.current = operationBusy;
  const navigationBusy = foregroundOperationBusy || loadSelectionBusy || projects.operation.kind === "working";
  // Names only (no data): lets native probes tell which operation keeps the app busy.
  const busyReasons = [
    datasetStatus.kind === "loading" && "dataset",
    datasetStatus.kind === "ready" && datasetStatus.pageLoading && "page",
    profileStatus.kind === "loading" && "profile",
    prepare.changeStatus.kind === "working" && "prepare",
    delivery.contract.gate.kind === "loading" && "quality-gate",
    delivery.exportStatus.kind === "loading" && "export",
    review.comparison.status.kind === "loading" && "comparison",
    review.comparison.joinStatus.kind === "loading" && "join",
    (review.comparison.mutationStatus?.kind === "running" || review.comparison.mutationStatus?.kind === "finalizing") && "review-mutation",
    loadSelectionBusy && `selection:${loadInspection.kind}${selectionFinalizing ? "+finalizing" : ""}`,
    projects.operation.kind === "working" && "project",
    projects.autoSave.kind === "saving" && "autosave",
  ].filter(Boolean).join(",");
  const activePhaseIndex = Math.max(0, workflowPhases.findIndex((phase) => phase.id === activePhase));
  const activePhaseMeta = workflowPhases[activePhaseIndex];
  const previousPhase = workflowPhases[activePhaseIndex - 1];
  const nextPhase = workflowPhases[activePhaseIndex + 1];
  const profileGatedPhase = activePhase === "review" || activePhase === "prepare";
  const primaryNextLabel = profileGatedPhase && profileStatus.kind !== "ready"
    ? profileStatus.kind === "error" || profileStatus.kind === "cancelled"
      ? "Reintentar análisis"
      : profileStatus.kind === "loading"
        ? "Analizando calidad…"
        : "Analizar calidad"
    : activePhase === "review"
      ? "Ver cambios propuestos"
      : activePhase === "prepare"
        ? "Continuar a Entregar"
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
        void review.analyzeQuality();
      }
      return;
    }
    setActivePhase(nextPhase.id);
  }

  function isPhaseComplete(phase: WorkflowPhase): boolean {
    switch (phase) {
      case "load":
      case "review":
        return completedPhaseRevisions[phase] === datasetRevision;
      case "prepare":
        return prepare.changeStatus.kind === "applied";
      case "deliver":
        return delivery.exportStatus.kind === "success";
    }
  }

  function applyReusableTask(task: ReusableTask, preserveActiveImportProfile = false) {
    setQueuedReusableTask(null);
    setReusableTaskApplicationReview(null);
    if (!preserveActiveImportProfile) setActiveImportProfile(task.importProfile);
    setRecipeDraft(task.recipe);
    setActiveExceptionPolicy(task.exceptionPolicy ?? null);
    delivery.applySettings({
      qualityRules: task.qualityRules,
      exportFormat: task.outputFormat,
      privacyMode: task.privacyMode,
    });
    setRecipeSession((current) => current + 1);
    setCompletedPhaseRevisions({ load: datasetRevisionRef.current });
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
    && review.reviewTab === "diagnosis"
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
                aria-description={phaseComplete ? "Completada" : undefined}
                className={`side-nav__item side-nav__item--${phaseState}${activePhase === phase.id ? " side-nav__active" : ""}`}
                aria-current={activePhase === phase.id ? "step" : undefined}
                aria-disabled={!available || undefined}
                aria-describedby={!available ? "dataset-required-hint" : undefined}
                onMouseEnter={() => preloadPhase(phase.id)}
                onFocus={() => preloadPhase(phase.id)}
                onClick={() => available && setActivePhase(phase.id)}
                disabled={navigationBusy}
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
            {/* The theme is what people change most; the resource monitor is technical detail. */}
            <ThemeSwitcher />
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
            {/* Builds without an updater have nothing the user can do here. */}
            {status.kind === "ready" && status.info?.updaterConfigured === true && (
              <UpdatePanel enabled currentVersion={status.info.version} />
            )}
            <button
              type="button"
              className="sidebar__diagnostics-trigger"
              onClick={() => {
                setSidebarUtilitiesOpen(false);
                setDiagnosticsOpen(true);
              }}
            >
              Crear informe de diagnóstico
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
        {/* The side navigation already marks the current step; the top bar only reports the engine. */}
        <header className="topbar">
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
          data-busy-reasons={busyReasons || undefined}
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
                onCancelLoad={() => void cancelDatasetLoad()}
                workbookInspectionCancellationPending={workbookInspectionCancellationPending}
                onCancelWorkbookInspection={() => void cancelWorkbookInspection()}
                onRetrySelectionCancellation={() => void retrySelectionCancellation()}
              >
                <ProjectsPanel
                  catalog={projects.catalog}
                  catalogCancellationPending={projects.catalogCancellationPending}
                  onCancelCatalogLoad={() => void projects.cancelCatalogLoad()}
                  operation={projects.operation}
                  deletion={projects.deletion}
                  activeProject={projects.activeProject}
                  versions={projects.versions}
                  versionsCancellationPending={projects.versionsCancellationPending}
                  autoSave={projects.autoSave}
                  autoSaveEnabled={projects.autoSaveEnabled}
                  datasetFileName={activeDataset?.dataset.fileName ?? null}
                  disabled={operationBusy}
                  saveCancellationPending={projects.saveCancellationPending}
                  autoSaveCancellationPending={projects.autoSaveCancellationPending}
                  deleteCancellationPending={projects.deleteCancellationPending}
                  openCancellationPending={projects.openCancellationPending}
                  restoreCancellationPending={projects.restoreCancellationPending}
                  onSave={(name) => void projects.save(name)}
                  onCancelSave={() => void projects.cancelSave()}
                  onOpen={(projectId) => void projects.open(projectId)}
                  onCancelOpen={() => void projects.cancelOpen()}
                  onRestore={(projectId, versionId) => void projects.restore(projectId, versionId)}
                  onCancelRestore={() => void projects.cancelRestore()}
                  onCancelAutoSave={() => void projects.cancelAutoSave()}
                  onAutoSaveChange={projects.setAutoSaveEnabled}
                  onDeleteRequest={projects.requestDelete}
                  onDeleteCancel={projects.cancelDelete}
                  onDeleteConfirm={() => void projects.confirmDelete()}
                  onCancelDeleteOperation={() => void projects.cancelProjectDelete()}
                  onCancelVersionsLoad={() => void projects.cancelVersionsLoad()}
                  onRetryVersions={() => {
                    if (projects.activeProject) void projects.refreshVersions(projects.activeProject.id);
                  }}
                  onRetry={() => void projects.refresh()}
                  onClearFeedback={projects.clearFeedback}
                />
              </LoadPhase>
            )}

            {activePhase === "review" && readyDataset && (
              <ReviewPhase
                datasetStatus={readyDataset}
                profileStatus={profileStatus}
                reviewTab={review.reviewTab}
                onTabChange={review.setReviewTab}
                onPageChange={changePage}
                onCancelPageChange={() => void cancelPageChange()}
                pageCancellationPending={pageCancellationPending}
                onCancelProfile={() => void review.cancelProfile()}
                onContinueToPrepare={(target) => {
                  setPrepareFocusTarget(target ?? null);
                  setCompletedPhaseRevisions((current) => ({
                    ...current,
                    review: datasetRevisionRef.current,
                  }));
                  setActivePhase("prepare");
                }}
                comparison={review.comparison}
                sqlHistory={review.sqlHistory}
                onSqlHistoryChange={review.setSqlHistory}
                datasetRevision={datasetRevision}
                queryEngine={review.queryEngine}
                onQueryEngineChange={review.setQueryEngine}
                analysisSampleRows={review.analysisSampleRows}
                onAnalysisSampleRowsChange={review.setAnalysisSampleRows}
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
                qualityRules={delivery.qualityRules}
                recipeDraft={recipeDraft}
                recipeSession={recipeSession}
                onCancelProfile={() => void review.cancelProfile()}
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
                personalDataColumns={personalDataColumnNames(
                  profileStatus.kind === "ready" ? profileStatus.profile.columns : null,
                )}
                suggestedRules={profileStatus.kind === "ready"
                  ? suggestQualityRules(profileStatus.profile.columns, profileStatus.profile.rowCount)
                  : []}
                recipeDraft={recipeDraft}
                preparationChanges={prepare.historyStatus.entries
                  .filter((entry) => entry.index > 0 && entry.index <= prepare.historyStatus.currentIndex)
                  .map((entry) => entry.label)}
                contract={delivery.contract}
                exportState={delivery.exportStatus}
                exportFormat={delivery.exportFormat}
                onExportFormatChange={delivery.setExportFormat}
                privacyMode={delivery.privacyMode}
                onPrivacyModeChange={delivery.setPrivacyMode}
                onContractAction={delivery.updateContract}
                onExport={delivery.exportActiveDataset}
                onCancelExport={() => void delivery.cancelExport()}
              />
            )}
          </Suspense>
          </div>
          {(previousPhase || activeDataset) && (
          <footer className={`flow-footer${nextPhase ? "" : " flow-footer--terminal"}`} aria-label="Navegación entre etapas">
            {nextPhase && activeDataset && !reviewHasContextualContinue && (
              <div className="flow-footer__copy">
                <p className="step">Siguiente paso</p>
                <strong>{nextPhase.label}</strong>
                {/* Only when it tells the user something the button does not. */}
                {profileGatedPhase && profileStatus.kind !== "ready" && (
                  <p>{primaryNextDescription}</p>
                )}
              </div>
            )}
            <div className="flow-footer__actions">
              {previousPhase && (
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => setActivePhase(previousPhase.id)}
                  disabled={navigationBusy}
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
                  disabled={!activeDataset || navigationBusy || (profileGatedPhase && profileStatus.kind === "loading")}
                >
                  {primaryNextLabel}
                </button>
              )}
            </div>
          </footer>
          )}
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
