import { useState } from "react";

import type { DatasetProfile, SafeCorrectionOptions } from "../../bridge";
import { CellText } from "../../components/CellText";
import { MissingValue } from "../../components/MissingValue";
import {
  applyLabel,
  defaultProposalSelection,
  proposalOptions,
  selectedProposalCount,
  type ProposalItem,
  type ProposalItemId,
  type ProposalSelection,
} from "./proposalModel";

export interface ProposalResult {
  before: DatasetProfile;
  after: DatasetProfile;
}

interface PrepareProposalProps {
  items: ProposalItem[];
  columnCount: number;
  busy: boolean;
  canUndo: boolean;
  result: ProposalResult | null;
  onApply: (options: SafeCorrectionOptions) => void;
  onUndo: () => void;
  onDismissResult: () => void;
}

const STEP_QUESTIONS: Record<ProposalItemId, { question: string; yes: string; no: string }> = {
  sentinels: { question: "¿Convertimos los marcadores de «sin dato» en vacíos reales?", yes: "Sí, convertir", no: "No, dejarlos como texto" },
  trim: { question: "¿Recortamos los espacios sobrantes del texto?", yes: "Sí, recortar", no: "No, dejarlos" },
  duplicates: { question: "¿Quitamos las filas duplicadas?", yes: "Sí, quitarlas", no: "No, pueden ser registros distintos" },
  impute: { question: "¿Rellenamos los valores vacíos?", yes: "Sí, rellenar", no: "No, dejarlos vacíos" },
};

function nullTotal(profile: DatasetProfile): number {
  return profile.columns.reduce((total, column) => total + column.nullCount, 0);
}

export function PrepareProposal({
  items,
  columnCount,
  busy,
  canUndo,
  result,
  onApply,
  onUndo,
  onDismissResult,
}: PrepareProposalProps) {
  const signature = items.map((item) => `${item.id}:${item.title}`).join("|");
  const [selection, setSelection] = useState<ProposalSelection>(() => defaultProposalSelection(items));
  const [openExamples, setOpenExamples] = useState<ProposalItemId | null>(null);
  const [mode, setMode] = useState<"proposal" | "steps">("proposal");
  const [step, setStep] = useState(0);
  const [normalizeNames, setNormalizeNames] = useState(false);

  const [shownSignature, setShownSignature] = useState(signature);
  if (shownSignature !== signature) {
    setShownSignature(signature);
    setSelection(defaultProposalSelection(items));
    setOpenExamples(null);
    setMode("proposal");
    setStep(0);
    setNormalizeNames(false);
  }

  if (result) {
    const rows = [
      { label: "Filas", before: result.before.rowCount, after: result.after.rowCount },
      { label: "Valores vacíos", before: nullTotal(result.before), after: nullTotal(result.after) },
      { label: "Filas duplicadas", before: result.before.duplicateRowCount, after: result.after.duplicateRowCount },
      { label: "Columnas", before: result.before.columns.length, after: result.after.columns.length },
    ];
    return (
      <section className="prepare-proposal" aria-labelledby="prepare-result-title">
        <div role="status">
          <h3 id="prepare-result-title" className="prepare-proposal__title">Listo: cambios aplicados</h3>
          <p className="prepare-proposal__lead">Así quedaron tus datos.</p>
        </div>
        <dl className="prepare-proposal__result">
          {rows.map((row) => (
            <div key={row.label}>
              <dt>{row.label}</dt>
              <dd>{row.before.toLocaleString()} → <strong>{row.after.toLocaleString()}</strong></dd>
            </div>
          ))}
        </dl>
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

  if (mode === "steps") {
    const totalSteps = items.length + 1;
    const isNamesStep = step === items.length;
    const current = isNamesStep ? null : items[step];
    const question = current
      ? STEP_QUESTIONS[current.id]
      : { question: `¿Normalizamos los nombres de las ${columnCount} columnas?`, yes: "Sí, normalizar", no: "No, dejarlos" };
    const hint = current ? `${current.title}. ${current.hint}` : "Minúsculas y sin espacios. Puede afectar consultas e integraciones.";
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
              disabled={disabled || (selectedProposalCount(items, selection) === 0 && !normalizeNames)}
              onClick={() => onApply(proposalOptions(items, selection, normalizeNames))}
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
  return (
    <section className="prepare-proposal" aria-labelledby="prepare-proposal-title">
      <h3 id="prepare-proposal-title" className="prepare-proposal__title">
        {items.length === 1 ? "Columnia propone 1 cambio" : `Columnia propone ${items.length} cambios`}
      </h3>
      <p className="prepare-proposal__lead">Revísalos y aplícalos. Puedes deshacerlo después.</p>
      <ul className="prepare-proposal__list">
        {items.map((item) => {
          const examplesId = `prepare-examples-${item.id}`;
          const open = openExamples === item.id;
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
                <label htmlFor={`prepare-item-${item.id}`}>{item.title}</label>
                {item.examples.length > 0 && (
                  <button
                    type="button"
                    className="prepare-proposal__examples-toggle"
                    aria-expanded={open}
                    aria-controls={examplesId}
                    onClick={() => setOpenExamples(open ? null : item.id)}
                  >
                    {open ? "Ocultar ejemplos" : "Ver ejemplos"}
                  </button>
                )}
              </div>
              <p id={`prepare-hint-${item.id}`} className="prepare-proposal__hint">{item.hint}</p>
              {open && (
                <ul id={examplesId} className="prepare-proposal__examples" aria-label={`Ejemplos: ${item.title}`}>
                  {item.examples.map((example, index) => (
                    <li key={`${example.column}-${index}`}>
                      <span className="prepare-proposal__column">{example.column}</span>
                      <span>
                        <span className="prepare-proposal__before">
                          {example.before === null ? <MissingValue /> : <CellText value={example.before} />}
                        </span>
                        {" → "}
                        <strong>{example.after}</strong>
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          );
        })}
      </ul>
      <div className="prepare-proposal__actions">
        <button
          type="button"
          className="prepare-proposal__primary"
          disabled={disabled || count === 0}
          onClick={() => onApply(proposalOptions(items, selection))}
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
