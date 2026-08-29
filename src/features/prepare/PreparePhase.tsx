import { useEffect, useState } from "react";

import { OperationProgressView } from "../../components/OperationProgressView";
import { ModalDialog } from "../../components/ModalDialog";
import type { DatasetPreview, DatasetProfile, HistoryState, SavedRecipe, TransformRecipe } from "../../bridge";
import type { ProfileStatus } from "../review/reviewModel";
import { ChangeFeedback, HistoryBar } from "./HistoryBar";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import type { ChangeStatus } from "./prepareModel";

interface PreparePhaseProps {
  dataset: DatasetPreview;
  profileStatus: ProfileStatus;
  changeStatus: ChangeStatus;
  historyStatus: HistoryState;
  recipeDraft: SavedRecipe | null;
  recipeSession: number;
  onAnalyzeQuality: () => void;
  onCancelProfile: () => void;
  onRemoveDuplicates: () => void;
  onRemoveNearDuplicates?: () => void;
  onRemoveEmptyRows: () => void;
  onRemoveConstantColumns: () => void;
  onRemoveEmptyColumns: () => void;
  onRemoveHighNullColumns: () => void;
  onRemoveIdentifierColumns?: () => void;
  onRemovePersonalColumns?: () => void;
  onMaskPersonalValues?: () => void;
  onNormalizeSentinels: () => void;
  onNormalizeBooleans: () => void;
  onParseDates?: () => void;
  onFixEncoding?: () => void;
  onNullifyInvalidTypes?: () => void;
  onImputeMissingValues: () => void;
  onImputeCategoricalValues?: () => void;
  onImputeOutliers?: () => void;
  onCapOutliers?: () => void;
  onDropOutliers?: () => void;
  onEnableRowAudit: () => void;
  onNormalizeColumns: () => void;
  onApplyRecommended: () => void;
  onTrimText: () => void;
  onNormalizeText: (columns: string[], removeAccents: boolean) => void;
  onApplyTransforms: (recipe: TransformRecipe) => void;
  onRecipeDraftChange: (draft: SavedRecipe) => void;
  onUndo: () => void;
  onRedo: () => void;
}

export function PreparePhase({
  dataset,
  profileStatus,
  changeStatus,
  historyStatus,
  recipeDraft,
  recipeSession,
  onAnalyzeQuality,
  onCancelProfile,
  onRemoveDuplicates,
  onRemoveNearDuplicates = () => undefined,
  onRemoveEmptyRows,
  onRemoveConstantColumns,
  onRemoveEmptyColumns,
  onRemoveHighNullColumns,
  onRemoveIdentifierColumns = () => undefined,
  onRemovePersonalColumns = () => undefined,
  onMaskPersonalValues = () => undefined,
  onNormalizeSentinels,
  onNormalizeBooleans,
  onParseDates = () => undefined,
  onFixEncoding = () => undefined,
  onNullifyInvalidTypes = () => undefined,
  onImputeMissingValues,
  onImputeCategoricalValues = () => undefined,
  onImputeOutliers = () => undefined,
  onCapOutliers = () => undefined,
  onDropOutliers = () => undefined,
  onEnableRowAudit,
  onNormalizeColumns,
  onApplyRecommended,
  onTrimText,
  onNormalizeText,
  onApplyTransforms,
  onRecipeDraftChange,
  onUndo,
  onRedo,
}: PreparePhaseProps) {
  const duplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.duplicateRowCount : null;
  const nearDuplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.nearDuplicateRowCount : null;
  const changing = changeStatus.kind === "working";
  const textColumns = dataset.columns.filter((column) => column.dataType === "String" && column.name !== "_cambios");
  const [selectedTextColumns, setSelectedTextColumns] = useState<string[]>([]);
  const [removeAccents, setRemoveAccents] = useState(true);
  const [activeTab, setActiveTab] = useState<"corrections" | "transformations">("corrections");
  const [nearDuplicateConfirmation, setNearDuplicateConfirmation] = useState(false);
  const [identifierConfirmation, setIdentifierConfirmation] = useState(false);
  const [personalConfirmation, setPersonalConfirmation] = useState(false);
  const [maskPersonalConfirmation, setMaskPersonalConfirmation] = useState(false);
  const [invalidTypeConfirmation, setInvalidTypeConfirmation] = useState(false);
  const [outlierConfirmation, setOutlierConfirmation] = useState<"cap" | "drop" | null>(null);
  const identifierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => column.privacySignal === "identifier")
    : [];
  const personalColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && column.name !== "_cambios")
    : [];
  const typeDriftColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.invalidTypeCount ?? 0) > 0)
    : [];
  const outlierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.outlierCount ?? 0) > 0 && column.name !== "_cambios")
    : [];
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);

  useEffect(() => {
    const available = new Set(textColumns.map((column) => column.name));
    setSelectedTextColumns((current) => current.filter((name) => available.has(name)));
  }, [dataset.columns]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Preparar · {activeTab === "corrections" ? "Correcciones" : "Transformaciones"}</p>
          <h2>Prepara datos consistentes</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
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
            key={recipeSession}
            dataset={dataset}
            busy={changing}
            initialDraft={recipeDraft}
            onApply={onApplyTransforms}
            onDraftChange={onRecipeDraftChange}
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
      {profileStatus.kind === "ready" && (
        <CleaningSignals
          profile={profileStatus.profile}
          busy={changing}
          onRemoveConstantColumns={onRemoveConstantColumns}
              onRemoveEmptyColumns={onRemoveEmptyColumns}
              onRemoveHighNullColumns={onRemoveHighNullColumns}
              onRemoveIdentifierColumns={() => setIdentifierConfirmation(true)}
              onRemovePersonalColumns={() => setPersonalConfirmation(true)}
              onMaskPersonalValues={() => setMaskPersonalConfirmation(true)}
              onNormalizeSentinels={onNormalizeSentinels}
              onNormalizeBooleans={onNormalizeBooleans}
              onParseDates={onParseDates}
              onFixEncoding={onFixEncoding}
              onNullifyInvalidTypes={() => setInvalidTypeConfirmation(true)}
               onImputeMissingValues={onImputeMissingValues}
               onImputeCategoricalValues={onImputeCategoricalValues}
               onImputeOutliers={onImputeOutliers}
               onCapOutliers={() => setOutlierConfirmation("cap")}
               onDropOutliers={() => setOutlierConfirmation("drop")}
         />
      )}
      <section className="prepare-card" aria-labelledby="row-audit-title">
        <div>
          <p className="step">Trazabilidad local</p>
          <h3 id="row-audit-title">Cambios por fila</h3>
          <p>
            {dataset.columns.some((column) => column.name === "_cambios")
              ? "La columna _cambios está activa; cada corrección posterior añadirá su operación a la fila afectada."
              : "Activa una columna reservada _cambios para conservar una etiqueta breve de las correcciones posteriores."}
          </p>
        </div>
        <button
          type="button"
          onClick={onEnableRowAudit}
          disabled={changing || dataset.columns.some((column) => column.name === "_cambios")}
        >
          {dataset.columns.some((column) => column.name === "_cambios") ? "Trazabilidad activa" : "Activar trazabilidad"}
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
      <details className="advanced-corrections">
        <summary>
          <span>Correcciones avanzadas</span>
          <small>Selección manual, eliminación de filas y duplicados</small>
        </summary>
        <div className="advanced-corrections__content">
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
      <section className="prepare-card" aria-labelledby="empty-rows-title">
        <div>
          <p className="step">Corrección segura</p>
          <h3 id="empty-rows-title">Filas completamente vacías</h3>
          <p>Elimina filas cuyos valores son todos nulos o texto en blanco. La operación conserva el orden y es reversible.</p>
        </div>
        <button type="button" onClick={onRemoveEmptyRows} disabled={changing}>
          Eliminar filas vacías
        </button>
      </section>
      {profileStatus.kind === "loading" ? (
        <OperationProgressView
          progress={profileStatus.progress}
          cancellation={profileStatus.cancelRequested
            ? { kind: "requested" }
            : { kind: "available", onCancel: onCancelProfile }}
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
      {profileStatus.kind === "ready" && (
        <section className="prepare-card" aria-labelledby="near-duplicates-title">
          <div>
            <p className="step">Revisión con confirmación</p>
            <h3 id="near-duplicates-title">Duplicados parecidos</h3>
            <p>
              {nearDuplicateCount === null || nearDuplicateCount === 0
                ? "No se detectaron filas parecidas adicionales después de excluir los duplicados exactos."
                : `Se identificaron ${nearDuplicateCount.toLocaleString()} filas parecidas por normalización de texto. La primera fila y las copias exactas se conservarán.`}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setNearDuplicateConfirmation(true)}
            disabled={nearDuplicateCount === null || nearDuplicateCount === 0 || changing}
          >
            Revisar y eliminar parecidos
          </button>
        </section>
      )}
      {profileStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {profileStatus.message}
        </p>
      )}
        </div>
      </details>
      </div>
      )}
      {nearDuplicateConfirmation && nearDuplicateCount !== null && nearDuplicateCount > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="near-duplicates-confirm-title"
          describedBy="near-duplicates-confirm-description"
          onDismiss={() => setNearDuplicateConfirmation(false)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="near-duplicates-confirm-title">Eliminar duplicados parecidos</h3>
          <p id="near-duplicates-confirm-description">
            Se eliminarán hasta {nearDuplicateCount.toLocaleString()} filas que coinciden después
            de normalizar espacios, mayúsculas y acentos. No se mostrarán valores del dataset.
            Se conservará la primera fila de cada grupo, el orden actual y las copias exactas.
            La operación podrá revertirse desde el historial.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setNearDuplicateConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                setNearDuplicateConfirmation(false);
                onRemoveNearDuplicates();
              }}
              disabled={changing}
            >
              Eliminar duplicados parecidos
            </button>
          </div>
        </ModalDialog>
      )}
      {identifierConfirmation && identifierColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="identifier-confirm-title"
          describedBy="identifier-confirm-description"
          onDismiss={() => setIdentifierConfirmation(false)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="identifier-confirm-title">Retirar identificadores detectados</h3>
          <p id="identifier-confirm-description">
            Se retirarán {identifierColumns.length === 1 ? "1 columna identificadora" : `${identifierColumns.length} columnas identificadoras`} detectadas por el nombre del encabezado: {identifierColumns.map((column) => column.name).join(", ")}.
            No se mostrarán celdas ni valores del dataset. Se conservará al menos una columna y el cambio podrá revertirse desde el historial.
            Las columnas de email, teléfono, dirección y nombre no se retiran con esta acción.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setIdentifierConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                setIdentifierConfirmation(false);
                onRemoveIdentifierColumns();
              }}
              disabled={changing}
            >
              Retirar identificadores
            </button>
          </div>
        </ModalDialog>
      )}
      {personalConfirmation && personalColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="personal-confirm-title"
          describedBy="personal-confirm-description"
          onDismiss={() => setPersonalConfirmation(false)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="personal-confirm-title">Retirar datos personales detectados</h3>
          <p id="personal-confirm-description">
            Se retirarán {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. Se excluirá _cambios,
            se conservará al menos una columna utilizable y el cambio podrá revertirse desde el historial.
            Los identificadores se gestionan con la acción separada de esta sección.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setPersonalConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                setPersonalConfirmation(false);
                onRemovePersonalColumns();
              }}
              disabled={changing}
            >
              Retirar datos personales
            </button>
          </div>
        </ModalDialog>
      )}
      {maskPersonalConfirmation && personalColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="personal-mask-confirm-title"
          describedBy="personal-mask-confirm-description"
          onDismiss={() => setMaskPersonalConfirmation(false)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="personal-mask-confirm-title">Proteger datos personales detectados</h3>
          <p id="personal-mask-confirm-description">
            Se sustituirán los valores no nulos de {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} por <code>[REDACTED]</code>, identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. Se conservarán las columnas,
            se excluirá _cambios y el cambio podrá revertirse desde el historial.
            Los identificadores se gestionan con la acción separada de esta sección.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setMaskPersonalConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                setMaskPersonalConfirmation(false);
                onMaskPersonalValues();
              }}
              disabled={changing}
            >
              Proteger valores personales
            </button>
          </div>
        </ModalDialog>
      )}
      {invalidTypeConfirmation && typeDriftColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="invalid-types-confirm-title"
          describedBy="invalid-types-confirm-description"
          onDismiss={() => setInvalidTypeConfirmation(false)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="invalid-types-confirm-title">Apartar valores incompatibles</h3>
          <p id="invalid-types-confirm-description">
            Se convertirán en nulos los valores que no coincidan con un tipo sugerido con al menos 90% de coincidencia en {typeDriftColumns.length === 1 ? "1 columna" : `${typeDriftColumns.length} columnas`}. Las celdas no se mostrarán y el cambio podrá revertirse desde el historial.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setInvalidTypeConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                setInvalidTypeConfirmation(false);
                onNullifyInvalidTypes();
              }}
              disabled={changing}
            >
              Apartar valores incompatibles
            </button>
          </div>
        </ModalDialog>
      )}
      {outlierConfirmation !== null && outlierColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="outliers-confirm-title"
          describedBy="outliers-confirm-description"
          onDismiss={() => setOutlierConfirmation(null)}
        >
          <p className="step">Confirmación requerida</p>
          <h3 id="outliers-confirm-title">
            {outlierConfirmation === "cap" ? "Limitar valores atípicos" : "Eliminar filas atípicas"}
          </h3>
          <p id="outliers-confirm-description">
            {outlierConfirmation === "cap"
              ? "Se limitarán los valores que excedan los límites IQR de 1.5 al límite correspondiente."
              : "Se eliminará cualquier fila que contenga un valor que exceda los límites IQR de 1.5."}
            {" "}No se mostrarán celdas ni valores del dataset. La operación será reversible desde el historial.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setOutlierConfirmation(null)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                const action = outlierConfirmation;
                setOutlierConfirmation(null);
                if (action === "cap") onCapOutliers();
                else onDropOutliers();
              }}
              disabled={changing}
            >
              {outlierConfirmation === "cap" ? "Limitar outliers" : "Eliminar filas atípicas"}
            </button>
          </div>
        </ModalDialog>
      )}
    </>
  );
}

const PERSONAL_PRIVACY_LABELS = {
  email: "correo electrónico",
  phone: "teléfono",
  address: "dirección",
  name: "nombre",
} as const;

function isPersonalPrivacySignal(signal: string | null): signal is keyof typeof PERSONAL_PRIVACY_LABELS {
  return signal !== null && signal in PERSONAL_PRIVACY_LABELS;
}

function summarizePersonalPrivacySignals(columns: Array<{ privacySignal: string | null }>): string {
  const counts = columns.reduce<Record<string, number>>((result, column) => {
    if (isPersonalPrivacySignal(column.privacySignal)) {
      result[column.privacySignal] = (result[column.privacySignal] ?? 0) + 1;
    }
    return result;
  }, {});
  return Object.entries(counts)
    .map(([signal, count]) => `${PERSONAL_PRIVACY_LABELS[signal as keyof typeof PERSONAL_PRIVACY_LABELS]} (${count})`)
    .join(", ");
}

function CleaningSignals({
  profile,
  busy,
  onRemoveConstantColumns,
  onRemoveEmptyColumns,
  onRemoveHighNullColumns,
  onRemoveIdentifierColumns,
  onRemovePersonalColumns,
  onMaskPersonalValues,
  onNormalizeSentinels,
  onNormalizeBooleans,
  onParseDates,
  onFixEncoding,
  onNullifyInvalidTypes,
  onImputeMissingValues,
  onImputeCategoricalValues,
  onImputeOutliers,
  onCapOutliers,
  onDropOutliers,
}: {
  profile: DatasetProfile;
  busy: boolean;
  onRemoveConstantColumns: () => void;
  onRemoveEmptyColumns: () => void;
  onRemoveHighNullColumns: () => void;
  onRemoveIdentifierColumns: () => void;
  onRemovePersonalColumns: () => void;
  onMaskPersonalValues: () => void;
  onNormalizeSentinels: () => void;
  onNormalizeBooleans: () => void;
  onParseDates: () => void;
  onFixEncoding: () => void;
  onNullifyInvalidTypes: () => void;
  onImputeMissingValues: () => void;
  onImputeCategoricalValues: () => void;
  onImputeOutliers: () => void;
  onCapOutliers: () => void;
  onDropOutliers: () => void;
}) {
  const incomplete = profile.columns.filter((column) => column.completenessPercentage < 100);
  const imputable = incomplete.filter(
    (column) => column.nullCount > 0 && column.nullCount < profile.rowCount && column.name !== "_cambios",
  );
  const constant = profile.columns.filter(
    (column) => profile.rowCount > 1 && column.uniqueCount <= 1 && column.nullCount < profile.rowCount,
  );
  const empty = profile.columns.filter(
    (column) => profile.rowCount > 0 && column.nullCount === profile.rowCount,
  );
  const highNull = profile.columns.filter(
    (column) => profile.rowCount > 0 && column.nullCount > 0 && column.nullCount < profile.rowCount &&
      column.nullCount * 100 >= profile.rowCount * 80,
  );
  const sentinels = profile.columns.filter((column) => (column.sentinelCount ?? 0) > 0);
  const encoding = profile.columns.filter((column) => (column.encodingIssueCount ?? 0) > 0);
  const nearDuplicates = profile.nearDuplicateRowCount > 0;
  const booleans = profile.columns.filter(
    (column) => column.suggestedType === "boolean" && (column.typeMatchPercentage ?? 0) >= 90,
  );
  const dateCandidates = profile.columns.filter(
    (column) => column.name !== "_cambios" && column.dataType === "String" && column.suggestedType === "date",
  );
  const typeDrift = profile.columns.filter(
    (column) => (column.invalidTypeCount ?? 0) > 0,
  );
  const typeDriftColumns = typeDrift;
  const outliers = profile.columns.filter(
    (column) => (column.outlierCount ?? 0) > 0 && column.name !== "_cambios",
  );
  const categoricalImputable = profile.columns.filter(
    (column) => column.dataType === "String" && column.nullCount > 0 && column.name !== "_cambios",
  );
  const personal = profile.columns.filter((column) => column.privacySignal !== null);
  const personalColumns = profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && column.name !== "_cambios");
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);
  const hasSignals = profile.duplicateRowCount > 0 || nearDuplicates || incomplete.length > 0 || constant.length > 0 || empty.length > 0 || highNull.length > 0 || sentinels.length > 0 || encoding.length > 0 || booleans.length > 0 || dateCandidates.length > 0 || typeDrift.length > 0 || outliers.length > 0 || personal.length > 0;

  return (
    <section className="prepare-card prepare-card--stacked cleaning-signals" aria-labelledby="cleaning-signals-title">
      <div>
        <p className="step">Catálogo de limpieza</p>
        <h3 id="cleaning-signals-title">Señales para revisar</h3>
        <p>Las señales usan solo esquema y métricas agregadas; no muestran celdas ni valores personales.</p>
      </div>
      {hasSignals ? (
        <>
          <ul className="cleaning-signals__list">
          {profile.duplicateRowCount > 0 && (
            <li><strong>Duplicados exactos:</strong> {profile.duplicateRowCount.toLocaleString()} filas adicionales; puedes eliminarlas de forma reversible.</li>
          )}
          {nearDuplicates && (
            <li><strong>Duplicados parecidos:</strong> {profile.nearDuplicateRowCount.toLocaleString()} filas adicionales coinciden al normalizar mayúsculas, espacios y acentos; requieren revisión manual.</li>
          )}
          {incomplete.length > 0 && (
            <li><strong>Completitud:</strong> {incomplete.length} {incomplete.length === 1 ? "columna tiene" : "columnas tienen"} al menos un nulo: {incomplete.map((column) => column.name).join(", ")}.</li>
          )}
          {constant.length > 0 && (
            <li><strong>Constantes:</strong> {constant.map((column) => column.name).join(", ")} {constant.length === 1 ? "no cambia" : "no cambian"} entre filas.</li>
          )}
          {empty.length > 0 && (
            <li><strong>Vacías:</strong> {empty.map((column) => column.name).join(", ")} no contiene valores en ninguna fila.</li>
          )}
          {highNull.length > 0 && (
            <li><strong>Alta nulidad:</strong> {highNull.map((column) => column.name).join(", ")} tiene al menos 80% de valores nulos.</li>
          )}
          {outliers.length > 0 && (
            <li><strong>Valores atípicos:</strong> {outliers.map((column) => `${column.name} (${(column.outlierCount ?? 0).toLocaleString()})`).join(", ")} supera los límites IQR de 1.5.</li>
          )}
          {sentinels.length > 0 && (
            <li><strong>Valores centinela:</strong> {sentinels.map((column) => `${column.name} (${(column.sentinelCount ?? 0).toLocaleString()})`).join(", ")} usa tokens textuales que pueden representar datos ausentes.</li>
          )}
          {encoding.length > 0 && (
            <li><strong>Doble codificación UTF-8:</strong> {encoding.map((column) => `${column.name} (${(column.encodingIssueCount ?? 0).toLocaleString()})`).join(", ")} contiene texto que puede repararse de forma segura.</li>
          )}
          {booleans.length > 0 && (
            <li><strong>Booleanos:</strong> {booleans.map((column) => column.name).join(", ")} admite alias textuales que pueden canonicalizarse como `true`/`false`.</li>
          )}
          {dateCandidates.length > 0 && (
            <li><strong>Fechas detectadas:</strong> {dateCandidates.map((column) => column.name).join(", ")} coincide con un formato de fecha cerrado.</li>
          )}
          {typeDrift.length > 0 && (
            <li><strong>Tipos sugeridos:</strong> {typeDrift.map((column) => column.name).join(", ")} contiene valores que no coinciden con la sugerencia detectada.</li>
          )}
          {personal.length > 0 && (
            <li className="cleaning-signals__privacy"><strong>Posible dato personal:</strong> {personalColumns.length} {personalColumns.length === 1 ? "columna detectada" : "columnas detectadas"} por categoría agregada: {personalCategories}. Revisa su tratamiento antes de exportar o compartir.</li>
          )}
          </ul>
          {constant.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes retirar estas columnas de baja información; se conservará al menos una
                columna para que el dataset siga siendo utilizable.
              </p>
              <button type="button" onClick={onRemoveConstantColumns} disabled={busy}>
                Eliminar columnas constantes
              </button>
            </div>
          )}
          {empty.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes retirar estas columnas sin información; se conservará al menos una
                columna para que el dataset siga siendo utilizable.
              </p>
              <button type="button" onClick={onRemoveEmptyColumns} disabled={busy}>
                Eliminar columnas vacías
              </button>
            </div>
          )}
          {highNull.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Estas columnas tienen poca información disponible; la acción usa un umbral explícito
                de 80% y no elimina columnas completamente vacías ni todas las columnas del dataset.
              </p>
              <button type="button" onClick={onRemoveHighNullColumns} disabled={busy}>
                Eliminar columnas con alta nulidad
              </button>
            </div>
          )}
          {outliers.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes reemplazar los valores atípicos por la mediana de cada columna usando
                límites IQR de 1.5. La operación conserva el tipo numérico, no muestra celdas y
                es reversible desde el historial.
              </p>
              <button type="button" onClick={onImputeOutliers} disabled={busy}>
                Imputar outliers con mediana
              </button>
              <button type="button" onClick={onCapOutliers} disabled={busy}>
                Limitar outliers con IQR
              </button>
              <button type="button" className="danger-action" onClick={onDropOutliers} disabled={busy}>
                Eliminar filas atípicas
              </button>
            </div>
          )}
          {profile.columns.some((column) => column.privacySignal === "identifier") && (
            <div className="cleaning-signals__action cleaning-signals__action--privacy">
              <p>
                Retira solo las columnas marcadas como identificadoras por su encabezado. No se
                inspeccionan ni muestran celdas, se conserva al menos una columna y la operación
                queda disponible para revertir desde el historial.
              </p>
              <button type="button" onClick={onRemoveIdentifierColumns} disabled={busy}>
                Revisar identificadores detectados
              </button>
            </div>
          )}
          {personalColumns.length > 0 && (
            <div className="cleaning-signals__action cleaning-signals__action--privacy">
              <p>
                Retira correo electrónico, teléfono, dirección y nombre detectados por el encabezado.
                La acción no muestra nombres ni valores, excluye _cambios, conserva al menos una columna
                y queda disponible para revertir desde el historial.
              </p>
              <button type="button" onClick={onRemovePersonalColumns} disabled={busy}>
                Revisar datos personales detectados
              </button>
              <button type="button" onClick={onMaskPersonalValues} disabled={busy}>
                Proteger valores personales detectados
              </button>
            </div>
          )}
          {sentinels.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir los tokens ausentes conocidos a valores nulos; la operación es
                reversible y no modifica números ni la columna de trazabilidad.
              </p>
              <button type="button" onClick={onNormalizeSentinels} disabled={busy}>
                Convertir centinelas a nulos
              </button>
            </div>
          )}
          {encoding.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Corrige secuencias heredadas como `Ã©` o `â€™`; solo se aplican reparaciones
                UTF-8 inequívocas y la operación es reversible desde el historial.
              </p>
              <button type="button" onClick={onFixEncoding} disabled={busy}>
                Corregir codificación
              </button>
            </div>
          )}
          {typeDrift.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes apartar como nulos los valores que no coincidan con una sugerencia con al
                menos 90% de confianza. La acción no muestra celdas, requiere confirmación y es
                reversible desde el historial.
              </p>
              <button type="button" onClick={onNullifyInvalidTypes} disabled={busy}>
                Revisar tipos incompatibles
              </button>
            </div>
          )}
          {booleans.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                La normalización solo convierte `yes`/`no`, `sí`/`no` y `true`/`false` a
                `true`/`false`; deja intactos los valores que no reconoce.
              </p>
              <button type="button" onClick={onNormalizeBooleans} disabled={busy}>
                Normalizar booleanos
              </button>
            </div>
          )}
          {dateCandidates.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir estas columnas de texto a <strong>Datetime</strong>. Solo se usa
                un formato dominante cerrado, se omiten columnas ambiguas y la operación es
                reversible desde el historial.
              </p>
              <button type="button" onClick={onParseDates} disabled={busy}>
                Interpretar fechas detectadas
              </button>
            </div>
          )}
          {imputable.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Intenta completar solo nulos: usa el valor textual más repetido cuando aparece al
                menos dos veces y la mediana observada para columnas numéricas. No modifica blancos,
                centinelas ni _cambios.
              </p>
              <button type="button" onClick={onImputeMissingValues} disabled={busy}>
                Intentar imputación conservadora
              </button>
            </div>
          )}
          {categoricalImputable.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes completar explícitamente los nulos de texto con la categoría
                <strong> Desconocido</strong>. Esta alternativa no infiere una moda ni cambia
                números, no modifica _cambios y es reversible desde el historial.
              </p>
              <button type="button" onClick={onImputeCategoricalValues} disabled={busy}>
                Completar categorías desconocidas
              </button>
            </div>
          )}
        </>
      ) : (
        <p className="notice notice--success" role="status">No se detectaron señales de limpieza en el perfil actual.</p>
      )}
    </section>
  );
}
