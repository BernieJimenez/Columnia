import { useEffect, useState, type TextareaHTMLAttributes } from "react";

import {
  QUALITY_DATASET_COLUMN,
  type DatasetPreview,
  type QualityComparison,
  type QualityAggregate,
  type QualityMonotonicDirection,
  type QualityRule,
  type QualityRuleKind,
} from "../../bridge";
import { MAX_QUALITY_RULES } from "./deliveryModel";

interface QualityRulesEditorProps {
  rules: QualityRule[];
  dataset: DatasetPreview;
  busy: boolean;
  /** The rule the current validation error points at, if any. */
  validationErrorRuleIndex: number | null;
  onRulesChange: (rules: QualityRule[]) => void;
}

interface ListTextareaProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "value" | "onChange"> {
  values: string[];
  /** Trims each line, for column names. */
  trim?: boolean;
  onValuesChange: (values: string[]) => void;
}

function listFromText(text: string, trim: boolean): string[] {
  return text
    .split(/\r?\n/)
    .map((value) => (trim ? value.trim() : value))
    .filter((value) => value.length > 0);
}

/**
 * A one-value-per-line list. The typed text stays as written, so Intro and
 * spaces survive while typing (FUN-04); only the list sent upward is cleaned.
 */
function ListTextarea({ values, trim = false, onValuesChange, ...props }: ListTextareaProps) {
  const [text, setText] = useState(() => values.join("\n"));
  const key = values.join("\n");
  useEffect(() => {
    setText((current) => (listFromText(current, trim).join("\n") === key ? current : key));
  }, [key, trim]);
  return (
    <textarea
      {...props}
      value={text}
      onChange={(event) => {
        setText(event.target.value);
        onValuesChange(listFromText(event.target.value, trim));
      }}
    />
  );
}

/** The rules with a new «not null» rule on the first column, or null when full. */
export function withAddedRule(rules: QualityRule[], dataset: DatasetPreview): QualityRule[] | null {
  const firstColumn = dataset.columns[0];
  if (rules.length >= MAX_QUALITY_RULES || !firstColumn) return null;
  return [...rules, { column: firstColumn.name, kind: "not_null", maxInvalid: 0 }];
}

/** The editable list of quality rules in Entregar; the contract state stays in DeliveryPhase. */
export function QualityRulesEditor({ rules, dataset, busy, validationErrorRuleIndex, onRulesChange }: QualityRulesEditorProps) {
  const changeRules = onRulesChange;

  function addRule() {
    const next = withAddedRule(rules, dataset);
    if (next) changeRules(next);
  }

  function updateRule(index: number, update: Partial<QualityRule>) {
    changeRules(rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...update } : rule));
  }

  function changeRuleKind(index: number, kind: QualityRuleKind) {
    const rule = rules[index];
    if (!rule) return;
    const firstColumn = dataset.columns[0]?.name ?? "";
    const isSchemaRule = kind === "schema_contract";
    const isReferentialRule = kind === "referential_integrity";
    const isMonotonicRule = kind === "monotonic";
    const isAggregateCheckRule = kind === "aggregate_check";
    const isAggregateReconciliationRule = kind === "aggregate_reconciliation";
    const isDistributionDriftRule = kind === "distribution_drift";
    const isAggregateRule = isAggregateCheckRule || isAggregateReconciliationRule;
    const usesMultipleColumns = kind === "unique_together"
      || kind === "column_compare"
      || isReferentialRule
      || isAggregateReconciliationRule;
    const minimumColumns = isReferentialRule ? 1 : 2;
    const retainedColumns = rule.columns?.filter((name) =>
      dataset.columns.some((column) => column.name === name));
    const nextColumns = usesMultipleColumns
      ? retainedColumns && retainedColumns.length >= minimumColumns
        ? retainedColumns
        : dataset.columns.slice(0, minimumColumns).map((column) => column.name)
      : undefined;
    const nextColumn = kind === "row_count" || isSchemaRule
      ? QUALITY_DATASET_COLUMN
      : usesMultipleColumns
        ? nextColumns?.[0] ?? firstColumn
        : rule.column === QUALITY_DATASET_COLUMN ? firstColumn : rule.column;
    const nextConditionColumn = rule.when?.column && dataset.columns.some((column) => column.name === rule.when?.column)
      ? rule.when.column
      : firstColumn;
    const nextThenColumn = rule.then?.column && dataset.columns.some((column) => column.name === rule.then?.column)
      ? rule.then.column
      : nextColumn === QUALITY_DATASET_COLUMN ? firstColumn : nextColumn;
    updateRule(index, {
      kind,
      column: nextColumn,
      min: undefined,
      max: undefined,
      values: kind === "allowed_values" ? rule.values ?? [] : undefined,
      referenceValues: isReferentialRule ? rule.referenceValues ?? [] : undefined,
      baseline: isDistributionDriftRule ? rule.baseline ?? [] : undefined,
      direction: isMonotonicRule ? rule.direction ?? "increasing" : undefined,
      // UX-19: no silent «exactly 0»; the editor asks for the expected value.
      expected: isAggregateCheckRule ? rule.expected : undefined,
      aggregate: isAggregateCheckRule ? rule.aggregate ?? "sum" : undefined,
      toleranceAbs: isAggregateRule ? rule.toleranceAbs : undefined,
      toleranceRel: isAggregateRule ? rule.toleranceRel : undefined,
      threshold: isDistributionDriftRule ? rule.threshold : undefined,
      pattern: kind === "regex" ? rule.pattern ?? "" : undefined,
      dtype: kind === "dtype" ? rule.dtype ?? "string" : undefined,
      columns: isSchemaRule
        ? rule.columns ?? dataset.columns.map((column) => column.name)
        : usesMultipleColumns ? nextColumns : undefined,
      operator: kind === "column_compare" ? rule.operator ?? "eq" : undefined,
      minDate: kind === "date_range" ? rule.minDate : undefined,
      maxDate: kind === "date_range" ? rule.maxDate : undefined,
      when: kind === "conditional"
        ? rule.when ?? { column: nextConditionColumn, operator: "eq", value: "" }
        : undefined,
      then: kind === "conditional"
        ? rule.then ?? { column: nextThenColumn, kind: "not_null", maxInvalid: 0 }
        : undefined,
      allowAdditional: isSchemaRule ? rule.allowAdditional ?? true : undefined,
      requiredOrder: isSchemaRule ? rule.requiredOrder : undefined,
    });
  }

  function updateConditionalThen(index: number, update: Partial<QualityRule>) {
    const rule = rules[index];
    if (!rule) return;
    const fallbackColumn = rule.column === QUALITY_DATASET_COLUMN
      ? dataset.columns[0]?.name ?? ""
      : rule.column;
    const then = rule.then ?? { column: fallbackColumn, kind: "not_null" as const, maxInvalid: 0 };
    updateRule(index, { then: { ...then, ...update } });
  }

  function updateTogetherColumns(index: number, name: string, checked: boolean) {
    const current = rules[index]?.columns ?? [];
    const next = checked
      ? [...current, name]
      : current.filter((column) => column !== name);
    updateRule(index, { columns: next, column: next[0] ?? dataset.columns[0]?.name ?? "" });
  }

  function updateComparisonColumn(index: number, position: 0 | 1, name: string) {
    const rule = rules[index];
    if (!rule) return;
    const current = rule.columns ?? dataset.columns.slice(0, 2).map((column) => column.name);
    const next = [...current];
    next[position] = name;
    updateRule(index, {
      columns: next,
      column: next[0] ?? dataset.columns[0]?.name ?? "",
    });
  }

  function updateAggregateColumn(index: number, position: 0 | 1, name: string) {
    const rule = rules[index];
    if (!rule) return;
    const current = rule.columns ?? dataset.columns.slice(0, 2).map((column) => column.name);
    const next = [...current];
    next[position] = name;
    updateRule(index, {
      columns: next,
      column: next[0] ?? dataset.columns[0]?.name ?? "",
    });
  }

  return (
    <>
              <div className="quality-rules">
              {rules.map((rule, index) => {
                const hasCountTolerance = rule.maxInvalid !== undefined;
                const hasPercentageTolerance = rule.maxInvalidPct !== undefined;
                const toleranceMode = hasCountTolerance && hasPercentageTolerance
                  ? "both"
                  : hasPercentageTolerance ? "percentage" : "count";
                const isDatasetRule = rule.kind === "row_count";
                const isTogetherRule = rule.kind === "unique_together";
                const isCompareRule = rule.kind === "column_compare";
                const isReferentialRule = rule.kind === "referential_integrity";
                const isMonotonicRule = rule.kind === "monotonic";
                const isAggregateCheckRule = rule.kind === "aggregate_check";
                const isAggregateReconciliationRule = rule.kind === "aggregate_reconciliation";
                const isAggregateRule = isAggregateCheckRule || isAggregateReconciliationRule;
                const isDistributionDriftRule = rule.kind === "distribution_drift";
                const isConditionalRule = rule.kind === "conditional";
                const isSchemaRule = rule.kind === "schema_contract";
                return (
                  <fieldset
                    className="quality-rule"
                    id={`quality-rule-${index + 1}`}
                    key={index}
                    tabIndex={-1}
                    aria-describedby={validationErrorRuleIndex === index
                      ? "quality-rule-validation-error"
                      : undefined}
                    disabled={busy}
                  >
                    <legend>Regla {index + 1}</legend>
                    {!isDatasetRule && !isSchemaRule && !isTogetherRule && !isCompareRule && !isReferentialRule && !isAggregateReconciliationRule && <label>Columna
                      <select aria-label={`Columna regla ${index + 1}`} value={rule.column}
                        onChange={(event) => updateRule(index, { column: event.target.value })}>
                        {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                      </select>
                    </label>}
                    <label>Comprobación
                      <select aria-label={`Comprobación regla ${index + 1}`} value={rule.kind}
                        onChange={(event) => changeRuleKind(index, event.target.value as QualityRuleKind)}>
                        <option value="not_null">Sin nulos</option>
                        <option value="non_empty">Texto no vacío</option>
                        <option value="unique">Valores únicos</option>
                        <option value="numeric_range">Rango numérico</option>
                        <option value="allowed_values">Valores permitidos</option>
                        <option value="regex">Expresión regular</option>
                        <option value="dtype">Tipo esperado</option>
                        <option value="unique_together" disabled={dataset.columns.length < 2}>Unicidad compuesta</option>
                        <option value="column_compare" disabled={dataset.columns.length < 2}>Comparar columnas</option>
                        <option value="referential_integrity">Integridad referencial</option>
                        <option value="monotonic">Monotonicidad</option>
                        <option value="aggregate_check">Comprobación agregada</option>
                        <option value="aggregate_reconciliation" disabled={dataset.columns.length < 2}>Reconciliación agregada</option>
                        <option value="distribution_drift">Cambio de la media</option>
                        <option value="date_range">Rango de fechas</option>
                        <option value="conditional">Comprobación condicional</option>
                        <option value="schema_contract">Contrato de esquema</option>
                        <option value="row_count">Conteo de filas</option>
                      </select>
                    </label>
                    <label>Tolerancia
                      <select aria-label={`Tolerancia regla ${index + 1}`} value={toleranceMode}
                        onChange={(event) => {
                          const mode = event.target.value;
                          updateRule(index, mode === "both"
                            ? { maxInvalid: rule.maxInvalid ?? 0, maxInvalidPct: rule.maxInvalidPct ?? 0 }
                            : mode === "percentage"
                              ? { maxInvalid: undefined, maxInvalidPct: rule.maxInvalidPct ?? 0 }
                              : { maxInvalid: rule.maxInvalid ?? 0, maxInvalidPct: undefined });
                        }}>
                        <option value="count">Máximo inválidos</option>
                        <option value="percentage">Máximo porcentaje</option>
                        <option value="both">Ambas tolerancias</option>
                      </select>
                    </label>
                    {(toleranceMode === "count" || toleranceMode === "both") && <label>Inválidos máximos
                      <input type="number" min="0" step="1"
                        aria-label={`Inválidos máximos regla ${index + 1}`}
                        value={numberFieldValue(rule.maxInvalid)}
                        onChange={(event) => updateRule(index, { maxInvalid: numberFieldInput(event.target.value) })} />
                    </label>}
                    {(toleranceMode === "percentage" || toleranceMode === "both") && <label>Porcentaje máximo
                      <input type="number" min="0" max="100" step="0.1"
                        aria-label={`Porcentaje máximo regla ${index + 1}`}
                        value={numberFieldValue(rule.maxInvalidPct)}
                        onChange={(event) => updateRule(index, { maxInvalidPct: numberFieldInput(event.target.value) })} />
                    </label>}
                    {(rule.kind === "numeric_range" || rule.kind === "row_count") && (
                      <>
                        <label>{rule.kind === "row_count" ? "Filas mínimas" : "Mínimo inclusivo"}
                          <input type="number" aria-label={`${rule.kind === "row_count" ? "Filas mínimas" : "Mínimo inclusivo"} regla ${index + 1}`}
                            value={rule.min ?? ""}
                            onChange={(event) => updateRule(index, { min: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                        <label>{rule.kind === "row_count" ? "Filas máximas" : "Máximo inclusivo"}
                          <input type="number" aria-label={`${rule.kind === "row_count" ? "Filas máximas" : "Máximo inclusivo"} regla ${index + 1}`}
                            value={rule.max ?? ""}
                            onChange={(event) => updateRule(index, { max: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                      </>
                    )}
                    {rule.kind === "date_range" && (
                      <>
                        <label>Fecha mínima inclusiva
                          <input
                            type="date"
                            aria-label={`Fecha mínima regla ${index + 1}`}
                            value={rule.minDate ?? ""}
                            onChange={(event) => updateRule(index, {
                              minDate: event.target.value || undefined,
                            })}
                          />
                        </label>
                        <label>Fecha máxima inclusiva
                          <input
                            type="date"
                            aria-label={`Fecha máxima regla ${index + 1}`}
                            value={rule.maxDate ?? ""}
                            onChange={(event) => updateRule(index, {
                              maxDate: event.target.value || undefined,
                            })}
                          />
                        </label>
                      </>
                    )}
                    {rule.kind === "allowed_values" && (
                      <label className="quality-rule__wide">Valores permitidos
                        <ListTextarea
                          rows={2}
                          aria-label={`Valores permitidos regla ${index + 1}`}
                          aria-describedby={`quality-values-help-${index}`}
                          values={rule.values ?? []}
                          onValuesChange={(values) => updateRule(index, {
                            values,
                          })}
                        />
                        <span id={`quality-values-help-${index}`} className="quality-rule__help">Un valor por línea; se compara sin transformar.</span>
                      </label>
                    )}
                    {rule.kind === "regex" && (
                      <label className="quality-rule__wide">Patrón regular
                        <input
                          type="text"
                          aria-label={`Patrón regular regla ${index + 1}`}
                          aria-describedby={`quality-regex-help-${index}`}
                          value={rule.pattern ?? ""}
                          onChange={(event) => updateRule(index, { pattern: event.target.value })}
                        />
                        <span id={`quality-regex-help-${index}`} className="quality-rule__help">Sintaxis regex compatible con Rust.</span>
                      </label>
                    )}
                    {rule.kind === "dtype" && (
                      <label>Tipo esperado
                        <select aria-label={`Tipo esperado regla ${index + 1}`} value={rule.dtype ?? "string"}
                          onChange={(event) => updateRule(index, { dtype: event.target.value })}>
                          <option value="string">Texto</option>
                          <option value="integer">Entero</option>
                          <option value="float">Decimal</option>
                          <option value="boolean">Booleano</option>
                          <option value="date">Fecha</option>
                          <option value="datetime">Fecha y hora</option>
                        </select>
                      </label>
                    )}
                    {isTogetherRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Columnas que deben ser únicas juntas</legend>
                        {dataset.columns.map((column) => (
                          <label key={column.name} className="quality-rule__check">
                            <input
                              type="checkbox"
                              aria-label={`Columna compuesta ${column.name}, regla ${index + 1}`}
                              checked={rule.columns?.includes(column.name) ?? false}
                              onChange={(event) => updateTogetherColumns(index, column.name, event.target.checked)}
                            />
                            {column.name}
                          </label>
                        ))}
                      </fieldset>
                    )}
                    {isReferentialRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Clave y valores de referencia</legend>
                        <p className="quality-rule__help">
                          Selecciona una o más columnas. Para una sola columna, escribe un valor por línea; para varias, usa un arreglo JSON por línea.
                        </p>
                        {dataset.columns.map((column) => (
                          <label key={column.name} className="quality-rule__check">
                            <input
                              type="checkbox"
                              aria-label={`Columna referencial ${column.name}, regla ${index + 1}`}
                              checked={rule.columns?.includes(column.name) ?? false}
                              onChange={(event) => updateTogetherColumns(index, column.name, event.target.checked)}
                            />
                            {column.name}
                          </label>
                        ))}
                        <label className="quality-rule__wide">Valores permitidos de referencia
                          <ListTextarea
                            rows={3}
                            aria-label={`Valores permitidos de referencia regla ${index + 1}`}
                            aria-describedby={`quality-reference-values-help-${index}`}
                            values={rule.referenceValues ?? []}
                            onValuesChange={(values) => updateRule(index, {
                              referenceValues: values,
                            })}
                          />
                          <span id={`quality-reference-values-help-${index}`} className="quality-rule__help">
                            Clave simple: texto, número o booleano. Clave compuesta: por ejemplo [&quot;DO&quot;, 1].
                          </span>
                        </label>
                      </fieldset>
                    )}
                    {isMonotonicRule && (
                      <label>Dirección de la secuencia
                        <select
                          aria-label={`Dirección monotónica regla ${index + 1}`}
                          value={rule.direction ?? "increasing"}
                          onChange={(event) => updateRule(index, {
                            direction: event.target.value as QualityMonotonicDirection,
                          })}
                        >
                          <option value="increasing">No decreciente</option>
                          <option value="decreasing">No creciente</option>
                        </select>
                      </label>
                    )}
                    {isAggregateCheckRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Comprobación agregada</legend>
                        <label>Agregación
                          <select
                            aria-label={`Agregación regla ${index + 1}`}
                            value={rule.aggregate ?? "sum"}
                            onChange={(event) => updateRule(index, {
                              aggregate: event.target.value as QualityAggregate,
                            })}
                          >
                            <option value="count">Conteo</option>
                            <option value="sum">Suma</option>
                            <option value="min">Mínimo</option>
                            <option value="max">Máximo</option>
                          </select>
                        </label>
                        <label>Valor esperado
                          <input
                            type="number"
                            step="any"
                            aria-label={`Valor esperado agregado regla ${index + 1}`}
                            value={rule.expected ?? ""}
                            onChange={(event) => updateRule(index, {
                              expected: event.target.value === "" ? undefined : Number(event.target.value),
                              referenceValues: undefined,
                            })}
                          />
                        </label>
                        <label className="quality-rule__wide">Referencias numéricas opcionales
                          <ListTextarea
                            rows={2}
                            aria-label={`Referencias numéricas opcionales regla ${index + 1}`}
                            values={rule.referenceValues ?? []}
                            onValuesChange={(values) => updateRule(index, {
                              referenceValues: values,
                              expected: undefined,
                            })}
                          />
                          <span className="quality-rule__help">Usa el valor esperado o estas referencias; se suman cuando la agregación es suma.</span>
                        </label>
                      </fieldset>
                    )}
                    {isDistributionDriftRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Cambio de la media respecto a la línea base</legend>
                        <label className="quality-rule__wide">Línea base numérica
                          <ListTextarea
                            rows={3}
                            aria-label={`Línea base numérica regla ${index + 1}`}
                            aria-describedby={`quality-drift-baseline-help-${index}`}
                            values={rule.baseline ?? []}
                            onValuesChange={(values) => updateRule(index, {
                              baseline: values,
                              referenceValues: undefined,
                            })}
                          />
                          <span id={`quality-drift-baseline-help-${index}`} className="quality-rule__help">
                            Un número por línea; se compara la media actual con la media de esta línea base.
                          </span>
                        </label>
                        <label>Umbral absoluto
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Umbral de drift regla ${index + 1}`}
                            value={rule.threshold ?? ""}
                            onChange={(event) => updateRule(index, {
                              threshold: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    {isAggregateReconciliationRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Reconciliar sumas</legend>
                        <label>Columna izquierda
                          <select
                            aria-label={`Columna izquierda agregada regla ${index + 1}`}
                            value={rule.columns?.[0] ?? ""}
                            onChange={(event) => updateAggregateColumn(index, 0, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                        <label>Columna derecha
                          <select
                            aria-label={`Columna derecha agregada regla ${index + 1}`}
                            value={rule.columns?.[1] ?? ""}
                            onChange={(event) => updateAggregateColumn(index, 1, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                      </fieldset>
                    )}
                    {isAggregateRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Tolerancia numérica del agregado</legend>
                        <label>Tolerancia absoluta
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Tolerancia absoluta agregada regla ${index + 1}`}
                            value={rule.toleranceAbs ?? ""}
                            onChange={(event) => updateRule(index, {
                              toleranceAbs: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                        <label>Tolerancia relativa
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Tolerancia relativa agregada regla ${index + 1}`}
                            value={rule.toleranceRel ?? ""}
                            onChange={(event) => updateRule(index, {
                              toleranceRel: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    {isCompareRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Comparar columnas</legend>
                        <label>Columna izquierda
                          <select
                            aria-label={`Columna izquierda comparar regla ${index + 1}`}
                            value={rule.columns?.[0] ?? ""}
                            onChange={(event) => updateComparisonColumn(index, 0, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                        <label>Operador
                          <select
                            aria-label={`Operador comparar regla ${index + 1}`}
                            value={rule.operator ?? "eq"}
                            onChange={(event) => updateRule(index, { operator: event.target.value as QualityComparison })}
                          >
                            <option value="eq">Igual a</option>
                            <option value="ne">Distinta de</option>
                            <option value="lt">Menor que</option>
                            <option value="lte">Menor o igual que</option>
                            <option value="gt">Mayor que</option>
                            <option value="gte">Mayor o igual que</option>
                          </select>
                        </label>
                        <label>Columna derecha
                          <select
                            aria-label={`Columna derecha comparar regla ${index + 1}`}
                            value={rule.columns?.[1] ?? ""}
                            onChange={(event) => updateComparisonColumn(index, 1, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                      </fieldset>
                    )}
                    {isConditionalRule && (
                      <>
                        <fieldset className="quality-rule__wide quality-rule__columns">
                          <legend>Cuando se cumpla</legend>
                          <label>Columna condición
                            <select
                              aria-label={`Columna condición regla ${index + 1}`}
                              value={rule.when?.column ?? rule.column}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: event.target.value,
                                  operator: rule.when?.operator ?? "eq",
                                  value: rule.when?.value ?? "",
                                },
                              })}
                            >
                              {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                            </select>
                          </label>
                          <label>Operador condición
                            <select
                              aria-label={`Operador condición regla ${index + 1}`}
                              value={rule.when?.operator ?? "eq"}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: rule.when?.column ?? rule.column,
                                  operator: event.target.value as QualityComparison,
                                  value: rule.when?.value ?? "",
                                },
                              })}
                            >
                              <option value="eq">Igual a</option>
                              <option value="ne">Distinta de</option>
                              <option value="lt">Menor que</option>
                              <option value="lte">Menor o igual que</option>
                              <option value="gt">Mayor que</option>
                              <option value="gte">Mayor o igual que</option>
                            </select>
                          </label>
                          <label>Valor esperado
                            <input
                              type="text"
                              aria-label={`Valor condición regla ${index + 1}`}
                              value={rule.when?.value ?? ""}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: rule.when?.column ?? rule.column,
                                  operator: rule.when?.operator ?? "eq",
                                  value: event.target.value,
                                },
                              })}
                            />
                          </label>
                        </fieldset>
                        <fieldset className="quality-rule__wide quality-rule__columns">
                          <legend>Comprobar entonces</legend>
                          <label>Columna objetivo
                            <select
                              aria-label={`Columna objetivo conditional regla ${index + 1}`}
                              value={rule.then?.column ?? rule.column}
                              onChange={(event) => updateConditionalThen(index, { column: event.target.value })}
                            >
                              {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                            </select>
                          </label>
                          <label>Comprobación then
                            <select
                              aria-label={`Comprobación then regla ${index + 1}`}
                              value={rule.then?.kind ?? "not_null"}
                              onChange={(event) => {
                                const kind = event.target.value as QualityRuleKind;
                                updateConditionalThen(index, {
                                  kind,
                                  min: undefined,
                                  max: undefined,
                                  values: kind === "allowed_values" ? rule.then?.values ?? [] : undefined,
                                  pattern: kind === "regex" ? rule.then?.pattern ?? "" : undefined,
                                  dtype: kind === "dtype" ? rule.then?.dtype ?? "string" : undefined,
                                  columns: undefined,
                                  operator: undefined,
                                  minDate: undefined,
                                  maxDate: undefined,
                                  when: undefined,
                                  then: undefined,
                                });
                              }}
                            >
                              <option value="not_null">Sin nulos</option>
                              <option value="non_empty">Texto no vacío</option>
                              <option value="numeric_range">Rango numérico</option>
                              <option value="allowed_values">Valores permitidos</option>
                              <option value="regex">Expresión regular</option>
                              <option value="dtype">Tipo esperado</option>
                            </select>
                          </label>
                          {(rule.then?.kind === "numeric_range") && (
                            <>
                              <label>Mínimo then
                                <input
                                  type="number"
                                  aria-label={`Mínimo then regla ${index + 1}`}
                                  value={rule.then.min ?? ""}
                                  onChange={(event) => updateConditionalThen(index, {
                                    min: event.target.value === "" ? undefined : Number(event.target.value),
                                  })}
                                />
                              </label>
                              <label>Máximo then
                                <input
                                  type="number"
                                  aria-label={`Máximo then regla ${index + 1}`}
                                  value={rule.then.max ?? ""}
                                  onChange={(event) => updateConditionalThen(index, {
                                    max: event.target.value === "" ? undefined : Number(event.target.value),
                                  })}
                                />
                              </label>
                            </>
                          )}
                          {rule.then?.kind === "allowed_values" && (
                            <label className="quality-rule__wide">Valores permitidos then
                              <ListTextarea
                                rows={2}
                                aria-label={`Valores permitidos then regla ${index + 1}`}
                                values={rule.then.values ?? []}
                                onValuesChange={(values) => updateConditionalThen(index, {
                                  values,
                                })}
                              />
                            </label>
                          )}
                          {rule.then?.kind === "regex" && (
                            <label className="quality-rule__wide">Patrón regular then
                              <input
                                type="text"
                                aria-label={`Patrón regular then regla ${index + 1}`}
                                value={rule.then.pattern ?? ""}
                                onChange={(event) => updateConditionalThen(index, { pattern: event.target.value })}
                              />
                            </label>
                          )}
                          {rule.then?.kind === "dtype" && (
                            <label>Tipo esperado then
                              <select
                                aria-label={`Tipo esperado then regla ${index + 1}`}
                                value={rule.then.dtype ?? "string"}
                                onChange={(event) => updateConditionalThen(index, { dtype: event.target.value })}
                              >
                                <option value="string">Texto</option>
                                <option value="integer">Entero</option>
                                <option value="float">Decimal</option>
                                <option value="boolean">Booleano</option>
                                <option value="date">Fecha</option>
                                <option value="datetime">Fecha y hora</option>
                              </select>
                            </label>
                          )}
                        </fieldset>
                      </>
                    )}
                    {isSchemaRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Contrato de esquema</legend>
                        <label className="quality-rule__wide">Columnas requeridas
                          <ListTextarea
                            rows={3}
                            aria-label={`Columnas requeridas esquema regla ${index + 1}`}
                            values={rule.columns ?? []}
                            trim
                            onValuesChange={(values) => updateRule(index, {
                              columns: values,
                            })}
                          />
                          <span className="quality-rule__help">Una columna por línea; el dataset puede tener columnas adicionales si se permite abajo.</span>
                        </label>
                        <label className="quality-rule__check">
                          <input
                            type="checkbox"
                            aria-label={`Permitir columnas adicionales esquema regla ${index + 1}`}
                            checked={rule.allowAdditional ?? true}
                            onChange={(event) => updateRule(index, { allowAdditional: event.target.checked })}
                          />
                          Permitir columnas adicionales
                        </label>
                        <label className="quality-rule__wide">Orden requerido, opcional
                          <ListTextarea
                            rows={2}
                            aria-label={`Orden requerido, opcional esquema regla ${index + 1}`}
                            values={rule.requiredOrder ?? []}
                            trim
                            onValuesChange={(values) => updateRule(index, {
                              requiredOrder: values.length === 0 ? undefined : values,
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    <button type="button" className="quality-rule__remove" aria-label={`Eliminar regla ${index + 1}`}
                      onClick={() => changeRules(rules.filter((_, ruleIndex) => ruleIndex !== index))}>Eliminar</button>
                  </fieldset>
                );
              })}
              </div>
              <div className="quality-contract__actions">
                <span>{rules.length}/{MAX_QUALITY_RULES} reglas</span>
                <button type="button" onClick={addRule} disabled={busy || rules.length >= MAX_QUALITY_RULES}>Añadir regla</button>
              </div>
    </>
  );
}

/**
 * UX-19: an emptied tolerance field stays empty (NaN, which the draft
 * validation reports) instead of jumping back to 0 and becoming «05».
 */
function numberFieldValue(value: number | undefined): number | string {
  return value === undefined || Number.isNaN(value) ? "" : value;
}

function numberFieldInput(text: string): number {
  return text.trim() === "" ? Number.NaN : Number(text);
}
