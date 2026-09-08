import type { ReactNode } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type {
  DatasetStatus,
  LoadInspectionState,
  SheetSelectionAction,
} from "./loadModel";
import type { SampleDatasetDescriptor } from "../../bridge";
import {
  formatRecentDatasetDate,
  formatRecentDatasetFormat,
  type RecentDataset,
} from "./recentFilesModel";

export type LoadRuntimeState =
  | { kind: "connected" }
  | { kind: "browser" }
  | { kind: "unavailable" };

interface LoadPhaseProps {
  children?: ReactNode;
  runtime: LoadRuntimeState;
  disabled?: boolean;
  datasetStatus: DatasetStatus;
  inspection: LoadInspectionState;
  recentDatasets: readonly RecentDataset[];
  sampleDatasets?: readonly SampleDatasetDescriptor[];
  onSelect: () => void;
  onSelectSample?: (sampleId: string) => void;
  onSelectRecent: (item: RecentDataset) => void;
  onClearRecent: () => void;
  onRemoveRecent: (id: string) => void;
  onSheetAction: (action: SheetSelectionAction) => void;
  onCancelLoad: () => void;
}

export function LoadPhase({
  children,
  runtime,
  disabled = false,
  datasetStatus,
  inspection,
  recentDatasets,
  sampleDatasets = [],
  onSelect,
  onSelectSample = () => undefined,
  onSelectRecent,
  onClearRecent,
  onRemoveRecent,
  onSheetAction,
  onCancelLoad,
}: LoadPhaseProps) {
  const current =
    datasetStatus.kind === "ready"
      ? datasetStatus.dataset
      : datasetStatus.kind === "loading"
        ? datasetStatus.previous?.dataset
        : undefined;
  const sheetSelection = inspection.kind === "sheet" ? inspection : undefined;
  const importError = inspection.kind === "error"
    ? inspection.message
    : sheetSelection?.error;
  const selectionDisabled = disabled || runtime.kind !== "connected" ||
    inspection.kind === "inspecting" ||
    datasetStatus.kind === "loading" ||
    inspection.kind === "sheet";

  const selectionAction = (
    <button className="primary-action" type="button" onClick={onSelect} disabled={selectionDisabled}>
      {inspection.kind === "inspecting" ? "Inspeccionando…" : current ? "Seleccionar otro dataset" : "Seleccionar dataset"}
    </button>
  );

  return (
    <>
      <header className="phase-header">
        <div>
          <p className="eyebrow">Cargar · Fuente local</p>
          <h2>{current ? "Dataset listo para continuar" : "Selecciona un dataset"}</h2>
          {current && <h3 className="phase-file">{current.fileName}</h3>}
          <p>
            {current
              ? "Se admiten CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS sin un límite fijo de tamaño. La capacidad depende de los recursos disponibles del equipo."
              : "Revisa, prepara y exporta tus datos desde un solo espacio, en tu equipo."}
          </p>
        </div>
        {current && selectionAction}
      </header>

      {current && runtime.kind === "connected" && (
        <p className="load-drop-hint" role="note">
          También puedes arrastrar un archivo compatible a esta ventana.
        </p>
      )}

      {!current && (
        <section className="load-brief" aria-labelledby="load-brief-title">
          <svg className="load-brief__illustration" viewBox="0 0 180 144" fill="none" aria-hidden="true">
            <rect x="16" y="12" width="148" height="120" rx="8" fill="var(--surface-raised)" stroke="currentColor" />
            <path d="M16 44h148M16 73h148M16 102h148M65 44v88M115 44v88" stroke="currentColor" opacity=".35" />
            <path d="M30 28h20m29 0h20m29 0h20" stroke="currentColor" strokeWidth="4" strokeLinecap="round" />
            <rect x="116" y="86" width="48" height="48" rx="8" fill="var(--accent)" />
            <path d="M140 121V99m-8 8 8-8 8 8" stroke="var(--surface)" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <div className="load-brief__lead">
            <div>
              <h3 id="load-brief-title">Trae tus datos a un espacio de trabajo local.</h3>
              <p>Selecciona un archivo{runtime.kind === "connected" ? " o arrástralo a esta ventana" : " desde la aplicación de escritorio"}. Conservamos el original mientras trabajas.</p>
              {selectionAction}
              <p className="load-brief__formats">CSV · TSV · TXT · JSON · Parquet · Excel · ODS</p>
            </div>
          </div>
          <p className="load-brief__meta">
            <span>7 formatos</span>
            <span aria-hidden="true">·</span>
            <span><strong>Sin límite fijo</strong></span>
            <span aria-hidden="true">·</span>
            <span>Procesamiento local</span>
          </p>
          <p className="load-brief__capacity">La capacidad depende de la memoria y el espacio disponibles en tu equipo.</p>
        </section>
      )}

      {!current && runtime.kind === "connected" && sampleDatasets.length > 0 && (
        <section className="sample-datasets" aria-labelledby="sample-datasets-title">
          <div>
            <p className="eyebrow">Sin preparar archivos</p>
            <h3 id="sample-datasets-title">Explora con un dataset de ejemplo</h3>
            <p>Los ejemplos se crean localmente en la carpeta de datos de Columnia; no se descargan ni se envían.</p>
          </div>
          <div className="sample-datasets__list">
            {sampleDatasets.map((sample) => (
              <button
                key={sample.id}
                type="button"
                className="sample-datasets__item"
                onClick={() => onSelectSample(sample.id)}
                disabled={selectionDisabled}
              >
                <strong>{sample.name}</strong>
                <span>{sample.description}</span>
                <small>{sample.format.toUpperCase()}</small>
              </button>
            ))}
          </div>
        </section>
      )}

      {recentDatasets.length > 0 && (
        <details className="load-secondary">
          <summary>
            <span>Retomar un archivo reciente</span>
            <small>{recentDatasets.length} {recentDatasets.length === 1 ? "archivo guardado" : "archivos guardados"}</small>
          </summary>
          <section className="recent-datasets" aria-labelledby="recent-datasets-title">
          <div className="recent-datasets__heading">
            <div>
              <p className="eyebrow">Historial local</p>
              <h3 id="recent-datasets-title">Archivos recientes</h3>
              <p>Solo guardamos el nombre y el formato. Al elegir uno se abrirá el selector nativo de archivos.</p>
            </div>
            <button type="button" className="secondary-action" onClick={onClearRecent}>
              Limpiar historial
            </button>
          </div>
          <ul className="recent-datasets__list">
            {recentDatasets.map((item) => (
              <li key={item.id} className="recent-datasets__item">
                <div className="recent-datasets__copy">
                  <strong title={item.fileName}>{item.fileName}</strong>
                  <span>{formatRecentDatasetFormat(item.format)} · {formatRecentDatasetDate(item.lastOpenedAt)}</span>
                </div>
                <div className="recent-datasets__actions">
                  <button
                    type="button"
                    className="inline-action"
                    onClick={() => onSelectRecent(item)}
                    disabled={selectionDisabled}
                    title={selectionDisabled && runtime.kind !== "connected"
                      ? "Disponible al ejecutar Columnia con Tauri"
                      : undefined}
                  >
                    Elegir de nuevo
                  </button>
                  <button
                    type="button"
                    className="recent-datasets__remove"
                    onClick={() => onRemoveRecent(item.id)}
                    aria-label={`Quitar ${item.fileName} del historial`}
                  >
                    Quitar
                  </button>
                </div>
              </li>
            ))}
          </ul>
          </section>
        </details>
      )}

      {datasetStatus.kind === "loading" && (
        <OperationProgressView
          progress={datasetStatus.progress}
          cancellation={datasetStatus.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel: onCancelLoad }}
        />
      )}
      {inspection.kind === "inspecting" && (
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
        <ModalDialog
          role="dialog"
          labelledBy="sheet-title"
          describedBy="sheet-description"
          onDismiss={() => onSheetAction({ kind: "cancelled" })}
        >
          <p className="eyebrow">Libro seleccionado</p>
          <h3 id="sheet-title">Elegir hoja de {sheetSelection.source.fileName}</h3>
          <p id="sheet-description">Columnia cargará únicamente la hoja elegida y conservará el dataset activo hasta terminar.</p>
          {sheetSelection.source.isCompressedContainer && (
            <p className="notice" role="note">
              Los libros comprimidos pueden ocupar bastante más memoria al abrirse que su tamaño en disco.
              Cierra otras aplicaciones si el archivo es grande.
            </p>
          )}
          <label htmlFor="workbook-sheet">Hoja</label>
          <select
            id="workbook-sheet"
            value={sheetSelection.selectedSheetId}
            onChange={(event) => onSheetAction({ kind: "sheet_changed", sheetId: event.target.value })}
          >
            {sheetSelection.source.sheets.map((sheet) => (
              <option key={sheet.id} value={sheet.id}>{sheet.name}</option>
            ))}
          </select>
          <fieldset className="sheet-dialog__options">
            <legend>Encabezados</legend>
            <label>
              <input
                type="radio"
                name="spreadsheet-header-mode"
                checked={sheetSelection.headerMode === "firstRow"}
                onChange={() => onSheetAction({ kind: "header_mode_changed", headerMode: "firstRow" })}
              />
              Usar la primera fila como encabezados
            </label>
            <label>
              <input
                type="radio"
                name="spreadsheet-header-mode"
                checked={sheetSelection.headerMode === "generated"}
                onChange={() => onSheetAction({ kind: "header_mode_changed", headerMode: "generated" })}
              />
              Generar encabezados (column_1, column_2…)
            </label>
          </fieldset>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => onSheetAction({ kind: "cancelled" })}>Cancelar</button>
            <button
              type="button"
              className="primary-action"
              onClick={() => onSheetAction({ kind: "confirmed" })}
              disabled={!sheetSelection.selectedSheetId}
            >
              Cargar hoja
            </button>
          </div>
        </ModalDialog>
      )}
      {runtime.kind === "browser" && (
        <p className="notice" role="status">
          Abre Columnia con Tauri para seleccionar archivos locales.
        </p>
      )}
      {children && (
        <details className="load-secondary">
          <summary>
            <span>Continuar un proyecto</span>
            <small>Guardar o retomar un espacio de trabajo</small>
          </summary>
          <div className="load-secondary__content">{children}</div>
        </details>
      )}
      {current && <DatasetMetrics dataset={current} />}
    </>
  );
}
