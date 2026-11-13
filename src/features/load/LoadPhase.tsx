import { useEffect, useEffectEvent, useRef, useState, type ReactNode } from "react";

import { renderCellValue } from "../../components/CellText";
import { ModalDialog } from "../../components/ModalDialog";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics } from "../delivery/DatasetMetrics";
import type {
  DatasetStatus,
  LoadInspectionState,
  ProfileReviewAction,
  ResourcePreflightAction,
  SchemaMismatchAction,
  SheetSelectionAction,
} from "./loadModel";
import type {
  DatasetSourceInspection,
  DelimitedHeaderModePreview,
  ImportDateConvention,
  ImportNumberConvention,
  ImportProfileMismatch,
  SampleDatasetDescriptor,
  SourceTextEncoding,
} from "../../bridge";
import { dateLikeTextColumns, legacyEncodingExample, looksLikeTitleRow } from "./loadModel";
import { DATE_CONVENTIONS, NUMBER_CONVENTIONS } from "./importProfile";
import type { DatabaseRead } from "./importOrigin";
import { plural } from "../../plural";
import {
  formatRecentDatasetDate,
  formatRecentDatasetFormat,
  type RecentDataset,
} from "./recentFilesModel";
import { busyDescribedBy } from "../../busyReasons";
import { formatBytes, formatDataType } from "../../format";
import { projectDate } from "../projects/projectModel";
import { preloadSourcePanel, preloadSourcePanelsWhenIdle } from "./secondaryPanels";

export type LoadRuntimeState =
  | { kind: "connected" }
  | { kind: "browser" }
  | { kind: "unavailable"; error?: string };

type LoadSource = "file" | "database" | "sample" | "project";

interface LoadSourceOption {
  id: LoadSource;
  label: string;
  description: string;
}

function formatFileSize(bytes: number): string {
  return formatBytes(bytes);
}

interface LoadPhaseProps {
  children?: ReactNode;
  reusableTaskPanel?: ReactNode;
  /** «Cargar desde una base de datos», shown when that source is chosen. */
  databaseImportPanel?: ReactNode;
  /** The selection under review was read from a database table: the dialog names the table, not the local copy. */
  databaseRead?: DatabaseRead | null;
  pendingTaskName?: string | null;
  runtime: LoadRuntimeState;
  disabled?: boolean;
  datasetStatus: DatasetStatus;
  inspection: LoadInspectionState;
  recentDatasets: readonly RecentDataset[];
  sampleDatasets?: readonly SampleDatasetDescriptor[];
  onSelect: () => void;
  /** «Continuar a Revisar» once a dataset is loaded: the one primary action of the view. */
  onContinue?: () => void;
  continueDisabled?: boolean;
  onSelectSample?: (sampleId: string) => void;
  onSelectRecent: (item: RecentDataset) => void;
  onClearRecent: () => void;
  onRemoveRecent: (id: string) => void;
  onSheetAction: (action: SheetSelectionAction) => void;
  onRetryHeaderPreview?: () => void;
  /** Approves reading a Windows-1252 file through a UTF-8 copy (RV20). */
  onConvertEncoding?: () => void;
  /** PROD-02: read the selection again with another separator or encoding. */
  onReinterpret?: (delimiter: string | null, encoding: SourceTextEncoding) => void;
  encodingConversionPending?: boolean;
  /** DAT-01: the previous session ended without a normal exit. */
  previousExitUnclean?: boolean;
  /** The most recently saved project; null when none exists, undefined while unknown (DAT-01). */
  lastSavedProject?: { name: string; updatedAt: string } | null;
  onDismissPreviousExit?: () => void;
  /** PROD-14: failure reports written since the previous session started. */
  recentCrashReports?: number;
  onDismissCrashReports?: () => void;
  /** ARQ-02: catalogs set aside at start-up because they could not be opened. */
  setAsideCatalogs?: string[];
  onDismissSetAsideCatalogs?: () => void;
  onProfileReviewAction?: (action: ProfileReviewAction) => void;
  onResourcePreflightAction?: (action: ResourcePreflightAction) => void;
  onSchemaMismatchAction?: (action: SchemaMismatchAction) => void;
  onCancelLoad: () => void;
  workbookInspectionCancellationPending?: boolean;
  onCancelWorkbookInspection?: () => void;
  onRetrySelectionCancellation?: () => void;
}

export function LoadPhase({
  children,
  reusableTaskPanel,
  databaseImportPanel,
  databaseRead = null,
  pendingTaskName = null,
  runtime,
  disabled = false,
  datasetStatus,
  inspection,
  recentDatasets,
  sampleDatasets = [],
  onSelect,
  onContinue,
  continueDisabled = false,
  onSelectSample = () => undefined,
  onSelectRecent,
  onClearRecent,
  onRemoveRecent,
  onSheetAction,
  onRetryHeaderPreview = () => undefined,
  onConvertEncoding = () => undefined,
  onReinterpret = () => undefined,
  encodingConversionPending = false,
  previousExitUnclean = false,
  lastSavedProject,
  onDismissPreviousExit = () => undefined,
  setAsideCatalogs = [],
  onDismissSetAsideCatalogs = () => undefined,
  recentCrashReports = 0,
  onDismissCrashReports = () => undefined,
  onProfileReviewAction = () => undefined,
  onResourcePreflightAction = () => undefined,
  onSchemaMismatchAction = () => undefined,
  onCancelLoad,
  workbookInspectionCancellationPending = false,
  onCancelWorkbookInspection = () => undefined,
  onRetrySelectionCancellation = () => undefined,
}: LoadPhaseProps) {
  const current =
    datasetStatus.kind === "ready"
      ? datasetStatus.dataset
      : datasetStatus.kind === "loading"
        ? datasetStatus.previous?.dataset
        : undefined;
  const sheetSelection = inspection.kind === "sheet" ? inspection : undefined;
  const profileReview = inspection.kind === "profile_review" ? inspection : undefined;
  const resourcePreflight = inspection.kind === "resource_preflight" ? inspection : undefined;
  const schemaMismatch = inspection.kind === "schema_mismatch" ? inspection : undefined;
  const isDelimitedSelection = sheetSelection?.source.format === "csv" || sheetSelection?.source.format === "tsv";
  const hasHeaderSelection = isDelimitedSelection || sheetSelection?.source.format === "excel";
  const importError = inspection.kind === "error"
    ? inspection.message
    : undefined;
  const selectionCancellationPending = inspection.kind === "selection_cancelling";
  const selectionDisabled = disabled || runtime.kind !== "connected" ||
    inspection.kind === "inspecting" ||
    inspection.kind === "workbook_inspecting" ||
    selectionCancellationPending ||
    inspection.kind === "selection_cancellation_failed" ||
    datasetStatus.kind === "loading" ||
    inspection.kind === "sheet" ||
    inspection.kind === "profile_review" ||
    inspection.kind === "resource_preflight" ||
    inspection.kind === "schema_mismatch";

  const showSamples = runtime.kind === "connected" && sampleDatasets.length > 0;
  const sourceOptions: LoadSourceOption[] = [
    { id: "file", label: "Archivo", description: "CSV, Excel, Parquet, JSON y más" },
    ...(databaseImportPanel ? [{ id: "database" as const, label: "Base de datos", description: "SQL Server, PostgreSQL o MySQL; solo lectura" }] : []),
    ...(showSamples ? [{ id: "sample" as const, label: "Ejemplo", description: "Datos de prueba creados en este equipo" }] : []),
    ...(children ? [{ id: "project" as const, label: "Proyecto", description: current ? "Guarda o retoma un trabajo" : "Retoma un trabajo guardado" }] : []),
  ];
  // After an unexpected exit the saved projects come first (DAT-01).
  const [chosenSource, setChosenSource] = useState<LoadSource | null>(null);
  const preferredSource = chosenSource ?? (previousExitUnclean && !current && children ? "project" : "file");
  const source = sourceOptions.some((option) => option.id === preferredSource) ? preferredSource : "file";
  useEffect(() => preloadSourcePanelsWhenIdle(), []);

  // The table the import dialog is reviewing, when the selection came from a database.
  const reviewedTable = sheetSelection && databaseRead?.selectionId === sheetSelection.source.selectionId
    ? databaseRead
    : null;

  // An error of the file source stays by its action and leaves when another
  // source is chosen; a new attempt shows it again even with the same text.
  const loadErrorKey = `${datasetStatus.kind === "error" ? datasetStatus.message : ""}\u0000${importError ?? ""}`;
  const [leftBehindErrorKey, setLeftBehindErrorKey] = useState<string | null>(null);
  const visibleErrorKey = loadErrorKey !== "\u0000" && loadErrorKey !== leftBehindErrorKey ? loadErrorKey : null;
  const selectButtonRef = useRef<HTMLButtonElement>(null);
  const loadErrorsRef = useRef<HTMLDivElement>(null);
  // A new error takes the focus only when nothing else had it: after the file
  // picker or the import dialog the focus is back on the button or on <body>.
  useEffect(() => {
    if (visibleErrorKey === null) return;
    const active = document.activeElement;
    if (active === null || active === document.body || active === selectButtonRef.current) {
      loadErrorsRef.current?.focus();
    }
  }, [visibleErrorKey]);
  const loadErrors = visibleErrorKey !== null && (
    <div className="load-errors" ref={loadErrorsRef} tabIndex={-1}>
      {datasetStatus.kind === "error" && (
        <div className="notice notice--error" role="alert">
          <p>No se pudo cargar el archivo: {datasetStatus.message}</p>
          <p>No se cargó nada. Comprueba que el archivo sigue en su sitio y no está abierto en otra aplicación, y vuelve a elegirlo.</p>
        </div>
      )}
      {importError && (
        <div className="notice notice--error" role="alert">
          <p>No se pudo importar el archivo: {importError}</p>
          <p>
            {current ? "Tu dataset actual no cambió. " : ""}
            Comprueba que el archivo sigue en su sitio y no está abierto en otra aplicación, y vuelve a elegirlo.
          </p>
        </div>
      )}
    </div>
  );
  const errorsBySelectButton = source === "file";

  function chooseFile() {
    if (selectionDisabled) return;
    setLeftBehindErrorKey(null);
    onSelect();
  }

  // Ctrl+O opens «Elegir archivo» while that button is on screen and usable.
  const fileShortcutAvailable = source === "file" && !selectionDisabled;
  const chooseFileFromShortcut = useEffectEvent(chooseFile);
  useEffect(() => {
    if (!fileShortcutAvailable) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || !event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
      if (event.key.toLowerCase() !== "o") return;
      const target = event.target;
      // Text fields keep their own shortcuts, and an open dialog owns the keyboard.
      if (target instanceof HTMLElement && (
        target.isContentEditable || target.closest("input, textarea, select, [contenteditable], dialog")
      )) return;
      event.preventDefault();
      chooseFileFromShortcut();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [fileShortcutAvailable]);

  const selectionAction = (
    // aria-disabled (not disabled) keeps the button reachable by keyboard so its reason is read; onClick ignores it.
    <button
      ref={selectButtonRef}
      className={current ? "secondary-action" : "primary-action"}
      type="button"
      onClick={chooseFile}
      aria-disabled={selectionDisabled || undefined}
      aria-describedby={busyDescribedBy(disabled)}
      aria-keyshortcuts="Control+O"
    >
      {inspection.kind === "inspecting" ? "Comprobando el archivo…" : current ? "Elegir otro archivo" : "Elegir archivo"}
    </button>
  );

  return (
    <>
      <header className="phase-header">
        <div>
          <h2>{current ? "Dataset listo para continuar" : "Carga un dataset"}</h2>
          {current && <h3 className="phase-file">{current.fileName}</h3>}
        </div>
        {current && onContinue && (
          <button
            type="button"
            className="primary-action"
            onClick={onContinue}
            disabled={continueDisabled}
            aria-describedby={continueDisabled ? "busy-reason" : undefined}
          >
            Continuar a Revisar
          </button>
        )}
      </header>
      {current && <DatasetMetrics dataset={current} />}

      {!errorsBySelectButton && loadErrors}

      {runtime.kind === "unavailable" && runtime.error && (
        <div className="notice notice--error" role="alert">
          <p>El motor de Columnia no responde: {runtime.error}</p>
          <p>Cierra y vuelve a abrir Columnia. Si se repite, crea un informe de diagnóstico en Preferencias.</p>
        </div>
      )}

      {previousExitUnclean && !current && (
        <section className="notice" aria-labelledby="previous-exit-title">
          <h3 id="previous-exit-title">La sesión anterior se cerró de forma inesperada</h3>
          {lastSavedProject ? (
            <p>
              Último guardado: {projectDate(lastSavedProject.updatedAt)} ({lastSavedProject.name}). Los cambios
              posteriores no se conservaron; puedes recuperarlo en la tarjeta «Proyecto» de más abajo.
            </p>
          ) : lastSavedProject === null ? (
            <p>No había ningún proyecto guardado, así que los cambios de esa sesión no se conservaron.</p>
          ) : (
            <p>Los cambios que no estaban guardados en un proyecto no se conservaron. Si guardaste uno, puedes recuperarlo en la tarjeta «Proyecto» de más abajo.</p>
          )}
          <button type="button" className="secondary-action" onClick={onDismissPreviousExit}>Entendido</button>
        </section>
      )}

      {recentCrashReports > 0 && (
        <section className="notice" aria-labelledby="crash-reports-title">
          <h3 id="crash-reports-title">Columnia tuvo un fallo interno</h3>
          <p>
            Desde la sesión anterior se guardó {recentCrashReports === 1 ? "1 informe de fallo" : `${recentCrashReports.toLocaleString()} informes de fallo`}.
            Están en la carpeta de datos de Columnia, en <code>crash-reports</code>; solo dicen la versión, la hora y el
            archivo del código donde ocurrió, así que no incluyen tus datos. Si el fallo se repite, guárdalos para revisarlo.
          </p>
          <button type="button" className="secondary-action" onClick={onDismissCrashReports}>Entendido</button>
        </section>
      )}

      {setAsideCatalogs.length > 0 && (
        <section className="notice" aria-labelledby="set-aside-catalogs-title">
          <h3 id="set-aside-catalogs-title">Se apartó un catálogo que no se podía abrir</h3>
          <p>
            Columnia empezó con {setAsideCatalogs.length === 1 ? "un catálogo nuevo y vacío" : "catálogos nuevos y vacíos"} de
            tareas o presets de entrega. Tus proyectos no se tocaron. El archivo anterior se conserva en:
          </p>
          <ul>
            {setAsideCatalogs.map((path) => <li key={path}><code>{path}</code></li>)}
          </ul>
          <button type="button" className="secondary-action" onClick={onDismissSetAsideCatalogs}>Entendido</button>
        </section>
      )}

      {current && <h3 className="load-change-title">Cambiar de datos</h3>}

      {sourceOptions.length > 1 && (
        <fieldset className="choice-cards load-sources">
          <legend>¿De dónde vienen los datos?</legend>
          <div className="choice-cards__options">
            {sourceOptions.map((option) => (
              <label
                key={option.id}
                className="choice-card"
                data-selected={source === option.id || undefined}
                onMouseEnter={() => preloadSourcePanel(option.id)}
                onFocus={() => preloadSourcePanel(option.id)}
              >
                <input
                  type="radio"
                  name="load-source"
                  value={option.id}
                  checked={source === option.id}
                  onChange={() => { setChosenSource(option.id); setLeftBehindErrorKey(loadErrorKey); }}
                />
                <span>
                  <strong>{option.label}</strong>
                  <small>{option.description}</small>
                </span>
              </label>
            ))}
          </div>
        </fieldset>
      )}

      {source === "file" && current && (
        <div className="load-change">
          {selectionAction}
          {loadErrors}
          {runtime.kind === "connected" && (
            <p className="load-drop-hint">
              También puedes arrastrar un archivo compatible a esta ventana. Atajo: Ctrl+O.
            </p>
          )}
        </div>
      )}

      {source === "file" && !current && (
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
              <h3 id="load-brief-title">{runtime.kind === "connected" ? "Elige un archivo o arrástralo a esta ventana" : "Elige un archivo"}</h3>
              <p>Columnia trabaja sobre una copia: el original no se modifica.</p>
              {selectionAction}
              {loadErrors}
              <p className="load-brief__formats">
                CSV · TSV · TXT · JSON · Parquet · Excel · ODS.{runtime.kind === "connected" && " Atajo: Ctrl+O."}<br />
                Los CSV, TSV, TXT, JSON, Parquet y Excel (.xlsx, .xlsb) de 512 MiB o más se leen por bloques desde el disco; en los CSV, TSV y TXT, las fechas y los números no se convierten al cargar.
              </p>
            </div>
          </div>
        </section>
      )}

      {source === "sample" && (
        <section className="sample-datasets" aria-labelledby="sample-datasets-title">
          <div>
            <h3 id="sample-datasets-title">Explora con un dataset de ejemplo</h3>
            <p>Los ejemplos se crean localmente en la carpeta de datos de Columnia; no se descargan ni se envían.</p>
          </div>
          {/* The same rows as «Archivos recientes»: name, one line of detail and the action. */}
          <ul className="recent-datasets__list">
            {sampleDatasets.map((sample) => (
              <li key={sample.id} className="recent-datasets__item">
                <div className="recent-datasets__copy">
                  <strong>{sample.name}</strong>
                  <span>{sample.description} · {sample.format.toUpperCase()}</span>
                </div>
                <div className="recent-datasets__actions">
                  <button
                    type="button"
                    className="inline-action"
                    onClick={() => { setLeftBehindErrorKey(null); onSelectSample(sample.id); }}
                    disabled={selectionDisabled}
                    aria-label={`Cargar el ejemplo ${sample.name}`}
                    aria-describedby={busyDescribedBy(disabled)}
                  >
                    Cargar
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}

      {source === "file" && recentDatasets.length > 0 && (
        <details className="load-secondary">
          <summary>
            <span>Archivos recientes</span>
            <small>{recentDatasets.length} {recentDatasets.length === 1 ? "nombre recordado" : "nombres recordados"}</small>
          </summary>
          <section className="recent-datasets" aria-labelledby="recent-datasets-title">
          <div className="recent-datasets__heading">
            <div>
              <h3 id="recent-datasets-title" className="visually-hidden">Archivos recientes</h3>
              <p>
                Columnia recuerda el nombre y el formato, no la ubicación.
                «Buscar en el equipo» abre el selector de archivos para que lo elijas de nuevo.
              </p>
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
                    onClick={() => { setLeftBehindErrorKey(null); onSelectRecent(item); }}
                    disabled={selectionDisabled}
                    aria-label={`Buscar en el equipo: ${item.fileName}`}
                    aria-describedby={busyDescribedBy(disabled)}
                    title={selectionDisabled && runtime.kind !== "connected"
                      ? "Disponible en la aplicación de escritorio"
                      : undefined}
                  >
                    Buscar en el equipo
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

      {source === "database" && databaseImportPanel}
      {source === "project" && <div className="load-source-panel">{children}</div>}
      {reusableTaskPanel}

      {datasetStatus.kind === "loading" && (
        <OperationProgressView
          progress={datasetStatus.progress}
          cancellation={datasetStatus.cancelRequested
            ? { kind: "requested" }
            : {
                kind: "available",
                onCancel: onCancelLoad,
                ...(datasetStatus.cancellationError ? { error: datasetStatus.cancellationError } : {}),
              }}
        />
      )}
      {inspection.kind === "inspecting" && inspection.origin !== "database" && (
        <p className="notice" role="status">Esperando el archivo y comprobando su formato…</p>
      )}
      {inspection.kind === "workbook_inspecting" && (
        <div className="notice" role="status">
          <p>Leyendo las hojas de {inspection.source.fileName}. Si cancelas, la lectura puede terminar antes de detenerse.</p>
          <button
            type="button"
            className="secondary-action"
            onClick={onCancelWorkbookInspection}
            disabled={workbookInspectionCancellationPending}
          >
            {workbookInspectionCancellationPending ? "Cancelando inspección…" : "Cancelar inspección"}
          </button>
        </div>
      )}
      {selectionCancellationPending && (
        <p className="notice" role="status">
          Cancelando la selección de {inspection.source.fileName}…
        </p>
      )}
      {inspection.kind === "selection_cancellation_failed" && (
        <div className="notice notice--error" role="alert">
          <p>
            No se pudo cancelar la selección de {inspection.source.fileName}. Tu dataset actual no cambió.
            Reintenta para detener la lectura.
          </p>
          <p>{inspection.message}</p>
          <button
            type="button"
            className="secondary-action"
            onClick={onRetrySelectionCancellation}
          >
            Reintentar cancelación
          </button>
        </div>
      )}
      {profileReview && (
        <ModalDialog
          role="dialog"
          labelledBy="import-profile-title"
          describedBy="import-profile-description"
          onDismiss={() => onProfileReviewAction({ kind: "cancelled" })}
        >
          <h3 id="import-profile-title">¿Usar el perfil de importación guardado?</h3>
          <p id="import-profile-description">
            El perfil guarda cómo se leyó un archivo {profileReview.profile.format.toUpperCase()} de {plural(profileReview.profile.schema.length, "columna", "columnas")}.
            Columnia lo compara con este archivo antes de reemplazar el dataset activo.
          </p>
          {pendingTaskName && (
            <p className="notice" role="note">
              El perfil de “{pendingTaskName}” se usará en esta importación. La receta, las reglas y la salida se ofrecerán para aplicar después de cargar.
            </p>
          )}
          <ResourceEstimateSummary source={profileReview.source} />
          <p className="notice" role="note">
            Este formato no usa convenciones de CSV/TSV; sus valores se conservan como aparecen en el archivo.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => onProfileReviewAction({ kind: "cancelled" })}>Cancelar</button>
            <button type="button" className="secondary-action" onClick={() => onProfileReviewAction({ kind: "use_defaults" })}>Importar sin perfil</button>
            <button type="button" className="primary-action" onClick={() => onProfileReviewAction({ kind: "use_profile" })}>Usar perfil y revisar esquema</button>
          </div>
        </ModalDialog>
      )}
      {resourcePreflight && (
        <ModalDialog
          role="dialog"
          labelledBy="resource-preflight-title"
          describedBy="resource-preflight-description"
          onDismiss={() => onResourcePreflightAction({ kind: "cancelled" })}
        >
          <h3 id="resource-preflight-title">Memoria y disco que usará la carga</h3>
          <p id="resource-preflight-description">
            Estimación antes de abrir {resourcePreflight.source.fileName}. Nada se carga hasta que continúes y el archivo original no se modifica.
          </p>
          <ResourceEstimateSummary source={resourcePreflight.source} />
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => onResourcePreflightAction({ kind: "cancelled" })}>Cancelar</button>
            <button type="button" className="primary-action" onClick={() => onResourcePreflightAction({ kind: "confirmed" })}>Continuar con la carga</button>
          </div>
        </ModalDialog>
      )}
      {schemaMismatch && (
        <ModalDialog
          role="alertdialog"
          labelledBy="import-schema-mismatch-title"
          describedBy="import-schema-mismatch-description"
          onDismiss={() => onSchemaMismatchAction({ kind: "cancelled" })}
        >
          <h3 id="import-schema-mismatch-title">El esquema difiere del perfil guardado</h3>
          <p id="import-schema-mismatch-description">
            Tu dataset actual no cambió: no se convirtió ningún tipo ni se reasignó ninguna columna. Revisa las diferencias antes de decidir.
            {pendingTaskName ? ` La tarea “${pendingTaskName}” no se aplicará sin una confirmación posterior.` : ""}
          </p>
          <SchemaDifferenceList mismatch={schemaMismatch.mismatch} />
          <p className="notice" role="note">
            Si continúas, el archivo se importa con el esquema nuevo. Las convenciones de fecha y número solo convierten una columna si todos sus valores las cumplen; las demás quedan como texto.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => onSchemaMismatchAction({ kind: "cancelled" })}>Cancelar y conservar dataset</button>
            <button type="button" className="primary-action" onClick={() => onSchemaMismatchAction({ kind: "import_new_schema" })}>Importar con esquema nuevo</button>
          </div>
        </ModalDialog>
      )}
      {sheetSelection && (
        <ModalDialog
          role="dialog"
          labelledBy="sheet-title"
          describedBy="sheet-description"
          onDismiss={() => onSheetAction({ kind: "cancelled" })}
        >
          <h3 id="sheet-title">
            {reviewedTable
              ? `Revisar la tabla ${reviewedTable.table}`
              : sheetSelection.source.format === "excel"
                ? `Elegir hoja de ${sheetSelection.source.fileName}`
                : isDelimitedSelection
                  ? `Revisar encabezados de ${sheetSelection.source.fileName}`
                  : `Revisar importación de ${sheetSelection.source.fileName}`}
          </h3>
          <p id="sheet-description">
            {reviewedTable
              ? `Las filas ya están leídas y guardadas en este equipo; el servidor no cambió. Revisa las columnas antes de cargarlas. ${current ? "Tu dataset actual no cambia hasta que confirmes." : "Nada se carga hasta que confirmes."}`
              : sheetSelection.source.format === "excel"
                ? "Elige la hoja y los encabezados. Tu dataset actual no cambia hasta que confirmes."
                : isDelimitedSelection
                  ? "Comprueba cómo se leen las columnas. Tu dataset actual no cambia hasta que confirmes."
                  : "Revisa las columnas antes de importar. Tu dataset actual no cambia hasta que confirmes."}
          </p>
          {sheetSelection.source.isCompressedContainer && (
            <p className="notice" role="note">
              Los libros comprimidos pueden ocupar bastante más memoria al abrirse que su tamaño en disco.
              Cierra otras aplicaciones si el archivo es grande.
            </p>
          )}
          {sheetSelection.source.format === "excel" && (
            <>
              <label htmlFor="workbook-sheet">Hoja</label>
              <select
                id="workbook-sheet"
                value={sheetSelection.selectedSheetId}
                onChange={(event) => onSheetAction({ kind: "sheet_changed", sheetId: event.target.value })}
                disabled={sheetSelection.schemaPreviewLoading === true || Boolean(pendingTaskName && sheetSelection.profileCanBeApplied)}
              >
                {sheetSelection.source.sheets.map((sheet) => (
                  <option key={sheet.id} value={sheet.id}>{sheet.hidden ? `${sheet.name} (oculta)` : sheet.name}</option>
                ))}
              </select>
            </>
          )}
          {sheetSelection.source.format === "excel" && (() => {
            // PROD-19: what the sheet choice alone does not show.
            const hiddenCount = sheetSelection.source.sheets.filter((sheet) => sheet.hidden).length;
            const mergedCount = sheetSelection.source.sheets
              .find((sheet) => sheet.id === sheetSelection.selectedSheetId)?.mergedCellCount ?? 0;
            const skippedTitleRows = sheetSelection.headerMode === "firstRow"
              ? sheetSelection.schemaPreview?.titleRowCount ?? 0
              : 0;
            const titleRow = sheetSelection.headerMode === "firstRow"
              && skippedTitleRows === 0
              && sheetSelection.schemaPreview != null
              && looksLikeTitleRow(sheetSelection.schemaPreview.columns);
            if (hiddenCount === 0 && mergedCount === 0 && !titleRow && skippedTitleRows === 0) return null;
            return (
              <section className="notice" aria-labelledby="workbook-notes-title">
                <h4 id="workbook-notes-title">Avisos del libro</h4>
                <ul>
                  {hiddenCount > 0 && (
                    <li>
                      El libro tiene {hiddenCount === 1 ? "1 hoja oculta" : `${hiddenCount} hojas ocultas`} en Excel; aparece
                      {hiddenCount === 1 ? "" : "n"} en la lista como «(oculta)» y no se elige sin que lo indiques.
                    </li>
                  )}
                  {mergedCount > 0 && (
                    <li>
                      Esta hoja tiene {mergedCount === 1 ? "1 rango de celdas combinadas" : `${mergedCount.toLocaleString()} rangos de celdas combinadas`}:
                      solo la primera celda de cada uno conserva el valor y las demás llegarán nulas.
                    </li>
                  )}
                  {skippedTitleRows > 0 && (
                    <li>
                      Encabezados en la fila {(skippedTitleRows + 1).toLocaleString()}:{" "}
                      {skippedTitleRows === 1 ? "la fila de encima parecía un título y no se carga" : `las ${skippedTitleRows.toLocaleString()} filas de encima parecían un título y no se cargan`}.
                      Si no es así, elige «Generar encabezados» para leer todas las filas como datos.
                    </li>
                  )}
                  {titleRow && (
                    <li>
                      La primera fila parece un título: solo la primera columna tiene nombre. Si los encabezados están más abajo,
                      quita las filas de título en Excel antes de cargar o elige «Generar encabezados».
                    </li>
                  )}
                </ul>
              </section>
            );
          })()}
          {hasHeaderSelection && (
            <fieldset className="sheet-dialog__options">
              <legend>Encabezados</legend>
              <label>
                <input
                  type="radio"
                  name="spreadsheet-header-mode"
                  checked={sheetSelection.headerMode === "firstRow"}
                  onChange={() => onSheetAction({ kind: "header_mode_changed", headerMode: "firstRow" })}
                  disabled={sheetSelection.schemaPreviewLoading === true || Boolean(pendingTaskName && sheetSelection.profileCanBeApplied)}
                />
                {sheetSelection.source.format === "excel"
                  ? "Usar la primera fila como encabezados"
                  : "Usar la primera fila como encabezados y excluirla de los datos"}
              </label>
              <label>
                <input
                  type="radio"
                  name="spreadsheet-header-mode"
                  checked={sheetSelection.headerMode === "generated"}
                  onChange={() => onSheetAction({ kind: "header_mode_changed", headerMode: "generated" })}
                  disabled={sheetSelection.schemaPreviewLoading === true || Boolean(pendingTaskName && sheetSelection.profileCanBeApplied)}
                />
                {sheetSelection.source.format === "excel"
                  ? "Generar encabezados (column_1, column_2…)"
                  : "Conservar la primera fila como datos y generar nombres (column_1, column_2…)"}
              </label>
            </fieldset>
          )}
          {/* No aria-live here: the messages inside are their own status regions,
              and a live summary would reread the whole preview on each change. */}
          <section className="sheet-import-summary sheet-import-summary--plain" aria-labelledby="sheet-import-summary-title">
            <h4 id="sheet-import-summary-title" className="visually-hidden">Resumen antes de cargar</h4>
            <dl className="sheet-import-facts">
              {reviewedTable ? (
                <>
                  <div>
                    <dt>Origen</dt>
                    <dd>{reviewedTable.engine} · {reviewedTable.server}</dd>
                  </div>
                  <div>
                    <dt>Filas leídas</dt>
                    <dd>
                      {reviewedTable.rowsImported.toLocaleString()}
                      {reviewedTable.moreRowsAvailable
                        ? ` (la tabla tiene más; el límite era de ${plural(reviewedTable.rowLimit, "fila", "filas")})`
                        : " (la tabla completa)"}
                    </dd>
                  </div>
                  <div>
                    <dt>Copia en este equipo</dt>
                    <dd>{formatFileSize(sheetSelection.source.fileSizeBytes)}</dd>
                  </div>
                </>
              ) : (
                <div>
                  <dt>Formato y tamaño</dt>
                  <dd>{sheetSelection.source.format.toUpperCase()} · {formatFileSize(sheetSelection.source.fileSizeBytes)}</dd>
                </div>
              )}
              {sheetSelection.source.format === "excel" && (
                <>
                  <div>
                    <dt>Hojas disponibles</dt>
                    <dd>{sheetSelection.source.sheets.length}</dd>
                  </div>
                  <div>
                    <dt>Se cargará</dt>
                    <dd>{sheetSelection.source.sheets.find((sheet) => sheet.id === sheetSelection.selectedSheetId)?.name ?? "Selecciona una hoja"}</dd>
                  </div>
                </>
              )}
              {isDelimitedSelection && (
                <div>
                  <dt>Separador detectado</dt>
                  <dd>
                    {sheetSelection.headerReview
                      ? sheetSelection.headerReview.delimiter === "\t" ? "Tabulador" : `“${sheetSelection.headerReview.delimiter}”`
                      : "Leyendo la muestra…"}
                  </dd>
                </div>
              )}
            </dl>
            {isDelimitedSelection ? (
              <>
                {sheetSelection.headerReviewLoading && (
                  <p role="status">Leyendo los primeros 64 KiB del archivo para la vista previa…</p>
                )}
                {sheetSelection.error && legacyEncodingExample(sheetSelection.error) === null && (
                  <p className="notice notice--error" role="alert">No se pudo previsualizar el archivo: {sheetSelection.error}</p>
                )}
                {sheetSelection.headerReview && (
                  <SourceReadingOptions
                    disabled={encodingConversionPending || sheetSelection.headerReviewLoading === true}
                    onApply={onReinterpret}
                  />
                )}
                {sheetSelection.headerReview && (
                  <HeaderInterpretationPreview
                    preview={sheetSelection.headerReview[sheetSelection.headerMode === "firstRow" ? "firstRow" : "generated"]}
                  />
                )}
                {sheetSelection.headerReview?.[sheetSelection.headerMode === "firstRow" ? "firstRow" : "generated"].sampleTruncated && (
                  <p role="note">La muestra quedó truncada y puede no representar el archivo entero.</p>
                )}
              </>
            ) : sheetSelection.source.format === "excel" ? (
              <p role="note">
                Las columnas y los tipos se calculan para la hoja y los encabezados elegidos, antes de cargar.
              </p>
            ) : reviewedTable ? null : (
              <p role="note">
                Este formato ya trae sus columnas y tipos: no hay que elegir encabezados.
              </p>
            )}
            {sheetSelection.schemaPreviewLoading && (
              <p role="status">Leyendo columnas y tipos…</p>
            )}
            {sheetSelection.schemaPreviewError && legacyEncodingExample(sheetSelection.schemaPreviewError) === null && (
              <p className="notice notice--error" role="alert">No se pudo revisar el esquema: {sheetSelection.schemaPreviewError}</p>
            )}
            {(() => {
              const example = legacyEncodingExample(sheetSelection.error) ?? legacyEncodingExample(sheetSelection.schemaPreviewError);
              if (example === null) return null;
              return (
                <section className="notice" aria-labelledby="legacy-encoding-title">
                  <h4 id="legacy-encoding-title">Leer como Excel para Windows</h4>
                  <p>
                    El archivo usa la codificación de Excel para Windows (Windows-1252).
                    {example && <> Así se leerá: «{example}».</>}
                  </p>
                  <p>Columnia lee una copia convertida a UTF-8; el original no cambia.</p>
                  <button type="button" className="primary-action" onClick={onConvertEncoding} disabled={encodingConversionPending}>
                    {encodingConversionPending ? "Convirtiendo…" : "Convertir y continuar"}
                  </button>
                </section>
              );
            })()}
            {sheetSelection.schemaPreview && (
              <section className="sheet-import-summary sheet-import-summary--nested" aria-labelledby="schema-preview-title">
                <h4 id="schema-preview-title" className="visually-hidden">Esquema detectado antes de importar</h4>
                <p role="status">{sheetSelection.schemaPreview.rowCount.toLocaleString()} filas · {sheetSelection.schemaPreview.columns.length} columnas</p>
                {sheetSelection.schemaPreview.schemaMismatch ? (
                  <div className="notice" role="alert">
                    <strong>El esquema no coincide con el perfil guardado.</strong> Si continúas, se importará con el esquema detectado y ese perfil no se aplicará.
                    {sheetSelection.schemaPreview.schemaMismatch.missingColumns.length > 0 && (
                      <p>Columnas faltantes: {sheetSelection.schemaPreview.schemaMismatch.missingColumns.join(", ")}</p>
                    )}
                    {sheetSelection.schemaPreview.schemaMismatch.addedColumns.length > 0 && (
                      <p>Columnas nuevas: {sheetSelection.schemaPreview.schemaMismatch.addedColumns.join(", ")}</p>
                    )}
                    {sheetSelection.schemaPreview.schemaMismatch.changedTypes.length > 0 && (
                      <p>Tipos distintos: {sheetSelection.schemaPreview.schemaMismatch.changedTypes.map((item) => `${item.column} (${item.expected} → ${item.actual})`).join("; ")}</p>
                    )}
                  </div>
                // `useSavedProfile` is a field, not a hook.
                // oxlint-disable-next-line react-hooks/rules-of-hooks
                ) : sheetSelection.useSavedProfile ? (
                  <p role="status">El esquema coincide con el perfil guardado; se aplicará al importar.</p>
                ) : null}
                {/* FUN-80: a convention that did not apply to a column says so. */}
                {(sheetSelection.schemaPreview.unconvertedColumns ?? []).length > 0 && (
                  <div className="notice" role="note">
                    <strong>Algunas columnas se quedan como texto.</strong> Una columna solo se convierte si todos sus valores siguen la convención elegida:
                    <ul>
                      {(sheetSelection.schemaPreview.unconvertedColumns ?? []).map((item) => (
                        <li key={item.column}>
                          {item.column}: {item.invalidCount === 1 ? "1 valor no es" : `${item.invalidCount.toLocaleString()} valores no son`} {item.convention === "date" ? "una fecha" : "un número"} (por ejemplo, «{item.example}»). Conviértela después en Preparar o deja ese valor nulo en el archivo.
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                {(() => {
                  // Prevention: a date column read as text is otherwise only seen in «Ver columnas y tipos».
                  if (!isDelimitedSelection || sheetSelection.dateConvention !== "unresolved") return null;
                  const dateLike = dateLikeTextColumns(sheetSelection.schemaPreview.columns);
                  if (dateLike.length === 0) return null;
                  const shown = dateLike.slice(0, 5).map((name) => `«${name}»`).join(", ");
                  const more = dateLike.length > 5 ? ` y ${(dateLike.length - 5).toLocaleString()} más` : "";
                  const one = dateLike.length === 1;
                  const sourceBacked = sheetSelection.source.resourceEstimate.processingPath === "sourceBacked";
                  return (
                    <p role="note" className="sheet-import-date-hint">
                      <strong>{one ? "Esta columna parece de fechas" : "Estas columnas parecen de fechas"}:</strong>{" "}
                      {shown}{more}. Con las fechas en «Sin definir» se {one ? "cargará" : "cargarán"} como texto.{" "}
                      {sourceBacked
                        ? `Este archivo se lee por bloques, así que ${one ? "conviértela" : "conviértelas"} después en Preparar.`
                        : "Puedes elegir su formato en «Interpretación de fechas y números (opcional)», más abajo."}
                    </p>
                  );
                })()}
                {sheetSelection.schemaPreview.columns.length === 0 ? (
                  <p role="note">No se detectaron columnas.</p>
                ) : (
                  <details className="sheet-import-columns">
                    <summary>Ver columnas y tipos</summary>
                    <div className="table-region" tabIndex={0} aria-label="Columnas y tipos detectados">
                      <table>
                        <thead><tr><th scope="col">Columna</th><th scope="col">Tipo detectado</th></tr></thead>
                        <tbody>
                          {sheetSelection.schemaPreview.columns.map((column, index) => (
                            <tr key={`${index}:${column.name}`}>
                              <th scope="row">{column.name}</th>
                              <td>{formatDataType(column.dataType)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </details>
                )}
              </section>
            )}
          </section>
          {isDelimitedSelection && (
            <details className="sheet-import-options">
              <summary>Interpretación de fechas y números (opcional)</summary>
              <p role="note">
                “Sin definir” conserva el texto. Una convención convierte una columna solo si todos sus valores no nulos cumplen; si alguno no cumple, se conserva la columna completa como texto.
                {sheetSelection.source.resourceEstimate.processingPath === "sourceBacked"
                  ? ` Este archivo se lee por bloques; las conversiones de fecha y número solo se aplican a archivos de menos de ${formatFileSize(sheetSelection.source.resourceEstimate.inMemoryLimitBytes ?? 512 * 1024 * 1024)}, que se cargan en memoria.`
                  : ""}
              </p>
              <label htmlFor="delimited-date-convention">Fechas</label>
              <select
                id="delimited-date-convention"
                value={sheetSelection.dateConvention}
                disabled={sheetSelection.schemaPreviewLoading === true}
                onChange={(event) => onSheetAction({
                  kind: "date_convention_changed",
                  value: event.target.value as ImportDateConvention,
                })}
              >
                {DATE_CONVENTIONS.map((item) => (
                  <option
                    key={item.value}
                    value={item.value}
                    disabled={sheetSelection.schemaPreviewLoading === true || (sheetSelection.source.resourceEstimate.processingPath === "sourceBacked" && item.value !== "unresolved")}
                  >
                    {item.label}
                  </option>
                ))}
              </select>
              <label htmlFor="delimited-number-convention">Números</label>
              <select
                id="delimited-number-convention"
                value={sheetSelection.numberConvention}
                disabled={sheetSelection.schemaPreviewLoading === true}
                onChange={(event) => onSheetAction({
                  kind: "number_convention_changed",
                  value: event.target.value as ImportNumberConvention,
                })}
              >
                {NUMBER_CONVENTIONS.map((item) => (
                  <option
                    key={item.value}
                    value={item.value}
                    disabled={sheetSelection.schemaPreviewLoading === true || (sheetSelection.source.resourceEstimate.processingPath === "sourceBacked" && item.value !== "unresolved")}
                  >
                    {item.label}
                  </option>
                ))}
              </select>
            </details>
          )}
          {sheetSelection.suggestedProfile && (
            <section className="sheet-import-summary" aria-labelledby="saved-import-profile-title">
              <h4 id="saved-import-profile-title">Perfil de importación guardado</h4>
              {sheetSelection.profileCanBeApplied ? (
                pendingTaskName ? (
                  <p role="note">
                    Se usará el perfil de “{pendingTaskName}” para elegir {sheetSelection.source.format === "excel" ? "hoja y encabezado" : isDelimitedSelection ? "encabezado" : "esquema"}. Si el esquema cambia, Columnia pedirá confirmación antes de importar.
                  </p>
                ) : (
                  <label>
                    <input
                      type="checkbox"
                      // `useSavedProfile` is a field, not a hook.
                      // oxlint-disable-next-line react-hooks/rules-of-hooks
                      checked={sheetSelection.useSavedProfile}
                      onChange={(event) => onSheetAction({ kind: "profile_toggled", useProfile: event.target.checked })}
                      disabled={sheetSelection.schemaPreviewLoading === true}
                    />
                    Usar {sheetSelection.source.format === "excel"
                      ? "la hoja, el encabezado y el esquema guardados"
                      : isDelimitedSelection ? "el encabezado y el esquema guardados" : "el esquema guardado"} si coincide
                  </label>
                )
              ) : (
                <p role="note">
                  {sheetSelection.source.format === "excel"
                    ? <>La hoja guardada “{sheetSelection.suggestedProfile.sheetName}” no está disponible. No se elegirá otra hoja automáticamente.</>
                    : "El esquema del perfil guardado no coincide; no se aplicará automáticamente."}
                </p>
              )}
              {isDelimitedSelection && (
                <p role="note">
                  Convenciones seleccionadas: {dateConventionLabel(sheetSelection.dateConvention)} · {numberConventionLabel(sheetSelection.numberConvention)}.
                </p>
              )}
            </section>
          )}
          <details className="sheet-import-options">
            <summary>Detalles técnicos: memoria y disco</summary>
            <ResourceEstimateSummary source={sheetSelection.source} />
          </details>
          {/* Wide dialog with a preview table: the actions stay at the foot while it scrolls. */}
          <div className="sheet-dialog__actions sheet-dialog__actions--sticky">
            <button type="button" className="secondary-action" onClick={() => onSheetAction({ kind: "cancelled" })}>Cancelar</button>
            {isDelimitedSelection && sheetSelection.error && legacyEncodingExample(sheetSelection.error) === null && (
              <button
                type="button"
                className="secondary-action"
                onClick={onRetryHeaderPreview}
                disabled={encodingConversionPending || sheetSelection.schemaPreviewLoading === true}
              >Reintentar muestra</button>
            )}
            <button
              type="button"
              className="primary-action"
              onClick={() => onSheetAction({ kind: "confirmed" })}
              disabled={encodingConversionPending || sheetSelection.schemaPreviewLoading === true || (sheetSelection.source.format === "excel"
                ? !sheetSelection.selectedSheetId
                : isDelimitedSelection
                  ? sheetSelection.headerReviewLoading === true || !sheetSelection.headerReview
                  : false)}
            >
              {sheetSelection.schemaPreviewLoading
                ? "Revisando esquema…"
                : !sheetSelection.schemaPreview
                  ? sheetSelection.schemaPreviewError ? "Reintentar esquema" : "Revisar esquema"
                  : sheetSelection.schemaPreview.schemaMismatch
                    ? "Importar con esquema nuevo"
                    : reviewedTable ? "Cargar tabla" : sheetSelection.source.format === "excel" ? "Cargar hoja" : "Cargar archivo"}
            </button>
          </div>
        </ModalDialog>
      )}
      {runtime.kind === "browser" && (
        <p className="notice" role="status">
          Desde el navegador no se pueden abrir archivos. Usa la aplicación de escritorio de Columnia.
        </p>
      )}
    </>
  );
}

function HeaderInterpretationPreview({ preview }: { preview: DelimitedHeaderModePreview }) {
  return (
    <section className="sheet-import-summary sheet-import-summary--nested" aria-label="Vista previa de la interpretación">
      <h4 className="visually-hidden">{preview.headerMode === "firstRow" ? "Con primera fila como encabezado" : "Con nombres generados"}</h4>
      {preview.columns.length === 0 ? (
        <p role="note">No se detectaron columnas en la muestra.</p>
      ) : (
        <div className="table-region" tabIndex={0} aria-label="Muestra importada">
          <table>
            <thead>
              {/* COD-10: a header may repeat a name (`id,id`); the position keeps keys unique. */}
              <tr>{preview.columns.map((column, columnIndex) => <th key={`${column.name}:${columnIndex}`} scope="col">{column.name}</th>)}</tr>
            </thead>
            <tbody>
              {preview.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {preview.columns.map((column, columnIndex) => (
                    <td key={`${column.name}:${columnIndex}`}>{renderCellValue(row[columnIndex])}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function dateConventionLabel(value: ImportDateConvention): string {
  return DATE_CONVENTIONS.find((item) => item.value === value)?.label ?? "Sin definir";
}

function numberConventionLabel(value: ImportNumberConvention): string {
  return NUMBER_CONVENTIONS.find((item) => item.value === value)?.label ?? "Sin definir";
}

function ResourceEstimateSummary({ source }: { source: DatasetSourceInspection }) {
  const estimate = source.resourceEstimate;
  const pathLabel = estimate.processingPath === "sourceBacked"
    ? "Por bloques: cada operación lee desde el disco el archivo, o una copia privada, cuando puede hacerlo así."
    : "En memoria: el dataset completo se carga para empezar el historial de cambios.";
  const temporaryDiskLabel = estimate.estimatedTemporaryDiskBytes === null
    ? "Abrir este archivo no crea otra copia de los datos. Los cambios posteriores sí pueden ocupar disco con el historial."
    : `Copia de trabajo e historial inicial: alrededor de ${formatFileSize(estimate.estimatedTemporaryDiskBytes)}; el tamaño real depende del contenido.`;

  return (
    <section className="sheet-import-summary load-resource-estimate" aria-label="Estimación de recursos">
      <h4>Estimación de memoria y disco</h4>
      <dl>
        <div>
          <dt>Tamaño del archivo</dt>
          <dd>{formatFileSize(source.fileSizeBytes)}</dd>
        </div>
        <div>
          <dt>Cómo se leerá</dt>
          <dd>{pathLabel}</dd>
        </div>
        <div>
          <dt>RAM si se carga entero</dt>
          <dd>Aprox. {formatFileSize(estimate.estimatedMaterializationRamBytes)} (4 veces el archivo + 256 MiB)</dd>
        </div>
        <div>
          <dt>Disco temporal</dt>
          <dd>{temporaryDiskLabel}</dd>
        </div>
      </dl>
      <p role="note">
        Son aproximaciones, no límites ni reservas. Los libros comprimidos y las columnas con muchos valores distintos pueden necesitar bastante más. Con archivos de 512 MiB o más, antes de cada operación que necesita los datos enteros en memoria el motor comprueba la RAM libre y, si no alcanza, no la ejecuta.
      </p>
    </section>
  );
}

function SchemaDifferenceList({ mismatch }: { mismatch: ImportProfileMismatch }) {
  const items = [
    ...mismatch.missingColumns.map((column) => ({ key: `missing:${column}`, text: `Falta la columna “${column}”` })),
    ...mismatch.addedColumns.map((column) => ({ key: `added:${column}`, text: `Columna nueva “${column}”` })),
    ...mismatch.changedTypes.map(({ column, expected, actual }) => ({
      key: `type:${column}`,
      text: `“${column}”: tipo guardado ${expected}; tipo actual ${actual}`,
    })),
  ];
  const visible = items.slice(0, 12);
  return (
    <ul aria-label="Diferencias de esquema">
      {visible.map((item) => <li key={item.key}>{item.text}</li>)}
      {items.length > visible.length && <li>Y {items.length - visible.length} diferencias más.</li>}
    </ul>
  );
}

const SEPARATOR_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "", label: "Como se detectó" },
  { value: ",", label: "Coma" },
  { value: ";", label: "Punto y coma" },
  { value: "\t", label: "Tabulador" },
  { value: "|", label: "Barra vertical" },
];

const ENCODING_OPTIONS: Array<{ value: SourceTextEncoding; label: string }> = [
  { value: "utf-8", label: "UTF-8" },
  { value: "windows-1252", label: "Windows-1252 (Excel en español)" },
  { value: "iso-8859-15", label: "ISO-8859-15" },
  { value: "utf-16le", label: "UTF-16 LE (Excel «Unicode»)" },
  { value: "utf-16be", label: "UTF-16 BE" },
];

/** PROD-02: a wrong separator or encoding can be fixed without leaving Cargar. */
function SourceReadingOptions({
  disabled,
  onApply,
}: {
  disabled: boolean;
  onApply: (delimiter: string | null, encoding: SourceTextEncoding) => void;
}) {
  const [delimiter, setDelimiter] = useState("");
  const [encoding, setEncoding] = useState<SourceTextEncoding>("utf-8");
  return (
    <details className="source-reading-options">
      <summary>¿Columnas o acentos mal leídos?</summary>
      <div className="source-reading-options__fields">
        <label>
          Separador
          <select value={delimiter} onChange={(event) => setDelimiter(event.target.value)} disabled={disabled}>
            {SEPARATOR_OPTIONS.map((option) => <option key={option.label} value={option.value}>{option.label}</option>)}
          </select>
        </label>
        <label>
          Codificación
          <select value={encoding} onChange={(event) => setEncoding(event.target.value as SourceTextEncoding)} disabled={disabled}>
            {ENCODING_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </label>
        <button
          type="button"
          className="secondary-action"
          disabled={disabled || (delimiter === "" && encoding === "utf-8")}
          onClick={() => onApply(delimiter === "" ? null : delimiter, encoding)}
        >
          Volver a leer
        </button>
      </div>
    </details>
  );
}
