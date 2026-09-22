import type { ReactNode } from "react";

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
} from "../../bridge";
import { DATE_CONVENTIONS, NUMBER_CONVENTIONS } from "./importProfile";
import {
  formatRecentDatasetDate,
  formatRecentDatasetFormat,
  type RecentDataset,
} from "./recentFilesModel";

export type LoadRuntimeState =
  | { kind: "connected" }
  | { kind: "browser" }
  | { kind: "unavailable" };

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${new Intl.NumberFormat("es-DO").format(bytes)} bytes`;

  const units = ["KiB", "MiB", "GiB", "TiB"];
  let size = bytes;
  let unitIndex = -1;
  do {
    size /= 1024;
    unitIndex += 1;
  } while (size >= 1024 && unitIndex < units.length - 1);

  return `${new Intl.NumberFormat("es-DO", { maximumFractionDigits: 1 }).format(size)} ${units[unitIndex]}`;
}

interface LoadPhaseProps {
  children?: ReactNode;
  reusableTaskPanel?: ReactNode;
  pendingTaskName?: string | null;
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
  onRetryHeaderPreview?: () => void;
  onProfileReviewAction?: (action: ProfileReviewAction) => void;
  onResourcePreflightAction?: (action: ResourcePreflightAction) => void;
  onSchemaMismatchAction?: (action: SchemaMismatchAction) => void;
  onCancelLoad: () => void;
  workbookInspectionCancellationPending?: boolean;
  onCancelWorkbookInspection?: () => void;
}

export function LoadPhase({
  children,
  reusableTaskPanel,
  pendingTaskName = null,
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
  onRetryHeaderPreview = () => undefined,
  onProfileReviewAction = () => undefined,
  onResourcePreflightAction = () => undefined,
  onSchemaMismatchAction = () => undefined,
  onCancelLoad,
  workbookInspectionCancellationPending = false,
  onCancelWorkbookInspection = () => undefined,
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
  const selectionDisabled = disabled || runtime.kind !== "connected" ||
    inspection.kind === "inspecting" ||
    inspection.kind === "workbook_inspecting" ||
    datasetStatus.kind === "loading" ||
    inspection.kind === "sheet" ||
    inspection.kind === "profile_review" ||
    inspection.kind === "resource_preflight" ||
    inspection.kind === "schema_mismatch";

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
      {inspection.kind === "inspecting" && (
        <p className="notice" role="status">Esperando la selección y verificando el formato local…</p>
      )}
      {inspection.kind === "workbook_inspecting" && (
        <div className="notice" role="status">
          <p>Revisando las hojas de {inspection.source.fileName}. La lectura interna del libro puede terminar antes de aplicar la cancelación.</p>
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
      {profileReview && (
        <ModalDialog
          role="dialog"
          labelledBy="import-profile-title"
          describedBy="import-profile-description"
          onDismiss={() => onProfileReviewAction({ kind: "cancelled" })}
        >
          <p className="eyebrow">Perfil del proyecto</p>
          <h3 id="import-profile-title">Reutilizar interpretación guardada</h3>
          <p id="import-profile-description">
            El perfil es v{profileReview.profile.version} para {profileReview.profile.format.toUpperCase()} y contiene {profileReview.profile.schema.length} columnas de esquema.
            Columnia lo comparará antes de reemplazar el dataset activo.
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
          <p className="eyebrow">Preflight local</p>
          <h3 id="resource-preflight-title">Revisa el costo estimado de la carga</h3>
          <p id="resource-preflight-description">
            El tamaño y la ruta orientan la decisión antes de abrir {resourcePreflight.source.fileName}. Columnia conserva el archivo original.
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
          <p className="eyebrow">Revisión requerida</p>
          <h3 id="import-schema-mismatch-title">El esquema difiere del perfil guardado</h3>
          <p id="import-schema-mismatch-description">
            El dataset activo se conserva. No se aplicó ningún cast ni mapeo. Revisa los cambios antes de decidir.
            {pendingTaskName ? ` La tarea “${pendingTaskName}” no se aplicará sin una confirmación posterior.` : ""}
          </p>
          <SchemaDifferenceList mismatch={schemaMismatch.mismatch} />
          <p className="notice" role="note">
            Si continúas, se importará el archivo con el nuevo esquema. Las convenciones seleccionadas convierten solo columnas cuyos valores completos cumplen; las demás se conservan como texto. Puedes cancelar y mantener el dataset anterior.
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
          <p className="eyebrow">
            {sheetSelection.source.format === "excel"
              ? "Libro seleccionado"
              : isDelimitedSelection ? "Archivo delimitado seleccionado" : "Archivo seleccionado"}
          </p>
          <h3 id="sheet-title">
            {sheetSelection.source.format === "excel"
              ? `Elegir hoja de ${sheetSelection.source.fileName}`
              : isDelimitedSelection
                ? `Revisar encabezados de ${sheetSelection.source.fileName}`
                : `Revisar importación de ${sheetSelection.source.fileName}`}
          </h3>
          <p id="sheet-description">
            {sheetSelection.source.format === "excel"
              ? "Elige la hoja y los encabezados. La estimación de recursos está aquí antes de cargar; el dataset activo se conserva hasta terminar."
              : isDelimitedSelection
                ? "Compara las dos interpretaciones y revisa los recursos. El dataset activo se conserva hasta que confirmes la carga."
                : "Revisa el formato, el perfil y los recursos antes de importar. El dataset activo se conserva hasta que confirmes la carga."}
          </p>
          {sheetSelection.source.isCompressedContainer && (
            <p className="notice" role="note">
              Los libros comprimidos pueden ocupar bastante más memoria al abrirse que su tamaño en disco.
              Cierra otras aplicaciones si el archivo es grande.
            </p>
          )}
          <ResourceEstimateSummary source={sheetSelection.source} />
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
                  <option key={sheet.id} value={sheet.id}>{sheet.name}</option>
                ))}
              </select>
            </>
          )}
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
          {isDelimitedSelection && (
            <details className="sheet-import-options">
              <summary>Interpretación de fechas y números (opcional)</summary>
              <p role="note">
                “Sin definir” conserva el texto. Una convención convierte una columna solo si todos sus valores no nulos cumplen; si alguno no cumple, se conserva la columna completa como texto.
                {sheetSelection.source.resourceEstimate.processingPath === "sourceBacked"
                  ? " Esta fuente requiere lectura por bloques; para aplicar conversiones, usa un archivo dentro del límite de carga en memoria."
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
              <h4 id="saved-import-profile-title">Perfil reutilizable del proyecto</h4>
              {sheetSelection.profileCanBeApplied ? (
                pendingTaskName ? (
                  <p role="note">
                    Se usará el perfil de “{pendingTaskName}” para elegir {sheetSelection.source.format === "excel" ? "hoja y encabezado" : isDelimitedSelection ? "encabezado" : "esquema"}. Si el esquema cambia, Columnia pedirá confirmación antes de importar.
                  </p>
                ) : (
                  <label>
                    <input
                      type="checkbox"
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
          <section className="sheet-import-summary" aria-labelledby="sheet-import-summary-title" aria-live="polite">
            <h4 id="sheet-import-summary-title">Resumen antes de cargar</h4>
            <dl>
              <div>
                <dt>Formato y tamaño</dt>
                <dd>{sheetSelection.source.format.toUpperCase()} · {formatFileSize(sheetSelection.source.fileSizeBytes)}</dd>
              </div>
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
                      : "Calculando muestra…"}
                  </dd>
                </div>
              )}
              {hasHeaderSelection && (
                <div>
                  <dt>Encabezados</dt>
                  <dd>{sheetSelection.headerMode === "firstRow" ? "Usar la primera fila" : "Generar nombres de columna"}</dd>
                </div>
              )}
            </dl>
            {isDelimitedSelection ? (
              <>
                {sheetSelection.headerReviewLoading && (
                  <p role="status">Preparando una muestra local de hasta 64 KiB…</p>
                )}
                {sheetSelection.error && (
                  <p className="notice notice--error" role="alert">No se pudo previsualizar el archivo: {sheetSelection.error}</p>
                )}
                {sheetSelection.headerReview && (
                  <HeaderInterpretationPreview
                    preview={sheetSelection.headerReview[sheetSelection.headerMode === "firstRow" ? "firstRow" : "generated"]}
                  />
                )}
                <p role="note">
                  La muestra lee como máximo 64 KiB y enseña hasta cinco filas. Revisar el esquema determina tipos sin activar ni reemplazar el dataset actual; la carga final empieza al confirmar.
                  {sheetSelection.dateConvention === "unresolved" && sheetSelection.numberConvention === "unresolved"
                    ? " Sin convenciones elegidas, los valores se conservan como texto."
                    : " Las conversiones elegidas se aplican solo cuando toda la columna cumple la convención."}
                  {sheetSelection.headerReview?.[sheetSelection.headerMode === "firstRow" ? "firstRow" : "generated"].sampleTruncated
                    ? " La muestra quedó truncada y puede no representar el archivo entero."
                    : ""}
                </p>
              </>
            ) : sheetSelection.source.format === "excel" ? (
              <p role="note">
                El esquema se calcula para la hoja y los encabezados seleccionados antes de activar el dataset.
              </p>
            ) : (
              <p role="note">
                Este formato conserva su estructura propia y no requiere elegir encabezados. Revisa las columnas y tipos detectados antes de importar.
              </p>
            )}
            {sheetSelection.schemaPreviewLoading && (
              <p role="status">Leyendo la estructura para mostrar columnas y tipos; el dataset actual permanece intacto…</p>
            )}
            {sheetSelection.schemaPreviewError && (
              <p className="notice notice--error" role="alert">No se pudo revisar el esquema: {sheetSelection.schemaPreviewError}</p>
            )}
            {sheetSelection.schemaPreview && (
              <section className="sheet-import-summary" aria-labelledby="schema-preview-title" aria-live="polite">
                <h4 id="schema-preview-title">Esquema detectado antes de importar</h4>
                <p role="note">{sheetSelection.schemaPreview.rowCount.toLocaleString()} filas · {sheetSelection.schemaPreview.columns.length} columnas</p>
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
                ) : sheetSelection.useSavedProfile ? (
                  <p role="status">El esquema coincide con el perfil guardado; se aplicará al importar.</p>
                ) : null}
                {sheetSelection.schemaPreview.columns.length === 0 ? (
                  <p role="note">No se detectaron columnas.</p>
                ) : (
                  <div className="table-region" tabIndex={0} aria-label="Columnas y tipos detectados">
                    <table>
                      <thead><tr><th scope="col">Columna</th><th scope="col">Tipo detectado</th></tr></thead>
                      <tbody>
                        {sheetSelection.schemaPreview.columns.map((column, index) => (
                          <tr key={`${index}:${column.name}`}>
                            <th scope="row">{column.name}</th>
                            <td>{column.dataType}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </section>
            )}
          </section>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => onSheetAction({ kind: "cancelled" })}>Cancelar</button>
            {isDelimitedSelection && sheetSelection.error && (
              <button
                type="button"
                className="secondary-action"
                onClick={onRetryHeaderPreview}
                disabled={sheetSelection.schemaPreviewLoading === true}
              >Reintentar muestra</button>
            )}
            <button
              type="button"
              className="primary-action"
              onClick={() => onSheetAction({ kind: "confirmed" })}
              disabled={sheetSelection.schemaPreviewLoading === true || (sheetSelection.source.format === "excel"
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
                    : sheetSelection.source.format === "excel" ? "Cargar hoja" : "Cargar archivo"}
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

function HeaderInterpretationPreview({ preview }: { preview: DelimitedHeaderModePreview }) {
  return (
    <section className="sheet-import-summary" aria-label="Vista previa de la interpretación">
      <h4>{preview.headerMode === "firstRow" ? "Con primera fila como encabezado" : "Con nombres generados"}</h4>
      <p role="note">
        {preview.includesFirstRow
          ? "La primera fila se conserva como un registro."
          : "La primera fila se usa para nombrar columnas y se excluye de los registros."}
      </p>
      {preview.columns.length === 0 ? (
        <p role="note">No se detectaron columnas en la muestra.</p>
      ) : (
        <div className="table-region" tabIndex={0} aria-label="Muestra importada">
          <table>
            <thead>
              <tr>{preview.columns.map((column) => <th key={column.name} scope="col">{column.name}</th>)}</tr>
            </thead>
            <tbody>
              {preview.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {preview.columns.map((column, columnIndex) => (
                    <td key={`${column.name}:${columnIndex}`}>{row[columnIndex] ?? <span className="null-value">null</span>}</td>
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
    ? "Lectura source-backed: procesa desde la fuente o un snapshot privado por bloques cuando la operación es compatible."
    : "Carga en memoria: el dataset completo se materializa para iniciar el historial de trabajo.";
  const temporaryDiskLabel = estimate.estimatedTemporaryDiskBytes === null
    ? "No se prevé un snapshot de datos adicional al abrir esta fuente. Las ediciones posteriores pueden crear historial temporal."
    : `Snapshot e historial inicial: alrededor de ${formatFileSize(estimate.estimatedTemporaryDiskBytes)}; el tamaño real depende del contenido.`;

  return (
    <section className="sheet-import-summary load-resource-estimate" aria-label="Estimación de recursos">
      <h4>Recursos orientativos</h4>
      <dl>
        <div>
          <dt>Tamaño en disco</dt>
          <dd>{formatFileSize(source.fileSizeBytes)}</dd>
        </div>
        <div>
          <dt>Ruta de procesamiento</dt>
          <dd>{pathLabel}</dd>
        </div>
        <div>
          <dt>RAM si se materializa</dt>
          <dd>Aprox. {formatFileSize(estimate.estimatedMaterializationRamBytes)} (4× archivo + 256 MiB)</dd>
        </div>
        <div>
          <dt>Disco temporal</dt>
          <dd>{temporaryDiskLabel}</dd>
        </div>
      </dl>
      <p role="note">
        Son aproximaciones, no límites ni reservas. Libros comprimidos y datos de alta cardinalidad pueden requerir bastante más espacio o memoria. La admisión nativa decide cada operación de materialización.
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
