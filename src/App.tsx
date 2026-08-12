import { useEffect, useState } from "react";

import {
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  pickAndLoadCsv,
  type AppInfo,
  type DatasetPreview,
  type DatasetProfile,
} from "./bridge";

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "browser" }
  | { kind: "error"; message: string };

type DatasetStatus =
  | { kind: "empty" }
  | { kind: "loading" }
  | {
      kind: "ready";
      dataset: DatasetPreview;
      pageOffset: number;
      pageLoading: boolean;
      pageError?: string;
    }
  | { kind: "error"; message: string };

const PAGE_SIZE = 50;

type ProfileStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; profile: DatasetProfile }
  | { kind: "error"; message: string };

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function readableFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function App() {
  const [status, setStatus] = useState<AppStatus>({ kind: "loading" });
  const [datasetStatus, setDatasetStatus] = useState<DatasetStatus>({ kind: "empty" });
  const [profileStatus, setProfileStatus] = useState<ProfileStatus>({ kind: "idle" });

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
    setDatasetStatus({ kind: "loading" });
    setProfileStatus({ kind: "idle" });
    try {
      const dataset = await pickAndLoadCsv();
      setDatasetStatus(
        dataset
          ? { kind: "ready", dataset, pageOffset: 0, pageLoading: false }
          : { kind: "empty" },
      );
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setDatasetStatus({ kind: "error", message });
    }
  }

  async function analyzeQuality() {
    setProfileStatus({ kind: "loading" });
    try {
      const profile = await getDatasetProfile();
      setProfileStatus({ kind: "ready", profile });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setProfileStatus({ kind: "error", message });
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
      <header className="topbar">
        <div>
          <p className="eyebrow">Estación local de datos</p>
          <h1 id="app-title">Columnia</h1>
        </div>
        <div className={`runtime runtime--${status.kind}`} role="status" aria-live="polite">
          {status.kind === "loading" && "Conectando con Rust…"}
          {status.kind === "browser" && "Vista web · motor no conectado"}
          {status.kind === "ready" && `${status.info.version} · ${status.info.platform}`}
          {status.kind === "error" && `Error del motor: ${status.message}`}
        </div>
      </header>

      <section className="workspace" aria-labelledby="workspace-title">
        <div className="workspace__intro">
          <div>
            <p className="step">Dataset activo</p>
            <h2 id="workspace-title">
              {datasetStatus.kind === "ready"
                ? datasetStatus.dataset.fileName
                : "Carga tu primer archivo CSV"}
            </h2>
            <p>
              El archivo se procesa en tu equipo. En este hito se admiten CSV de hasta 100 MB.
            </p>
          </div>
          <button
            className="primary-action"
            type="button"
            onClick={selectCsv}
            disabled={
              !isDesktopReady ||
              datasetStatus.kind === "loading" ||
              profileStatus.kind === "loading"
            }
          >
            {datasetStatus.kind === "loading" ? "Cargando…" : "Seleccionar CSV"}
          </button>
        </div>

        {status.kind === "browser" && (
          <p className="notice">Abre Columnia con Tauri para seleccionar archivos locales.</p>
        )}

        {datasetStatus.kind === "error" && (
          <p className="notice notice--error" role="alert">
            {datasetStatus.message}
          </p>
        )}

        {datasetStatus.kind === "empty" && isDesktopReady && (
          <div className="empty-state">
            <span aria-hidden="true">CSV</span>
            <p>Selecciona un archivo para inspeccionar sus columnas y primeras filas.</p>
          </div>
        )}

        {datasetStatus.kind === "ready" && (
          <DatasetView
            dataset={datasetStatus.dataset}
            pageOffset={datasetStatus.pageOffset}
            pageLoading={datasetStatus.pageLoading || profileStatus.kind === "loading"}
            pageError={datasetStatus.pageError}
            profileStatus={profileStatus}
            onPageChange={changePage}
            onAnalyzeQuality={analyzeQuality}
          />
        )}
      </section>
    </main>
  );
}

interface DatasetViewProps {
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
  profileStatus: ProfileStatus;
  onPageChange: (offset: number) => void;
  onAnalyzeQuality: () => void;
}

function DatasetView({
  dataset,
  pageOffset,
  pageLoading,
  pageError,
  profileStatus,
  onPageChange,
  onAnalyzeQuality,
}: DatasetViewProps) {
  const pageEnd = pageOffset + dataset.rows.length;
  const hasPrevious = pageOffset > 0;
  const hasNext = pageEnd < dataset.rowCount;

  return (
    <div className="dataset">
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
            Calcula duplicados, completitud, valores únicos y estadísticas numéricas y textuales
            sin enviar datos fuera del equipo.
          </p>
        )}
        {profileStatus.kind === "error" && (
          <p className="notice notice--error" role="alert">
            No se pudo calcular el perfil: {profileStatus.message}
          </p>
        )}
        {profileStatus.kind === "ready" && <QualityProfile profile={profileStatus.profile} />}
      </section>
    </div>
  );
}

function QualityProfile({ profile }: { profile: DatasetProfile }) {
  const textColumns = profile.columns.filter((column) => column.emptyCount !== null);

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
      {textColumns.length > 0 && (
        <>
          <h4 className="text-profile-title">Detalle de columnas de texto</h4>
          <div
            className="profile-region profile-region--text"
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
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="profile-note">
            “Vacíos” incluye cadenas sin caracteres o compuestas solamente por espacios.
          </p>
        </>
      )}
    </>
  );
}
