import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import type { ReviewTab } from "./components/ReviewTabList";
import {
  INITIAL_DELIVERY_CONTRACT,
  deliveryContractFromRules,
  deliveryRules,
  invalidateDeliveryContract,
  reduceDeliveryContract,
  type DeliveryContractAction,
  type DeliveryContractState,
  type DeliveryExportRequest,
  type DeliveryExportState,
} from "./features/delivery/deliveryModel";
import { LoadPhase, type LoadRuntimeState } from "./features/load/LoadPhase";
import {
  beginDatasetLoad,
  clearLoadInspectionError,
  createReadyDatasetStatus,
  requestDatasetLoadCancellation,
  restoreDatasetAfterLoadFailure,
  setLoadInspectionError,
  updateDatasetLoadProgress,
  updateSheetSelection,
  workbookInspection,
  type DatasetStatus,
  type LoadInspectionState,
  type SheetSelectionAction,
} from "./features/load/loadModel";
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
} from "./features/review/joinModel";
import {
  PAGE_SIZE,
  beginPageLoad,
  beginProfileAnalysis,
  completePageLoad,
  failPageLoad,
  normalizePageOffset,
  readAnalysisSampleRowsPreference,
  requestProfileCancellation,
  updateProfileProgress,
  type AnalysisSampleRows,
  type ProfileStatus,
} from "./features/review/reviewModel";
import { ResourceMonitor } from "./components/ResourceMonitor";
import { ThemeSwitcher } from "./components/ThemeSwitcher";
import { UpdatePanel } from "./components/UpdatePanel";

import {
  cancelOperation,
  clearDatasetComparison,
  compareDataset,
  discardDatasetSelection,
  exportDataset,
  getAppInfo,
  getDatasetConflictPage,
  getDatasetPage,
  getDatasetProfile,
  inspectDroppedDataset as inspectDroppedDatasetSource,
  inspectSampleDataset,
  joinDataset,
  listSampleDatasets,
  loadDatasetSelection,
  pickDatasetSource,
  resolveDatasetConflicts,
  useConsolidatedDataset,
  type AppInfo,
  type CancellableOperation,
  type DatasetJoinType,
  type DatasetPreview,
  type ConflictResolution,
  type DatasetSourceInspection,
  type OperationProgress,
  type SavedRecipe,
  type SampleDatasetDescriptor,
  type SqlQueryHistoryEntry,
  type SpreadsheetHeaderMode,
} from "./bridge";

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo | null }
  | { kind: "browser" }
  | { kind: "error"; message: string };

const phases = [
  { id: "load", number: "01", label: "Cargar", description: "Elegir una fuente local" },
  { id: "review", number: "02", label: "Revisar", description: "Entender señales y calidad" },
  { id: "prepare", number: "03", label: "Preparar", description: "Corregir y transformar" },
  { id: "deliver", number: "04", label: "Entregar", description: "Validar y exportar" },
] as const;

const CONFLICT_PAGE_SIZE = 50;

type ActivePhase = (typeof phases)[number]["id"];

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

function preloadPhase(phase: ActivePhase): void {
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

export function App() {
  const [status, setStatus] = useState<AppStatus>(initialAppStatus);
  const [datasetStatus, setDatasetStatus] = useState<DatasetStatus>({ kind: "empty" });
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });
  const [analysisSampleRows, setAnalysisSampleRows] = useState<AnalysisSampleRows>(readAnalysisSampleRowsPreference);
  const [comparisonStatus, setComparisonStatus] = useState<ComparisonStatus>({ kind: "idle" });
  const [comparisonKeyColumns, setComparisonKeyColumns] = useState<string[]>([]);
  const [joinStatus, setJoinStatus] = useState<JoinStatus>({ kind: "idle" });
  const [joinType, setJoinType] = useState<DatasetJoinType>("inner");
  const [exportStatus, setExportStatus] = useState<DeliveryExportState>({ kind: "idle" });
  const [deliveryContract, setDeliveryContract] = useState<DeliveryContractState>(INITIAL_DELIVERY_CONTRACT);
  const [activePhase, setActivePhase] = useState<ActivePhase>("load");
  const [reviewTab, setReviewTab] = useState<ReviewTab>("diagnosis");
  const [loadInspection, setLoadInspection] = useState<LoadInspectionState>({ kind: "idle" });
  const [recentDatasets, setRecentDatasets] = useState<RecentDataset[]>(readRecentDatasets);
  const [sampleDatasets, setSampleDatasets] = useState<SampleDatasetDescriptor[]>([]);
  const [recipeDraft, setRecipeDraft] = useState<SavedRecipe | null>(null);
  const [sqlHistory, setSqlHistory] = useState<SqlQueryHistoryEntry[]>([]);
  const [recipeSession, setRecipeSession] = useState(0);
  const [sidebarUtilitiesOpen, setSidebarUtilitiesOpen] = useState(false);
  const prepare = usePrepareController({
    activeDataset: datasetStatus.kind === "ready" ? datasetStatus.dataset : null,
    onDatasetChanged: (dataset) => {
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
      setSqlHistory([]);
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      void clearDatasetComparison().catch(() => undefined);
    },
    onProfileInvalidated: () => setProfileStatus({ kind: "idle" }),
    onDeliveryInvalidated: invalidateDeliveryGate,
  });
  const coreOperationBusy =
    datasetStatus.kind === "loading" ||
    profileStatus.kind === "loading" ||
    prepare.changeStatus.kind === "working" ||
    deliveryContract.gate.kind === "loading" ||
    exportStatus.kind === "loading" ||
    comparisonStatus.kind === "loading" ||
    joinStatus.kind === "loading";
  const projects = useProjectsController({
    connected: status.kind === "ready",
    blocked: coreOperationBusy,
    hasDataset: datasetStatus.kind === "ready",
    workspace: {
      qualityRules: deliveryRules(deliveryContract),
      recipeDraft,
      ...(sqlHistory.length > 0 ? { sqlHistory } : {}),
      reviewTab,
      previewOffset: datasetStatus.kind === "ready" ? datasetStatus.pageOffset : 0,
      activePhase,
    },
    onActiveProjectDeleted: () => setSqlHistory([]),
    onProjectOpened: async ({ dataset, workspace, profile }) => {
      const initialDataset = createReadyDatasetStatus(dataset);
      setDatasetStatus(initialDataset);
      const previewOffset = normalizePageOffset(workspace.previewOffset ?? 0, dataset.rowCount);
      if (previewOffset > 0) {
        try {
          const page = await getDatasetPage(previewOffset, PAGE_SIZE);
          setDatasetStatus(completePageLoad(initialDataset, page));
        } catch {
          // El snapshot sigue siendo válido; la muestra vuelve a su primera página.
        }
      }
      setLoadInspection({ kind: "idle" });
      setProfileStatus(profile ? { kind: "ready", profile } : { kind: "idle" });
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      setDeliveryContract(deliveryContractFromRules(workspace.qualityRules));
      setExportStatus({ kind: "idle" });
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      await clearDatasetComparison().catch(() => undefined);
      setRecipeDraft(workspace.recipeDraft);
      setSqlHistory(workspace.sqlHistory ?? []);
      setRecipeSession((current) => current + 1);
      setReviewTab(workspace.reviewTab ?? "diagnosis");
      setActivePhase(workspace.activePhase ?? "review");
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

  async function loadSelection(
    source: DatasetSourceInspection,
    sheetId: string | null,
    headerMode: SpreadsheetHeaderMode | null = null,
  ) {
    setDatasetStatus((current) => beginDatasetLoad(current));
    setLoadInspection(clearLoadInspectionError);
    try {
      const dataset = await loadDatasetSelection(source.selectionId, sheetId, headerMode, (progress) => {
        setDatasetStatus((current) => updateDatasetLoadProgress(current, progress));
      });
      setRecentDatasets((current) => rememberRecentDataset(current, {
        fileName: source.fileName,
        format: source.format,
      }));
      setDatasetStatus(createReadyDatasetStatus(dataset));
      projects.unlinkActiveProject();
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setRecipeSession((current) => current + 1);
      setLoadInspection({ kind: "idle" });
      setProfileStatus({ kind: "idle" });
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      await clearDatasetComparison().catch(() => undefined);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
      setExportStatus({ kind: "idle" });
      setReviewTab("diagnosis");
      setActivePhase("review");
    } catch (error: unknown) {
      setDatasetStatus(restoreDatasetAfterLoadFailure);
      if (isCancellationError(error)) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) => setLoadInspectionError(current, message));
    }
  }

  async function inspectDatasetSource(sourcePromise: Promise<DatasetSourceInspection | null>) {
    setActivePhase("load");
    setLoadInspection({ kind: "inspecting" });
    try {
      const source = await sourcePromise;
      if (!source) {
        setLoadInspection({ kind: "idle" });
        return;
      }
      if (source.format === "excel") {
        setLoadInspection(workbookInspection(source));
        return;
      }
      await loadSelection(source, source.sheets[0]?.id ?? null);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setLoadInspection((current) => setLoadInspectionError(current, message));
    } finally {
      setLoadInspection((current) => current.kind === "inspecting" ? { kind: "idle" } : current);
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

  async function cancelSheetSelection() {
    const source = loadInspection.kind === "sheet" ? loadInspection.source : undefined;
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
      void cancelSheetSelection();
      return;
    }
    if (action.kind === "confirmed") {
      if (loadInspection.kind === "sheet") {
        void loadSelection(
          loadInspection.source,
          loadInspection.selectedSheetId,
          loadInspection.headerMode,
        );
      }
      return;
    }
    setLoadInspection((current) => updateSheetSelection(current, action));
  }

  async function analyzeQuality() {
    setProfileStatus(beginProfileAnalysis());
    try {
      const profile = await getDatasetProfile((progress) => {
        setProfileStatus((current) => updateProfileProgress(current, progress));
      }, analysisSampleRows);
      setProfileStatus({ kind: "ready", profile });
    } catch (error: unknown) {
      if (isCancellationError(error)) {
        setProfileStatus({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setProfileStatus({ kind: "error", message });
    }
  }

  async function compareActiveDataset() {
    setComparisonStatus(beginComparison());
    try {
      const comparison = await compareDataset(comparisonKeyColumns);
      setComparisonStatus(comparison ? completeComparison(comparison) : clearComparison());
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function clearActiveComparison() {
    try {
      await clearDatasetComparison();
      setComparisonStatus(clearComparison());
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function changeConflictPage(offset: number) {
    if (comparisonStatus.kind !== "ready") return;
    try {
      const page = await getDatasetConflictPage(offset, CONFLICT_PAGE_SIZE);
      if (!page) return;
      setComparisonStatus(completeComparison({
        ...comparisonStatus.comparison,
        conflicts: page.conflicts,
        conflictOffset: page.offset,
        conflictsTruncated: page.hasNext,
      }));
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function consolidateComparedDataset() {
    try {
      const dataset = await useConsolidatedDataset();
      setDatasetStatus(createReadyDatasetStatus(dataset));
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function resolveComparedConflicts(decisions: ConflictResolution[]) {
    setComparisonStatus(beginComparison());
    try {
      const dataset = await resolveDatasetConflicts(decisions);
      setDatasetStatus(createReadyDatasetStatus(dataset));
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await prepare.refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setComparisonStatus(failComparison(message));
    }
  }

  async function joinActiveDataset(requestedJoinType: DatasetJoinType) {
    if (comparisonKeyColumns.length === 0) {
      setJoinStatus(failJoin("Selecciona al menos una columna clave para unir datasets."));
      return;
    }
    setJoinStatus(beginJoin(requestedJoinType));
    try {
      const dataset = await joinDataset(comparisonKeyColumns, requestedJoinType);
      if (!dataset) {
        setJoinStatus(clearJoin());
        return;
      }
      setDatasetStatus(createReadyDatasetStatus(dataset));
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
      setSqlHistory([]);
      setDeliveryContract(INITIAL_DELIVERY_CONTRACT);
      setRecipeDraft(null);
      setRecipeSession((current) => current + 1);
      prepare.resetChangeStatus();
      await clearDatasetComparison().catch(() => undefined);
      await prepare.refreshHistory();
      setReviewTab("diagnosis");
      setActivePhase("review");
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setJoinStatus(failJoin(message));
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
      }
    }
  }

  async function exportActiveDataset(request: DeliveryExportRequest) {
    if (datasetStatus.kind !== "ready") return;
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
        setExportStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      };
      const result = recipeDraft && request.format === "bundle"
        ? await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode, recipeDraft)
        : await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode);
      setExportStatus(result ? { kind: "success", result } : { kind: "idle" });
    } catch (error: unknown) {
      if (isCancellationError(error)) {
        setExportStatus({ kind: "idle" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setExportStatus({ kind: "error", message });
    }
  }

  async function changePage(offset: number) {
    if (datasetStatus.kind !== "ready") return;

    const previous = datasetStatus;
    setDatasetStatus(beginPageLoad(previous));

    try {
      const page = await getDatasetPage(offset, PAGE_SIZE);
      setDatasetStatus(completePageLoad(previous, page));
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus(failPageLoad(previous, message));
    }
  }

  const readyDataset = datasetStatus.kind === "ready" ? datasetStatus : undefined;
  const retainedDataset =
    datasetStatus.kind === "loading" ? datasetStatus.previous : undefined;
  const activeDataset = readyDataset ?? retainedDataset;
  const operationBusy = coreOperationBusy || projects.isBusy;
  const activePhaseIndex = Math.max(0, phases.findIndex((phase) => phase.id === activePhase));
  const activePhaseMeta = phases[activePhaseIndex];
  const previousPhase = phases[activePhaseIndex - 1];
  const nextPhase = phases[activePhaseIndex + 1];
  const progressValue = activePhaseIndex + 1;
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
          <h1 id="app-title">Columnia</h1>
        </div>

        <nav className="side-nav" aria-label="Flujo de preparación de datos">
          {phases.map((phase, phaseIndex) => {
            const available = phase.id === "load" || Boolean(activeDataset);
            const phaseState = phaseIndex < activePhaseIndex
              ? "complete"
              : phaseIndex === activePhaseIndex ? "current" : "upcoming";
            return (
              <button
                key={phase.id}
                type="button"
                aria-label={phase.label}
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
                  {phaseState === "complete" ? "✓" : phase.number}
                </span>
                <span className="side-nav__copy">
                  <span className="side-nav__label-row">
                    <strong>{phase.label}</strong>
                    <small className="side-nav__state" aria-hidden="true">
                      {phaseState === "complete" ? "Hecho" : phaseState === "current" ? "Ahora" : "Después"}
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

        <details
          className="sidebar__utilities"
          open={sidebarUtilitiesOpen}
          onToggle={(event) => setSidebarUtilitiesOpen(event.currentTarget.open)}
        >
          <summary>Preferencias y recursos</summary>
          <div className="sidebar__utilities-content">
            <ResourceMonitor enabled={status.kind === "ready"} />
            <ThemeSwitcher />
            <UpdatePanel
              enabled={status.kind === "ready" && status.info?.updaterConfigured === true}
              currentVersion={status.kind === "ready" && status.info ? status.info.version : null}
            />
          </div>
        </details>

        <details className="sidebar__legal">
          <summary>Licencia y privacidad</summary>
          <div className="sidebar__legal-content">
            <p><strong>Columnia</strong> funciona localmente y no envía datasets a servicios externos.</p>
            <p><strong>Versión:</strong> {status.kind === "ready" && status.info ? status.info.version : "no disponible"}</p>
            <h2>Licencia</h2>
            <p>El producto se distribuye bajo MIT. Las dependencias conservan sus avisos en <code>THIRD_PARTY_NOTICES.md</code>.</p>
            <h2>Privacidad local</h2>
            <p>Los archivos seleccionados, proyectos, perfiles y artefactos temporales permanecen en el dispositivo. No se usan telemetría, cuentas ni analítica remota.</p>
            <h2>Retención y borrado</h2>
            <p>El usuario controla la carpeta de proyectos y puede eliminar proyectos desde la aplicación o borrar sus archivos locales. Los snapshots huérfanos se limpian de forma oportunista después de una hora de gracia.</p>
            <p className="sidebar__legal-note">Responsable y canal de contacto: deben definirse para la jurisdicción de publicación antes de distribuir.</p>
          </div>
        </details>
      </aside>

      <main id="main-content" className="main-content" tabIndex={-1}>
        <header className="topbar">
          <div className="flow-overview">
            <div className="flow-overview__copy">
              <p className="flow-overview__step">Paso {progressValue} de {phases.length}</p>
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
              aria-valuemax={phases.length}
              aria-valuenow={progressValue}
              aria-valuetext={`Paso ${progressValue} de ${phases.length}: ${activePhaseMeta.label}`}
            >
              <span style={{ width: `${(progressValue / phases.length) * 100}%` }} />
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
            {status.kind === "ready" && status.info
              ? `${status.info.version} · ${status.info.platform}`
              : status.kind === "ready"
                ? "Motor local listo"
                : null}
            {status.kind === "error" && `Error del motor: ${status.message}`}
          </div>
        </header>

        <section
          className={`workspace workspace--${activePhase}`}
          aria-label={`Etapa ${activePhaseMeta.label}`}
          aria-busy={operationBusy}
        >
          <Suspense fallback={<div className="phase-loading" role="status">Cargando etapa…</div>}>
            {activePhase === "load" && (
              <LoadPhase
                runtime={loadRuntime}
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
                onCancelLoad={() => cancelActiveOperation("load")}
              >
                <ProjectsPanel
                  catalog={projects.catalog}
                  operation={projects.operation}
                  deletion={projects.deletion}
                  activeProject={projects.activeProject}
                  datasetFileName={activeDataset?.dataset.fileName ?? null}
                  disabled={operationBusy}
                  onSave={(name) => void projects.save(name)}
                  onOpen={(projectId) => void projects.open(projectId)}
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
                onAnalyzeQuality={analyzeQuality}
                onCancelProfile={() => cancelActiveOperation("profile")}
                comparisonStatus={comparisonStatus}
                comparisonKeyColumns={comparisonKeyColumns}
                onComparisonKeyColumnsChange={setComparisonKeyColumns}
                datasetColumns={readyDataset.dataset.columns}
                joinStatus={joinStatus}
                joinType={joinType}
                onJoinTypeChange={setJoinType}
                onCompare={() => void compareActiveDataset()}
                onClearComparison={() => void clearActiveComparison()}
                onConsolidate={() => void consolidateComparedDataset()}
                onResolveConflicts={(decisions) => void resolveComparedConflicts(decisions)}
                onConflictPageChange={(offset) => void changeConflictPage(offset)}
                onJoin={(requestedJoinType) => void joinActiveDataset(requestedJoinType)}
                sqlHistory={sqlHistory}
                onSqlHistoryChange={setSqlHistory}
                analysisSampleRows={analysisSampleRows}
                onAnalysisSampleRowsChange={setAnalysisSampleRows}
              />
            )}

            {activePhase === "prepare" && readyDataset && (
              <PreparePhase
                dataset={readyDataset.dataset}
                profileStatus={profileStatus}
                changeStatus={prepare.changeStatus}
                historyStatus={prepare.historyStatus}
                recipeDraft={recipeDraft}
                recipeSession={recipeSession}
                onAnalyzeQuality={analyzeQuality}
                onCancelProfile={() => cancelActiveOperation("profile")}
                onRemoveDuplicates={prepare.applyDuplicateRemoval}
                onRemoveNearDuplicates={prepare.applyNearDuplicateRemoval}
                onRemoveEmptyRows={prepare.applyEmptyRowRemoval}
                onRemoveConstantColumns={prepare.applyConstantColumnRemoval}
                onRemoveEmptyColumns={prepare.applyEmptyColumnRemoval}
                onRemoveHighNullColumns={prepare.applyHighNullColumnRemoval}
                onRemoveIdentifierColumns={prepare.applyIdentifierColumnRemoval}
                onRemovePersonalColumns={prepare.applyPersonalColumnRemoval}
                onMaskPersonalValues={prepare.applyPersonalValueMasking}
                onNormalizeSentinels={prepare.applySentinelNormalization}
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
                onRecipeDraftChange={setRecipeDraft}
                onUndo={prepare.undoChange}
                onRedo={prepare.redoChange}
              />
            )}

            {activePhase === "deliver" && readyDataset && (
              <DeliveryPhase
                dataset={readyDataset.dataset}
                recipeDraft={recipeDraft}
                contract={deliveryContract}
                exportState={exportStatus}
                onContractAction={updateDeliveryContract}
                onExport={exportActiveDataset}
                onCancelExport={() => cancelActiveOperation("export")}
              />
            )}
          </Suspense>
          <footer className="flow-footer" aria-label="Navegación entre etapas">
            <div className="flow-footer__copy">
              <p className="step">{nextPhase ? "Siguiente paso" : "Última etapa"}</p>
              <strong>{nextPhase ? nextPhase.label : "Completa la entrega"}</strong>
              <p>
                {nextPhase
                  ? activeDataset
                    ? nextPhase.description
                    : "Carga un dataset para continuar con la revisión."
                  : "Elige una ruta de validación y exporta cuando todo esté listo."}
              </p>
            </div>
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
              {nextPhase && (
                <button
                  type="button"
                  className="primary-action"
                  onMouseEnter={() => preloadPhase(nextPhase.id)}
                  onFocus={() => preloadPhase(nextPhase.id)}
                  onClick={() => setActivePhase(nextPhase.id)}
                  disabled={!activeDataset || operationBusy}
                >
                  Continuar a {nextPhase.label}
                </button>
              )}
            </div>
          </footer>
        </section>
      </main>
    </div>
  );
}
