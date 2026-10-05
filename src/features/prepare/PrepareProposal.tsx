import { useEffect, useState } from "react";

import type { DatasetProfile, SafeCorrectionOptions, SafeCorrectionsPreview } from "../../bridge";
import { CellText } from "../../components/CellText";
import { MissingValue } from "../../components/MissingValue";
import {
  applyLabel,
  dateExample,
  dateExamples,
  defaultProposalSelection,
  hasAmbiguousDates,
  resolvedDateColumns,
  datesItemTitle,
  imputationExamples,
  proposalItemTitle,
  proposalOptions,
  selectedProposalCount,
  type DateOrder,
  type ProposalItem,
  type ProposalItemId,
  type ProposalSelection,
} from "./proposalModel";

export interface ProposalResult {
  before: DatasetProfile;
  /** `null` while the profile of the new dataset is still being computed (UX-10). */
  after: DatasetProfile | null;
  /** What the proposal applied, one line per kind of change. */
  changes: string[];
}

interface PrepareProposalProps {
  items: ProposalItem[];
  columnCount: number;
  busy: boolean;
  canUndo: boolean;
  result: ProposalResult | null;
  onApply: (options: SafeCorrectionOptions) => void;
  /** Simulates the selected chain so «Rellenar» announces exactly what it fills. */
  onPreview?: (options: SafeCorrectionOptions) => Promise<SafeCorrectionsPreview>;
  onUndo: () => void;
  onDismissResult: () => void;
}

const STEP_QUESTIONS: Record<ProposalItemId, { question: string; yes: string; no: string }> = {
  sentinels: { question: "¿Convertimos los marcadores de «sin dato» en vacíos reales?", yes: "Sí, convertir", no: "No, dejarlos como texto" },
  trim: { question: "¿Recortamos los espacios sobrantes del texto?", yes: "Sí, recortar", no: "No, dejarlos" },
  types: { question: "¿Convertimos a número las columnas que solo tienen números?", yes: "Sí, convertir", no: "No, dejarlas como texto" },
  dates: { question: "¿Convertimos a fecha las columnas que solo tienen fechas?", yes: "Sí, convertir", no: "No, dejarlas como texto" },
  duplicates: { question: "¿Quitamos las filas duplicadas?", yes: "Sí, quitarlas", no: "No, pueden ser registros distintos" },
  impute: { question: "¿Rellenamos los valores vacíos?", yes: "Sí, rellenar", no: "No, dejarlos vacíos" },
};

/**
 * Empty cells plus text markers such as «N/A»: both mean the value is missing,
 * so converting markers into real gaps does not read as a regression.
 */
function missingTotal(profile: DatasetProfile): number {
  return profile.columns.reduce(
    (total, column) => total + column.nullCount + (column.sentinelCount ?? 0),
    0,
  );
}

const MAX_BEFORE_AFTER_ROWS = 8;

export function PrepareProposal({
  items,
  columnCount,
  busy,
  canUndo,
  result,
  onApply,
  onPreview,
  onUndo,
  onDismissResult,
}: PrepareProposalProps) {
  const signature = items.map((item) => `${item.id}:${item.title}`).join("|");
  const [selection, setSelection] = useState<ProposalSelection>(() => defaultProposalSelection(items));
  const [showBeforeAfter, setShowBeforeAfter] = useState(false);
  const [mode, setMode] = useState<"proposal" | "steps">("proposal");
  const [step, setStep] = useState(0);
  const [normalizeNames, setNormalizeNames] = useState(false);
  // One answer for every column whose dates read both ways (01/02/2024).
  const [ambiguousDateOrder, setAmbiguousDateOrder] = useState<DateOrder | null>(null);

  const [shownSignature, setShownSignature] = useState(signature);
  if (shownSignature !== signature) {
    setShownSignature(signature);
    setSelection(defaultProposalSelection(items));
    setAmbiguousDateOrder(null);
    setShowBeforeAfter(false);
    setMode("proposal");
    setStep(0);
    setNormalizeNames(false);
  }

  // Filling gaps and removing duplicates depend on the rest of the chain
  // (markers become gaps, trimmed spaces make rows identical), so while one of
  // them is checked the engine simulates the selection and the proposal shows
  // that (FUN-12).
  const dependsOnChain = (selection.impute && items.some((item) => item.id === "impute"))
    || (selection.duplicates && selection.trim && items.some((item) => item.id === "duplicates"));
  const previewKey = onPreview && dependsOnChain
    ? JSON.stringify(proposalOptions(items, selection, false, ambiguousDateOrder))
    : null;
  const [fills, setFills] = useState<{ key: string; preview: SafeCorrectionsPreview | null } | null>(null);
  useEffect(() => {
    if (previewKey === null || !onPreview) return;
    let current = true;
    onPreview(JSON.parse(previewKey) as SafeCorrectionOptions).then(
      (preview) => { if (current) setFills({ key: previewKey, preview }); },
      // Without a simulation the profile estimate stays visible.
      () => { if (current) setFills({ key: previewKey, preview: null }); },
    );
    return () => { current = false; };
  }, [previewKey, onPreview]);
  const fillsReady = previewKey === null || fills?.key === previewKey;
  const preview = fillsReady && previewKey !== null ? fills?.preview ?? null : null;
  const titleOf = (item: ProposalItem) => (
    item.id === "impute" && !fillsReady
      ? "Rellenar valores vacíos: calculando cuántos…"
      : item.id === "duplicates" && !fillsReady
        ? "Quitar filas duplicadas: calculando cuántas…"
      : item.id === "dates"
        ? datesItemTitle(item, ambiguousDateOrder)
        : proposalItemTitle(item, selection, preview)
  );

  if (result) {
    // Only the figures that moved: the list above already says what changed.
    const after = result.after;
    const rows = after === null ? [] : [
      { label: "Filas", before: result.before.rowCount, after: after.rowCount },
      { label: "Valores sin dato", before: missingTotal(result.before), after: missingTotal(after) },
      { label: "Filas duplicadas", before: result.before.duplicateRowCount, after: after.duplicateRowCount },
      { label: "Columnas", before: result.before.columns.length, after: after.columns.length },
    ].filter((row) => row.before !== row.after);
    return (
      <section className="prepare-proposal" aria-labelledby="prepare-result-title">
        <div role="status">
          <h3 id="prepare-result-title" className="prepare-proposal__title">Listo: cambios aplicados</h3>
        </div>
        {result.changes.length > 0 && (
          <ul className="prepare-proposal__changes" aria-label="Cambios aplicados">
            {result.changes.map((change) => <li key={change}>{change}</li>)}
          </ul>
        )}
        {after === null && (
          <p className="prepare-proposal__pending" role="status">Calculando las cifras de antes y después…</p>
        )}
        {rows.length > 0 && (
          <dl className="prepare-proposal__result">
            {rows.map((row) => (
              <div key={row.label}>
                <dt>{row.label}</dt>
                <dd>{row.before.toLocaleString()} → <strong>{row.after.toLocaleString()}</strong></dd>
              </div>
            ))}
          </dl>
        )}
        <div className="prepare-proposal__actions">
          <button type="button" className="prepare-proposal__primary" onClick={onDismissResult}>
            Ver qué más se puede mejorar
          </button>
          <button type="button" className="secondary-action" onClick={onUndo} disabled={busy || !canUndo}>
            Deshacer
          </button>
        </div>
      </section>
    );
  }

  const disabled = busy;
  // «Convertir a fecha» checked with only ambiguous columns and no answer yet.
  const datesItem = items.find((item) => item.id === "dates");
  const datesPending = Boolean(selection.dates && datesItem && resolvedDateColumns(datesItem, ambiguousDateOrder).length === 0);

  // «¿Cómo se lee…?» for the dates whose order is ambiguous, in both modes.
  function dateOrderQuestion(item: ProposalItem) {
                const sample = item.dateColumns?.find((column) => column.order === null)?.sample ?? "01/02/2024";
                const options: { order: DateOrder; label: string }[] = [
                  { order: "dmy", label: `Día/mes: ${dateExample(sample, "dmy") ?? ""}` },
                  { order: "mdy", label: `Mes/día: ${dateExample(sample, "mdy") ?? ""}` },
                ];
                return (
                  <fieldset className="prepare-proposal__date-order" disabled={disabled}>
                    <legend>¿Cómo se lee «{sample}»?</legend>
                    {options.map((option) => (
                      <label key={option.order} className="prepare-proposal__option">
                        <input
                          type="radio"
                          name="prepare-date-order"
                          checked={ambiguousDateOrder === option.order}
                          onChange={() => {
                            setAmbiguousDateOrder(option.order);
                            setSelection((previous) => ({ ...previous, dates: true }));
                          }}
                        />
                        {option.label}
                      </label>
                    ))}
                  </fieldset>
                );
              }

  if (mode === "steps") {
    const totalSteps = items.length + 1;
    const isNamesStep = step === items.length;
    const current = isNamesStep ? null : items[step];
    const question = current
      ? STEP_QUESTIONS[current.id]
      : { question: `¿Normalizamos los nombres de las ${columnCount} columnas?`, yes: "Sí, normalizar", no: "No, dejarlos" };
    const hint = current ? `${titleOf(current)}. ${current.hint}` : "Minúsculas y sin espacios. Puede afectar consultas e integraciones.";
    const value = current ? selection[current.id] : normalizeNames;
    const choose = (next: boolean) => {
      if (current) setSelection((previous) => ({ ...previous, [current.id]: next }));
      else setNormalizeNames(next);
    };
    return (
      <section className="prepare-proposal" aria-labelledby="prepare-step-title">
        <p className="prepare-proposal__progress">Paso {step + 1} de {totalSteps}</p>
        <div className="prepare-proposal__bar" aria-hidden="true">
          <span style={{ width: `${Math.round(((step + 1) / totalSteps) * 100)}%` }} />
        </div>
        <fieldset className="prepare-proposal__step" disabled={disabled}>
          <legend id="prepare-step-title" className="prepare-proposal__title">{question.question}</legend>
          <p className="prepare-proposal__lead">{hint}</p>
          {[true, false].map((option) => (
            <label key={String(option)} className="prepare-proposal__option">
              <input
                type="radio"
                name={`prepare-step-${step}`}
                checked={value === option}
                onChange={() => choose(option)}
              />
              {option ? question.yes : question.no}
            </label>
          ))}
        </fieldset>
        {current?.id === "dates" && value && hasAmbiguousDates(current) && dateOrderQuestion(current)}
        <div className="prepare-proposal__actions">
          <button
            type="button"
            className="secondary-action"
            onClick={() => (step === 0 ? setMode("proposal") : setStep(step - 1))}
          >
            {step === 0 ? "Volver a la propuesta" : "Atrás"}
          </button>
          {isNamesStep ? (
            <button
              type="button"
              className="prepare-proposal__primary"
              disabled={disabled || !fillsReady || datesPending || (selectedProposalCount(items, selection) === 0 && !normalizeNames)}
              onClick={() => onApply(proposalOptions(items, selection, normalizeNames, ambiguousDateOrder))}
            >
              Aplicar
            </button>
          ) : (
            <button type="button" className="prepare-proposal__primary" onClick={() => setStep(step + 1)}>
              Siguiente
            </button>
          )}
        </div>
      </section>
    );
  }

  if (items.length === 0) {
    return (
      <section className="prepare-proposal" aria-labelledby="prepare-proposal-title">
        <h3 id="prepare-proposal-title" className="prepare-proposal__title">No hay cambios que proponer</h3>
        <p className="prepare-proposal__lead">No se detectaron espacios sobrantes, duplicados ni vacíos que rellenar.</p>
        <div className="prepare-proposal__actions">
          <button type="button" className="link-action" disabled={disabled} onClick={() => { setMode("steps"); setStep(0); }}>
            Personalizar paso a paso
          </button>
        </div>
      </section>
    );
  }

  const count = selectedProposalCount(items, selection);
  // One table for every checked change, instead of a toggle per change.
  // Every filled column keeps its row: it shows the value it will receive.
  const beforeAfter = [
    ...items
      .filter((item) => selection[item.id] && item.id !== "impute")
      .flatMap((item) => (item.id === "dates" ? dateExamples(item, ambiguousDateOrder) : item.examples)
        .map((example) => ({ item, example })))
      .slice(0, MAX_BEFORE_AFTER_ROWS),
    ...items
      .filter((item) => selection[item.id] && item.id === "impute" && fillsReady)
      .flatMap((item) => (preview ? imputationExamples(preview) : item.examples).map((example) => ({ item, example }))),
  ];
  return (
    <section className="prepare-proposal" aria-labelledby="prepare-proposal-title">
      <h3 id="prepare-proposal-title" className="prepare-proposal__title">
        {items.length === 1 ? "Columnia propone 1 cambio" : `Columnia propone ${items.length} cambios`}
      </h3>
      <p className="prepare-proposal__lead">Revísalos y aplícalos. Puedes deshacerlo después.</p>
      <ul className="prepare-proposal__list">
        {items.map((item) => {
          return (
            <li key={item.id} className="prepare-proposal__item">
              <div className="prepare-proposal__row">
                <input
                  id={`prepare-item-${item.id}`}
                  type="checkbox"
                  checked={selection[item.id]}
                  disabled={disabled}
                  aria-describedby={`prepare-hint-${item.id}`}
                  onChange={(event) => setSelection((previous) => ({ ...previous, [item.id]: event.target.checked }))}
                />
                <label htmlFor={`prepare-item-${item.id}`}>{titleOf(item)}</label>
              </div>
              <p id={`prepare-hint-${item.id}`} className="prepare-proposal__hint">{item.hint}</p>
              {item.id === "dates" && hasAmbiguousDates(item) && dateOrderQuestion(item)}
            </li>
          );
        })}
      </ul>
      {beforeAfter.length > 0 && (
        <div className="prepare-proposal__before-after">
          <button
            type="button"
            className="prepare-proposal__examples-toggle"
            aria-expanded={showBeforeAfter}
            aria-controls="prepare-before-after"
            onClick={() => setShowBeforeAfter(!showBeforeAfter)}
          >
            {showBeforeAfter ? "Ocultar antes y después" : "Ver antes y después"}
          </button>
          {showBeforeAfter && (
            <div id="prepare-before-after" className="table-region" tabIndex={0} aria-label="Antes y después de los cambios marcados">
              <table>
                <thead>
                  <tr><th scope="col">Cambio</th><th scope="col">Columna</th><th scope="col">Antes</th><th scope="col">Después</th></tr>
                </thead>
                <tbody>
                  {beforeAfter.map(({ item, example }, index) => (
                    <tr key={`${item.id}-${example.column}-${index}`}>
                      <td>{titleOf(item)}</td>
                      <th scope="row">{example.column}</th>
                      <td className="prepare-proposal__before">
                        {example.before === null ? <MissingValue /> : <CellText value={example.before} />}
                      </td>
                      <td><strong>{example.after}</strong></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
      <div className="prepare-proposal__actions">
        <button
          type="button"
          className="prepare-proposal__primary"
          disabled={disabled || count === 0 || !fillsReady || datesPending}
          // Still the action of this screen while the app is busy, the fill simulation
          // runs or a date answer is pending, so the footer never flashes as primary.
          data-waiting={count > 0 && (disabled || !fillsReady || datesPending) ? "" : undefined}
          onClick={() => onApply(proposalOptions(items, selection, false, ambiguousDateOrder))}
        >
          {applyLabel(count)}
        </button>
        <button type="button" className="link-action" disabled={disabled} onClick={() => { setMode("steps"); setStep(0); }}>
          Personalizar paso a paso
        </button>
        {canUndo && (
          <button type="button" className="link-action" disabled={busy} onClick={onUndo}>
            Deshacer último cambio
          </button>
        )}
      </div>
    </section>
  );
}
