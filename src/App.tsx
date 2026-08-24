import { useEffect, useRef, useState } from "react";

import type { ReviewTab } from "./components/ReviewTabList";
import { DeliveryPhase } from "./features/delivery/DeliveryPhase";
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
import { PreparePhase } from "./features/prepare/PreparePhase";
import { usePrepareController } from "./features/prepare/usePrepareController";
import { ProjectsPanel } from "./features/projects/ProjectsPanel";
import { useProjectsController } from "./features/projects/useProjectsController";
import { ReviewPhase } from "./features/review/ReviewPhase";
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
  requestProfileCancellation,
  updateProfileProgress,
  type ProfileStatus,
} from "./features/review/reviewModel";
import { ResourceMonitor } from "./components/ResourceMonitor";

import {
  cancelOperation,
  clearDatasetComparison,
  compareDataset,
  discardDatasetSelection,
  exportDataset,
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  joinDataset,
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
  type SavedRecipe,
  type SpreadsheetHeaderMode,
} from "./bridge";

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "browser" }
  | { kind: "error"; message: string };

const phases = [
  { id: "load", number: "01", label: "Cargar", description: "Elegir una fuente local" },
  { id: "review", number: "02", label: "Revisar", description: "Entender señales y calidad" },
  { id: "prepare", number: "03", label: "Preparar", description: "Corregir y transformar" },
  { id: "deliver", number: "04", label: "Entregar", description: "Validar y exportar" },
] as const;

type ActivePhase = (typeof phases)[number]["id"];

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function isCancellationError(error: unknown): boolean {
  return String(error).includes("cancelada por el usuario");
}

export function App() {
  const [status, setStatus] = useState<AppStatus>({ kind: "loading" });
  const [datasetStatus, setDatasetStatus] = useState<DatasetStatus>({ kind: "empty" });
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });
  const [comparisonStatus, setComparisonStatus] = useState<ComparisonStatus>({ kind: "idle" });
  const [comparisonKeyColumns, setComparisonKeyColumns] = useState<string[]>([]);
  const [joinStatus, setJoinStatus] = useState<JoinStatus>({ kind: "idle" });
  const [joinType, setJoinType] = useState<DatasetJoinType>("inner");
  const [exportStatus, setExportStatus] = useState<DeliveryExportState>({ kind: "idle" });
  const [deliveryContract, setDeliveryContract] = useState<DeliveryContractState>(INITIAL_DELIVERY_CONTRACT);
  const [activePhase, setActivePhase] = useState<ActivePhase>("load");
  const [reviewTab, setReviewTab] = useState<ReviewTab>("diagnosis");
  const [loadInspection, setLoadInspection] = useState<LoadInspectionState>({ kind: "idle" });
  const [recipeDraft, setRecipeDraft] = useState<SavedRecipe | null>(null);
  const [recipeSession, setRecipeSession] = useState(0);
  const prepare = usePrepareController({
    activeDataset: datasetStatus.kind === "ready" ? datasetStatus.dataset : null,
    onDatasetChanged: (dataset) => {
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
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
    workspace: { qualityRules: deliveryRules(deliveryContract), recipeDraft },
    onProjectOpened: async ({ dataset, workspace, profile }) => {
      setDatasetStatus(createReadyDatasetStatus(dataset));
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
      setRecipeSession((current) => current + 1);
      setReviewTab("diagnosis");
      setActivePhase("review");
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
    if (!isTauriRuntime()) {
      setStatus({ kind: "browser" });
      return;
    }

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
      setDatasetStatus(createReadyDatasetStatus(dataset));
      projects.unlinkActiveProject();
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

  async function selectDataset() {
    setActivePhase("load");
    setLoadInspection({ kind: "inspecting" });
    try {
      const source = await pickDatasetSource();
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
      });
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

  async function consolidateComparedDataset() {
    try {
      const dataset = await useConsolidatedDataset();
      setDatasetStatus(createReadyDatasetStatus(dataset));
      setComparisonStatus(clearComparison());
      setComparisonKeyColumns([]);
      setJoinStatus(clearJoin());
      setProfileStatus({ kind: "idle" });
      projects.unlinkActiveProject();
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
    } else {
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
      } else {
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
      const result = await exportDataset(request.format, rules, allowUnvalidated, (progress) => {
        setExportStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      }, request.privacyMode);
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
  const activePhaseMeta = phases.find((phase) => phase.id === activePhase) ?? phases[0];
  const loadRuntime: LoadRuntimeState = status.kind === "ready"
    ? { kind: "connected" }
    : status.kind === "browser"
      ? { kind: "browser" }
      : { kind: "unavailable" };

  return (
    <div className="shell">
      <a className="skip-link" href="#main-content">Saltar al contenido principal</a>
      <aside className="sidebar" aria-label="Navegación principal">
        <div className="brand">
          <p className="eyebrow">Estación local de datos</p>
          <h1 id="app-title">Columnia</h1>
        </div>

        <nav className="side-nav" aria-label="Flujo de preparación de datos">
          {phases.map((phase) => {
            const available = phase.id === "load" || Boolean(activeDataset);
            return (
              <button
                key={phase.id}
                type="button"
                aria-label={phase.label}
                className={activePhase === phase.id ? "side-nav__active" : undefined}
                aria-current={activePhase === phase.id ? "step" : undefined}
                onClick={() => setActivePhase(phase.id)}
                disabled={!available || operationBusy}
                title={!available ? "Carga un dataset para habilitar esta etapa" : undefined}
              >
                <span aria-hidden="true">{phase.number}</span>
                <span className="side-nav__copy">
                  <strong>{phase.label}</strong>
                  <small aria-hidden="true">{phase.description}</small>
                </span>
              </button>
            );
          })}
        </nav>

        <ResourceMonitor enabled={status.kind === "ready"} />

        <div className="sidebar__dataset">
          <span>Dataset activo</span>
          <strong>
            {activeDataset ? activeDataset.dataset.fileName : "Sin dataset"}
          </strong>
        </div>
      </aside>

      <main id="main-content" className="main-content" tabIndex={-1}>
        <header className="topbar">
          <div>
            <p className="step">Vista actual</p>
            <p className="page-title">{activePhaseMeta.label}</p>
          </div>
          <div
            className={`runtime runtime--${status.kind}`}
            role={status.kind === "error" ? "alert" : "status"}
            aria-live={status.kind === "error" ? "assertive" : "polite"}
            aria-atomic="true"
          >
            {status.kind === "loading" && "Conectando con Rust…"}
            {status.kind === "browser" && "Vista web · motor no conectado"}
            {status.kind === "ready" && `${status.info.version} · ${status.info.platform}`}
            {status.kind === "error" && `Error del motor: ${status.message}`}
          </div>
        </header>

        <section
          className={`workspace workspace--${activePhase}`}
          aria-label={`Etapa ${activePhaseMeta.label}`}
          aria-busy={operationBusy}
        >
          {activePhase === "load" && (
            <LoadPhase
              runtime={loadRuntime}
              datasetStatus={datasetStatus}
              inspection={loadInspection}
              onSelect={selectDataset}
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
              onJoin={(requestedJoinType) => void joinActiveDataset(requestedJoinType)}
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
              onRemoveEmptyRows={prepare.applyEmptyRowRemoval}
              onRemoveConstantColumns={prepare.applyConstantColumnRemoval}
              onRemoveEmptyColumns={prepare.applyEmptyColumnRemoval}
              onRemoveHighNullColumns={prepare.applyHighNullColumnRemoval}
              onNormalizeSentinels={prepare.applySentinelNormalization}
              onNormalizeBooleans={prepare.applyBooleanNormalization}
              onImputeMissingValues={prepare.applyMissingValueImputation}
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
              contract={deliveryContract}
              exportState={exportStatus}
              onContractAction={updateDeliveryContract}
              onExport={exportActiveDataset}
              onCancelExport={() => cancelActiveOperation("export")}
            />
          )}
        </section>
      </main>
    </div>
  );
}
