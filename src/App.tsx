import { useEffect, useRef, useState } from "react";

import {
  applySafeCorrections,
  applyTransformRecipe,
  cancelOperation,
  discardDatasetSelection,
  exportDataset,
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  getHistoryState,
  normalizeColumnNames,
  normalizeTextValues,
  loadDatasetSelection,
  pickDatasetSource,
  pickTransformRecipe,
  removeDuplicates,
  redoLastChange,
  saveTransformRecipe,
  trimTextValues,
  undoLastChange,
  type AppInfo,
  type CancellableOperation,
  type DatasetPreview,
  type DatasetProfile,
  type DatasetSourceInspection,
  type ExportFormat,
  type ExportResult,
  type OperationProgress,
  type HistoryState,
  type LoadedRecipe,
  type SpreadsheetHeaderMode,
  type TransformRecipe,
} from "./bridge";

function isLoadedRecipe(value: unknown): value is LoadedRecipe {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<LoadedRecipe>;
  const recipe = candidate.recipe as Partial<TransformRecipe> | undefined;
  return candidate.version === 1 && typeof candidate.name === "string" &&
    typeof candidate.savedAt === "string" && !!recipe &&
    Array.isArray(recipe.renames) && Array.isArray(recipe.casts) &&
    Array.isArray(recipe.dateParses) && Array.isArray(recipe.filters) &&
    Array.isArray(recipe.outlierTreatments) && Array.isArray(recipe.contactNormalizations) &&
    Array.isArray(recipe.textExtractions) && "calculatedColumn" in recipe &&
    "findReplace" in recipe && "keepColumns" in recipe && "splitColumn" in recipe &&
    "mergeColumns" in recipe && "groupSummary" in recipe;
}

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "browser" }
  | { kind: "error"; message: string };

type ReadyDatasetStatus = {
  kind: "ready";
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
};

type DatasetStatus =
  | { kind: "empty" }
  | {
      kind: "loading";
      progress: OperationProgress;
      cancelRequested: boolean;
      previous?: ReadyDatasetStatus;
    }
  | ReadyDatasetStatus
  | { kind: "error"; message: string };

const PAGE_SIZE = 50;

type ProfileStatus =
  | { kind: "idle" }
  | { kind: "loading"; progress: OperationProgress; cancelRequested: boolean }
  | { kind: "ready"; profile: DatasetProfile }
  | { kind: "error"; message: string };

type ChangeStatus =
  | { kind: "idle" }
  | { kind: "working"; action: "safe" | "duplicates" | "columns" | "trim" | "text" | "transform" | "undo" | "redo" }
  | { kind: "applied"; message: string }
  | { kind: "error"; message: string };

const EMPTY_HISTORY: HistoryState = {
  canUndo: false, canRedo: false, currentIndex: 0, entryCount: 0, entries: [],
  snapshotsEnabled: true, degradedReason: null, maxEntries: 0, diskBytes: 0, diskBudgetBytes: 0,
};

type ExportStatus =
  | { kind: "idle" }
  | {
      kind: "loading";
      format: ExportFormat;
      progress: OperationProgress;
      cancelRequested: boolean;
    }
  | { kind: "success"; result: ExportResult }
  | { kind: "error"; message: string };

const phases = [
  { id: "load", number: "01", label: "Cargar", description: "Elegir una fuente local" },
  { id: "review", number: "02", label: "Revisar", description: "Entender señales y calidad" },
  { id: "prepare", number: "03", label: "Preparar", description: "Corregir y transformar" },
  { id: "deliver", number: "04", label: "Entregar", description: "Validar y exportar" },
] as const;

type ActivePhase = (typeof phases)[number]["id"];
type ReviewTab = "diagnosis" | "preview";

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function readableFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function isCancellationError(error: unknown): boolean {
  return String(error).includes("cancelada por el usuario");
}

export function App() {
  const [status, setStatus] = useState<AppStatus>({ kind: "loading" });
  const [datasetStatus, setDatasetStatus] = useState<DatasetStatus>({ kind: "empty" });
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });
  const [changeStatus, setChangeStatus] = useState<ChangeStatus>({ kind: "idle" });
  const [historyStatus, setHistoryStatus] = useState<HistoryState>(EMPTY_HISTORY);
  const [exportStatus, setExportStatus] = useState<ExportStatus>({ kind: "idle" });
  const [activePhase, setActivePhase] = useState<ActivePhase>("load");
  const [reviewTab, setReviewTab] = useState<ReviewTab>("diagnosis");
  const [sheetSelection, setSheetSelection] = useState<DatasetSourceInspection | null>(null);
  const [selectedSheetId, setSelectedSheetId] = useState("");
  const [spreadsheetHeaderMode, setSpreadsheetHeaderMode] = useState<SpreadsheetHeaderMode>("firstRow");
  const [importError, setImportError] = useState<string | null>(null);
  const [importInspecting, setImportInspecting] = useState(false);

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

  async function refreshHistory() {
    try {
      const history = await getHistoryState();
      setHistoryStatus(history);
      return history;
    } catch {
      return historyStatus;
    }
  }

  async function loadSelection(
    source: DatasetSourceInspection,
    sheetId: string | null,
    headerMode: SpreadsheetHeaderMode | null = null,
  ) {
    const previous = datasetStatus.kind === "ready" ? datasetStatus : undefined;
    setDatasetStatus({
      kind: "loading",
      progress: { operation: "load", stage: "Preparando carga", percent: 0 },
      cancelRequested: false,
      previous,
    });
    setImportError(null);
    try {
      const dataset = await loadDatasetSelection(source.selectionId, sheetId, headerMode, (progress) => {
        setDatasetStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      });
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
      setSheetSelection(null);
      setProfileStatus({ kind: "idle" });
      setChangeStatus({ kind: "idle" });
      await refreshHistory();
      setExportStatus({ kind: "idle" });
      setReviewTab("diagnosis");
      setActivePhase("review");
    } catch (error: unknown) {
      setDatasetStatus(previous ?? { kind: "empty" });
      if (isCancellationError(error)) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setImportError(message);
    }
  }

  async function selectDataset() {
    setImportError(null);
    setActivePhase("load");
    setImportInspecting(true);
    try {
      const source = await pickDatasetSource();
      if (!source) return;
      if (source.format === "excel") {
        setSheetSelection(source);
        setSelectedSheetId(source.defaultSheetId ?? source.sheets[0]?.id ?? "");
        setSpreadsheetHeaderMode("firstRow");
        return;
      }
      await loadSelection(source, source.sheets[0]?.id ?? null);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setImportError(message);
    } finally {
      setImportInspecting(false);
    }
  }

  async function cancelSheetSelection() {
    const source = sheetSelection;
    setSheetSelection(null);
    if (source) {
      try {
        await discardDatasetSelection(source.selectionId);
      } catch (error: unknown) {
        setImportError(error instanceof Error ? error.message : String(error));
      }
    }
  }

  async function applyDuplicateRemoval() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "duplicates" });
    try {
      const result = await removeDuplicates();
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      setChangeStatus({
        kind: "applied",
        message: `Se eliminaron ${result.affectedRowCount.toLocaleString()} filas duplicadas adicionales.`,
      });
      await refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function applyColumnNormalization() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "columns" });
    try {
      const result = await normalizeColumnNames();
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      setChangeStatus({
        kind: "applied",
        message:
          result.renamedColumnCount === 0
            ? "Los nombres de las columnas ya estaban normalizados."
            : result.renamedColumnCount === 1
              ? "Se normalizó 1 nombre de columna."
              : `Se normalizaron ${result.renamedColumnCount.toLocaleString()} nombres de columnas.`,
      });
      await refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function applyTextChange(
    action: "trim" | "text",
    operation: () => ReturnType<typeof trimTextValues>,
  ) {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action });
    try {
      const result = await operation();
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      const cells =
        result.changedCellCount === 1
          ? "1 celda"
          : `${result.changedCellCount.toLocaleString()} celdas`;
      const rows =
        result.affectedRowCount === 1
          ? "1 fila"
          : `${result.affectedRowCount.toLocaleString()} filas`;
      const detail = `${cells} en ${rows}`;
      setChangeStatus({
        kind: "applied",
        message:
          result.changedCellCount === 0
            ? "No se encontraron valores que necesitaran esta corrección."
            : action === "trim"
              ? `Se recortaron espacios en ${detail}.`
              : `Se normalizó texto en ${detail}.`,
      });
      await refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function applyRecommendedCorrections() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "safe" });
    try {
      const result = await applySafeCorrections();
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      const changed = result.changedCellCount > 0 || result.renamedColumnCount > 0;
      const changedCells =
        result.changedCellCount === 1
          ? "1 celda recortada"
          : `${result.changedCellCount.toLocaleString()} celdas recortadas`;
      const renamedColumns =
        result.renamedColumnCount === 1
          ? "1 columna renombrada"
          : `${result.renamedColumnCount.toLocaleString()} columnas renombradas`;
      setChangeStatus({
        kind: "applied",
        message: changed
          ? `Correcciones recomendadas aplicadas: ${changedCells} y ${renamedColumns}.`
          : "El dataset ya cumplía las correcciones recomendadas.",
      });
      await refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function applyStructuralTransforms(recipe: TransformRecipe) {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "transform" });
    try {
      const result = await applyTransformRecipe(recipe);
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      const total =
        result.renamedColumnCount +
        result.convertedColumnCount +
        result.parsedDateColumnCount +
        result.removedRowCount +
        result.calculatedColumnCount +
        result.replacedCellCount +
        result.droppedColumnCount +
        result.splitColumnCount +
        result.mergedColumnCount +
        result.droppedSourceColumnCount +
        result.adjustedOutlierCellCount +
        result.outlierRemovedRowCount +
        result.collapsedRowCount +
        result.aggregatedColumnCount +
        result.normalizedContactCellCount +
        result.extractedColumnCount;
      setChangeStatus({
        kind: "applied",
        message:
          total === 0
            ? "La receta no produjo cambios en el dataset."
            : `Receta aplicada: ${result.renamedColumnCount.toLocaleString()} renombres, ${result.convertedColumnCount.toLocaleString()} conversiones, ${result.parsedDateColumnCount.toLocaleString()} fechas interpretadas, ${result.removedRowCount.toLocaleString()} filas filtradas, ${result.outlierRemovedRowCount.toLocaleString()} filas atípicas eliminadas, ${result.calculatedColumnCount.toLocaleString()} columnas calculadas, ${result.replacedCellCount.toLocaleString()} celdas reemplazadas, ${result.splitColumnCount.toLocaleString()} columnas divididas, ${result.mergedColumnCount.toLocaleString()} columnas combinadas, ${(result.droppedColumnCount + result.droppedSourceColumnCount).toLocaleString()} columnas descartadas, ${result.adjustedOutlierCellCount.toLocaleString()} outliers ajustados, ${result.normalizedContactCellCount.toLocaleString()} contactos normalizados en ${result.normalizedContactColumnCount.toLocaleString()} columnas, ${result.extractedColumnCount.toLocaleString()} columnas extraídas y resumen de ${result.groupCount.toLocaleString()} grupos con ${result.aggregatedColumnCount.toLocaleString()} agregaciones.`,
      });
      await refreshHistory();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function undoChange() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "undo" });
    try {
      const result = await undoLastChange();
      setDatasetStatus({ kind: "ready", dataset: result.dataset, pageOffset: 0, pageLoading: false });
      setProfileStatus({ kind: "idle" });
      setHistoryStatus(result.history);
      setChangeStatus({ kind: "applied", message: result.message });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function redoChange() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "redo" });
    try {
      const result = await redoLastChange();
      setDatasetStatus({ kind: "ready", dataset: result.dataset, pageOffset: 0, pageLoading: false });
      setProfileStatus({ kind: "idle" });
      setHistoryStatus(result.history);
      setChangeStatus({ kind: "applied", message: result.message });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message });
    }
  }

  async function analyzeQuality() {
    setProfileStatus({
      kind: "loading",
      progress: { operation: "profile", stage: "Iniciando análisis", percent: 0 },
      cancelRequested: false,
    });
    try {
      const profile = await getDatasetProfile((progress) => {
        setProfileStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
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

  async function cancelActiveOperation(operation: CancellableOperation) {
    if (operation === "load") {
      setDatasetStatus((current) =>
        current.kind === "loading" ? { ...current, cancelRequested: true } : current,
      );
    } else if (operation === "profile") {
      setProfileStatus((current) =>
        current.kind === "loading" ? { ...current, cancelRequested: true } : current,
      );
    } else {
      setExportStatus((current) =>
        current.kind === "loading" ? { ...current, cancelRequested: true } : current,
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

  async function exportActiveDataset(format: ExportFormat) {
    if (datasetStatus.kind !== "ready") return;
    setExportStatus({
      kind: "loading",
      format,
      progress: { operation: "export", stage: "Esperando destino", percent: 0 },
      cancelRequested: false,
    });
    try {
      const result = await exportDataset(format, (progress) => {
        setExportStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      });
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
    setDatasetStatus({ ...previous, pageLoading: true, pageError: undefined });

    try {
      const page = await getDatasetPage(offset, PAGE_SIZE);
      setDatasetStatus({
        kind: "ready",
        dataset: { ...previous.dataset, rows: page.rows },
        pageOffset: page.offset,
        pageLoading: false,
      });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus({ ...previous, pageLoading: false, pageError: message });
    }
  }

  const isDesktopReady = status.kind === "ready";
  const readyDataset = datasetStatus.kind === "ready" ? datasetStatus : undefined;
  const retainedDataset =
    datasetStatus.kind === "loading" ? datasetStatus.previous : undefined;
  const activeDataset = readyDataset ?? retainedDataset;
  const operationBusy =
    datasetStatus.kind === "loading" ||
    profileStatus.kind === "loading" ||
    changeStatus.kind === "working" ||
    exportStatus.kind === "loading";
  const activePhaseMeta = phases.find((phase) => phase.id === activePhase) ?? phases[0];

  return (
    <main className="shell">
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
                <span>{phase.number}</span>
                <span className="side-nav__copy">
                  <strong>{phase.label}</strong>
                  <small>{phase.description}</small>
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
      </aside>

      <div className="main-content">
        <header className="topbar">
          <div>
            <p className="step">Vista actual</p>
            <p className="page-title">{activePhaseMeta.label}</p>
          </div>
          <div className={`runtime runtime--${status.kind}`} role="status" aria-live="polite">
            {status.kind === "loading" && "Conectando con Rust…"}
            {status.kind === "browser" && "Vista web · motor no conectado"}
            {status.kind === "ready" && `${status.info.version} · ${status.info.platform}`}
            {status.kind === "error" && `Error del motor: ${status.message}`}
          </div>
        </header>

        <section
          className={`workspace workspace--${activePhase}`}
          aria-label={`Etapa ${activePhaseMeta.label}`}
        >
          {activePhase === "load" && (
            <LoadPhase
              status={status}
              datasetStatus={datasetStatus}
              sheetSelection={sheetSelection}
              selectedSheetId={selectedSheetId}
              spreadsheetHeaderMode={spreadsheetHeaderMode}
              importError={importError}
              importInspecting={importInspecting}
              isDesktopReady={isDesktopReady}
              onSelect={selectDataset}
              onSheetChange={setSelectedSheetId}
              onHeaderModeChange={setSpreadsheetHeaderMode}
              onConfirmSheet={() => {
                if (sheetSelection) void loadSelection(sheetSelection, selectedSheetId, spreadsheetHeaderMode);
              }}
              onCancelSheet={() => void cancelSheetSelection()}
              onCancel={() => cancelActiveOperation("load")}
            />
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
            />
          )}

          {activePhase === "prepare" && readyDataset && (
            <PreparePhase
              dataset={readyDataset.dataset}
              profileStatus={profileStatus}
              changeStatus={changeStatus}
              historyStatus={historyStatus}
              onAnalyzeQuality={analyzeQuality}
              onCancelProfile={() => cancelActiveOperation("profile")}
              onRemoveDuplicates={applyDuplicateRemoval}
              onNormalizeColumns={applyColumnNormalization}
              onApplyRecommended={applyRecommendedCorrections}
              onTrimText={() => applyTextChange("trim", trimTextValues)}
              onNormalizeText={(columns, removeAccents) =>
                applyTextChange("text", () => normalizeTextValues(columns, removeAccents))
              }
              onApplyTransforms={applyStructuralTransforms}
              onUndo={undoChange}
              onRedo={redoChange}
            />
          )}

          {activePhase === "deliver" && readyDataset && (
            <DeliverPhase
              dataset={readyDataset.dataset}
              exportStatus={exportStatus}
              onExport={exportActiveDataset}
              onCancel={() => cancelActiveOperation("export")}
            />
          )}
        </section>
      </div>
    </main>
  );
}

interface LoadPhaseProps {
  status: AppStatus;
  datasetStatus: DatasetStatus;
  sheetSelection: DatasetSourceInspection | null;
  selectedSheetId: string;
  spreadsheetHeaderMode: SpreadsheetHeaderMode;
  importError: string | null;
  importInspecting: boolean;
  isDesktopReady: boolean;
  onSelect: () => void;
  onSheetChange: (sheetId: string) => void;
  onHeaderModeChange: (mode: SpreadsheetHeaderMode) => void;
  onConfirmSheet: () => void;
  onCancelSheet: () => void;
  onCancel: () => void;
}

function LoadPhase({
  status,
  datasetStatus,
  sheetSelection,
  selectedSheetId,
  spreadsheetHeaderMode,
  importError,
  importInspecting,
  isDesktopReady,
  onSelect,
  onSheetChange,
  onHeaderModeChange,
  onConfirmSheet,
  onCancelSheet,
  onCancel,
}: LoadPhaseProps) {
  const current =
    datasetStatus.kind === "ready"
      ? datasetStatus.dataset
      : datasetStatus.kind === "loading"
        ? datasetStatus.previous?.dataset
        : undefined;

  return (
    <>
      <header className="phase-header">
        <div>
          <p className="eyebrow">Cargar · Fuente local</p>
          <h2>{current ? current.fileName : "Selecciona un dataset"}</h2>
          <p>
            Se admiten CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS de hasta 500 MB. El procesamiento se realiza
            localmente y tus datos no salen del equipo.
          </p>
        </div>
        <button
          className="primary-action"
          type="button"
          onClick={onSelect}
          disabled={
            !isDesktopReady || importInspecting || datasetStatus.kind === "loading" || Boolean(sheetSelection)
          }
        >
          {importInspecting
            ? "Inspeccionando…"
            : current
              ? "Seleccionar otro dataset"
              : "Seleccionar dataset"}
        </button>
      </header>

      {datasetStatus.kind === "loading" && (
        <OperationProgressView
          progress={datasetStatus.progress}
          cancelRequested={datasetStatus.cancelRequested}
          onCancel={onCancel}
        />
      )}
      {importInspecting && (
        <p className="notice" role="status">Esperando la selección y verificando el formato local…</p>
      )}
      {datasetStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo cargar el archivo: {datasetStatus.message}
        </p>
      )}
      {importError && (
        <p className="notice notice--error" role="alert">
          No se pudo importar el archivo: {importError}
        </p>
      )}
      {sheetSelection && (
        <div className="sheet-dialog" role="dialog" aria-modal="true" aria-labelledby="sheet-title">
          <div className="sheet-dialog__panel">
            <p className="eyebrow">Libro seleccionado</p>
            <h3 id="sheet-title">Elegir hoja de {sheetSelection.fileName}</h3>
            <p>Columnia cargará únicamente la hoja elegida y conservará el dataset activo hasta terminar.</p>
            {sheetSelection.isCompressedContainer && (
              <p className="notice" role="note">
                Los libros comprimidos pueden ocupar bastante más memoria al abrirse que su tamaño en disco.
                Cierra otras aplicaciones si el archivo es grande.
              </p>
            )}
            <label htmlFor="workbook-sheet">Hoja</label>
            <select
              id="workbook-sheet"
              value={selectedSheetId}
              onChange={(event) => onSheetChange(event.target.value)}
            >
              {sheetSelection.sheets.map((sheet) => (
                <option key={sheet.id} value={sheet.id}>{sheet.name}</option>
              ))}
            </select>
            <fieldset className="sheet-dialog__options">
              <legend>Encabezados</legend>
              <label>
                <input
                  type="radio"
                  name="spreadsheet-header-mode"
                  checked={spreadsheetHeaderMode === "firstRow"}
                  onChange={() => onHeaderModeChange("firstRow")}
                />
                Usar la primera fila como encabezados
              </label>
              <label>
                <input
                  type="radio"
                  name="spreadsheet-header-mode"
                  checked={spreadsheetHeaderMode === "generated"}
                  onChange={() => onHeaderModeChange("generated")}
                />
                Generar encabezados (column_1, column_2…)
              </label>
            </fieldset>
            <div className="sheet-dialog__actions">
              <button type="button" className="secondary-action" onClick={onCancelSheet}>Cancelar</button>
              <button type="button" className="primary-action" onClick={onConfirmSheet} disabled={!selectedSheetId}>
                Cargar hoja
              </button>
            </div>
          </div>
        </div>
      )}
      {status.kind === "browser" && (
        <p className="notice" role="status">
          Abre Columnia con Tauri para seleccionar archivos locales.
        </p>
      )}
      {current && <DatasetMetrics dataset={current} />}
    </>
  );
}

interface ReviewPhaseProps {
  datasetStatus: ReadyDatasetStatus;
  profileStatus: ProfileStatus;
  reviewTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
  onPageChange: (offset: number) => void;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
}

function ReviewPhase({
  datasetStatus,
  profileStatus,
  reviewTab,
  onTabChange,
  onPageChange,
  onAnalyzeQuality,
  onCancelProfile,
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
      <div className="stage-tabs" role="tablist" aria-label="Vistas de revisión">
        <button
          type="button"
          role="tab"
          aria-selected={reviewTab === "diagnosis"}
          className={reviewTab === "diagnosis" ? "stage-tab--active" : undefined}
          onClick={() => onTabChange("diagnosis")}
        >
          Diagnóstico
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={reviewTab === "preview"}
          className={reviewTab === "preview" ? "stage-tab--active" : undefined}
          onClick={() => onTabChange("preview")}
        >
          Vista previa
        </button>
      </div>

      {reviewTab === "diagnosis" ? (
        <QualitySection
          dataset={datasetStatus.dataset}
          status={profileStatus}
          onAnalyze={onAnalyzeQuality}
          onCancel={onCancelProfile}
        />
      ) : (
        <DataPreview
          dataset={datasetStatus.dataset}
          pageOffset={datasetStatus.pageOffset}
          pageLoading={datasetStatus.pageLoading}
          pageError={datasetStatus.pageError}
          onPageChange={onPageChange}
        />
      )}
    </>
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
      {status.kind === "loading" && (
        <OperationProgressView
          progress={status.progress}
          cancelRequested={status.cancelRequested}
          onCancel={onCancel}
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

interface PreparePhaseProps {
  dataset: DatasetPreview;
  profileStatus: ProfileStatus;
  changeStatus: ChangeStatus;
  historyStatus: HistoryState;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
  onRemoveDuplicates: () => void;
  onNormalizeColumns: () => void;
  onApplyRecommended: () => void;
  onTrimText: () => void;
  onNormalizeText: (columns: string[], removeAccents: boolean) => void;
  onApplyTransforms: (recipe: TransformRecipe) => void;
  onUndo: () => void;
  onRedo: () => void;
}

function PreparePhase({
  dataset,
  profileStatus,
  changeStatus,
  historyStatus,
  onAnalyzeQuality,
  onCancelProfile,
  onRemoveDuplicates,
  onNormalizeColumns,
  onApplyRecommended,
  onTrimText,
  onNormalizeText,
  onApplyTransforms,
  onUndo,
  onRedo,
}: PreparePhaseProps) {
  const duplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.duplicateRowCount : null;
  const changing = changeStatus.kind === "working";
  const textColumns = dataset.columns.filter((column) => column.dataType === "String" && column.name !== "_cambios");
  const [selectedTextColumns, setSelectedTextColumns] = useState<string[]>([]);
  const [removeAccents, setRemoveAccents] = useState(true);
  const [activeTab, setActiveTab] = useState<"corrections" | "transformations">("corrections");

  useEffect(() => {
    const available = new Set(textColumns.map((column) => column.name));
    setSelectedTextColumns((current) => current.filter((name) => available.has(name)));
  }, [dataset.columns]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Preparar · {activeTab === "corrections" ? "Correcciones" : "Transformaciones"}</p>
          <h2>{dataset.fileName}</h2>
          <p>Aplica cambios controlados al dataset activo. Cada corrección indica su impacto.</p>
        </div>
      </header>
      <HistoryBar
        status={historyStatus}
        busy={changing}
        onUndo={onUndo}
        onRedo={onRedo}
      />
      <ChangeFeedback status={changeStatus} />
      <div className="stage-tabs" role="tablist" aria-label="Herramientas de preparación">
        <button
          id="prepare-corrections-tab"
          type="button"
          role="tab"
          aria-selected={activeTab === "corrections"}
          aria-controls="prepare-corrections-panel"
          tabIndex={activeTab === "corrections" ? 0 : -1}
          className={activeTab === "corrections" ? "stage-tab--active" : undefined}
          onClick={() => setActiveTab("corrections")}
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault();
              setActiveTab("transformations");
              document.getElementById("prepare-transformations-tab")?.focus();
            }
          }}
        >
          Correcciones
        </button>
        <button
          id="prepare-transformations-tab"
          type="button"
          role="tab"
          aria-selected={activeTab === "transformations"}
          aria-controls="prepare-transformations-panel"
          tabIndex={activeTab === "transformations" ? 0 : -1}
          className={activeTab === "transformations" ? "stage-tab--active" : undefined}
          onClick={() => setActiveTab("transformations")}
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault();
              setActiveTab("corrections");
              document.getElementById("prepare-corrections-tab")?.focus();
            }
          }}
        >
          Transformaciones
        </button>
      </div>
      {activeTab === "transformations" ? (
        <div
          id="prepare-transformations-panel"
          role="tabpanel"
          aria-labelledby="prepare-transformations-tab"
        >
          <TransformRecipeEditor
            key={`${dataset.fileName}:${dataset.fileSizeBytes}:${dataset.rowCount}:${dataset.columns.map((column) => column.name).join("|")}:${changeStatus.kind === "applied" ? changeStatus.message : ""}`}
            dataset={dataset}
            busy={changing}
            onApply={onApplyTransforms}
          />
        </div>
      ) : (
      <div
        id="prepare-corrections-panel"
        role="tabpanel"
        aria-labelledby="prepare-corrections-tab"
      >
      <section className="recommended-batch" aria-labelledby="recommended-batch-title">
        <div>
          <p className="step">Aplicación agrupada</p>
          <h3 id="recommended-batch-title">Correcciones recomendadas</h3>
          <p>
            Recorta espacios exteriores y normaliza los encabezados en una sola operación
            atómica y reversible.
          </p>
        </div>
        <button type="button" onClick={onApplyRecommended} disabled={changing}>
          Aplicar recomendadas
        </button>
      </section>
      <section className="prepare-card" aria-labelledby="normalize-columns-title">
        <div>
          <p className="step">Recomendada y segura</p>
          <h3 id="normalize-columns-title">Normalizar nombres de columnas</h3>
          <p>
            Convierte los encabezados a nombres consistentes en minúsculas, sin acentos y con
            guiones bajos. Las colisiones se numeran de forma determinista.
          </p>
        </div>
        <button type="button" onClick={onNormalizeColumns} disabled={changing}>
          Normalizar columnas
        </button>
      </section>
      <section className="prepare-card" aria-labelledby="trim-text-title">
        <div>
          <p className="step">Recomendada y segura</p>
          <h3 id="trim-text-title">Eliminar espacios exteriores</h3>
          <p>
            Recorta espacios al inicio y al final de todas las columnas de texto sin cambiar
            mayúsculas, acentos ni espacios internos.
          </p>
        </div>
        <button type="button" onClick={onTrimText} disabled={changing || textColumns.length === 0}>
          Recortar espacios
        </button>
      </section>
      <section className="prepare-card prepare-card--stacked" aria-labelledby="normalize-text-title">
        <div>
          <p className="step">Requiere selección</p>
          <h3 id="normalize-text-title">Normalizar texto</h3>
          <p>
            Convierte a minúsculas, compacta espacios y, opcionalmente, elimina acentos. Puede
            unir categorías que antes eran distintas; elige las columnas conscientemente.
          </p>
        </div>
        {textColumns.length > 0 ? (
          <div className="text-cleaning-options">
            <fieldset>
              <legend>Columnas de texto</legend>
              {textColumns.map((column) => (
                <label key={column.name}>
                  <input
                    type="checkbox"
                    checked={selectedTextColumns.includes(column.name)}
                    onChange={(event) =>
                      setSelectedTextColumns((current) =>
                        event.target.checked
                          ? [...current, column.name]
                          : current.filter((name) => name !== column.name),
                      )
                    }
                  />
                  {column.name}
                </label>
              ))}
            </fieldset>
            <label className="option-toggle">
              <input
                type="checkbox"
                checked={removeAccents}
                onChange={(event) => setRemoveAccents(event.target.checked)}
              />
              Eliminar acentos
            </label>
            <button
              type="button"
              onClick={() => onNormalizeText(selectedTextColumns, removeAccents)}
              disabled={changing || selectedTextColumns.length === 0}
            >
              Normalizar texto seleccionado
            </button>
          </div>
        ) : (
          <p className="profile-note">Este dataset no contiene columnas de texto.</p>
        )}
      </section>
      {profileStatus.kind === "loading" ? (
        <OperationProgressView
          progress={profileStatus.progress}
          cancelRequested={profileStatus.cancelRequested}
          onCancel={onCancelProfile}
        />
      ) : (
        <section className="prepare-card" aria-labelledby="duplicates-title">
          <div>
            <p className="step">Corrección disponible</p>
            <h3 id="duplicates-title">Filas duplicadas</h3>
            <p>
              {duplicateCount === null
                ? "Analiza la calidad para identificar duplicados antes de modificar los datos."
                : duplicateCount === 0
                  ? "No se detectaron filas duplicadas adicionales."
                  : `Se detectaron ${duplicateCount.toLocaleString()} filas duplicadas adicionales.`}
            </p>
          </div>
          {duplicateCount === null ? (
            <button type="button" onClick={onAnalyzeQuality}>
              Analizar antes de preparar
            </button>
          ) : (
            <button
              type="button"
              onClick={onRemoveDuplicates}
              disabled={duplicateCount === 0 || changing}
            >
              Eliminar duplicados
            </button>
          )}
        </section>
      )}
      {profileStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {profileStatus.message}
        </p>
      )}
      </div>
      )}
    </>
  );
}

function TransformRecipeEditor({
  dataset,
  busy,
  onApply,
}: {
  dataset: DatasetPreview;
  busy: boolean;
  onApply: (recipe: TransformRecipe) => void;
}) {
  type RenameDraft = TransformRecipe["renames"][number];
  type CastDraft = TransformRecipe["casts"][number];
  type DateDraft = TransformRecipe["dateParses"][number];
  type FilterDraft = TransformRecipe["filters"][number];
  type CalculationDraft = NonNullable<TransformRecipe["calculatedColumn"]>;
  type FindReplaceDraft = NonNullable<TransformRecipe["findReplace"]>;
  type SplitDraft = NonNullable<TransformRecipe["splitColumn"]>;
  type MergeDraft = NonNullable<TransformRecipe["mergeColumns"]>;
  type OutlierDraft = TransformRecipe["outlierTreatments"][number];
  type GroupSummaryDraft = NonNullable<TransformRecipe["groupSummary"]>;
  type ContactDraft = TransformRecipe["contactNormalizations"][number];
  type ExtractionDraft = TransformRecipe["textExtractions"][number];

  const [renames, setRenames] = useState<RenameDraft[]>([{ from: "", to: "" }]);
  const [casts, setCasts] = useState<CastDraft[]>([{ column: "", target: "string" }]);
  const [dateParses, setDateParses] = useState<DateDraft[]>([
    { column: "", format: "iso8601", target: "date" },
  ]);
  const [filters, setFilters] = useState<FilterDraft[]>([]);
  const [calculationEnabled, setCalculationEnabled] = useState(false);
  const [calculation, setCalculation] = useState<CalculationDraft>({
    name: "",
    source: "",
    operation: "add",
    operand: { kind: "literal", value: "" },
  });
  const [pendingConfirmation, setPendingConfirmation] = useState<TransformRecipe | null>(null);
  const [findReplaceEnabled, setFindReplaceEnabled] = useState(false);
  const [findReplace, setFindReplace] = useState<FindReplaceDraft>({ scope: "column", column: null, find: "", replace: "" });
  const [keptColumns, setKeptColumns] = useState<string[]>(dataset.columns.map((column) => column.name));
  const [splitEnabled, setSplitEnabled] = useState(false);
  const [split, setSplit] = useState<SplitDraft>({ source: "", delimiter: "", names: [], dropSource: false });
  const [splitNamesInput, setSplitNamesInput] = useState("");
  const [mergeEnabled, setMergeEnabled] = useState(false);
  const [merge, setMerge] = useState<MergeDraft>({ sources: [], name: "", separator: "", dropSources: false });
  const [outlierTreatments, setOutlierTreatments] = useState<OutlierDraft[]>([]);
  const [groupEnabled, setGroupEnabled] = useState(false);
  const [groupSummary, setGroupSummary] = useState<GroupSummaryDraft>({ groupBy: [], aggregations: [] });
  const [contacts, setContacts] = useState<ContactDraft[]>([]);
  const [extractions, setExtractions] = useState<ExtractionDraft[]>([]);
  const [recipeName, setRecipeName] = useState("Mi receta");
  const [recipeFileStatus, setRecipeFileStatus] = useState<
    | { kind: "idle" }
    | { kind: "working"; action: "save" | "load" }
    | { kind: "success"; message: string }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const acknowledgedRecipeFingerprint = useRef<string | null>(null);
  const recipeBusy = busy || recipeFileStatus.kind === "working";
  const datasetSignature = `${dataset.fileName}:${dataset.fileSizeBytes}:${dataset.rowCount}:${dataset.columns.map((column) => `${column.name}:${column.dataType}`).join("|")}`;

  useEffect(() => {
    setRenames([{ from: "", to: "" }]);
    setCasts([{ column: "", target: "string" }]);
    setDateParses([{ column: "", format: "iso8601", target: "date" }]);
    setFilters([]);
    setCalculationEnabled(false);
    setPendingConfirmation(null);
    setFindReplaceEnabled(false);
    setFindReplace({ scope: "column", column: null, find: "", replace: "" });
    setKeptColumns(dataset.columns.map((column) => column.name));
    setSplitEnabled(false);
    setSplit({ source: "", delimiter: "", names: [], dropSource: false });
    setSplitNamesInput("");
    setMergeEnabled(false);
    setMerge({ sources: [], name: "", separator: "", dropSources: false });
    setOutlierTreatments([]);
    setGroupEnabled(false);
    setGroupSummary({ groupBy: [], aggregations: [] });
    setContacts([]);
    setExtractions([]);
    setRecipeName("Mi receta");
    setRecipeFileStatus({ kind: "idle" });
    acknowledgedRecipeFingerprint.current = null;
    setCalculation({ name: "", source: "", operation: "add", operand: { kind: "literal", value: "" } });
  }, [datasetSignature]);

  useEffect(() => {
    setRenames([{ from: "", to: "" }]);
    setCasts([{ column: "", target: "string" }]);
    setDateParses([{ column: "", format: "iso8601", target: "date" }]);
  }, [dataset.columns]);

  const activeRenames = renames.filter((item) => item.from || item.to);
  const activeCasts = casts.filter((item) => item.column);
  const activeDateParses = dateParses.filter((item) => item.column);
  const activeFilters = filters.filter((item) => item.column);
  const searchableTextColumns = dataset.columns.filter((column) => {
    if (activeDateParses.some((item) => item.column === column.name)) return false;
    const cast = [...activeCasts].reverse().find((item) => item.column === column.name);
    return cast ? cast.target === "string" : column.dataType === "String";
  });
  const numericColumns = dataset.columns.filter((column) => {
    if (activeDateParses.some((item) => item.column === column.name)) return false;
    const cast = [...activeCasts].reverse().find((item) => item.column === column.name);
    return cast ? ["integer", "decimal"].includes(cast.target) : ["Int64", "Float64"].includes(column.dataType);
  });
  function effectiveName(name: string) { return activeRenames.find((item) => item.from === name)?.to.trim() || name; }
  function effectiveType(name: string) {
    if (activeDateParses.some((item) => item.column === name)) return activeDateParses.find((item) => item.column === name)?.target === "date" ? "Date" : "Datetime";
    const cast = [...activeCasts].reverse().find((item) => item.column === name);
    if (cast) return cast.target === "integer" ? "Int64" : cast.target === "decimal" ? "Float64" : cast.target === "string" ? "String" : "Boolean";
    return dataset.columns.find((column) => column.name === name)?.dataType ?? "";
  }
  const operandRequired = !["year", "month", "day"].includes(calculation.operation);
  const calculationInvalid =
    calculationEnabled &&
    (!calculation.name.trim() ||
      !calculation.source ||
      (operandRequired && calculation.operation !== "concat" && !calculation.operand?.value.trim()) ||
      (calculation.operand?.kind === "column" && !calculation.operand.value));
  const calculationValid =
    !calculationEnabled ||
    !calculationInvalid;
  const calculationSourceDropped = calculationEnabled &&
    (!keptColumns.includes(calculation.source) ||
      (calculation.operand?.kind === "column" && !keptColumns.includes(calculation.operand.value)));
  const findReplaceInvalid = findReplaceEnabled &&
    (!findReplace.find || searchableTextColumns.length === 0 || (findReplace.scope === "column" && !findReplace.column));
  const dropsColumns = keptColumns.length < dataset.columns.length;
  const parsedSplitNames = splitNamesInput.split(",").map((name) => name.trim()).filter(Boolean);
  const postRenameNames = new Set(dataset.columns.map((column) => activeRenames.find((item) => item.from === column.name)?.to.trim() || column.name));
  const calculatedName = calculationEnabled ? calculation.name.trim() : "";
  const splitInvalid = splitEnabled && (!split.source || !split.delimiter || parsedSplitNames.length < 2 || parsedSplitNames.length > 16 || new Set(parsedSplitNames).size !== parsedSplitNames.length || parsedSplitNames.some((name) => postRenameNames.has(name) || name === calculatedName));
  const mergeInvalid = mergeEnabled && (merge.sources.length < 2 || merge.sources.length > 16 || !merge.name.trim() || postRenameNames.has(merge.name.trim()) || merge.name.trim() === calculatedName || parsedSplitNames.includes(merge.name.trim()));
  const sourceConflict = splitEnabled && mergeEnabled && split.dropSource && merge.sources.includes(split.source);
  const sourceNotKept = (splitEnabled && !keptColumns.includes(split.source)) || (mergeEnabled && merge.sources.some((source) => !keptColumns.includes(source)));
  const outlierDuplicate = new Set(outlierTreatments.map((item) => item.column)).size !== outlierTreatments.length;
  const outlierDependencyInvalid = outlierTreatments.some((item) => !keptColumns.includes(item.column) || (splitEnabled && split.dropSource && split.source === item.column) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.column)));
  const outlierInvalid = outlierTreatments.some((item) => !item.column || !numericColumns.some((column) => column.name === item.column)) || outlierDuplicate || outlierTreatments.length > 16 || outlierDependencyInvalid;
  const groupPairs = groupSummary.aggregations.map((item) => `${item.column}:${item.operation}`);
  const groupOutputs = groupSummary.aggregations.map((item) => `${effectiveName(item.column)}_${item.operation}`);
  const groupDependenciesInvalid = [...groupSummary.groupBy, ...groupSummary.aggregations.map((item) => item.column)].some((name) => !keptColumns.includes(name) || (splitEnabled && split.dropSource && split.source === name) || (mergeEnabled && merge.dropSources && merge.sources.includes(name)));
  const groupOperationInvalid = groupSummary.aggregations.some((item) => {
    const dtype = effectiveType(item.column);
    if (!item.column) return true;
    if (["sum", "mean"].includes(item.operation)) return !["Int64", "Float64"].includes(dtype);
    if (["min", "max"].includes(item.operation)) return !["Int64", "Float64", "String", "Date", "Datetime"].includes(dtype);
    return false;
  });
  const groupInvalid = groupEnabled && (groupSummary.groupBy.length < 1 || groupSummary.groupBy.length > 8 || groupSummary.aggregations.length < 1 || groupSummary.aggregations.length > 32 || new Set(groupPairs).size !== groupPairs.length || new Set(groupOutputs).size !== groupOutputs.length || groupOutputs.some((name) => groupSummary.groupBy.map(effectiveName).includes(name)) || groupDependenciesInvalid || groupOperationInvalid);
  const contactDuplicate = new Set(contacts.map((item) => item.column)).size !== contacts.length;
  const extractionNames = extractions.map((item) => item.name.trim());
  const contactDependencyInvalid = contacts.some((item) => !keptColumns.includes(item.column) || (splitEnabled && split.dropSource && split.source === item.column) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.column)));
  const extractionDependencyInvalid = extractions.some((item) => !keptColumns.includes(item.source) || (splitEnabled && split.dropSource && split.source === item.source) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.source)));
  const contactInvalid = contacts.some((item) => !item.column || !searchableTextColumns.some((column) => column.name === item.column)) || contactDuplicate || contacts.length > 16 || contactDependencyInvalid;
  const extractionInvalid = extractions.length > 16 || (groupEnabled && extractions.length > 0) || extractionDependencyInvalid || new Set(extractionNames).size !== extractionNames.length || extractions.some((item) => !item.source || !searchableTextColumns.some((column) => column.name === item.source) || !item.name.trim() || item.name !== item.name.trim() || postRenameNames.has(item.name.trim()) || item.name.trim() === calculatedName || parsedSplitNames.includes(item.name.trim()) || item.name.trim() === merge.name.trim() || (["before", "after"].includes(item.kind) && !item.delimiter));
  const operationCount = activeRenames.length + activeCasts.length + activeDateParses.length + activeFilters.length + (calculationEnabled ? 1 : 0) + (findReplaceEnabled ? 1 : 0) + (dropsColumns ? 1 : 0) + (splitEnabled ? 1 : 0) + (mergeEnabled ? 1 : 0) + outlierTreatments.length + (groupEnabled ? 1 : 0) + contacts.length + extractions.length;
  const renameInvalid = activeRenames.some((item) => !item.from || !item.to.trim());
  const filterInvalid = activeFilters.some((item) =>
    !["eq", "neq", "is_null", "not_null"].includes(item.operator) && !item.value?.trim(),
  );
  const invalid = renameInvalid || filterInvalid || findReplaceInvalid || keptColumns.length === 0 || calculationSourceDropped || splitInvalid || mergeInvalid || sourceConflict || sourceNotKept || outlierInvalid || groupInvalid || contactInvalid || extractionInvalid ||
    !calculationValid;

  function columnOptions() {
    return dataset.columns.map((column) => (
      <option key={column.name} value={column.name}>
        {column.name}
      </option>
    ));
  }

  function buildRecipe(): TransformRecipe {
    return {
      renames: activeRenames.map((item) => ({ from: item.from, to: item.to.trim() })),
      casts: activeCasts,
      dateParses: activeDateParses,
      filters: activeFilters.map((item) => ({
        ...item,
        value: ["is_null", "not_null"].includes(item.operator) ? null : item.value,
      })),
      calculatedColumn: calculationEnabled
        ? {
            ...calculation,
            name: calculation.name.trim(),
            operand: operandRequired ? calculation.operand : null,
          }
        : null,
      findReplace: findReplaceEnabled ? findReplace : null,
      keepColumns: dropsColumns ? dataset.columns.map((column) => column.name).filter((name) => keptColumns.includes(name)) : null,
      splitColumn: splitEnabled ? { ...split, names: parsedSplitNames } : null,
      mergeColumns: mergeEnabled ? { ...merge, name: merge.name.trim() } : null,
      outlierTreatments,
      groupSummary: groupEnabled ? groupSummary : null,
      contactNormalizations: contacts,
      textExtractions: extractions.map((item) => ({ ...item, name: item.name.trim(), delimiter: ["before", "after"].includes(item.kind) ? item.delimiter : null })),
    };
  }

  const draftFingerprint = JSON.stringify(buildRecipe());
  useEffect(() => {
    if (recipeFileStatus.kind !== "success") return;
    if (acknowledgedRecipeFingerprint.current === null) {
      acknowledgedRecipeFingerprint.current = draftFingerprint;
      return;
    }
    if (acknowledgedRecipeFingerprint.current !== draftFingerprint) {
      setRecipeFileStatus({ kind: "idle" });
    }
  }, [draftFingerprint, recipeFileStatus.kind]);

  function submitRecipe() {
    if (operationCount === 0 || invalid) return;
    const recipe = buildRecipe();
    if (recipe.filters.length > 0 || recipe.keepColumns !== null || recipe.splitColumn?.dropSource || recipe.mergeColumns?.dropSources || recipe.outlierTreatments.length > 0 || recipe.groupSummary || recipe.contactNormalizations.length > 0) setPendingConfirmation(recipe);
    else onApply(recipe);
  }

  async function saveRecipeDraft() {
    if (recipeBusy || operationCount === 0 || invalid || !recipeName.trim()) return;
    setRecipeFileStatus({ kind: "working", action: "save" });
    try {
      const saved = await saveTransformRecipe(buildRecipe(), recipeName.trim());
      if (saved) acknowledgedRecipeFingerprint.current = draftFingerprint;
      setRecipeFileStatus(saved
        ? { kind: "success", message: `Receta guardada: ${saved.name}. Los cambios posteriores no se guardan automáticamente.` }
        : { kind: "idle" });
    } catch (error) {
      setRecipeFileStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  function replaceDraft(loaded: LoadedRecipe) {
    const recipe = loaded.recipe;
    setRenames(recipe.renames.length > 0 ? recipe.renames : [{ from: "", to: "" }]);
    setCasts(recipe.casts.length > 0 ? recipe.casts : [{ column: "", target: "string" }]);
    setDateParses(recipe.dateParses.length > 0 ? recipe.dateParses : [{ column: "", format: "iso8601", target: "date" }]);
    setFilters(recipe.filters);
    setCalculationEnabled(recipe.calculatedColumn !== null);
    setCalculation(recipe.calculatedColumn ?? { name: "", source: "", operation: "add", operand: { kind: "literal", value: "" } });
    setFindReplaceEnabled(recipe.findReplace !== null);
    setFindReplace(recipe.findReplace ?? { scope: "column", column: null, find: "", replace: "" });
    setKeptColumns(recipe.keepColumns ?? dataset.columns.map((column) => column.name));
    setSplitEnabled(recipe.splitColumn !== null);
    setSplit(recipe.splitColumn ?? { source: "", delimiter: "", names: [], dropSource: false });
    setSplitNamesInput(recipe.splitColumn?.names.join(", ") ?? "");
    setMergeEnabled(recipe.mergeColumns !== null);
    setMerge(recipe.mergeColumns ?? { sources: [], name: "", separator: "", dropSources: false });
    setOutlierTreatments(recipe.outlierTreatments);
    setGroupEnabled(recipe.groupSummary !== null);
    setGroupSummary(recipe.groupSummary ?? { groupBy: [], aggregations: [] });
    setContacts(recipe.contactNormalizations);
    setExtractions(recipe.textExtractions);
    setPendingConfirmation(null);
    setRecipeName(loaded.name);
  }

  async function loadRecipeDraft() {
    if (recipeBusy) return;
    setRecipeFileStatus({ kind: "working", action: "load" });
    try {
      const loaded = await pickTransformRecipe();
      if (loaded === null) {
        setRecipeFileStatus({ kind: "idle" });
        return;
      }
      if (!isLoadedRecipe(loaded)) throw new Error("El archivo no contiene una receta compatible con Columnia.");
      if (operationCount > 0 && !window.confirm("La receta cargada reemplazará el borrador actual. ¿Deseas continuar?")) {
        setRecipeFileStatus({ kind: "idle" });
        return;
      }
      replaceDraft(loaded);
      acknowledgedRecipeFingerprint.current = null;
      setRecipeFileStatus({ kind: "success", message: `Receta cargada: ${loaded.name}. Revísala antes de aplicarla.` });
    } catch (error) {
      setRecipeFileStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  return (
    <section className="transform-recipe" aria-labelledby="transform-recipe-title">
      <div className="transform-recipe__intro">
        <div>
          <p className="step">Receta estructural</p>
          <h3 id="transform-recipe-title">Preparar estructura y tipos</h3>
          <p>
            Configura varios cambios y aplícalos juntos. Si una operación no es válida, no se
            modifica ninguna columna.
          </p>
        </div>
        <span aria-live="polite">{operationCount} operaciones listas</span>
      </div>

      <div className="recipe-files" aria-label="Archivo de receta">
        <label>
          <span>Nombre de la receta</span>
          <input aria-label="Nombre de la receta" value={recipeName} maxLength={80}
            disabled={recipeBusy}
            onChange={(event) => { setRecipeName(event.target.value); if (recipeFileStatus.kind !== "working") setRecipeFileStatus({ kind: "idle" }); }} />
        </label>
        <button type="button" onClick={saveRecipeDraft}
          disabled={recipeBusy || operationCount === 0 || invalid || !recipeName.trim()}>
          {recipeFileStatus.kind === "working" && recipeFileStatus.action === "save" ? "Guardando…" : "Guardar receta"}
        </button>
        <button type="button" onClick={loadRecipeDraft} disabled={recipeBusy}>
          {recipeFileStatus.kind === "working" && recipeFileStatus.action === "load" ? "Cargando…" : "Cargar receta"}
        </button>
        {recipeFileStatus.kind === "success" && <p className="recipe-file-status" role="status">{recipeFileStatus.message}</p>}
        {recipeFileStatus.kind === "error" && <p className="recipe-error recipe-file-status" role="alert">No se pudo completar la operación: {recipeFileStatus.message}</p>}
      </div>

      <div className="transform-recipe__grid">
        <fieldset>
          <legend>Renombrar columnas</legend>
          {renames.map((rename, index) => (
            <div className="recipe-row recipe-row--rename" key={`rename-${index}`}>
              <label>
                <span>Columna</span>
                <select
                  aria-label={`Columna para renombrar ${index + 1}`}
                  value={rename.from}
                  onChange={(event) =>
                    setRenames((current) =>
                      current.map((item, itemIndex) =>
                        itemIndex === index ? { ...item, from: event.target.value } : item,
                      ),
                    )
                  }
                >
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Nuevo nombre</span>
                <input
                  aria-label={`Nuevo nombre ${index + 1}`}
                  value={rename.to}
                  onChange={(event) =>
                    setRenames((current) =>
                      current.map((item, itemIndex) =>
                        itemIndex === index ? { ...item, to: event.target.value } : item,
                      ),
                    )
                  }
                />
              </label>
              <button type="button" aria-label={`Quitar renombre ${index + 1}`} onClick={() => setRenames((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setRenames((current) => [...current, { from: "", to: "" }])}>+ Añadir renombre</button>
        </fieldset>

        <fieldset>
          <legend>Convertir tipos</legend>
          {casts.map((cast, index) => (
            <div className="recipe-row" key={`cast-${index}`}>
              <label>
                <span>Columna</span>
                <select aria-label={`Columna para convertir ${index + 1}`} value={cast.column} onChange={(event) => setCasts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}>
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Tipo destino</span>
                <select aria-label={`Tipo destino ${index + 1}`} value={cast.target} onChange={(event) => setCasts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, target: event.target.value as CastDraft["target"] } : item))}>
                  <option value="string">Texto</option>
                  <option value="integer">Entero</option>
                  <option value="decimal">Decimal</option>
                  <option value="boolean">Booleano</option>
                </select>
              </label>
              <button type="button" aria-label={`Quitar conversión ${index + 1}`} onClick={() => setCasts((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setCasts((current) => [...current, { column: "", target: "string" }])}>+ Añadir conversión</button>
        </fieldset>

        <fieldset>
          <legend>Interpretar fechas</legend>
          {dateParses.map((dateParse, index) => (
            <div className="recipe-row recipe-row--date" key={`date-${index}`}>
              <label>
                <span>Columna</span>
                <select aria-label={`Columna de fecha ${index + 1}`} value={dateParse.column} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}>
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Formato origen</span>
                <select aria-label={`Formato de fecha ${index + 1}`} value={dateParse.format} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, format: event.target.value as DateDraft["format"] } : item))}>
                  <option value="iso8601">ISO 8601</option>
                  <option value="ymd">AAAA-MM-DD</option>
                  <option value="dmy">DD/MM/AAAA</option>
                  <option value="mdy">MM/DD/AAAA</option>
                </select>
              </label>
              <label>
                <span>Tipo destino</span>
                <select aria-label={`Tipo de fecha destino ${index + 1}`} value={dateParse.target} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, target: event.target.value as DateDraft["target"] } : item))}>
                  <option value="date">Fecha</option>
                  <option value="datetime">Fecha y hora</option>
                </select>
              </label>
              <button type="button" aria-label={`Quitar fecha ${index + 1}`} onClick={() => setDateParses((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setDateParses((current) => [...current, { column: "", format: "iso8601", target: "date" }])}>+ Añadir fecha</button>
        </fieldset>

        <fieldset>
          <legend>Filtrar filas (AND)</legend>
          <p className="recipe-hint">
            Todas las condiciones deben cumplirse. Puedes añadir hasta 3 filtros. Mayor que,
            menor que, mayor o igual y menor o igual son comparaciones numéricas estrictas; para
            fechas, extrae primero año, mes o día. La comparación directa de fechas se incorporará
            cuando exista conversión compatible.
          </p>
          {filters.map((filter, index) => {
            const unary = ["is_null", "not_null"].includes(filter.operator);
            return (
              <div className="recipe-row recipe-row--date" key={`filter-${index}`}>
                <label><span>Columna</span><select aria-label={`Columna del filtro ${index + 1}`} value={filter.column} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{columnOptions()}</select></label>
                <label><span>Condición</span><select aria-label={`Operador del filtro ${index + 1}`} value={filter.operator} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, operator: event.target.value as FilterDraft["operator"], value: ["is_null", "not_null"].includes(event.target.value) ? null : (item.value ?? "") } : item))}>
                  <option value="eq">Igual a</option><option value="neq">Distinto de</option><option value="gt">Mayor que</option><option value="lt">Menor que</option><option value="gte">Mayor o igual</option><option value="lte">Menor o igual</option><option value="contains">Contiene</option><option value="not_contains">No contiene</option><option value="is_null">Es nulo</option><option value="not_null">No es nulo</option>
                </select></label>
                <label><span>Valor</span><input aria-label={`Valor del filtro ${index + 1}`} value={filter.value ?? ""} disabled={unary} placeholder={unary ? "No requerido" : "Valor estricto"} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, value: event.target.value } : item))} /></label>
                <button type="button" aria-label={`Quitar filtro ${index + 1}`} onClick={() => setFilters((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
              </div>
            );
          })}
          {filters.length < 3 && <button type="button" className="recipe-add" onClick={() => setFilters((current) => [...current, { column: "", operator: "eq", value: "" }])}>+ Añadir filtro AND</button>}
        </fieldset>

        <fieldset>
          <legend>Columna calculada</legend>
          <label className="option-toggle"><input type="checkbox" checked={calculationEnabled} onChange={(event) => setCalculationEnabled(event.target.checked)} />Crear una columna en esta receta</label>
          {calculationEnabled && (
            <div className="calculation-grid">
              <label><span>Nombre nuevo</span><input aria-label="Nombre de la columna calculada" value={calculation.name} onChange={(event) => setCalculation((current) => ({ ...current, name: event.target.value }))} /></label>
              <label><span>Columna origen</span><select aria-label="Columna origen del cálculo" value={calculation.source} onChange={(event) => setCalculation((current) => ({ ...current, source: event.target.value }))}><option value="">Selecciona…</option>{columnOptions()}</select></label>
              <label><span>Operación</span><select aria-label="Operación calculada" value={calculation.operation} onChange={(event) => { const operation = event.target.value as CalculationDraft["operation"]; setCalculation((current) => ({ ...current, operation, operand: ["year", "month", "day"].includes(operation) ? null : (current.operand ?? { kind: "literal", value: "" }) })); }}>
                <option value="add">Sumar</option><option value="subtract">Restar</option><option value="multiply">Multiplicar</option><option value="divide">Dividir</option><option value="concat">Concatenar</option><option value="year">Extraer año</option><option value="month">Extraer mes</option><option value="day">Extraer día</option>
              </select></label>
              {operandRequired && <><label><span>Operando</span><select aria-label="Origen del operando" value={calculation.operand?.kind ?? "literal"} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: event.target.value as "literal" | "column", value: "" } }))}><option value="literal">Valor fijo</option><option value="column">Columna</option></select></label>{calculation.operand?.kind === "column" ? <label><span>Columna operando</span><select aria-label="Columna operando" value={calculation.operand.value} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: "column", value: event.target.value } }))}><option value="">Selecciona…</option>{columnOptions()}</select></label> : <label><span>Valor fijo</span><input aria-label="Valor fijo del cálculo" value={calculation.operand?.value ?? ""} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: "literal", value: event.target.value } }))} /></label>}</>}
            </div>
          )}
          <p className="recipe-hint">La evaluación es estricta: tipos incompatibles o división por cero cancelan toda la receta.</p>
        </fieldset>

        <fieldset>
          <legend>Buscar y reemplazar literal</legend>
          <label className="option-toggle"><input type="checkbox" checked={findReplaceEnabled} onChange={(event) => setFindReplaceEnabled(event.target.checked)} />Añadir búsqueda y reemplazo</label>
          {findReplaceEnabled && <div className="calculation-grid">
            <label><span>Alcance</span><select aria-label="Alcance de búsqueda" value={findReplace.scope} disabled={searchableTextColumns.length === 0} onChange={(event) => { const scope = event.target.value as FindReplaceDraft["scope"]; setFindReplace((current) => ({ ...current, scope, column: scope === "column" ? current.column : null })); }}><option value="column">Una columna</option><option value="all_text_columns">Todas las columnas de texto</option></select></label>
            {findReplace.scope === "column" && <label><span>Columna</span><select aria-label="Columna para buscar" value={findReplace.column ?? ""} disabled={searchableTextColumns.length === 0} onChange={(event) => setFindReplace((current) => ({ ...current, column: event.target.value || null }))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>}
            <label><span>Buscar</span><input aria-label="Texto a buscar" value={findReplace.find} onChange={(event) => setFindReplace((current) => ({ ...current, find: event.target.value }))} /></label>
            <label><span>Reemplazar por</span><input aria-label="Texto de reemplazo" value={findReplace.replace} placeholder="Vacío elimina la coincidencia" onChange={(event) => setFindReplace((current) => ({ ...current, replace: event.target.value }))} /></label>
          </div>}
          <p className="recipe-hint">Busca texto literal, distingue mayúsculas y minúsculas y no interpreta expresiones regulares. Se permite buscar espacios y reemplazar por vacío.</p>
          {searchableTextColumns.length === 0 && <p className="recipe-error">Este dataset no contiene columnas de texto disponibles.</p>}
        </fieldset>

        <fieldset>
          <legend>Columnas a conservar</legend>
          <div className="keep-columns" role="group" aria-label="Seleccionar columnas a conservar">
            {dataset.columns.map((column) => <label key={column.name}><input type="checkbox" checked={keptColumns.includes(column.name)} onChange={(event) => setKeptColumns((current) => event.target.checked ? dataset.columns.map((item) => item.name).filter((name) => current.includes(name) || name === column.name) : current.filter((name) => name !== column.name))} />{column.name}</label>)}
          </div>
          <p className="recipe-hint">Se conserva el orden actual. Debe permanecer al menos una columna.</p>
        </fieldset>

        <fieldset>
          <legend>Dividir columna de texto</legend>
          <label className="option-toggle"><input type="checkbox" checked={splitEnabled} onChange={(event) => setSplitEnabled(event.target.checked)} />Dividir una columna</label>
          {splitEnabled && <div className="calculation-grid">
            <label><span>Columna origen</span><select aria-label="Columna para dividir" value={split.source} onChange={(event) => setSplit((current) => ({ ...current, source: event.target.value }))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>
            <label><span>Delimitador literal</span><input aria-label="Delimitador para dividir" value={split.delimiter} onChange={(event) => setSplit((current) => ({ ...current, delimiter: event.target.value }))} /></label>
            <label className="calculation-grid__wide"><span>Nombres separados por coma</span><input aria-label="Nombres de columnas divididas" value={splitNamesInput} placeholder="nombre, apellido" onChange={(event) => setSplitNamesInput(event.target.value)} /></label>
            <label className="option-toggle"><input type="checkbox" checked={split.dropSource} onChange={(event) => setSplit((current) => ({ ...current, dropSource: event.target.checked }))} />Eliminar columna origen</label>
          </div>}
          <p className="recipe-hint">Define entre 2 y 16 nombres únicos. La última columna recibe el resto; las partes faltantes quedan como null.</p>
        </fieldset>

        <fieldset>
          <legend>Combinar columnas de texto</legend>
          <label className="option-toggle"><input type="checkbox" checked={mergeEnabled} onChange={(event) => setMergeEnabled(event.target.checked)} />Combinar columnas</label>
          {mergeEnabled && <>
            <div className="keep-columns" role="group" aria-label="Columnas para combinar">{searchableTextColumns.map((column) => <label key={column.name}><input type="checkbox" checked={merge.sources.includes(column.name)} onChange={(event) => setMerge((current) => ({ ...current, sources: event.target.checked ? searchableTextColumns.map((item) => item.name).filter((name) => current.sources.includes(name) || name === column.name) : current.sources.filter((name) => name !== column.name) }))} />{column.name}</label>)}</div>
            <div className="calculation-grid"><label><span>Nombre nuevo</span><input aria-label="Nombre de columna combinada" value={merge.name} onChange={(event) => setMerge((current) => ({ ...current, name: event.target.value }))} /></label><label><span>Separador</span><input aria-label="Separador para combinar" value={merge.separator} placeholder="Vacío permitido" onChange={(event) => setMerge((current) => ({ ...current, separator: event.target.value }))} /></label><label className="option-toggle"><input type="checkbox" checked={merge.dropSources} onChange={(event) => setMerge((current) => ({ ...current, dropSources: event.target.checked }))} />Eliminar columnas origen</label></div>
          </>}
          <p className="recipe-hint">Selecciona entre 2 y 16 columnas existentes. Los valores null se omiten; si todos son null, el resultado queda null.</p>
        </fieldset>

        <fieldset>
          <legend>Tratar valores atípicos</legend>
          <p className="recipe-hint">Usa límites IQR de 1.5 con al menos 4 valores finitos. Los null se preservan; valores no finitos cancelan toda la receta. Limitar puede convertir enteros a decimal y rechaza enteros fuera del rango exacto ±2^53 para evitar pérdida de precisión.</p>
          {outlierTreatments.map((treatment, index) => <div className="recipe-row" key={`outlier-${index}`}>
            <label><span>Columna numérica</span><select aria-label={`Columna de outliers ${index + 1}`} value={treatment.column} onChange={(event) => setOutlierTreatments((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{numericColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>
            <label><span>Acción</span><select aria-label={`Acción de outliers ${index + 1}`} value={treatment.action} onChange={(event) => setOutlierTreatments((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, action: event.target.value as OutlierDraft["action"] } : item))}><option value="cap">Limitar a los umbrales</option><option value="drop">Eliminar filas</option></select></label>
            <button type="button" aria-label={`Quitar tratamiento ${index + 1}`} onClick={() => setOutlierTreatments((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
          </div>)}
          {outlierTreatments.length < 16 && <button type="button" className="recipe-add" onClick={() => setOutlierTreatments((current) => [...current, { column: "", action: "cap" }])}>+ Añadir tratamiento</button>}
        </fieldset>

        <fieldset>
          <legend>Resumen agrupado</legend>
          <label className="option-toggle"><input type="checkbox" checked={groupEnabled} onChange={(event) => setGroupEnabled(event.target.checked)} />Reemplazar el dataset por un resumen</label>
          {groupEnabled && <>
            <div className="keep-columns" role="group" aria-label="Columnas para agrupar">{dataset.columns.map((column) => <label key={column.name}><input type="checkbox" checked={groupSummary.groupBy.includes(column.name)} disabled={!groupSummary.groupBy.includes(column.name) && groupSummary.groupBy.length >= 8} onChange={(event) => setGroupSummary((current) => ({ ...current, groupBy: event.target.checked ? dataset.columns.map((item) => item.name).filter((name) => current.groupBy.includes(name) || name === column.name) : current.groupBy.filter((name) => name !== column.name) }))} />{effectiveName(column.name)}</label>)}</div>
            {groupSummary.aggregations.map((aggregation, index) => {
              const dtype = effectiveType(aggregation.column);
              const numeric = ["Int64", "Float64"].includes(dtype);
              const orderable = numeric || ["String", "Date", "Datetime"].includes(dtype);
              return <div className="recipe-row" key={`aggregation-${index}`}><label><span>Columna</span><select aria-label={`Columna de agregación ${index + 1}`} value={aggregation.column} onChange={(event) => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value, operation: "count" } : item) }))}><option value="">Selecciona…</option>{dataset.columns.map((column) => <option key={column.name} value={column.name}>{effectiveName(column.name)}</option>)}</select></label><label><span>Operación</span><select aria-label={`Operación de agregación ${index + 1}`} value={aggregation.operation} onChange={(event) => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.map((item, itemIndex) => itemIndex === index ? { ...item, operation: event.target.value as GroupSummaryDraft["aggregations"][number]["operation"] } : item) }))}>{numeric && <><option value="sum">Suma</option><option value="mean">Promedio</option></>}{orderable && <><option value="min">Mínimo</option><option value="max">Máximo</option></>}<option value="count">Contar filas</option><option value="count_unique">Contar únicos</option></select></label><span className="recipe-output" aria-label={`Salida ${index + 1}`}>{aggregation.column ? `${effectiveName(aggregation.column)}_${aggregation.operation}` : "—"}</span><button type="button" aria-label={`Quitar agregación ${index + 1}`} onClick={() => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.filter((_, itemIndex) => itemIndex !== index) }))}>×</button></div>;
            })}
            {groupSummary.aggregations.length < 32 && <button type="button" className="recipe-add" onClick={() => setGroupSummary((current) => ({ ...current, aggregations: [...current.aggregations, { column: "", operation: "count" }] }))}>+ Añadir agregación</button>}
          </>}
          <p className="recipe-hint">Los grupos null forman un grupo propio. Contar filas incluye null; contar únicos excluye null. Se conserva el orden de primera aparición y el resumen reemplaza la granularidad actual.</p>
        </fieldset>

        <fieldset>
          <legend>Normalizar datos de contacto</legend>
          {contacts.map((contact, index) => <div className="recipe-row" key={`contact-${index}`}><label><span>Columna de texto</span><select aria-label={`Columna de contacto ${index + 1}`} value={contact.column} onChange={(event) => setContacts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label><label><span>Regla</span><select aria-label={`Regla de contacto ${index + 1}`} value={contact.kind} onChange={(event) => setContacts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, kind: event.target.value as ContactDraft["kind"] } : item))}><option value="email">Email: recortar y minúsculas</option><option value="phone">Teléfono: + opcional y dígitos ASCII</option><option value="address">Dirección: compactar espacios</option></select></label><button type="button" aria-label={`Quitar contacto ${index + 1}`} onClick={() => setContacts((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button></div>)}
          {contacts.length < 16 && <button type="button" className="recipe-add" onClick={() => setContacts((current) => [...current, { column: "", kind: "email" }])}>+ Añadir contacto</button>}
          <p className="recipe-hint">Email recorta y pasa a minúsculas; teléfono conserva un + inicial opcional y dígitos ASCII; dirección compacta espacios sin aplicar título.</p>
        </fieldset>

        <fieldset>
          <legend>Extraer texto</legend>
          {extractions.map((extraction, index) => <div className="recipe-row recipe-row--date" key={`extraction-${index}`}><label><span>Origen</span><select aria-label={`Columna de extracción ${index + 1}`} value={extraction.source} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, source: event.target.value } : item))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label><label><span>Extracción</span><select aria-label={`Regla de extracción ${index + 1}`} value={extraction.kind} onChange={(event) => { const kind = event.target.value as ExtractionDraft["kind"]; setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, kind, delimiter: ["before", "after"].includes(kind) ? "" : null } : item)); }}><option value="first_token">Primer token</option><option value="last_token">Último token</option><option value="digits">Dígitos</option><option value="letters">Letras</option><option value="before">Antes de delimitador</option><option value="after">Después de delimitador</option></select></label><label><span>Nombre nuevo</span><input aria-label={`Nombre de extracción ${index + 1}`} value={extraction.name} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.target.value } : item))} /></label>{["before", "after"].includes(extraction.kind) && <label><span>Delimitador literal</span><input aria-label={`Delimitador de extracción ${index + 1}`} value={extraction.delimiter ?? ""} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, delimiter: event.target.value } : item))} /></label>}<button type="button" aria-label={`Quitar extracción ${index + 1}`} onClick={() => setExtractions((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button></div>)}
          {extractions.length < 16 && <button type="button" className="recipe-add" disabled={groupEnabled} onClick={() => setExtractions((current) => [...current, { source: "", kind: "first_token", name: "", delimiter: null }])}>+ Añadir extracción</button>}
          <p className="recipe-hint">Las extracciones crean columnas nuevas desde entradas originales. Antes/después requiere delimitador literal no vacío; se permiten espacios.</p>
          {groupEnabled && <p className="recipe-error">Las extracciones no son compatibles con un resumen agrupado en la misma receta; las normalizaciones de contacto sí.</p>}
        </fieldset>
      </div>

      {renameInvalid && <p className="recipe-error" role="alert">Renombres: completa la columna y su nombre nuevo.</p>}
      {filterInvalid && <p className="recipe-error" role="alert">Filtros: las comparaciones numéricas y de contenido requieren un valor.</p>}
      {calculationInvalid && <p className="recipe-error" role="alert">Columna calculada: completa el nombre, el origen y el operando requerido.</p>}
      {calculationSourceDropped && <p className="recipe-error" role="alert">Columna calculada: conserva la columna origen y la columna usada como operando.</p>}
      {findReplaceInvalid && <p className="recipe-error" role="alert">Buscar y reemplazar: selecciona el alcance y escribe un texto de búsqueda; el reemplazo puede quedar vacío.</p>}
      {keptColumns.length === 0 && <p className="recipe-error" role="alert">Columnas: conserva al menos una columna.</p>}
      {splitInvalid && <p className="recipe-error" role="alert">Dividir: selecciona una columna, un delimitador no vacío y entre 2 y 16 nombres únicos que no colisionen.</p>}
      {mergeInvalid && <p className="recipe-error" role="alert">Combinar: selecciona entre 2 y 16 columnas y usa un nombre nuevo sin colisiones.</p>}
      {sourceConflict && <p className="recipe-error" role="alert">Dependencias: no elimines al dividir una columna que también usarás para combinar.</p>}
      {sourceNotKept && <p className="recipe-error" role="alert">Dependencias: conserva todas las columnas usadas para dividir o combinar.</p>}
      {outlierDuplicate && <p className="recipe-error" role="alert">Outliers: configura una sola acción por columna.</p>}
      {outlierDependencyInvalid && <p className="recipe-error" role="alert">Outliers: conserva cada columna objetivo y no la elimines como fuente antes del tratamiento.</p>}
      {outlierInvalid && !outlierDuplicate && !outlierDependencyInvalid && <p className="recipe-error" role="alert">Outliers: selecciona únicamente columnas numéricas elegibles.</p>}
      {groupInvalid && <p className="recipe-error" role="alert">Resumen: elige entre 1 y 8 claves, agrega al menos una operación, evita pares o salidas duplicadas y conserva todas las columnas utilizadas.</p>}
      {contactDuplicate && <p className="recipe-error" role="alert">Contactos: configura una sola regla por columna.</p>}
      {contactInvalid && !contactDuplicate && <p className="recipe-error" role="alert">Contactos: usa columnas de texto que sobrevivan a la receta.</p>}
      {extractionInvalid && <p className="recipe-error" role="alert">Extracciones: completa entradas y nombres únicos sin colisiones, conserva sus fuentes y no las combines con un resumen.</p>}
      <div className="transform-recipe__footer">
        <p>
          Toda la receta referencia los nombres actuales. Booleano acepta únicamente true/false;
          decimal usa punto y las fechas ambiguas requieren formato explícito.
        </p>
        <button type="button" className="primary-action" onClick={submitRecipe} disabled={recipeBusy || operationCount === 0 || invalid}>
          {busy ? "Aplicando receta…" : "Aplicar receta"}
        </button>
      </div>
      {pendingConfirmation && (
        <div className="sheet-dialog" role="presentation">
          <section className="sheet-dialog__panel" role="alertdialog" aria-modal="true" aria-labelledby="filter-confirm-title" aria-describedby="filter-confirm-description">
            <p className="step">Cambio de alto impacto</p>
            <h3 id="filter-confirm-title">Confirmar cambios de alto impacto</h3>
            <p id="filter-confirm-description">
              {pendingConfirmation.filters.length > 0 && <>La receta aplicará {pendingConfirmation.filters.length} filtros unidos por AND sobre {dataset.rowCount.toLocaleString()} filas actuales. El número final de filas depende de los datos. </>}
              {(pendingConfirmation.keepColumns !== null || pendingConfirmation.splitColumn?.dropSource || pendingConfirmation.mergeColumns?.dropSources) && <> En total se eliminarán {new Set([...(pendingConfirmation.keepColumns ? dataset.columns.map((column) => column.name).filter((name) => !pendingConfirmation.keepColumns?.includes(name)) : []), ...(pendingConfirmation.splitColumn?.dropSource ? [pendingConfirmation.splitColumn.source] : []), ...(pendingConfirmation.mergeColumns?.dropSources ? pendingConfirmation.mergeColumns.sources : [])]).size} columnas originales, sin contar dos veces las fuentes compartidas.</>}
              {pendingConfirmation.outlierTreatments.some((item) => item.action === "cap") && <> Se limitarán valores atípicos en {pendingConfirmation.outlierTreatments.filter((item) => item.action === "cap").length} columnas.</>}
              {pendingConfirmation.outlierTreatments.some((item) => item.action === "drop") && <> Se podrán eliminar filas atípicas detectadas en {pendingConfirmation.outlierTreatments.filter((item) => item.action === "drop").length} columnas.</>}
              {pendingConfirmation.groupSummary && <> El dataset será reemplazado por un resumen de {pendingConfirmation.groupSummary.groupBy.length} claves y {pendingConfirmation.groupSummary.aggregations.length} agregaciones sobre {dataset.rowCount.toLocaleString()} filas actuales.</>}
              {pendingConfirmation.contactNormalizations.length > 0 && <> Se normalizarán valores de contacto en {pendingConfirmation.contactNormalizations.length} columnas.</>}
            </p>
            <div className="sheet-dialog__actions"><button type="button" onClick={() => setPendingConfirmation(null)}>Cancelar</button><button type="button" className="primary-action" onClick={() => { const recipe = pendingConfirmation; setPendingConfirmation(null); onApply(recipe); }}>Confirmar y aplicar</button></div>
          </section>
        </div>
      )}
    </section>
  );
}

function DeliverPhase({
  dataset,
  exportStatus,
  onExport,
  onCancel,
}: {
  dataset: DatasetPreview;
  exportStatus: ExportStatus;
  onExport: (format: ExportFormat) => void;
  onCancel: () => void;
}) {
  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Entregar · Exportación local</p>
          <h2>{dataset.fileName}</h2>
          <p>Genera una copia del dataset preparado. El archivo original nunca se modifica.</p>
        </div>
      </header>
      <DatasetMetrics dataset={dataset} />
      <section className="export-panel" aria-labelledby="export-title">
        <div>
          <p className="step">Formato de entrega</p>
          <h3 id="export-title">Exportar dataset activo</h3>
          <p>El destino solo aparece cuando el archivo está completo.</p>
        </div>
        <div className="export-actions">
          <button type="button" onClick={() => onExport("csv")} disabled={exportStatus.kind === "loading"}>
            Exportar CSV
          </button>
          <button type="button" onClick={() => onExport("parquet")} disabled={exportStatus.kind === "loading"}>
            Exportar Parquet
          </button>
        </div>
      </section>
      {exportStatus.kind === "loading" && (
        <OperationProgressView
          progress={exportStatus.progress}
          cancelRequested={exportStatus.cancelRequested}
          onCancel={onCancel}
        />
      )}
      {exportStatus.kind === "success" && (
        <p className="notice notice--success" role="status">
          {exportStatus.result.format} exportado como {exportStatus.result.fileName} ({readableFileSize(exportStatus.result.fileSizeBytes)}).
        </p>
      )}
      {exportStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo exportar: {exportStatus.message}
        </p>
      )}
    </>
  );
}

function DatasetMetrics({ dataset }: { dataset: DatasetPreview }) {
  return (
    <dl className="metrics" aria-label="Resumen del dataset">
      <div><dt>Filas</dt><dd>{dataset.rowCount.toLocaleString()}</dd></div>
      <div><dt>Columnas</dt><dd>{dataset.columnCount.toLocaleString()}</dd></div>
      <div><dt>Tamaño</dt><dd>{readableFileSize(dataset.fileSizeBytes)}</dd></div>
    </dl>
  );
}


interface OperationProgressViewProps {
  progress: OperationProgress;
  cancelRequested: boolean;
  onCancel: () => void;
}

function OperationProgressView({
  progress,
  cancelRequested,
  onCancel,
}: OperationProgressViewProps) {
  return (
    <div className="operation-progress" role="status" aria-live="polite">
      <div>
        <span>{progress.stage}</span>
        <strong>{progress.percent}%</strong>
      </div>
      <progress
        aria-label={`Progreso: ${progress.stage}`}
        max={100}
        value={progress.percent}
      />
      <button type="button" onClick={onCancel} disabled={cancelRequested}>
        {cancelRequested ? "Cancelando…" : "Cancelar"}
      </button>
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

function DataPreview({
  dataset,
  pageOffset,
  pageLoading,
  pageError,
  onPageChange,
}: DataPreviewProps) {
  const pageEnd = pageOffset + dataset.rows.length;
  const hasPrevious = pageOffset > 0;
  const hasNext = pageEnd < dataset.rowCount;

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
            onClick={() => onPageChange(Math.max(0, pageOffset - PAGE_SIZE))}
            disabled={!hasPrevious || pageLoading}
          >
            Anterior
          </button>
          <button
            type="button"
            onClick={() => onPageChange(pageOffset + PAGE_SIZE)}
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

interface QualityProfileProps {
  profile: DatasetProfile;
}

function QualityProfile({ profile }: QualityProfileProps) {
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

function HistoryBar({
  status,
  busy,
  onUndo,
  onRedo,
}: {
  status: HistoryState;
  busy: boolean;
  onUndo: () => void;
  onRedo: () => void;
}) {
  return (
    <section className="history-bar" aria-label="Continuidad de trabajo">
      <div>
        <strong>Continuidad de trabajo</strong>
        <small>
          {status.snapshotsEnabled
            ? status.entryCount > 0
              ? `Etapa actual: ${status.entries.find((entry) => entry.isCurrent)?.label ?? "Dataset cargado"} · ${status.currentIndex + 1} de ${status.entryCount}`
              : "Todavía no hay etapas guardadas."
            : status.degradedReason ?? "El historial reversible no está disponible."}
        </small>
        {status.entries.length > 0 && (
          <details className="history-details">
            <summary>Ver etapas ({status.entryCount})</summary>
            <ol>
              {status.entries.slice(-12).map((entry) => (
                <li key={entry.index} aria-current={entry.isCurrent ? "step" : undefined}>
                  <span>{entry.label}</span>{entry.isCurrent && <strong>Actual</strong>}
                </li>
              ))}
            </ol>
          </details>
        )}
      </div>
      <div className="history-actions">
        <button type="button" onClick={onUndo} disabled={busy || !status.snapshotsEnabled || !status.canUndo}>
          Deshacer
        </button>
        <button type="button" onClick={onRedo} disabled={busy || !status.snapshotsEnabled || !status.canRedo}>
          Rehacer
        </button>
      </div>
    </section>
  );
}

function ChangeFeedback({ status }: { status: ChangeStatus }) {
  if (status.kind === "idle") return null;

  if (status.kind === "working") {
    const message =
      status.action === "safe"
        ? "Aplicando correcciones recomendadas…"
        : status.action === "duplicates"
        ? "Eliminando duplicados…"
        : status.action === "columns"
          ? "Normalizando nombres de columnas…"
          : status.action === "trim"
            ? "Recortando espacios exteriores…"
            : status.action === "text"
              ? "Normalizando texto seleccionado…"
              : status.action === "transform"
                ? "Aplicando receta estructural…"
                : status.action === "undo"
                  ? "Deshaciendo cambio…"
                  : "Rehaciendo cambio…";
    return (
      <p className="notice" role="status">
        {message}
      </p>
    );
  }

  if (status.kind === "error") {
    return (
      <div className="change-feedback change-feedback--error" role="alert">
        <span>{status.message}</span>
      </div>
    );
  }

  return (
    <div className="change-feedback" role="status">
      <span>{status.message}</span>
    </div>
  );
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
