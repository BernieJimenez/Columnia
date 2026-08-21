import type { ReactNode } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type {
  DatasetStatus,
  LoadInspectionState,
  SheetSelectionAction,
} from "./loadModel";

export type LoadRuntimeState =
  | { kind: "connected" }
  | { kind: "browser" }
  | { kind: "unavailable" };

interface LoadPhaseProps {
  children?: ReactNode;
  runtime: LoadRuntimeState;
  datasetStatus: DatasetStatus;
  inspection: LoadInspectionState;
  onSelect: () => void;
  onSheetAction: (action: SheetSelectionAction) => void;
  onCancelLoad: () => void;
}

export function LoadPhase({
  children,
  runtime,
  datasetStatus,
  inspection,
  onSelect,
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
            runtime.kind !== "connected" ||
            inspection.kind === "inspecting" ||
            datasetStatus.kind === "loading" ||
            inspection.kind === "sheet"
          }
        >
          {inspection.kind === "inspecting"
            ? "Inspeccionando…"
            : current
              ? "Seleccionar otro dataset"
              : "Seleccionar dataset"}
        </button>
      </header>

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
