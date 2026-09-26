import { useEffect, useRef, useState } from "react";

import { OperationProgressView } from "../../components/OperationProgressView";
import { ModalDialog } from "../../components/ModalDialog";
import type { DatasetPreview, DatasetProfile, HistoryState, QualityRule, SafeCorrectionOptions, SavedRecipe, TransformRecipe } from "../../bridge";
import type { ProfileStatus } from "../review/reviewModel";
import { ChangeFeedback, HistoryBar } from "./HistoryBar";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import { RevisionComparison } from "./RevisionComparison";
import { PrepareProposal, type ProposalResult } from "./PrepareProposal";
import { buildPrepareProposal } from "./proposalModel";
import type { ChangeStatus } from "./prepareModel";
import { qualityActionTargetDomId } from "../review/qualityActionPlan";
import type { QualityActionTarget } from "../review/qualityActionPlan";

import { formatPercent } from "../../format";
import { isTextType } from "../../dataTypes";

interface PreparePhaseProps {
  dataset: DatasetPreview;
  datasetRevision?: number;
  initialQualityFocus?: QualityActionTarget | null;
  onQualityFocusHandled?: () => void;
  profileStatus: ProfileStatus;
  changeStatus: ChangeStatus;
  historyStatus: HistoryState;
  qualityRules?: QualityRule[];
  recipeDraft: SavedRecipe | null;
  recipeSession: number;
  onCancelProfile: () => void;
  onCancelPrepare?: () => void;
  onRemoveDuplicates: () => void;
  onRemoveNearDuplicates?: () => void;
  onRemoveEmptyRows: () => void;
  onRemoveConstantColumns: () => void;
  onRemoveEmptyColumns: () => void;
  onRemoveHighNullColumns: () => void;
  onRemoveIdentifierColumns?: () => void;
  onRemovePersonalColumns?: () => void;
  onMaskPersonalValues?: () => void;
  onNormalizeBooleans: () => void;
  onParseDates?: () => void;
  onCastNumeric?: () => void;
  onFixEncoding?: () => void;
  onNullifyInvalidTypes?: () => void;
  onImputeMissingValues: () => void;
  onImputeCategoricalValues?: () => void;
  onImputeOutliers?: () => void;
  onCapOutliers?: () => void;
  onDropOutliers?: () => void;
  onEnableRowAudit: () => void;
  onNormalizeColumns: () => void;
  onApplyRecommended: (options: SafeCorrectionOptions) => void;
  onTrimText: () => void;
  onNormalizeText: (columns: string[], removeAccents: boolean) => void;
  onApplyTransforms: (recipe: TransformRecipe) => void;
  onRecipeDraftChange: (draft: SavedRecipe) => void;
  onUndo: () => void;
  onRedo: () => void;
}

export function PreparePhase({
  dataset,
  datasetRevision = 0,
  initialQualityFocus = null,
  onQualityFocusHandled = () => undefined,
  profileStatus,
  changeStatus,
  historyStatus,
  qualityRules = [],
  recipeDraft,
  recipeSession,
  onCancelProfile,
  onCancelPrepare,
  onRemoveNearDuplicates = () => undefined,
  onRemoveEmptyRows,
  onRemoveConstantColumns,
  onRemoveEmptyColumns,
  onRemoveHighNullColumns,
  onRemoveIdentifierColumns = () => undefined,
  onRemovePersonalColumns = () => undefined,
  onMaskPersonalValues = () => undefined,
  onNormalizeBooleans,
  onParseDates = () => undefined,
  onCastNumeric = () => undefined,
  onFixEncoding = () => undefined,
  onNullifyInvalidTypes = () => undefined,
  onImputeMissingValues,
  onImputeCategoricalValues = () => undefined,
  onImputeOutliers = () => undefined,
  onCapOutliers = () => undefined,
  onDropOutliers = () => undefined,
  onEnableRowAudit,
  onApplyRecommended,
  onNormalizeText,
  onApplyTransforms,
  onRecipeDraftChange,
  onUndo,
  onRedo,
}: PreparePhaseProps) {
  const nearDuplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.nearDuplicateRowCount : null;
  const changing = changeStatus.kind === "working";
  const textColumns = dataset.columns.filter((column) => isTextType(column.dataType) && column.name !== "_cambios");
  const hasColumns = dataset.columns.length > 0;
  const hasRowsAndColumns = dataset.rowCount > 0 && hasColumns;
  const hasRowAuditColumn = dataset.columns.some((column) => column.name === "_cambios");
  const [selectedTextColumns, setSelectedTextColumns] = useState<string[]>([]);
  const [removeAccents, setRemoveAccents] = useState(true);
  const pendingPlanComparison = useRef<{
    sourceRevision: number;
    profile: DatasetProfile;
  } | null>(null);
  const [planComparison, setPlanComparison] = useState<ProposalResult | null>(null);
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
    const available = new Set(dataset.columns
      .filter((column) => isTextType(column.dataType) && column.name !== "_cambios")
      .map((column) => column.name));
    setSelectedTextColumns((current) => current.filter((name) => available.has(name)));
  }, [dataset.columns]);

  useEffect(() => {
    const pending = pendingPlanComparison.current;
    if (!pending) return;
    if (changeStatus.kind === "error"
      || (changeStatus.kind === "applied" && changeStatus.message.startsWith("El dataset ya cumplía"))) {
      pendingPlanComparison.current = null;
      return;
    }
    if (datasetRevision > pending.sourceRevision + 1) {
      pendingPlanComparison.current = null;
      return;
    }
    if (datasetRevision !== pending.sourceRevision + 1 || profileStatus.kind !== "ready") return;
    setPlanComparison({ before: pending.profile, after: profileStatus.profile });
    pendingPlanComparison.current = null;
  }, [changeStatus, datasetRevision, profileStatus]);

  function applyProposal(options: SafeCorrectionOptions) {
    if (profileStatus.kind !== "ready" || changing) return;
    pendingPlanComparison.current = { sourceRevision: datasetRevision, profile: profileStatus.profile };
    setPlanComparison(null);
    onApplyRecommended(options);
  }

  function undoFromProposal() {
    setPlanComparison(null);
    onUndo();
  }

  useEffect(() => {
    if (!initialQualityFocus) return;
    setActiveTab("corrections");
    const target = document.getElementById(qualityActionTargetDomId(initialQualityFocus));
    if (target) {
      let ancestor = target.parentElement;
      while (ancestor) {
        if (ancestor instanceof HTMLDetailsElement) ancestor.open = true;
        ancestor = ancestor.parentElement;
      }
      target.focus({ preventScroll: true });
      target.scrollIntoView?.({ behavior: "smooth", block: "center" });
    }
    onQualityFocusHandled();
  }, [initialQualityFocus, onQualityFocusHandled]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Prepara datos consistentes</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
        </div>
      </header>
      <ChangeFeedback status={changeStatus} onCancel={onCancelPrepare} />
      {profileStatus.kind === "idle" && (
        <section className="prepare-analysis-prompt" aria-labelledby="prepare-analysis-title" role="status">
          <div>
            <h3 id="prepare-analysis-title">Analizando tus datos</h3>
            <p>En cuanto termine verás los cambios propuestos.</p>
          </div>
        </section>
      )}
      {profileStatus.kind === "loading" && (
        <section className="prepare-analysis-prompt" aria-labelledby="prepare-analysis-loading-title">
          <div>
            <h3 id="prepare-analysis-loading-title">Actualizando el análisis</h3>
          </div>
          <OperationProgressView
            progress={profileStatus.progress}
            cancellation={profileStatus.cancelRequested
              ? { kind: "requested" }
              : {
                  kind: "available",
                  onCancel: onCancelProfile,
                  ...(profileStatus.cancellationError ? { error: profileStatus.cancellationError } : {}),
                }}
          />
        </section>
      )}
      {profileStatus.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo analizar la calidad: {profileStatus.message} Reintenta desde el pie de la aplicación.
        </p>
      )}
      {profileStatus.kind === "cancelled" && (
        <p className="notice" role="status">
          El diagnóstico se canceló y no publicó resultados parciales. Puedes
          reintentarlo desde el pie de la aplicación.
        </p>
      )}
      {profileStatus.kind === "ready" && hasColumns && (
        <PrepareProposal
          items={buildPrepareProposal(profileStatus.profile, dataset)}
          columnCount={dataset.columns.length}
          busy={changing}
          canUndo={historyStatus.canUndo}
          result={planComparison}
          onApply={applyProposal}
          onUndo={undoFromProposal}
          onDismissResult={() => setPlanComparison(null)}
        />
      )}
      <details className="advanced-corrections prepare-advanced">
        <summary>
          <span>Diagnóstico y herramientas avanzadas</span>
          <small>Historial, señales individuales y transformaciones</small>
        </summary>
        <div className="advanced-corrections__content">
      <HistoryBar
        status={historyStatus}
        busy={changing}
        latestChange={changeStatus.kind === "applied" ? changeStatus.message : undefined}
        onUndo={onUndo}
        onRedo={onRedo}
      />
      <details className="review-tool review-tool--nested prepare-revision-comparison">
        <summary>
          <span>Medir el efecto de los cambios</span>
          <small>Opcional · comparar métricas agregadas entre revisiones</small>
        </summary>
        <RevisionComparison
          historyStatus={historyStatus}
          qualityRules={qualityRules}
          datasetRevision={datasetRevision}
          busy={changing || profileStatus.kind === "loading"}
        />
      </details>
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
            datasetRevision={datasetRevision}
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
      {profileStatus.kind === "ready" && (
        <details className="advanced-corrections prepare-signal-details">
          <summary>
            <span>Revisar señales individuales</span>
            <small>Diagnóstico completo y acciones específicas</small>
          </summary>
          <div className="advanced-corrections__content">
            <CleaningSignals
              profile={profileStatus.profile}
              busy={changing}
              onRemoveConstantColumns={onRemoveConstantColumns}
              onRemoveEmptyColumns={onRemoveEmptyColumns}
              onRemoveHighNullColumns={onRemoveHighNullColumns}
              onRemoveIdentifierColumns={() => setIdentifierConfirmation(true)}
              onRemovePersonalColumns={() => setPersonalConfirmation(true)}
              onMaskPersonalValues={() => setMaskPersonalConfirmation(true)}
              onNormalizeBooleans={onNormalizeBooleans}
              onParseDates={onParseDates}
              onCastNumeric={onCastNumeric}
              onFixEncoding={onFixEncoding}
              onNullifyInvalidTypes={() => setInvalidTypeConfirmation(true)}
              onImputeMissingValues={onImputeMissingValues}
              onImputeCategoricalValues={onImputeCategoricalValues}
              onImputeOutliers={onImputeOutliers}
              onCapOutliers={() => setOutlierConfirmation("cap")}
              onDropOutliers={() => setOutlierConfirmation("drop")}
            />
            {nearDuplicateCount !== null && nearDuplicateCount > 0 && (
              <section className="prepare-card" aria-labelledby="near-duplicates-title">
                <div>
                  <p className="step">Revisión con confirmación</p>
                  <h3 id="near-duplicates-title">Duplicados parecidos</h3>
                  <p>
                    Se identificaron {nearDuplicateCount.toLocaleString()} filas parecidas por normalización de texto.
                    La primera fila y las copias exactas se conservarán.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => setNearDuplicateConfirmation(true)}
                  disabled={changing}
                >
                  Revisar y eliminar parecidos
                </button>
              </section>
            )}
          </div>
        </details>
      )}
      {hasColumns && (
      <details className="advanced-corrections">
        <summary>
          <span>Más herramientas</span>
          <small>Correcciones avanzadas</small>
        </summary>
        <div className="advanced-corrections__content">
      {hasRowsAndColumns && (
      <section className="prepare-card" aria-labelledby="row-audit-title">
        <div>
          <p className="step">Trazabilidad local</p>
          <h3 id="row-audit-title">Cambios por fila</h3>
          <p>
            {hasRowAuditColumn
              ? "La columna _cambios está activa; cada corrección posterior añadirá su operación a la fila afectada."
              : "Activa una columna reservada _cambios para conservar una etiqueta breve de las correcciones posteriores."}
          </p>
        </div>
        {!hasRowAuditColumn && (
          <button type="button" onClick={onEnableRowAudit} disabled={changing}>
            Activar trazabilidad
          </button>
        )}
      </section>
      )}
      {textColumns.length > 0 && (
      <section className="prepare-card prepare-card--stacked" aria-labelledby="normalize-text-title">
        <div>
          <p className="step">Requiere selección</p>
          <h3 id="normalize-text-title">Normalizar texto</h3>
          <p>
            Convierte a minúsculas, compacta espacios y, opcionalmente, elimina acentos. Puede
            unir categorías que antes eran distintas; elige las columnas conscientemente.
          </p>
        </div>
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
      </section>
      )}
      {hasRowsAndColumns && (
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
      )}
        </div>
      </details>
      )}
      </div>
      )}
        </div>
      </details>
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
  onNormalizeBooleans,
  onParseDates,
  onCastNumeric,
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
  onNormalizeBooleans: () => void;
  onParseDates: () => void;
  onCastNumeric: () => void;
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
    (column) => column.name !== "_cambios" && isTextType(column.dataType) && column.suggestedType === "date",
  );
  const numericCandidates = profile.columns.filter(
    (column) => column.name !== "_cambios" && isTextType(column.dataType) &&
      (column.suggestedType === "integer" || column.suggestedType === "decimal") &&
      (column.typeMatchPercentage ?? 0) > 90 &&
      column.privacySignal !== "identifier",
  );
  const typeDrift = profile.columns.filter(
    (column) => (column.invalidTypeCount ?? 0) > 0,
  );
  const outliers = profile.columns.filter(
    (column) => (column.outlierCount ?? 0) > 0 && column.name !== "_cambios",
  );
  const categoricalImputable = profile.columns.filter(
    (column) => isTextType(column.dataType) && column.nullCount > 0 && column.name !== "_cambios",
  );
  const hasNullActions = empty.length > 0 || highNull.length > 0 || imputable.length > 0 || categoricalImputable.length > 0;
  const dataColumns = profile.columns.filter((column) => column.name !== "_cambios");
  const totalNullCount = dataColumns.reduce((total, column) => total + column.nullCount, 0);
  const columnsWithNulls = dataColumns.filter((column) => column.nullCount > 0);
  const totalCellCount = profile.rowCount * dataColumns.length;
  const datasetCompleteness = totalCellCount === 0
    ? 100
    : ((totalCellCount - totalNullCount) / totalCellCount) * 100;
  const personal = profile.columns.filter((column) => column.privacySignal !== null);
  const personalColumns = profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && column.name !== "_cambios");
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);
  const hasOtherSignals = profile.duplicateRowCount > 0 || nearDuplicates || constant.length > 0 || encoding.length > 0 || booleans.length > 0 || dateCandidates.length > 0 || numericCandidates.length > 0 || typeDrift.length > 0 || outliers.length > 0 || personal.length > 0;

  return (
    <section className="prepare-card prepare-card--stacked cleaning-signals" aria-labelledby="cleaning-signals-title">
      <div>
        <p className="step">Catálogo de limpieza</p>
        <h3 id="cleaning-signals-title">Señales para revisar</h3>
        <p>Las señales usan solo esquema y métricas agregadas; no muestran celdas ni valores personales.</p>
      </div>
      <section
        id={qualityActionTargetDomId("missingValues")}
        className="missing-data-plan"
        aria-labelledby="missing-data-title"
        tabIndex={-1}
      >
        <div className="missing-data-plan__heading">
          <div>
            <p className="step">Ruta guiada</p>
            <h4 id="missing-data-title">Valores nulos y datos faltantes</h4>
            <p>Conservamos los nulos hasta que elijas una corrección.</p>
          </div>
          <div className="missing-data-plan__metrics" aria-label="Resumen de valores nulos">
            <span><strong>{totalNullCount.toLocaleString()}</strong> nulos</span>
            <span><strong>{columnsWithNulls.length.toLocaleString()}</strong> columnas afectadas</span>
            <span><strong>{formatPercent(datasetCompleteness, 1)}</strong> completitud total</span>
          </div>
        </div>
        {totalNullCount === 0 && sentinels.length === 0 ? (
          <p className="notice notice--success" role="status">
            No se detectaron nulos ni marcadores conocidos de datos ausentes.
          </p>
        ) : totalNullCount === 0 && sentinels.length > 0 ? (
          <p className="notice" role="note">
            Se detectaron marcadores de ausencia. Puedes convertirlos desde el plan de correcciones de arriba.
          </p>
        ) : hasNullActions ? (
          <ol className="missing-data-plan__steps">
            {(empty.length > 0 || highNull.length > 0) && (
              <li>
                <div>
                  <strong>Retirar columnas sin información útil</strong>
                  <p>
                    Las columnas totalmente vacías pueden retirarse directamente. Las que tienen
                    al menos 80% de nulos se ofrecen por separado para evitar pérdida accidental.
                  </p>
                </div>
                <div className="missing-data-plan__actions">
                  {empty.length > 0 && (
                    <button type="button" onClick={onRemoveEmptyColumns} disabled={busy}>
                      Eliminar columnas vacías
                    </button>
                  )}
                  {highNull.length > 0 && (
                    <button type="button" onClick={onRemoveHighNullColumns} disabled={busy}>
                      Eliminar columnas con alta nulidad
                    </button>
                  )}
                </div>
              </li>
            )}
            {(imputable.length > 0 || categoricalImputable.length > 0) && (
              <li>
                <div>
                  <strong>Completar solo cuando tenga sentido</strong>
                  <p>Usa la mediana en números y la moda en texto; nunca inventa valores.</p>
                </div>
                <div className="missing-data-plan__actions">
                  {imputable.length > 0 && (
                    <button type="button" onClick={onImputeMissingValues} disabled={busy}>
                      Intentar imputación conservadora
                    </button>
                  )}
                  {categoricalImputable.length > 0 && (
                    <button type="button" onClick={onImputeCategoricalValues} disabled={busy}>
                      Completar categorías desconocidas
                    </button>
                  )}
                </div>
              </li>
            )}
          </ol>
        ) : null}
        <details className="missing-data-plan__help">
          <summary>Cómo decide Columnia</summary>
          <div>
            <p>Columnia conserva los nulos por defecto. Un nulo no siempre es un error. Primero normaliza los marcadores de ausencia y después decide si conviene completar o retirar.</p>
            <p>Los espacios en blanco no son nulos. Puedes recortarlos con las correcciones recomendadas; todas las acciones son reversibles desde el historial.</p>
          </div>
        </details>
      </section>
      {hasOtherSignals ? (
        <details className="detected-signals">
          <summary>
            <span>Otras señales detectadas</span>
            <small>Duplicados, privacidad, tipos y valores atípicos</small>
          </summary>
          <div className="detected-signals__content">
          <ul className="cleaning-signals__list" aria-label="Señales de limpieza detectadas">
          {profile.duplicateRowCount > 0 && (
            <li><strong>Duplicados exactos:</strong> {profile.duplicateRowCount.toLocaleString()} filas adicionales; puedes eliminarlas de forma reversible.</li>
          )}
          {nearDuplicates && (
            <li><strong>Duplicados parecidos:</strong> {profile.nearDuplicateRowCount.toLocaleString()} filas adicionales coinciden al normalizar mayúsculas, espacios y acentos; requieren revisión manual.</li>
          )}
          {constant.length > 0 && (
            <li><strong>Constantes:</strong> {constant.map((column) => column.name).join(", ")} {constant.length === 1 ? "no cambia" : "no cambian"} entre filas.</li>
          )}
          {outliers.length > 0 && (
            <li><strong>Valores atípicos:</strong> {outliers.map((column) => `${column.name} (${(column.outlierCount ?? 0).toLocaleString()})`).join(", ")} supera los límites IQR de 1.5.</li>
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
          {numericCandidates.length > 0 && (
            <li><strong>Números detectados:</strong> {numericCandidates.map((column) => column.name).join(", ")} admite una conversión numérica segura.</li>
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
            <div
              id={qualityActionTargetDomId("incompatibleTypes")}
              className="cleaning-signals__action"
              tabIndex={-1}
            >
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
          {numericCandidates.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir estas columnas de texto a números. Se exige al menos 90% de
                valores numéricos válidos, se rechaza la pérdida de precisión y se conservan los
                identificadores o códigos con ceros iniciales.
              </p>
              <button type="button" onClick={onCastNumeric} disabled={busy}>
                Convertir números detectados
              </button>
            </div>
          )}
          </div>
        </details>
      ) : (
        <p className="notice notice--success" role="status">No se detectaron otras señales de limpieza en el perfil actual.</p>
      )}
    </section>
  );
}
