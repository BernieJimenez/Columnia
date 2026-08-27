import type { ReactNode } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type {
  DatasetStatus,
  LoadInspectionState,
  SheetSelectionAction,
} from "./loadModel";
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
  datasetStatus: DatasetStatus;
  inspection: LoadInspectionState;
  recentDatasets: readonly RecentDataset[];
  onSelect: () => void;
  onSelectRecent: (item: RecentDataset) => void;
  onClearRecent: () => void;
  onRemoveRecent: (id: string) => void;
  onSheetAction: (action: SheetSelectionAction) => void;
  onCancelLoad: () => void;
}

export function LoadPhase({
  children,
  runtime,
  datasetStatus,
  inspection,
  recentDatasets,
  onSelect,
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
  const selectionDisabled = runtime.kind !== "connected" ||
    inspection.kind === "inspecting" ||
    datasetStatus.kind === "loading" ||
    inspection.kind === "sheet";

  return (
    <>
      <header className="phase-header">
        <div>
          <p className="eyebrow">Cargar · Fuente local</p>
          <h2>{current ? "Dataset listo para continuar" : "Selecciona un dataset"}</h2>
          {current && <h3 className="phase-file">{current.fileName}</h3>}
          <p>
            Se admiten CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS sin un límite fijo de tamaño. El procesamiento
            se realiza localmente y la capacidad depende de los recursos disponibles del equipo.
          </p>
        </div>
        <button
          className="primary-action"
          type="button"
          onClick={onSelect}
          disabled={selectionDisabled}
        >
          {inspection.kind === "inspecting"
            ? "Inspeccionando…"
            : current
              ? "Seleccionar otro dataset"
              : "Seleccionar dataset"}
        </button>
      </header>

      {!current && (
        <section className="load-brief" aria-labelledby="load-brief-title">
          <div className="load-brief__lead">
            <div>
              <h3 id="load-brief-title">Trae tus datos a un espacio de trabajo local.</h3>
              <p>Primero inspeccionamos la estructura; después podrás revisar señales, aplicar cambios reversibles y exportar con control.</p>
            </div>
          </div>
          <p className="load-brief__meta">
            <span>7 formatos</span>
            <span aria-hidden="true">·</span>
            <span><strong>Sin límite fijo</strong></span>
            <span aria-hidden="true">·</span>
            <span>Procesamiento local</span>
          </p>
        </section>
      )}

      {recentDatasets.length > 0 && (
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
      {children}
      {current && <DatasetMetrics dataset={current} />}
    </>
  );
}
