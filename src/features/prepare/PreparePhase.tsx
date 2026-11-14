import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { OperationProgressView } from "../../components/OperationProgressView";
import { ModalDialog } from "../../components/ModalDialog";
import { busyDescribedBy } from "../../busyReasons";
import { previewSafeCorrections } from "../../bridge";
import type { DatasetPreview, DatasetProfile, HistoryState, QualityRule, SafeCorrectionOptions, SavedRecipe, TransformRecipe } from "../../bridge";
import type { ProfileStatus } from "../review/reviewModel";
import { ChangeFeedback, ChangeResultLine, HistoryBar } from "./HistoryBar";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import { RevisionComparison } from "./RevisionComparison";
import { CombineWithFileSection } from "./CombineWithFileSection";
import type { ReviewComparison } from "../review/useReviewController";
import { PrepareProposal, type ProposalResult } from "./PrepareProposal";
import { buildPrepareProposal, looksLikeIdentifier, type ProposalItem } from "./proposalModel";
import {
  cellCount,
  conversionNullEstimate,
  dateConversionCandidates,
  numericConversionCandidates,
  type ChangeStatus,
  type TypeConversionKind,
} from "./prepareModel";
import { qualityActionTargetDomId } from "../review/qualityActionPlan";
import type { QualityActionTarget } from "../review/qualityActionPlan";

import { formatPercent } from "../../format";
import { plural } from "../../plural";
import { isTextType } from "../../dataTypes";
import { isRowAuditColumn } from "../../rowAudit";

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
  onApplyRecommended: (options: SafeCorrectionOptions) => void;
  onNormalizeText: (columns: string[], removeAccents: boolean) => void;
  onApplyTransforms: (recipe: TransformRecipe) => void;
  onRecipeDraftChange: (draft: SavedRecipe) => void;
  /** UX-13: an unfinished (invalid) recipe edit kept outside the phase. */
  recipeEdit?: SavedRecipe | null;
  onRecipeEditChange?: (draft: SavedRecipe | null) => void;
  onUndo: () => void;
  onRedo: () => void;
  /**
   * The comparison with another file, shared with Revisar: joining,
   * consolidating and resolving conflicts are done here, with history.
   */
  comparison?: ReviewComparison;
  /** «Combinar en Preparar» from Revisar: open and focus that section. */
  focusCombine?: boolean;
  onCombineFocusHandled?: () => void;
  /** Runs the quality analysis again after it failed or was cancelled. */
  onRetryProfile?: () => void;
  /** «Continuar a Explorar» from the result of the proposal. */
  onContinue?: () => void;
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
  onRemoveNearDuplicates,
  onRemoveEmptyRows,
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
  onEnableRowAudit,
  onApplyRecommended,
  onNormalizeText,
  onApplyTransforms,
  onRecipeDraftChange,
  recipeEdit,
  onRecipeEditChange,
  onUndo,
  onRedo,
  comparison,
  focusCombine = false,
  onCombineFocusHandled = () => undefined,
  onRetryProfile,
  onContinue,
}: PreparePhaseProps) {
  const nearDuplicateCount = profileStatus.kind === "ready" ? profileStatus.profile.nearDuplicateRowCount : null;
  const changing = changeStatus.kind === "working";
  const textColumns = dataset.columns.filter((column) => isTextType(column.dataType) && !isRowAuditColumn(column.name));
  const hasColumns = dataset.columns.length > 0;
  const hasRowsAndColumns = dataset.rowCount > 0 && hasColumns;
  const hasRowAuditColumn = dataset.columns.some((column) => isRowAuditColumn(column.name));
  const [selectedTextColumns, setSelectedTextColumns] = useState<string[]>([]);
  const [removeAccents, setRemoveAccents] = useState(true);
  const pendingPlanComparison = useRef<{
    sourceRevision: number;
    profile: DatasetProfile;
  } | null>(null);
  // UX-13: kept here while «Transformar columnas» is closed (the editor
  // unmounts), and in App across phases when it passes `recipeEdit`.
  // Tied to the recipe session: another dataset or recipe starts clean.
  const [localEdit, setLocalEdit] = useState<{ session: number; draft: SavedRecipe } | null>(null);
  const unfinishedEdit = recipeEdit !== undefined
    ? recipeEdit
    : localEdit?.session === recipeSession ? localEdit.draft : null;
  const changeUnfinishedEdit = (draft: SavedRecipe | null) => {
    setLocalEdit(draft ? { session: recipeSession, draft } : null);
    onRecipeEditChange?.(draft);
  };
  const [planComparison, setPlanComparison] = useState<(ProposalResult & { revision: number }) | null>(null);
  // The result belongs to the revision it describes: any later change, such
  // as Deshacer from the history panel, brings back the proposal (FUN-11).
  const currentPlanComparison = planComparison?.revision === datasetRevision ? planComparison : null;
  // The result line closes for one result; the next change brings it back.
  const [dismissedResult, setDismissedResult] = useState<ChangeStatus | null>(null);
  const [transformOpen, setTransformOpen] = useState(false);
  const [nearDuplicateConfirmation, setNearDuplicateConfirmation] = useState(false);
  const [identifierConfirmation, setIdentifierConfirmation] = useState(false);
  const [personalConfirmation, setPersonalConfirmation] = useState(false);
  const [maskPersonalConfirmation, setMaskPersonalConfirmation] = useState(false);
  const [invalidTypeConfirmation, setInvalidTypeConfirmation] = useState(false);
  const [conversionConfirmation, setConversionConfirmation] = useState<TypeConversionKind | null>(null);
  // TXT-01: changes can be undone only while the history keeps snapshots.
  const undoNote = undoAvailabilityNote(historyStatus.snapshotsEnabled);
  const [outlierConfirmation, setOutlierConfirmation] = useState<"impute" | "cap" | "drop" | null>(null);
  const identifierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => column.privacySignal === "identifier")
    : [];
  const personalColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && !isRowAuditColumn(column.name))
    : [];
  const typeDriftColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.invalidTypeCount ?? 0) > 0)
    : [];
  const outlierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.outlierCount ?? 0) > 0 && !isRowAuditColumn(column.name))
    : [];
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);
  const outlierTotal = outlierColumns.reduce((total, column) => total + (column.outlierCount ?? 0), 0);
  const outlierTotalPhrase = `${outlierTotal === 1 ? "el valor atípico" : `los ${outlierTotal.toLocaleString()} valores atípicos`} de ${listNames(outlierColumns.map((column) => column.name))}`;
  const conversionLoss = profileStatus.kind === "ready" && conversionConfirmation !== null
    ? conversionNullEstimate(profileStatus.profile, conversionConfirmation)
    : [];
  const conversionLossTotal = conversionLoss.reduce((total, column) => total + column.count, 0);

  /** Converts right away when nothing would become empty; asks first otherwise. */
  function requestConversion(kind: TypeConversionKind) {
    const loss = profileStatus.kind === "ready" ? conversionNullEstimate(profileStatus.profile, kind) : [];
    if (loss.length === 0) {
      if (kind === "numeric") onCastNumeric?.();
      else onParseDates?.();
      return;
    }
    setConversionConfirmation(kind);
  }

  useEffect(() => {
    const available = new Set(dataset.columns
      .filter((column) => isTextType(column.dataType) && !isRowAuditColumn(column.name))
      .map((column) => column.name));
    setSelectedTextColumns((current) => current.filter((name) => available.has(name)));
  }, [dataset.columns]);

  useEffect(() => {
    const pending = pendingPlanComparison.current;
    if (!pending) return;
    if (changeStatus.kind === "error" || changeStatus.kind === "cancelled"
      || (changeStatus.kind === "applied" && changeStatus.unchanged === true)) {
      pendingPlanComparison.current = null;
      return;
    }
    if (datasetRevision > pending.sourceRevision + 1) {
      pendingPlanComparison.current = null;
      return;
    }
    if (datasetRevision !== pending.sourceRevision + 1 || changeStatus.kind !== "applied") return;
    // UX-10: the result and «Deshacer» appear as soon as the change is
    // applied; the figures after it arrive with the new profile.
    setPlanComparison({
      before: pending.profile,
      after: profileStatus.kind === "ready" && profileStatus.profile !== pending.profile ? profileStatus.profile : null,
      changes: changeStatus.changes ?? [],
      revision: datasetRevision,
    });
    pendingPlanComparison.current = null;
  }, [changeStatus, datasetRevision, profileStatus]);

  useEffect(() => {
    if (profileStatus.kind !== "ready") return;
    setPlanComparison((current) => current && current.after === null && current.revision === datasetRevision
      ? { ...current, after: profileStatus.profile }
      : current);
  }, [profileStatus, datasetRevision]);

  function applyProposal(options: SafeCorrectionOptions) {
    if (profileStatus.kind !== "ready" || changing) return;
    pendingPlanComparison.current = { sourceRevision: datasetRevision, profile: profileStatus.profile };
    setPlanComparison(null);
    onApplyRecommended(options);
  }

  // ACC: the button pressed to undo disappears with the result it belonged
  // to; when the change ends, the focus goes to the result line («Rehacer»).
  // The status when the undo was asked for: the next result takes the focus.
  const [focusAfterStatus, setFocusAfterStatus] = useState<ChangeStatus | null>(null);
  const focusResultLine = focusAfterStatus !== null && changeStatus !== focusAfterStatus
    && changeStatus.kind === "applied" && changeStatus.origin !== undefined;
  const clearResultLineFocus = useCallback(() => setFocusAfterStatus(null), []);
  const requestResultLineFocus = () => setFocusAfterStatus(changeStatus);

  function undoFromProposal() {
    setPlanComparison(null);
    requestResultLineFocus();
    onUndo();
  }

  const proposalItems = useMemo(
    () => (profileStatus.kind === "ready" ? buildPrepareProposal(profileStatus.profile, dataset) : []),
    [profileStatus, dataset],
  );
  const proposalImpute = proposalItems.find((item) => item.id === "impute") ?? null;
  const emptyRowBound = profileStatus.kind === "ready" ? emptyRowUpperBound(profileStatus.profile) : null;

  useEffect(() => {
    if (!focusCombine) return;
    const target = document.getElementById("combine-title");
    if (target) {
      const disclosure = target.parentElement;
      if (disclosure instanceof HTMLDetailsElement) disclosure.open = true;
      target.focus({ preventScroll: true });
      target.scrollIntoView?.({ behavior: "smooth", block: "start" });
    }
    onCombineFocusHandled();
  }, [focusCombine, onCombineFocusHandled]);

  useEffect(() => {
    if (!initialQualityFocus) return;
    // The target sits inside «Corregir columna por columna»: open it first.
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
          <p className="phase-lead">Corrige y transforma con historial: cada cambio se puede deshacer.</p>
        </div>
      </header>
      <div className="prepare-layout">
      <div className="prepare-layout__main">
      <ChangeFeedback status={changeStatus} onCancel={onCancelPrepare} />
      {/* AUD-057: the proposal already shows its own result with «Deshacer». */}
      {currentPlanComparison === null && dismissedResult !== changeStatus && (
        <ChangeResultLine
          status={changeStatus}
          canUndo={historyStatus.snapshotsEnabled && historyStatus.canUndo}
          canRedo={historyStatus.snapshotsEnabled && historyStatus.canRedo}
          busy={changing}
          focusAction={focusResultLine}
          onFocusHandled={clearResultLineFocus}
          onUndo={() => { requestResultLineFocus(); onUndo(); }}
          onRedo={() => { requestResultLineFocus(); onRedo(); }}
          onDismiss={() => setDismissedResult(changeStatus)}
        />
      )}
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
      {/* As in Revisar, the way to try again sits in the notice itself. */}
      {profileStatus.kind === "error" && (
        <div className="notice notice--error profile-error" role="alert">
          <p>No se pudo analizar la calidad: {profileStatus.message}</p>
          <p>Sin el análisis no hay cambios que proponer. Comprueba que el archivo sigue donde estaba y vuelve a intentarlo.</p>
          {onRetryProfile && (
            <button type="button" className="secondary-action" onClick={onRetryProfile}>
              Reintentar análisis
            </button>
          )}
        </div>
      )}
      {profileStatus.kind === "cancelled" && (
        <div className="notice profile-error" role="status">
          <p>El análisis se canceló y no publicó resultados parciales.</p>
          {onRetryProfile && (
            <button type="button" className="secondary-action" onClick={onRetryProfile}>
              Reintentar análisis
            </button>
          )}
        </div>
      )}
      {(profileStatus.kind === "ready" || currentPlanComparison !== null) && hasColumns && (
        <PrepareProposal
          items={proposalItems}
          columnCount={dataset.columns.length}
          busy={changing}
          canUndo={historyStatus.canUndo}
          result={currentPlanComparison}
          onApply={applyProposal}
          onPreview={previewSafeCorrections}
          onUndo={undoFromProposal}
          onDismissResult={() => setPlanComparison(null)}
          onContinue={onContinue}
        />
      )}
      {/* One level of disclosures under the proposal: each is a kind of change. */}
      <section className="prepare-more" aria-labelledby="prepare-more-title">
        <div className="prepare-more__heading">
          <h3 id="prepare-more-title">Más cambios</h3>
          <p>Correcciones sueltas, transformaciones y combinaciones. Cada cambio queda en el historial.</p>
        </div>
        {hasColumns && (
          <details className="prepare-tool prepare-tool--signals" aria-describedby={busyDescribedBy(changing)}>
            <summary>
              <span>Corregir columna por columna</span>
              <small>
                {profileStatus.kind === "ready"
                  ? "Nulos, duplicados, tipos, privacidad y valores atípicos"
                  : profileStatus.kind === "idle" || profileStatus.kind === "loading"
                    ? "Disponible cuando termine el análisis"
                    : "Necesita el análisis de calidad"}
              </small>
            </summary>
            <div className="prepare-tool__content">
              {profileStatus.kind === "ready" ? (
                <>
                  <CleaningSignals
                    profile={profileStatus.profile}
                    proposalImpute={proposalImpute}
                    busy={changing}
                    undoNote={undoNote}
                    onRemoveConstantColumns={onRemoveConstantColumns}
                    onRemoveEmptyColumns={onRemoveEmptyColumns}
                    onRemoveHighNullColumns={onRemoveHighNullColumns}
                    onRemoveIdentifierColumns={onRemoveIdentifierColumns && (() => setIdentifierConfirmation(true))}
                    onRemovePersonalColumns={onRemovePersonalColumns && (() => setPersonalConfirmation(true))}
                    onMaskPersonalValues={onMaskPersonalValues && (() => setMaskPersonalConfirmation(true))}
                    onNormalizeBooleans={onNormalizeBooleans}
                    onParseDates={onParseDates && (() => requestConversion("dates"))}
                    onCastNumeric={onCastNumeric && (() => requestConversion("numeric"))}
                    onFixEncoding={onFixEncoding}
                    onNullifyInvalidTypes={onNullifyInvalidTypes && (() => setInvalidTypeConfirmation(true))}
                    onImputeMissingValues={onImputeMissingValues}
                    onImputeCategoricalValues={onImputeCategoricalValues}
                    onImputeOutliers={onImputeOutliers && (() => setOutlierConfirmation("impute"))}
                    onCapOutliers={onCapOutliers && (() => setOutlierConfirmation("cap"))}
                    onDropOutliers={onDropOutliers && (() => setOutlierConfirmation("drop"))}
                  />
                  {nearDuplicateCount !== null && nearDuplicateCount > 0 && onRemoveNearDuplicates && (
                    <section className="prepare-card" aria-labelledby="near-duplicates-title">
                      <div>
                        <h3 id="near-duplicates-title">Duplicados parecidos</h3>
                        <p>
                          Se identificaron {plural(nearDuplicateCount, "fila parecida", "filas parecidas")} por normalización de texto.
                          La primera fila y las copias exactas se conservarán.
                        </p>
                      </div>
                      <EffectButton busy={changing} onClick={() => setNearDuplicateConfirmation(true)}>
                        {nearDuplicateCount === 1 ? "Eliminar 1 fila parecida…" : `Eliminar ${nearDuplicateCount.toLocaleString()} filas parecidas…`}
                      </EffectButton>
                    </section>
                  )}
                </>
              ) : (
                <p className="prepare-tool__empty">
                  {profileStatus.kind === "idle" || profileStatus.kind === "loading"
                    ? "Las señales por columna aparecen cuando termine el análisis de arriba."
                    : "Sin el análisis de calidad no hay señales por columna. Reinténtalo arriba."}
                </p>
              )}
            </div>
          </details>
        )}
        {(textColumns.length > 0 || hasRowsAndColumns) && (
          <details className="prepare-tool" aria-describedby={busyDescribedBy(changing)}>
            <summary>
              <span>Limpiar texto y filas</span>
              <small>Normalizar texto, quitar filas vacías y anotar cambios por fila</small>
            </summary>
            <div className="prepare-tool__content">
              {textColumns.length > 0 && (
                <section className="prepare-card prepare-card--stacked" aria-labelledby="normalize-text-title">
                  <div>
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
                    {selectedTextColumns.length === 0 && (
                      <p id="normalize-text-reason" className="prepare-card__reason">Elige al menos una columna para normalizar.</p>
                    )}
                    <EffectButton
                      busy={changing}
                      blocked={selectedTextColumns.length === 0}
                      describedBy={selectedTextColumns.length === 0 ? "normalize-text-reason" : undefined}
                      onClick={() => onNormalizeText(selectedTextColumns, removeAccents)}
                    >
                      {selectedTextColumns.length === 0
                        ? "Normalizar texto seleccionado"
                        : `Normalizar ${plural(selectedTextColumns.length, "columna", "columnas")}`}
                    </EffectButton>
                  </div>
                </section>
              )}
              {hasRowsAndColumns && (
                <section className="prepare-card" aria-labelledby="empty-rows-title">
                  <div>
                    <h3 id="empty-rows-title">Filas completamente vacías</h3>
                    <p id="empty-rows-description">
                      Elimina filas cuyos valores son todos nulos o texto en blanco. La operación conserva el orden.{undoNote}
                      {emptyRowBound === 0
                        ? " El análisis no encontró ninguna: hay columnas sin ningún nulo ni texto en blanco."
                        : emptyRowBound !== null
                          ? ` Según el análisis puede haber hasta ${plural(emptyRowBound, "fila vacía", "filas vacías")}; al aplicarlo se dice cuántas se eliminaron.`
                          : ""}
                    </p>
                  </div>
                  <EffectButton
                    busy={changing}
                    blocked={emptyRowBound === 0}
                    describedBy="empty-rows-description"
                    onClick={onRemoveEmptyRows}
                  >
                    {emptyRowBound !== null && emptyRowBound > 0
                      ? `Eliminar hasta ${plural(emptyRowBound, "fila vacía", "filas vacías")}`
                      : "Eliminar filas vacías"}
                  </EffectButton>
                </section>
              )}
              {hasRowsAndColumns && (
                <section className="prepare-card" aria-labelledby="row-audit-title">
                  <div>
                    <h3 id="row-audit-title">Cambios por fila</h3>
                    <p>
                      {hasRowAuditColumn
                        ? "La columna _cambios está activa; cada corrección posterior añadirá su operación a la fila afectada."
                        : "Activa una columna reservada _cambios para conservar una etiqueta breve de las correcciones posteriores."}
                    </p>
                  </div>
                  {!hasRowAuditColumn && (
                    <EffectButton busy={changing} onClick={onEnableRowAudit}>
                      Activar trazabilidad
                    </EffectButton>
                  )}
                </section>
              )}
            </div>
          </details>
        )}
        {/* The editor mounts only while open; an unfinished edit is kept
            above it (UX-13), so closing the disclosure never loses it. */}
        <details
          className="prepare-tool prepare-tool--transform"
          open={transformOpen}
          onToggle={(event) => setTransformOpen(event.currentTarget.open)}
          aria-describedby={busyDescribedBy(changing)}
        >
          <summary
            onClick={(event) => {
              event.preventDefault();
              setTransformOpen((current) => !current);
            }}
          >
            <span>Transformar columnas</span>
            <small>Renombrar, convertir, filtrar y más, en una receta</small>
          </summary>
          {transformOpen && (
            <div className="prepare-tool__content">
              <TransformRecipeEditor
                key={recipeSession}
                dataset={dataset}
                datasetRevision={datasetRevision}
                busy={changing}
                initialDraft={unfinishedEdit ?? recipeDraft}
                onApply={onApplyTransforms}
                onDraftChange={onRecipeDraftChange}
                onUnfinishedChange={changeUnfinishedEdit}
              />
            </div>
          )}
        </details>
        {comparison && (
          <details className="prepare-tool prepare-combine" aria-describedby={busyDescribedBy(changing)}>
            {/* «Combinar en Preparar» opens this and focuses its summary. */}
            <summary id="combine-title">
              <span>Combinar con otro archivo</span>
              <small>Unir por clave, añadir filas nuevas o resolver conflictos</small>
            </summary>
            <div className="prepare-tool__content">
              <CombineWithFileSection
                comparison={comparison}
                datasetColumns={dataset.columns}
                datasetRevision={datasetRevision}
                historyStatus={historyStatus}
                busy={changing}
              />
            </div>
          </details>
        )}
        <details className="prepare-tool prepare-revision-comparison" aria-describedby={busyDescribedBy(changing)}>
          <summary>
            <span>Medir el efecto de los cambios</span>
            <small>Comparar métricas entre dos versiones del historial</small>
          </summary>
          <div className="prepare-tool__content">
            <RevisionComparison
              historyStatus={historyStatus}
              qualityRules={qualityRules}
              datasetRevision={datasetRevision}
              busy={changing || profileStatus.kind === "loading"}
            />
          </div>
        </details>
      </section>
      </div>
      {/* The history stays in view: every change here can be undone from it. */}
      <aside className="prepare-layout__aside" aria-label="Historial de cambios" aria-describedby={busyDescribedBy(changing)}>
        <HistoryBar
          status={historyStatus}
          busy={changing}
          latestChange={changeStatus.kind === "applied" ? changeStatus.message : undefined}
          onUndo={onUndo}
          onRedo={onRedo}
        />
      </aside>
      </div>
      {nearDuplicateConfirmation && nearDuplicateCount !== null && nearDuplicateCount > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="near-duplicates-confirm-title"
          describedBy="near-duplicates-confirm-description"
          onDismiss={() => setNearDuplicateConfirmation(false)}
        >
          <h3 id="near-duplicates-confirm-title">Eliminar duplicados parecidos</h3>
          <p id="near-duplicates-confirm-description">
            Se eliminarán hasta {plural(nearDuplicateCount, "fila que coincide", "filas que coinciden")} después
            de normalizar espacios, mayúsculas y acentos. No se mostrarán valores del dataset.
            Se conservará la primera fila de cada grupo, el orden actual y las copias exactas.{undoNote}
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
                onRemoveNearDuplicates?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
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
          <h3 id="identifier-confirm-title">Retirar identificadores detectados</h3>
          <p id="identifier-confirm-description">
            Se retirarán {identifierColumns.length === 1 ? "1 columna identificadora" : `${identifierColumns.length} columnas identificadoras`} detectadas por el nombre del encabezado: {identifierColumns.map((column) => column.name).join(", ")}.
            No se mostrarán celdas ni valores del dataset. Se conservará al menos una columna.{undoNote}
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
                onRemoveIdentifierColumns?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
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
          <h3 id="personal-confirm-title">Retirar datos personales detectados</h3>
          <p id="personal-confirm-description">
            Se retirarán {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. Se excluirá _cambios,
            y se conservará al menos una columna utilizable.{undoNote}
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
                onRemovePersonalColumns?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
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
          <h3 id="personal-mask-confirm-title">Ocultar datos personales detectados</h3>
          <p id="personal-mask-confirm-description">
            Se sustituirán los valores no nulos de {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} por el texto <code>[REDACTED]</code> («oculto»): la celda conserva que había un valor, pero ya no se puede leer. Columnas identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. Se conservarán las columnas,
            y se excluirá _cambios.{undoNote}
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
                onMaskPersonalValues?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              Ocultar valores personales
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
          <h3 id="invalid-types-confirm-title">Apartar valores incompatibles</h3>
          <p id="invalid-types-confirm-description">
            Se convertirán en nulos los valores que no coincidan con un tipo sugerido con al menos 90% de coincidencia en {typeDriftColumns.length === 1 ? "1 columna" : `${typeDriftColumns.length} columnas`}. Las celdas no se mostrarán.{undoNote}
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
                onNullifyInvalidTypes?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              Apartar valores incompatibles
            </button>
          </div>
        </ModalDialog>
      )}
      {conversionConfirmation !== null && conversionLossTotal > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="conversion-confirm-title"
          describedBy="conversion-confirm-description"
          onDismiss={() => setConversionConfirmation(null)}
        >
          <h3 id="conversion-confirm-title">
            {conversionConfirmation === "numeric" ? "Convertir números detectados" : "Interpretar fechas detectadas"}
          </h3>
          <p id="conversion-confirm-description">
            {cellCount(conversionLossTotal)} que no {conversionConfirmation === "numeric" ? "son números" : "son fechas"} {conversionLossTotal === 1 ? "quedará nula" : "quedarán nulas"}:{" "}
            {conversionLoss.map((column) => `${column.name} (${column.count.toLocaleString()})`).join(", ")}. Puedes deshacerlo desde el historial.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setConversionConfirmation(null)}>
              Cancelar
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                const kind = conversionConfirmation;
                setConversionConfirmation(null);
                if (kind === "numeric") onCastNumeric?.();
                else onParseDates?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              Convertir y dejar nulas
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
          <h3 id="outliers-confirm-title">
            {OUTLIER_ACTION_TITLES[outlierConfirmation]}
          </h3>
          <p id="outliers-confirm-description">
            {outlierConfirmation === "impute"
              ? `${outlierTotal === 1 ? "Se reemplazará" : "Se reemplazarán"} por la mediana de su columna ${outlierTotalPhrase}.`
              : outlierConfirmation === "cap"
                ? `${outlierTotal === 1 ? "Se llevará" : "Se llevarán"} al límite del rango normal de su columna ${outlierTotalPhrase}.`
                : outlierTotal === 1
                  ? `Se eliminará la fila que contiene ${outlierTotalPhrase}.`
                  : `Se eliminarán las filas que contienen alguno de ${outlierTotalPhrase}.`}
            {" "}{OUTLIER_RULE}
            {" "}No se mostrarán celdas ni valores del dataset.{undoNote}
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
                if (action === "impute") onImputeOutliers?.();
                else if (action === "cap") onCapOutliers?.();
                else onDropOutliers?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              {OUTLIER_ACTION_TITLES[outlierConfirmation]}
            </button>
          </div>
        </ModalDialog>
      )}
    </>
  );
}

/**
 * A Preparar action that stays in the tab order while it cannot run
 * (aria-disabled, with the reason read with it), so pressing it never sends
 * the focus to the page body.
 */
function EffectButton({
  busy,
  blocked = false,
  describedBy,
  className,
  onClick,
  children,
}: {
  busy: boolean;
  blocked?: boolean;
  describedBy?: string;
  className?: string;
  onClick: () => void;
  children: ReactNode;
}) {
  const inert = busy || blocked;
  return (
    <button
      type="button"
      className={className}
      aria-disabled={inert || undefined}
      aria-describedby={busyDescribedBy(busy, describedBy)}
      onClick={() => { if (!inert) onClick(); }}
    >
      {children}
    </button>
  );
}

/** «x, y y z»: the names an action will touch, in one phrase. */
function listNames(names: string[]): string {
  return names.length <= 1 ? names.join("") : `${names.slice(0, -1).join(", ")} y ${names.at(-1)}`;
}

/**
 * The most rows that can be completely empty: no more than the column with
 * the fewest empty cells (nulls plus blank text). 0 means there are none.
 */
function emptyRowUpperBound(profile: DatasetProfile): number | null {
  const columns = profile.columns.filter((column) => !isRowAuditColumn(column.name));
  if (columns.length === 0 || profile.rowCount === 0) return null;
  return Math.min(...columns.map((column) => column.nullCount + (column.emptyCount ?? 0)));
}

/** How Columnia decides that a value is atypical, in words. */
const OUTLIER_RULE = "Un valor es atípico si queda fuera del rango normal de su columna (regla IQR: más de 1,5 veces el rango intercuartílico por debajo del primer cuartil o por encima del tercero).";

/** UX-14: the three outlier actions all change data and ask first. */
const OUTLIER_ACTION_TITLES = {
  impute: "Reemplazar valores atípicos por la mediana",
  cap: "Limitar valores atípicos",
  drop: "Eliminar filas atípicas",
} as const;

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

/** The sentence that says whether a change can be undone (TXT-01). */
function undoAvailabilityNote(snapshotsEnabled: boolean): string {
  return snapshotsEnabled
    ? " Podrás deshacerlo desde el historial."
    : " El historial reversible está desactivado: este cambio no se podrá deshacer.";
}

function CleaningSignals({
  profile,
  proposalImpute,
  busy,
  undoNote,
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
  /** The «Rellenar nulos» item of the proposal, to say how its figure relates to this one. */
  proposalImpute: ProposalItem | null;
  busy: boolean;
  undoNote: string;
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
}) {
  const incomplete = profile.columns.filter((column) => column.completenessPercentage < 100);
  const imputable = incomplete.filter(
    (column) => column.nullCount > 0 && column.nullCount < profile.rowCount && !isRowAuditColumn(column.name)
      // FUN-47: the engine leaves personal data and identifiers unfilled.
      && !column.privacySignal && !looksLikeIdentifier(column.name),
  );
  // FUN-71: the same rule as the engine: one value in every row, no empty cell.
  const constant = profile.columns.filter(
    (column) => profile.rowCount > 1 && column.uniqueCount <= 1 && column.nullCount === 0 && !isRowAuditColumn(column.name),
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
  const dateCandidates = dateConversionCandidates(profile);
  const numericCandidates = numericConversionCandidates(profile);
  const typeDrift = profile.columns.filter(
    (column) => (column.invalidTypeCount ?? 0) > 0,
  );
  const outliers = profile.columns.filter(
    (column) => (column.outlierCount ?? 0) > 0 && !isRowAuditColumn(column.name),
  );
  // FUN-47: the same guards as the proposal, and as the engine applies.
  const categoricalImputable = profile.columns.filter(
    (column) => isTextType(column.dataType) && column.nullCount > 0 && !isRowAuditColumn(column.name)
      && !column.privacySignal && column.suggestedType !== "date" && !looksLikeIdentifier(column.name),
  );
  const hasNullActions = empty.length > 0 || highNull.length > 0 || imputable.length > 0 || categoricalImputable.length > 0;
  const dataColumns = profile.columns.filter((column) => !isRowAuditColumn(column.name));
  const totalNullCount = dataColumns.reduce((total, column) => total + column.nullCount, 0);
  const columnsWithNulls = dataColumns.filter((column) => column.nullCount > 0);
  const totalCellCount = profile.rowCount * dataColumns.length;
  const datasetCompleteness = totalCellCount === 0
    ? 100
    : ((totalCellCount - totalNullCount) / totalCellCount) * 100;
  const personal = profile.columns.filter((column) => column.privacySignal !== null);
  const personalColumns = profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && !isRowAuditColumn(column.name));
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);
  const identifierColumns = profile.columns.filter((column) => column.privacySignal === "identifier");
  const hasOtherSignals = profile.duplicateRowCount > 0 || nearDuplicates || constant.length > 0 || encoding.length > 0 || booleans.length > 0 || dateCandidates.length > 0 || numericCandidates.length > 0 || typeDrift.length > 0 || outliers.length > 0 || personal.length > 0;
  const sum = (columns: typeof profile.columns, count: (column: (typeof profile.columns)[number]) => number) =>
    columns.reduce((total, column) => total + count(column), 0);
  const imputableNulls = sum(imputable, (column) => column.nullCount);
  const categoricalNulls = sum(categoricalImputable, (column) => column.nullCount);
  const invalidValues = sum(typeDrift, (column) => column.invalidTypeCount ?? 0);
  const names = (columns: typeof profile.columns) => listNames(columns.map((column) => column.name));
  // The proposal fills only small gaps in measures and short categories: its
  // figure is part of this total, never the same count (RV17).
  const proposalNulls = proposalImpute?.columns?.reduce((total, column) => total + column.missing, 0) ?? 0;
  const proposalColumns = proposalImpute?.columns?.length ?? 0;

  return (
    <div className="prepare-card prepare-card--stacked cleaning-signals">
      <p className="cleaning-signals__lead">Las señales usan solo esquema y métricas agregadas; no muestran celdas ni valores personales.</p>
      <section
        id={qualityActionTargetDomId("missingValues")}
        className="missing-data-plan"
        aria-labelledby="missing-data-title"
        tabIndex={-1}
      >
        <div className="missing-data-plan__heading">
          <div>
            <h4 id="missing-data-title">Nulos y datos faltantes</h4>
            <p>Conservamos los nulos hasta que elijas una corrección.</p>
          </div>
          <div className="missing-data-plan__metrics" aria-label="Resumen de nulos">
            <span><strong>{totalNullCount.toLocaleString()}</strong> nulos en total</span>
            <span><strong>{columnsWithNulls.length.toLocaleString()}</strong> columnas afectadas</span>
            <span><strong>{formatPercent(datasetCompleteness, 1)}</strong> completitud total</span>
          </div>
        </div>
        {totalNullCount > 0 && (
          <p className="missing-data-plan__source">
            {proposalNulls > 0
              ? `De estos ${plural(totalNullCount, "nulo", "nulos")}, la propuesta de arriba solo ofrece rellenar ${plural(proposalNulls, "nulo", "nulos")} en ${plural(proposalColumns, "columna", "columnas")}: huecos pequeños (5 % o menos) en números y categorías cortas. Aquí puedes tratar el resto.`
              : "La propuesta de arriba no rellena ninguno: solo rellena huecos pequeños (5 % o menos) en números y categorías cortas."}
          </p>
        )}
        {totalNullCount === 0 && sentinels.length === 0 ? (
          <p className="notice notice--success" role="status">
            No se detectaron nulos ni marcadores «sin dato».
          </p>
        ) : totalNullCount === 0 && sentinels.length > 0 ? (
          <p className="notice" role="note">
            Se detectaron marcadores «sin dato». Puedes convertirlos desde el plan de correcciones de arriba.
          </p>
        ) : hasNullActions ? (
          <ol className="missing-data-plan__steps">
            {(empty.length > 0 || highNull.length > 0) && (
              <li>
                <div>
                  <strong>Retirar columnas sin información útil</strong>
                  <p>
                    Las columnas totalmente vacías pueden retirarse directamente. Las que tienen
                    al menos 80 % de nulos se ofrecen por separado para evitar pérdida accidental.
                    {empty.length > 0 && <> Vacías: {names(empty)}.</>}
                    {highNull.length > 0 && <> Con 80 % o más de nulos: {names(highNull)}.</>}
                    {undoNote}
                  </p>
                </div>
                <div className="missing-data-plan__actions">
                  {empty.length > 0 && (
                    <EffectButton busy={busy} onClick={onRemoveEmptyColumns}>
                      {empty.length === 1 ? `Eliminar la columna vacía ${empty[0]?.name ?? ""}` : `Eliminar ${empty.length.toLocaleString()} columnas vacías`}
                    </EffectButton>
                  )}
                  {highNull.length > 0 && (
                    <EffectButton busy={busy} onClick={onRemoveHighNullColumns}>
                      {highNull.length === 1
                        ? `Eliminar ${highNull[0]?.name ?? ""} (80 % o más de nulos)`
                        : `Eliminar ${highNull.length.toLocaleString()} columnas con 80 % o más de nulos`}
                    </EffectButton>
                  )}
                </div>
              </li>
            )}
            {(imputable.length > 0 || categoricalImputable.length > 0) && (
              <li>
                <div>
                  <strong>Completar solo cuando tenga sentido</strong>
                  <p>
                    Rellena con la mediana en números y la moda en texto: esos valores no son reales, son una estimación.
                    {imputable.length > 0 && <> Columnas que se pueden rellenar: {names(imputable)}; el motor solo rellena las que tienen una señal conservadora.</>}
                    {categoricalImputable.length > 0 && <> Columnas de texto que se pueden completar como «Desconocido»: {names(categoricalImputable)}.</>}
                    {undoNote}
                  </p>
                </div>
                <div className="missing-data-plan__actions">
                  {imputable.length > 0 && (
                    <EffectButton busy={busy} onClick={onImputeMissingValues}>
                      {`Rellenar hasta ${plural(imputableNulls, "nulo", "nulos")} en ${plural(imputable.length, "columna", "columnas")}`}
                    </EffectButton>
                  )}
                  {categoricalImputable.length > 0 && onImputeCategoricalValues && (
                    <EffectButton busy={busy} onClick={onImputeCategoricalValues}>
                      {`Completar ${plural(categoricalNulls, "nulo de texto", "nulos de texto")} como «Desconocido»`}
                    </EffectButton>
                  )}
                </div>
              </li>
            )}
          </ol>
        ) : null}
        <details className="missing-data-plan__help">
          <summary>Cómo decide Columnia</summary>
          <div>
            <p>Columnia conserva los nulos por defecto. Un nulo no siempre es un error. Primero normaliza los marcadores «sin dato» y después decide si conviene completar o retirar.</p>
            <p>Los espacios en blanco no son nulos. Puedes recortarlos con las correcciones recomendadas.{undoNote}</p>
          </div>
        </details>
      </section>
      {hasOtherSignals ? (
        <section className="detected-signals" aria-labelledby="detected-signals-title">
          <h4 id="detected-signals-title">Otras señales detectadas</h4>
          <ul className="cleaning-signals__list" aria-label="Señales de limpieza detectadas">
          {profile.duplicateRowCount > 0 && (
            <li><strong>Duplicados exactos:</strong> {plural(profile.duplicateRowCount, "fila adicional", "filas adicionales")}; puedes {profile.duplicateRowCount === 1 ? "eliminarla" : "eliminarlas"}.</li>
          )}
          {nearDuplicates && (
            <li><strong>Duplicados parecidos:</strong> {plural(profile.nearDuplicateRowCount, "fila adicional coincide", "filas adicionales coinciden")} al normalizar mayúsculas, espacios y acentos; requieren revisión manual.</li>
          )}
          {constant.length > 0 && (
            <li><strong>Constantes:</strong> {names(constant)} {constant.length === 1 ? "no cambia" : "no cambian"} entre filas.</li>
          )}
          {outliers.length > 0 && (
            <li><strong>Valores atípicos:</strong> {outliers.map((column) => `${column.name} (${(column.outlierCount ?? 0).toLocaleString()})`).join(", ")} {outliers.length === 1 ? "tiene valores" : "tienen valores"} fuera del rango normal de su columna (regla IQR: 1,5 veces el rango intercuartílico).</li>
          )}
          {encoding.length > 0 && (
            <li><strong>Acentos mal leídos:</strong> {encoding.map((column) => `${column.name} (${(column.encodingIssueCount ?? 0).toLocaleString()})`).join(", ")} {encoding.length === 1 ? "tiene" : "tienen"} texto como «Ã¡» en lugar de «á» (doble codificación UTF-8) que puede repararse de forma segura.</li>
          )}
          {booleans.length > 0 && (
            <li><strong>Booleanos:</strong> {names(booleans)} admite alias textuales que pueden unificarse como «true» y «false».</li>
          )}
          {dateCandidates.length > 0 && (
            <li><strong>Fechas detectadas:</strong> {names(dateCandidates)} coincide con un formato de fecha cerrado.</li>
          )}
          {numericCandidates.length > 0 && (
            <li><strong>Números detectados:</strong> {names(numericCandidates)} admite una conversión numérica segura.</li>
          )}
          {typeDrift.length > 0 && (
            <li><strong>Tipos sugeridos:</strong> {names(typeDrift)} contiene valores que no coinciden con la sugerencia detectada.</li>
          )}
          {personal.length > 0 && (
            <li className="cleaning-signals__privacy"><strong>Posible dato personal:</strong> {personalColumns.length} {personalColumns.length === 1 ? "columna detectada" : "columnas detectadas"} por categoría agregada: {personalCategories}. Revisa su tratamiento antes de exportar o compartir.</li>
          )}
          </ul>
          {constant.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes retirar estas columnas de baja información ({names(constant)}); se conservará al menos una
                columna para que el dataset siga siendo utilizable.{undoNote}
              </p>
              <EffectButton busy={busy} onClick={onRemoveConstantColumns}>
                {constant.length === 1 ? `Eliminar la columna constante ${constant[0]?.name ?? ""}` : `Eliminar ${constant.length.toLocaleString()} columnas constantes`}
              </EffectButton>
            </div>
          )}
          {outliers.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes tratar los valores atípicos de {names(outliers)}: reemplazarlos por la mediana de su
                columna, llevarlos al límite del rango normal o eliminar sus filas. Cada opción pide confirmación
                y no muestra celdas.{undoNote}
              </p>
              {onImputeOutliers && (
                <EffectButton busy={busy} onClick={onImputeOutliers}>
                  Reemplazar valores atípicos por la mediana…
                </EffectButton>
              )}
              {onCapOutliers && (
                <EffectButton busy={busy} onClick={onCapOutliers}>
                  Limitar valores atípicos al rango normal…
                </EffectButton>
              )}
              {onDropOutliers && (
                <EffectButton busy={busy} className="danger-action" onClick={onDropOutliers}>
                  Eliminar filas atípicas…
                </EffectButton>
              )}
            </div>
          )}
          {identifierColumns.length > 0 && (
            <div className="cleaning-signals__action cleaning-signals__action--privacy">
              <p>
                Retira solo las columnas marcadas como identificadoras por su encabezado ({names(identifierColumns)}).
                No se inspeccionan ni muestran celdas, se conserva al menos una columna y la operación
                queda disponible para revertir desde el historial.
              </p>
              {onRemoveIdentifierColumns && (
                <EffectButton busy={busy} onClick={onRemoveIdentifierColumns}>
                  {identifierColumns.length === 1 ? "Retirar 1 columna identificadora…" : `Retirar ${identifierColumns.length.toLocaleString()} columnas identificadoras…`}
                </EffectButton>
              )}
            </div>
          )}
          {personalColumns.length > 0 && (
            <div className="cleaning-signals__action cleaning-signals__action--privacy">
              <p>
                Retira correo electrónico, teléfono, dirección y nombre detectados por el encabezado, o
                sustituye sus valores por el texto [REDACTED] («oculto») para que no se puedan leer.
                La acción no muestra nombres ni valores, excluye _cambios, conserva al menos una columna
                y queda disponible para revertir desde el historial.
              </p>
              {onRemovePersonalColumns && (
                <EffectButton busy={busy} onClick={onRemovePersonalColumns}>
                  {personalColumns.length === 1 ? "Retirar 1 columna de datos personales…" : `Retirar ${personalColumns.length.toLocaleString()} columnas de datos personales…`}
                </EffectButton>
              )}
              {onMaskPersonalValues && (
                <EffectButton busy={busy} onClick={onMaskPersonalValues}>
                  Ocultar valores personales…
                </EffectButton>
              )}
            </div>
          )}
          {encoding.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Corrige acentos y signos mal leídos, como «Ã©» en lugar de «é» o «â€™» en lugar de «’»;
                solo se aplican reparaciones inequívocas.{undoNote}
              </p>
              {onFixEncoding && (
                <EffectButton busy={busy} onClick={onFixEncoding}>
                  {`Corregir acentos en ${plural(sum(encoding, (column) => column.encodingIssueCount ?? 0), "valor", "valores")}`}
                </EffectButton>
              )}
            </div>
          )}
          {typeDrift.length > 0 && (
            <div
              id={qualityActionTargetDomId("incompatibleTypes")}
              className="cleaning-signals__action"
              tabIndex={-1}
            >
              <p>
                Puedes dejar como nulos los valores que no coincidan con una sugerencia con al
                menos 90 % de confianza. La acción no muestra celdas y requiere confirmación.{undoNote}
              </p>
              {onNullifyInvalidTypes && (
                <EffectButton busy={busy} onClick={onNullifyInvalidTypes}>
                  {`Dejar como nulos hasta ${plural(invalidValues, "valor incompatible", "valores incompatibles")}…`}
                </EffectButton>
              )}
            </div>
          )}
          {booleans.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                La normalización solo convierte «yes»/«no», «sí»/«no» y «true»/«false» a
                «true»/«false»; deja intactos los valores que no reconoce.{undoNote}
              </p>
              <EffectButton busy={busy} onClick={onNormalizeBooleans}>
                {`Normalizar booleanos en ${plural(booleans.length, "columna", "columnas")}`}
              </EffectButton>
            </div>
          )}
          {dateCandidates.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir estas columnas de texto a <strong>fecha y hora</strong>. Solo se usa
                un formato dominante cerrado y se omiten columnas ambiguas. Los valores que no
                sean fechas quedan nulos: si hay alguno, se pide confirmación.
              </p>
              {onParseDates && (
                <EffectButton busy={busy} onClick={onParseDates}>
                  Interpretar fechas detectadas
                </EffectButton>
              )}
            </div>
          )}
          {numericCandidates.length > 0 && (
            <div className="cleaning-signals__action">
              <p>
                Puedes convertir estas columnas de texto a números. Se exige más del 90 % de
                valores numéricos, se rechaza la pérdida de precisión y se conservan los
                identificadores o códigos con ceros iniciales. Los valores que no sean números
                quedan nulos: si hay alguno, se pide confirmación.
              </p>
              {onCastNumeric && (
                <EffectButton busy={busy} onClick={onCastNumeric}>
                  Convertir números detectados
                </EffectButton>
              )}
            </div>
          )}
        </section>
      ) : (
        <p className="notice notice--success" role="status">No se detectaron otras señales de limpieza en el perfil actual.</p>
      )}
    </div>
  );
}
