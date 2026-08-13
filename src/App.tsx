import { useEffect, useState } from "react";

import {
  applySafeCorrections,
  applyTransformRecipe,
  cancelOperation,
  discardDatasetSelection,
  exportDataset,
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  normalizeColumnNames,
  normalizeTextValues,
  loadDatasetSelection,
  pickDatasetSource,
  removeDuplicates,
  redoLastChange,
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
  type SpreadsheetHeaderMode,
  type TransformRecipe,
} from "./bridge";

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

type HistoryStatus = { canUndo: boolean; canRedo: boolean };

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
  const [historyStatus, setHistoryStatus] = useState<HistoryStatus>({ canUndo: false, canRedo: false });
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
      setHistoryStatus({ canUndo: false, canRedo: false });
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
      if (result.affectedRowCount > 0) setHistoryStatus({ canUndo: true, canRedo: false });
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
      if (result.renamedColumnCount > 0) setHistoryStatus({ canUndo: true, canRedo: false });
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
      if (result.changedCellCount > 0) setHistoryStatus({ canUndo: true, canRedo: false });
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
      if (changed) setHistoryStatus({ canUndo: true, canRedo: false });
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
        result.calculatedColumnCount;
      setChangeStatus({
        kind: "applied",
        message:
          total === 0
            ? "La receta no produjo cambios en el dataset."
            : `Receta aplicada: ${result.renamedColumnCount.toLocaleString()} renombres, ${result.convertedColumnCount.toLocaleString()} conversiones, ${result.parsedDateColumnCount.toLocaleString()} fechas interpretadas, ${result.removedRowCount.toLocaleString()} filas filtradas y ${result.calculatedColumnCount.toLocaleString()} columnas calculadas.`,
      });
      if (total > 0) setHistoryStatus({ canUndo: true, canRedo: false });
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
      setHistoryStatus({ canUndo: result.canUndo, canRedo: result.canRedo });
      setChangeStatus({ kind: "applied", message: "Se deshizo el último cambio." });
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
      setHistoryStatus({ canUndo: result.canUndo, canRedo: result.canRedo });
      setChangeStatus({ kind: "applied", message: "Se rehízo el último cambio." });
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
  historyStatus: HistoryStatus;
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
  const datasetSignature = `${dataset.fileName}:${dataset.fileSizeBytes}:${dataset.rowCount}:${dataset.columns.map((column) => `${column.name}:${column.dataType}`).join("|")}`;

  useEffect(() => {
    setRenames([{ from: "", to: "" }]);
    setCasts([{ column: "", target: "string" }]);
    setDateParses([{ column: "", format: "iso8601", target: "date" }]);
    setFilters([]);
    setCalculationEnabled(false);
    setPendingConfirmation(null);
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
  const operationCount = activeRenames.length + activeCasts.length + activeDateParses.length + activeFilters.length + (calculationEnabled ? 1 : 0);
  const renameInvalid = activeRenames.some((item) => !item.from || !item.to.trim());
  const filterInvalid = activeFilters.some((item) =>
    !["eq", "neq", "is_null", "not_null"].includes(item.operator) && !item.value?.trim(),
  );
  const invalid = renameInvalid || filterInvalid ||
    !calculationValid;

  function columnOptions() {
    return dataset.columns.map((column) => (
      <option key={column.name} value={column.name}>
        {column.name}
      </option>
    ));
  }

  function submitRecipe() {
    if (operationCount === 0 || invalid) return;
    const recipe: TransformRecipe = {
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
    };
    if (recipe.filters.length > 0) setPendingConfirmation(recipe);
    else onApply(recipe);
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
      </div>

      {renameInvalid && <p className="recipe-error" role="alert">Renombres: completa la columna y su nombre nuevo.</p>}
      {filterInvalid && <p className="recipe-error" role="alert">Filtros: las comparaciones numéricas y de contenido requieren un valor.</p>}
      {calculationInvalid && <p className="recipe-error" role="alert">Columna calculada: completa el nombre, el origen y el operando requerido.</p>}
      <div className="transform-recipe__footer">
        <p>
          Toda la receta referencia los nombres actuales. Booleano acepta únicamente true/false;
          decimal usa punto y las fechas ambiguas requieren formato explícito.
        </p>
        <button type="button" className="primary-action" onClick={submitRecipe} disabled={busy || operationCount === 0 || invalid}>
          {busy ? "Aplicando receta…" : "Aplicar receta"}
        </button>
      </div>
      {pendingConfirmation && (
        <div className="sheet-dialog" role="presentation">
          <section className="sheet-dialog__panel" role="alertdialog" aria-modal="true" aria-labelledby="filter-confirm-title" aria-describedby="filter-confirm-description">
            <p className="step">Cambio de alto impacto</p>
            <h3 id="filter-confirm-title">Confirmar filtrado de filas</h3>
            <p id="filter-confirm-description">Se aplicarán {pendingConfirmation.filters.length} filtros unidos por AND sobre {dataset.rowCount.toLocaleString()} filas actuales. El número final depende de los datos.</p>
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
  status: HistoryStatus;
  busy: boolean;
  onUndo: () => void;
  onRedo: () => void;
}) {
  return (
    <section className="history-bar" aria-label="Continuidad de trabajo">
      <div>
        <strong>Continuidad de trabajo</strong>
        <small>
          {status.canUndo || status.canRedo
            ? "Columnia conserva una revisión reversible de esta sesión."
            : "Todavía no hay cambios para deshacer o rehacer."}
        </small>
      </div>
      <div className="history-actions">
        <button type="button" onClick={onUndo} disabled={busy || !status.canUndo}>
          Deshacer
        </button>
        <button type="button" onClick={onRedo} disabled={busy || !status.canRedo}>
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
