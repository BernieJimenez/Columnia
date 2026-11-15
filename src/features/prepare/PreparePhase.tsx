import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";

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
import { buildPrepareProposal, looksLikeIdentifier, proposalImputeCells, type ProposalItem } from "./proposalModel";
import {
  cellCount,
  changeProgressMessage,
  conversionNullEstimate,
  type ChangeStatus,
  type TypeConversionKind,
} from "./prepareModel";
import {
  COLUMN_FIX_GROUPS,
  REMOVAL_NULL_PERCENTAGE,
  actionColumns,
  actionEffect,
  actionLabel,
  buildColumnFixes,
  columnFixScopes,
  listNames,
  sharedEntries,
  type ColumnFill,
  type FixAction,
  type SharedActionId,
} from "./columnFixes";
import { qualityActionTargetDomId } from "../review/qualityActionPlan";
import type { QualityActionTarget } from "../review/qualityActionPlan";

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
  /**
   * Fills every column at once. «Corregir columna por columna» fills one
   * column at a time through `onApplyRecommended` with `imputeColumns`, so
   * this one is no longer offered; kept for the callers that pass it.
   */
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
  // Keys, dates and numbers stored as text are listed apart: lowercasing or
  // stripping accents there breaks codes and formats.
  const structuredText = new Set(textColumns.filter((column) => {
    const profiled = profileStatus.kind === "ready"
      ? profileStatus.profile.columns.find((candidate) => candidate.name === column.name)
      : undefined;
    return looksLikeIdentifier(column.name)
      || profiled?.privacySignal === "identifier"
      || (profiled?.suggestedType !== undefined && profiled.suggestedType !== null && profiled.suggestedType !== "boolean");
  }).map((column) => column.name));
  const freeTextColumns = textColumns.filter((column) => !structuredText.has(column.name));
  const structuredTextColumns = textColumns.filter((column) => structuredText.has(column.name));
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
  // AUD-057: the proposal already shows its own result with «Deshacer».
  const resultLineShown = currentPlanComparison === null && dismissedResult !== changeStatus && changeStatus.kind === "applied";
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
  // Filling invents values: the median, the most frequent value or «Desconocido» ask first.
  const [fillConfirmation, setFillConfirmation] = useState<ColumnFill | null>(null);
  const [unknownConfirmation, setUnknownConfirmation] = useState(false);
  const fixScopes = profileStatus.kind === "ready" ? columnFixScopes(profileStatus.profile) : null;
  const unknownColumns = fixScopes?.fillUnknown ?? [];
  const unknownTotal = unknownColumns.reduce((total, column) => total + column.nullCount, 0);
  const identifierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => column.privacySignal === "identifier")
    : [];
  const personalColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => isPersonalPrivacySignal(column.privacySignal) && !isRowAuditColumn(column.name))
    : [];
  const typeDriftColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.invalidTypeCount ?? 0) > 0)
    : [];
  const typeDriftTotal = typeDriftColumns.reduce((total, column) => total + (column.invalidTypeCount ?? 0), 0);
  const outlierColumns = profileStatus.kind === "ready"
    ? profileStatus.profile.columns.filter((column) => (column.outlierCount ?? 0) > 0 && !isRowAuditColumn(column.name))
    : [];
  const personalCategories = summarizePersonalPrivacySignals(personalColumns);
  const outlierTotal = outlierColumns.reduce((total, column) => total + (column.outlierCount ?? 0), 0);
  const outlierTotalPhrase = `${outlierTotal === 1 ? "el valor atípico" : `los ${outlierTotal.toLocaleString("es-ES")} valores atípicos`} de ${listNames(outlierColumns.map((column) => column.name))}`;
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

  /** Whether the app offers a column action (some callbacks are optional). */
  function sharedAvailable(id: SharedActionId): boolean {
    switch (id) {
      case "castNumeric": return onCastNumeric !== undefined;
      case "parseDates": return onParseDates !== undefined;
      case "nullifyInvalid": return onNullifyInvalidTypes !== undefined;
      case "maskPersonal": return onMaskPersonalValues !== undefined;
      case "removePersonal": return onRemovePersonalColumns !== undefined;
      case "removeIdentifier": return onRemoveIdentifierColumns !== undefined;
      case "capOutliers": return onCapOutliers !== undefined;
      case "imputeOutliers": return onImputeOutliers !== undefined;
      case "dropOutliers": return onDropOutliers !== undefined;
      case "fixEncoding": return onFixEncoding !== undefined;
      case "fillUnknown": return onImputeCategoricalValues !== undefined;
      default: return true;
    }
  }

  /** Runs a column action, or opens its confirmation when it needs one. */
  function runShared(id: SharedActionId) {
    switch (id) {
      case "removeEmpty": onRemoveEmptyColumns(); break;
      case "removeHighNull": onRemoveHighNullColumns(); break;
      case "removeConstant": onRemoveConstantColumns(); break;
      case "castNumeric": requestConversion("numeric"); break;
      case "parseDates": requestConversion("dates"); break;
      case "nullifyInvalid": setInvalidTypeConfirmation(true); break;
      case "maskPersonal": setMaskPersonalConfirmation(true); break;
      case "removePersonal": setPersonalConfirmation(true); break;
      case "removeIdentifier": setIdentifierConfirmation(true); break;
      case "capOutliers": setOutlierConfirmation("cap"); break;
      case "imputeOutliers": setOutlierConfirmation("impute"); break;
      case "dropOutliers": setOutlierConfirmation("drop"); break;
      case "fixEncoding": onFixEncoding?.(); break;
      case "normalizeBooleans": onNormalizeBooleans(); break;
      case "fillUnknown": setUnknownConfirmation(true); break;
    }
  }

  /** Fills one column through the engine's imputation with an explicit column list. */
  function fillColumn(fill: ColumnFill) {
    onApplyRecommended({
      trimText: false,
      normalizeSentinels: false,
      normalizeColumnNames: false,
      removeDuplicates: false,
      imputeMissing: true,
      imputeColumns: [fill.column],
    });
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
      {resultLineShown && (
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
          applyingLabel={proposalProgressLabel(changeStatus)}
          // One «Deshacer» per zone: the result line above already has it.
          canUndo={historyStatus.canUndo && !resultLineShown}
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
                    available={sharedAvailable}
                    onShared={runShared}
                    onFill={setFillConfirmation}
                    nearDuplicateCount={nearDuplicateCount ?? 0}
                    onRemoveNearDuplicates={onRemoveNearDuplicates && (() => setNearDuplicateConfirmation(true))}
                  />
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
                    {[
                      { legend: "Columnas de texto", columns: freeTextColumns },
                      {
                        legend: "Identificadores, fechas y números escritos como texto (normalizarlos puede romper claves y formatos)",
                        columns: structuredTextColumns,
                      },
                    ].filter((group) => group.columns.length > 0).map((group) => (
                    <fieldset key={group.legend}>
                      <legend>{group.legend}</legend>
                      {group.columns.map((column) => (
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
                    ))}
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
                      {emptyRowBound === 0
                        ? "El análisis no encontró ninguna: hay columnas sin ningún nulo ni texto en blanco."
                        : <>
                            Elimina filas cuyos valores son todos nulos o texto en blanco. La operación conserva el orden.{undoNote}
                            {emptyRowBound !== null
                              ? ` Según el análisis puede haber hasta ${plural(emptyRowBound, "fila vacía", "filas vacías")}; al aplicarlo se dice cuántas se eliminaron.`
                              : ""}
                          </>}
                    </p>
                  </div>
                  {emptyRowBound !== 0 && (
                    <EffectButton
                      busy={changing}
                      describedBy="empty-rows-description"
                      onClick={onRemoveEmptyRows}
                    >
                      {emptyRowBound !== null && emptyRowBound > 0
                        ? `Eliminar hasta ${plural(emptyRowBound, "fila vacía", "filas vacías")}`
                        : "Eliminar filas vacías"}
                    </EffectButton>
                  )}
                </section>
              )}
              {hasRowsAndColumns && (
                <section className="prepare-card" aria-labelledby="row-audit-title">
                  <div>
                    <h3 id="row-audit-title">Cambios por fila</h3>
                    <p>
                      {hasRowAuditColumn
                        ? "La columna de auditoría _cambios está activa: en cada fila anota qué corrección posterior la cambió."
                        : "Añade una columna de auditoría, _cambios, que anota en cada fila qué corrección aplicada después la cambió."}
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
          <h3 id="identifier-confirm-title">Eliminar columnas identificadoras</h3>
          <p id="identifier-confirm-description">
            Se eliminarán {identifierColumns.length === 1 ? "1 columna identificadora" : `${identifierColumns.length} columnas identificadoras`} detectadas por el nombre del encabezado: {identifierColumns.map((column) => column.name).join(", ")}.
            Un identificador suele ser la clave de la tabla: sin él no se pueden unir ni distinguir las filas.
            No se mostrarán celdas ni valores del dataset. Se conservará al menos una columna.{undoNote}
            Las columnas de email, teléfono, dirección y nombre no se eliminan con esta acción.
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
              Eliminar {plural(identifierColumns.length, "columna", "columnas")}
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
          <h3 id="personal-confirm-title">Eliminar columnas con datos personales</h3>
          <p id="personal-confirm-description">
            Se eliminarán {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. La columna de auditoría _cambios (qué
            corrección cambió cada fila) no se toca y se conservará al menos una columna utilizable.{undoNote}
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
              Eliminar {plural(personalColumns.length, "columna", "columnas")}
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
            Se sustituirán los valores no nulos de {personalColumns.length === 1 ? "1 columna personal" : `${personalColumns.length} columnas personales`} por la marca de valor oculto <code>[REDACTED]</code> («censurado», en inglés): la celda sigue diciendo que había un valor, pero ya no se puede leer. Sirve para entregar los datos sin exponer a nadie; no es obligatorio. Columnas identificadas por categorías agregadas: {personalCategories}.
            No se mostrarán nombres de columnas, celdas ni valores del dataset. Se conservarán las columnas,
            y la columna de auditoría _cambios (qué corrección cambió cada fila) no se toca.{undoNote}
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
          <h3 id="invalid-types-confirm-title">Dejar como nulos los valores incompatibles</h3>
          <p id="invalid-types-confirm-description">
            Se convertirán en nulos los valores que no coincidan con un tipo sugerido con al menos 90 % de coincidencia en {typeDriftColumns.length === 1 ? "1 columna" : `${typeDriftColumns.length} columnas`}: {listNames(typeDriftColumns.map((column) => `${column.name} (${(column.invalidTypeCount ?? 0).toLocaleString("es-ES")})`))}. Las celdas no se mostrarán.{undoNote}
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
              Dejar como nulos {plural(typeDriftTotal, "valor", "valores")}
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
            {conversionLoss.map((column) => `${column.name} (${column.count.toLocaleString("es-ES")})`).join(", ")}.{undoNote}
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
      {fillConfirmation !== null && (
        <ModalDialog
          role="alertdialog"
          labelledBy="fill-confirm-title"
          describedBy="fill-confirm-description"
          onDismiss={() => setFillConfirmation(null)}
        >
          <h3 id="fill-confirm-title">Rellenar nulos de {fillConfirmation.column}</h3>
          <p id="fill-confirm-description">
            {fillConfirmation.method === "median"
              ? `Se escribirá la mediana de la columna (${fillConfirmation.value ?? ""}) en ${plural(fillConfirmation.count, "nulo", "nulos")} de ${fillConfirmation.column}.`
              : `Se escribirá el valor más frecuente de la columna en ${plural(fillConfirmation.count, "nulo", "nulos")} de ${fillConfirmation.column}.`}
            {" "}Es una estimación, no un dato real: después no se distingue de los valores que ya había. Solo cambia esta columna.{undoNote}
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setFillConfirmation(null)}>
              Cancelar
            </button>
            {/* Filling can be undone: a normal primary, not a danger action. */}
            <button
              type="button"
              className="primary-action"
              onClick={() => {
                const fill = fillConfirmation;
                setFillConfirmation(null);
                fillColumn(fill);
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              Rellenar {plural(fillConfirmation.count, "nulo", "nulos")}
            </button>
          </div>
        </ModalDialog>
      )}
      {unknownConfirmation && unknownColumns.length > 0 && (
        <ModalDialog
          role="alertdialog"
          labelledBy="unknown-confirm-title"
          describedBy="unknown-confirm-description"
          onDismiss={() => setUnknownConfirmation(false)}
        >
          <h3 id="unknown-confirm-title">Completar nulos como «Desconocido»</h3>
          <p id="unknown-confirm-description">
            Se escribirá «Desconocido» en {plural(unknownTotal, "nulo", "nulos")} de {plural(unknownColumns.length, "columna", "columnas")} de
            texto: {listNames(unknownColumns.map((column) => `${column.name} (${column.nullCount.toLocaleString("es-ES")})`))}. «Desconocido» es un valor
            inventado: contará como una categoría más. No se tocan los datos personales, la columna de auditoría _cambios ni las
            columnas vacías o con {REMOVAL_NULL_PERCENTAGE} % o más de nulos.{undoNote}
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setUnknownConfirmation(false)}>
              Cancelar
            </button>
            <button
              type="button"
              className="primary-action"
              onClick={() => {
                setUnknownConfirmation(false);
                onImputeCategoricalValues?.();
              }}
              disabled={changing}
              aria-describedby={busyDescribedBy(changing)}
            >
              Completar {plural(unknownTotal, "nulo", "nulos")}
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
const OUTLIER_RULE = "Un valor es atípico si queda fuera del rango normal de su columna, lejos de la mitad central de sus datos: más de 1,5 veces el ancho de esa mitad por debajo del primer cuartil o por encima del tercero (regla IQR, del rango intercuartílico).";

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

/**
 * What the proposal's primary says while the proposal is being applied, so
 * the progress is where the person pressed, not only in the notice above.
 */
function proposalProgressLabel(status: ChangeStatus): string | null {
  if (status.kind !== "working" || status.action !== "safe") return null;
  if (status.cancelRequested) return "Cancelando…";
  const message = changeProgressMessage(status.action);
  return status.progress ? `${message} ${Math.round(status.progress.percent)} %` : message;
}

/** The sentence that says whether a change can be undone (TXT-01). */
function undoAvailabilityNote(snapshotsEnabled: boolean): string {
  return snapshotsEnabled
    ? " Podrás deshacer el cambio desde el historial."
    : " El historial reversible está desactivado: este cambio no se podrá deshacer.";
}

function CleaningSignals({
  profile,
  proposalImpute,
  busy,
  undoNote,
  available,
  onShared,
  onFill,
  nearDuplicateCount,
  onRemoveNearDuplicates,
}: {
  profile: DatasetProfile;
  /** «Rellenar nulos» of the proposal: its figure is quoted, never recounted. */
  proposalImpute: ProposalItem | null;
  busy: boolean;
  undoNote: string;
  /** Whether the app offers this action (some callbacks are optional). */
  available: (id: SharedActionId) => boolean;
  onShared: (id: SharedActionId) => void;
  onFill: (fill: ColumnFill) => void;
  nearDuplicateCount: number;
  onRemoveNearDuplicates?: () => void;
}) {
  // An action the app does not offer gives way to the next one.
  const plan = buildColumnFixes(profile, available);
  const { scopes, rows } = plan;
  const groups = COLUMN_FIX_GROUPS
    .map((group) => ({ ...group, rows: rows.filter((row) => row.group === group.id) }))
    .filter((group) => group.rows.length > 0);
  const idPrefix = useId();
  const entries = sharedEntries(plan);
  const dataColumns = profile.columns.filter((column) => !isRowAuditColumn(column.name));
  const withNulls = dataColumns.filter((column) => column.nullCount > 0);
  const withSentinels = dataColumns.filter((column) => (column.sentinelCount ?? 0) > 0);
  const totalNulls = withNulls.reduce((total, column) => total + column.nullCount, 0);
  const totalSentinels = withSentinels.reduce((total, column) => total + (column.sentinelCount ?? 0), 0);
  const proposalCells = proposalImpute ? proposalImputeCells(proposalImpute) : 0;
  const proposalNames = listNames(proposalImpute?.columns?.map((column) => column.name) ?? []);
  const firstIncompatible = rows.find((row) => (row.column.invalidTypeCount ?? 0) > 0)?.column.name;

  const run = (action: FixAction) => (action.kind === "fill" ? onFill(action) : onShared(action.id));
  const isShared = (action: FixAction) => action.kind === "shared" && scopes[action.id].length > 1;
  const others = (action: FixAction, name: string) =>
    listNames(actionColumns(action, scopes).filter((column) => column !== name));

  return (
    <div className="prepare-card prepare-card--stacked cleaning-signals">
      <p className="cleaning-signals__lead">
        Una fila por cada columna con algo que corregir, de lo más grave a lo menos, con una acción recomendada
        cuando la hay. Las cifras son de todas las filas y salen de métricas agregadas: no se muestran celdas.{undoNote}
      </p>
      <p
        id={qualityActionTargetDomId("missingValues")}
        className="cleaning-signals__nulls"
        tabIndex={-1}
      >
        {totalNulls === 0 && totalSentinels === 0
          ? "No hay nulos ni marcadores «sin dato»."
          : <>
              <strong>Nulos:</strong>{" "}
              {totalNulls > 0 ? `${plural(totalNulls, "nulo", "nulos")} en ${plural(withNulls.length, "columna", "columnas")}` : "ningún nulo"}
              {totalSentinels > 0 && ` y ${plural(totalSentinels, "marcador «sin dato»", "marcadores «sin dato»")} en ${plural(withSentinels.length, "columna", "columnas")}`}
              {". "}
              {proposalCells > 0
                ? `La propuesta de arriba rellena ${plural(proposalCells, "de ellos", "de ellos")} en ${proposalNames}${proposalImpute?.columns?.some((column) => column.sentinels > 0) ? ", contando los marcadores que antes convierte en nulos" : ""}.`
                : "La propuesta de arriba no rellena ninguno: solo rellena huecos pequeños (5 % o menos) en números y categorías cortas."}
            </>}
      </p>
      <section className="cleaning-signals__section" aria-labelledby="column-fixes-title">
        <h4 id="column-fixes-title">Columnas con algo que corregir</h4>
        {rows.length === 0 ? (
          <p className="cleaning-signals__empty">Ninguna columna tiene nada que corregir según el análisis.</p>
        ) : groups.map((group) => (
          <section key={group.id} className="cleaning-signals__group" aria-labelledby={`${idPrefix}-group-${group.id}`}>
          <h5 id={`${idPrefix}-group-${group.id}`}>{group.title}</h5>
          <ul className="cleaning-signals__columns" aria-labelledby={`${idPrefix}-group-${group.id}`}>
            {group.rows.map((row) => {
              const name = row.column.name;
              const recommended = row.recommended;
              const nameId = `${idPrefix}-column-${rows.indexOf(row)}`;
              return (
                <li
                  key={name}
                  id={name === firstIncompatible ? qualityActionTargetDomId("incompatibleTypes") : undefined}
                  tabIndex={name === firstIncompatible ? -1 : undefined}
                  className="cleaning-signals__column"
                  // Named by the column only; the content is still read in full.
                  aria-labelledby={nameId}
                >
                  <div className="cleaning-signals__column-text">
                    <p className="cleaning-signals__column-name">
                      <strong id={nameId}>{name}</strong> <span>{row.typeLabel}</span>
                    </p>
                    {row.issues.length > 0 && <p className="cleaning-signals__issue">{row.issues.join(" · ")}</p>}
                    {row.notes.map((note) => <p key={note} className="cleaning-signals__note">{note}</p>)}
                    {recommended && isShared(recommended) && (
                      <p className="cleaning-signals__note">
                        Recomendado: «{actionLabel(recommended, scopes)}», en «En varias columnas», más abajo; toca también {others(recommended, name)}.
                      </p>
                    )}
                  </div>
                  {recommended && !isShared(recommended) && (
                    <EffectButton busy={busy} className="cleaning-signals__action-button" onClick={() => run(recommended)}>
                      {actionLabel(recommended, scopes)}
                    </EffectButton>
                  )}
                  {row.alternatives.length > 0 && (
                    <details className="cleaning-signals__more">
                      <summary>Otras opciones</summary>
                      <ul>
                        {row.alternatives.map((action) => (
                          <li key={action.kind === "fill" ? `fill-${action.method}` : action.id}>
                            {isShared(action) ? (
                              <p>«{actionLabel(action, scopes)}» está en «En varias columnas»: toca también {others(action, name)}.</p>
                            ) : (
                              <EffectButton busy={busy} onClick={() => run(action)}>
                                {actionLabel(action, scopes)}
                              </EffectButton>
                            )}
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                </li>
              );
            })}
          </ul>
          </section>
        ))}
      </section>
      {(entries.length > 0 || (nearDuplicateCount > 0 && onRemoveNearDuplicates)) && (
        <section className="cleaning-signals__section" aria-labelledby="shared-fixes-title">
          <h4 id="shared-fixes-title">En varias columnas</h4>
          <ul className="cleaning-signals__shared">
            {entries.map((entry) => {
              const primary: FixAction = { kind: "shared", id: entry.primary };
              return (
                <li key={entry.primary} className="cleaning-signals__column">
                  <div className="cleaning-signals__column-text">
                    <p className="cleaning-signals__issue">
                      {actionEffect(entry.primary)} Columnas: {listNames(actionColumns(primary, scopes))}.
                    </p>
                  </div>
                  <EffectButton busy={busy} className="cleaning-signals__action-button" onClick={() => run(primary)}>
                    {actionLabel(primary, scopes)}
                  </EffectButton>
                  {entry.alternatives.length > 0 && (
                    <details className="cleaning-signals__more">
                      <summary>Otras opciones</summary>
                      <ul>
                        {entry.alternatives.map((id) => (
                          <li key={id}>
                            <EffectButton busy={busy} onClick={() => onShared(id)}>
                              {actionLabel({ kind: "shared", id }, scopes)}
                            </EffectButton>
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                </li>
              );
            })}
            {nearDuplicateCount > 0 && onRemoveNearDuplicates && (
              <li className="cleaning-signals__column" aria-labelledby="near-duplicates-title">
                <div className="cleaning-signals__column-text">
                  <p className="cleaning-signals__column-name"><strong id="near-duplicates-title">Duplicados parecidos</strong></p>
                  <p className="cleaning-signals__issue">
                    {plural(nearDuplicateCount, "fila coincide", "filas coinciden")} con otra al ignorar mayúsculas, espacios y
                    acentos. Se conservan la primera de cada grupo y las copias exactas.
                  </p>
                </div>
                <EffectButton busy={busy} className="cleaning-signals__action-button" onClick={onRemoveNearDuplicates}>
                  {nearDuplicateCount === 1 ? "Eliminar 1 fila parecida…" : `Eliminar ${nearDuplicateCount.toLocaleString("es-ES")} filas parecidas…`}
                </EffectButton>
              </li>
            )}
          </ul>
        </section>
      )}
      <details className="missing-data-plan__help">
        <summary>Cómo decide Columnia</summary>
        <div>
          <p>
            Cada columna recibe como mucho una recomendación, y las filas van por este orden: eliminar la columna si
            está vacía, tiene{` ${REMOVAL_NULL_PERCENTAGE}`} % o más de nulos o el mismo valor en todas las filas (entonces
            nunca se rellena); convertir el tipo o dejar como nulos los valores incompatibles; ocultar datos personales;
            tratar los valores atípicos; reparar el texto; y, al final, rellenar nulos.
          </p>
          <p>
            Un identificador suele ser la clave de la tabla: no se recomienda tocarlo, y eliminarlo queda en «Otras
            opciones». Las fechas que se leen de dos formas (día/mes o mes/día) se convierten en la propuesta de
            arriba, cuando eliges el orden.
          </p>
          <p>
            Un nulo no siempre es un error y se conserva hasta que elijas. Rellenar con la mediana, el valor más
            frecuente o «Desconocido» inventa valores: siempre pide confirmación. Los marcadores «sin dato» y los
            espacios en blanco se tratan en la propuesta de arriba.
          </p>
        </div>
      </details>
    </div>
  );
}
