import { useEffect, useState } from "react";

import {
  cancelOperation,
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  pickAndLoadCsv,
  removeDuplicates,
  undoLastChange,
  type AppInfo,
  type CancellableOperation,
  type DatasetPreview,
  type DatasetProfile,
  type OperationProgress,
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
  | { kind: "working"; action: "apply" | "undo" }
  | { kind: "applied"; affectedRowCount: number }
  | { kind: "error"; message: string; canUndo: boolean };

type ActiveView = "data" | "quality";

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
  const [activeView, setActiveView] = useState<ActiveView>("data");

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

  async function selectCsv() {
    const previous = datasetStatus.kind === "ready" ? datasetStatus : undefined;
    setDatasetStatus({
      kind: "loading",
      progress: { operation: "load", stage: "Esperando selección", percent: 0 },
      cancelRequested: false,
      previous,
    });
    setProfileStatus({ kind: "idle" });
    setChangeStatus({ kind: "idle" });
    setActiveView("data");
    try {
      const dataset = await pickAndLoadCsv((progress) => {
        setDatasetStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      });
      setDatasetStatus(
        dataset
          ? { kind: "ready", dataset, pageOffset: 0, pageLoading: false }
          : (previous ?? { kind: "empty" }),
      );
    } catch (error: unknown) {
      if (isCancellationError(error)) {
        setDatasetStatus(previous ?? { kind: "empty" });
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus({ kind: "error", message });
    }
  }

  async function applyDuplicateRemoval() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "apply" });
    try {
      const result = await removeDuplicates();
      setDatasetStatus({
        kind: "ready",
        dataset: result.dataset,
        pageOffset: 0,
        pageLoading: false,
      });
      setProfileStatus({ kind: "idle" });
      setChangeStatus({ kind: "applied", affectedRowCount: result.affectedRowCount });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message, canUndo: false });
    }
  }

  async function undoChange() {
    if (datasetStatus.kind !== "ready") return;

    setChangeStatus({ kind: "working", action: "undo" });
    try {
      const dataset = await undoLastChange();
      setDatasetStatus({ kind: "ready", dataset, pageOffset: 0, pageLoading: false });
      setProfileStatus({ kind: "idle" });
      setChangeStatus({ kind: "idle" });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setChangeStatus({ kind: "error", message, canUndo: true });
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
    } else {
      setProfileStatus((current) =>
        current.kind === "loading" ? { ...current, cancelRequested: true } : current,
      );
    }

    try {
      await cancelOperation(operation);
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      if (operation === "load") {
        setDatasetStatus({ kind: "error", message });
      } else {
        setProfileStatus({ kind: "error", message });
      }
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

  return (
    <main className="shell">
      <aside className="sidebar" aria-label="Navegación principal">
        <div className="brand">
          <p className="eyebrow">Estación local de datos</p>
          <h1 id="app-title">Columnia</h1>
        </div>

        <nav className="side-nav" aria-label="Secciones del dataset">
          <button
            type="button"
            aria-label="Datos"
            className={activeView === "data" ? "side-nav__active" : undefined}
            aria-current={activeView === "data" ? "page" : undefined}
            onClick={() => setActiveView("data")}
          >
            <span>01</span>
            Datos
          </button>
          <button
            type="button"
            aria-label="Calidad"
            className={activeView === "quality" ? "side-nav__active" : undefined}
            aria-current={activeView === "quality" ? "page" : undefined}
            onClick={() => setActiveView("quality")}
            disabled={datasetStatus.kind !== "ready"}
          >
            <span>02</span>
            Calidad
          </button>
        </nav>

        <div className="sidebar__dataset">
          <span>Dataset activo</span>
          <strong>
            {datasetStatus.kind === "ready" ? datasetStatus.dataset.fileName : "Sin dataset"}
          </strong>
        </div>
      </aside>

      <div className="main-content">
        <header className="topbar">
          <div>
            <p className="step">Vista actual</p>
            <p className="page-title">{activeView === "data" ? "Datos" : "Calidad"}</p>
          </div>
          <div className={`runtime runtime--${status.kind}`} role="status" aria-live="polite">
            {status.kind === "loading" && "Conectando con Rust…"}
            {status.kind === "browser" && "Vista web · motor no conectado"}
            {status.kind === "ready" && `${status.info.version} · ${status.info.platform}`}
            {status.kind === "error" && `Error del motor: ${status.message}`}
          </div>
        </header>

        <section
          className={`workspace workspace--${activeView}`}
          aria-label={activeView === "data" ? "Datos del dataset" : "Calidad del dataset"}
        >
          {activeView === "data" && (
            <div className="workspace__intro">
              <div>
                <p className="step">Dataset activo</p>
                <h2>
                  {datasetStatus.kind === "ready"
                    ? datasetStatus.dataset.fileName
                    : "Carga tu primer archivo CSV"}
                </h2>
                <p>
                  Se admiten CSV de hasta 500 MB. Los archivos grandes pueden requerir bastante más
                  memoria mientras completamos el procesamiento por streaming.
                </p>
              </div>
              <button
                className="primary-action"
                type="button"
                onClick={selectCsv}
                disabled={
                  !isDesktopReady ||
                  datasetStatus.kind === "loading" ||
                  profileStatus.kind === "loading" ||
                  changeStatus.kind === "working"
                }
              >
                {datasetStatus.kind === "loading" ? "Cargando…" : "Seleccionar CSV"}
              </button>
            </div>
          )}

          {status.kind === "browser" && (
            <p className="notice">Abre Columnia con Tauri para seleccionar archivos locales.</p>
          )}

          {datasetStatus.kind === "error" && (
            <p className="notice notice--error" role="alert">
              {datasetStatus.message}
            </p>
          )}

          {datasetStatus.kind === "loading" && (
            <OperationProgressView
              progress={datasetStatus.progress}
              cancelRequested={datasetStatus.cancelRequested}
              onCancel={() => cancelActiveOperation("load")}
            />
          )}

          {datasetStatus.kind === "empty" && isDesktopReady && (
            <div className="empty-state">
              <span aria-hidden="true">CSV</span>
              <p>Selecciona un archivo para inspeccionar sus columnas y primeras filas.</p>
            </div>
          )}

          {datasetStatus.kind === "ready" && (
            <DatasetView
              activeView={activeView}
              dataset={datasetStatus.dataset}
              pageOffset={datasetStatus.pageOffset}
              pageLoading={
                datasetStatus.pageLoading ||
                profileStatus.kind === "loading" ||
                changeStatus.kind === "working"
              }
              pageError={datasetStatus.pageError}
              profileStatus={profileStatus}
              changeStatus={changeStatus}
              onPageChange={changePage}
              onAnalyzeQuality={analyzeQuality}
              onCancelOperation={cancelActiveOperation}
              onRemoveDuplicates={applyDuplicateRemoval}
              onUndoChange={undoChange}
            />
          )}
        </section>
      </div>
    </main>
  );
}

interface DatasetViewProps {
  activeView: ActiveView;
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
  profileStatus: ProfileStatus;
  changeStatus: ChangeStatus;
  onPageChange: (offset: number) => void;
  onAnalyzeQuality: () => void;
  onCancelOperation: (operation: CancellableOperation) => void;
  onRemoveDuplicates: () => void;
  onUndoChange: () => void;
}

function DatasetView({
  activeView,
  dataset,
  pageOffset,
  pageLoading,
  pageError,
  profileStatus,
  changeStatus,
  onPageChange,
  onAnalyzeQuality,
  onCancelOperation,
  onRemoveDuplicates,
  onUndoChange,
}: DatasetViewProps) {
  return (
    <div className={`dataset dataset--${activeView}`}>
      {activeView === "data" ? (
        <DataPreview
          dataset={dataset}
          pageOffset={pageOffset}
          pageLoading={pageLoading}
          pageError={pageError}
          onPageChange={onPageChange}
        />
      ) : (
        <>
          <ChangeFeedback status={changeStatus} onUndo={onUndoChange} />
          <section className="quality" aria-labelledby="quality-title">
            <div className="quality__header">
              <div>
                <p className="step">Calidad inicial</p>
                <h3 id="quality-title">Perfil por columna</h3>
              </div>
              <button
                type="button"
                onClick={onAnalyzeQuality}
                disabled={profileStatus.kind === "loading" || profileStatus.kind === "ready"}
              >
                {profileStatus.kind === "loading"
                  ? "Analizando…"
                  : profileStatus.kind === "ready"
                    ? "Perfil listo"
                    : "Analizar calidad"}
              </button>
            </div>

            {profileStatus.kind === "idle" && (
              <p className="quality__hint">
                Calcula duplicados, completitud, valores únicos y estadísticas numéricas y
                textuales y detecta tipos ocultos sin enviar datos fuera del equipo.
              </p>
            )}
            {profileStatus.kind === "loading" && (
              <OperationProgressView
                progress={profileStatus.progress}
                cancelRequested={profileStatus.cancelRequested}
                onCancel={() => onCancelOperation("profile")}
              />
            )}
            {profileStatus.kind === "error" && (
              <p className="notice notice--error" role="alert">
                No se pudo calcular el perfil: {profileStatus.message}
              </p>
            )}
            {profileStatus.kind === "ready" && (
              <QualityProfile
                profile={profileStatus.profile}
                busy={changeStatus.kind === "working"}
                onRemoveDuplicates={onRemoveDuplicates}
              />
            )}
          </section>
        </>
      )}
    </div>
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
      <dl className="metrics" aria-label="Resumen del dataset">
        <div>
          <dt>Filas</dt>
          <dd>{dataset.rowCount.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Columnas</dt>
          <dd>{dataset.columnCount.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Tamaño</dt>
          <dd>{readableFileSize(dataset.fileSizeBytes)}</dd>
        </div>
      </dl>

      <div className="table-region" tabIndex={0} aria-label="Vista previa del CSV">
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
  busy: boolean;
  onRemoveDuplicates: () => void;
}

function QualityProfile({ profile, busy, onRemoveDuplicates }: QualityProfileProps) {
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
          <button
            className="inline-action"
            type="button"
            onClick={onRemoveDuplicates}
            disabled={profile.duplicateRowCount === 0 || busy}
          >
            Eliminar duplicados
          </button>
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

function ChangeFeedback({ status, onUndo }: { status: ChangeStatus; onUndo: () => void }) {
  if (status.kind === "idle") return null;

  if (status.kind === "working") {
    return (
      <p className="notice" role="status">
        {status.action === "apply" ? "Eliminando duplicados…" : "Deshaciendo cambio…"}
      </p>
    );
  }

  if (status.kind === "error") {
    return (
      <div className="change-feedback change-feedback--error" role="alert">
        <span>{status.message}</span>
        {status.canUndo && (
          <button type="button" onClick={onUndo}>
            Reintentar deshacer
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="change-feedback" role="status">
      <span>
        Se eliminaron {status.affectedRowCount.toLocaleString()} filas duplicadas adicionales.
      </span>
      <button type="button" onClick={onUndo}>
        Deshacer
      </button>
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
