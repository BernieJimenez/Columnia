import { useState } from "react";

import {
  QUALITY_DATASET_COLUMN,
  openLastExport,
  pickQualityRulesMigration,
  saveQualityRulesDocument,
  validateQualityRules,
  type DatasetPreview,
  type ExportFormat,
  type PrivacyMode,
  type QualityMigrationResult,
  type QualityRulesDocument,
  type QualityComparison,
  type QualityAggregate,
  type QualityMonotonicDirection,
  type QualityRule,
  type QualityRuleKind,
  type SavedRecipe,
} from "../../bridge";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics, formatFileSize } from "./DatasetMetrics";
import {
  MAX_QUALITY_RULES,
  type DeliveryContractAction,
  type DeliveryContractState,
  type DeliveryExportRequest,
  type DeliveryExportState,
  validateQualityRuleDraft,
} from "./deliveryModel";

interface DeliveryPhaseProps {
  dataset: DatasetPreview;
  recipeDraft?: SavedRecipe | null;
  contract: DeliveryContractState;
  exportState: DeliveryExportState;
  onContractAction: (action: DeliveryContractAction) => void;
  onExport: (request: DeliveryExportRequest) => void;
  onCancelExport: () => void;
}

export function DeliveryPhase({
  dataset,
  recipeDraft = null,
  contract,
  exportState,
  onContractAction,
  onExport,
  onCancelExport,
}: DeliveryPhaseProps) {
  const [privacyMode, setPrivacyMode] = useState<PrivacyMode>("none");
  const [exportFormat, setExportFormat] = useState<ExportFormat>("csv");
  const [migrationState, setMigrationState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; result: QualityMigrationResult }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [qualityFileState, setQualityFileState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; document: QualityRulesDocument }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [openOutputState, setOpenOutputState] = useState<
    "idle" | "working" | "opened" | "error"
  >("idle");
  const rules = contract.kind === "with_contract" ? contract.rules : [];
  const validationError = validateQualityRuleDraft(rules, dataset);
  const gatePassed = contract.gate.kind === "ready" && contract.gate.result.passed;
  const exportAllowed = contract.kind === "with_contract"
    ? gatePassed
    : contract.confirmation === "confirmed";
  const busy = exportState.kind === "loading"
    || contract.gate.kind === "loading"
    || migrationState.kind === "working"
    || qualityFileState.kind === "working";
  const exportFormatLabel = {
    csv: "CSV",
    json: "JSON",
    parquet: "Parquet",
    sql: "SQL",
    excel: "Excel",
    sqlite: "SQLite",
    bundle: "Paquete ZIP",
  }[exportFormat];
  const exportRequirement = contract.kind === "with_contract"
    ? "Valida y aprueba las reglas para habilitar la exportación."
    : "Confirma abajo que quieres exportar sin validar la calidad.";

  function changeRules(nextRules: QualityRule[]) {
    setQualityFileState({ kind: "idle" });
    onContractAction({ kind: "rules_changed", rules: nextRules });
  }

  function addRule() {
    if (rules.length >= MAX_QUALITY_RULES || dataset.columns.length === 0) return;
    changeRules([...rules, {
      column: dataset.columns[0].name,
      kind: "not_null",
      maxInvalid: 0,
    }]);
  }

  function updateRule(index: number, update: Partial<QualityRule>) {
    changeRules(rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...update } : rule));
  }

  function changeRuleKind(index: number, kind: QualityRuleKind) {
    const rule = rules[index];
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
      expected: isAggregateCheckRule ? rule.expected ?? 0 : undefined,
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
    const fallbackColumn = rule.column === QUALITY_DATASET_COLUMN
      ? dataset.columns[0]?.name ?? ""
      : rule.column;
    const then = rule.then ?? { column: fallbackColumn, kind: "not_null" as const, maxInvalid: 0 };
    updateRule(index, { then: { ...then, ...update } });
  }

  function updateTogetherColumns(index: number, name: string, checked: boolean) {
    const current = rules[index].columns ?? [];
    const next = checked
      ? [...current, name]
      : current.filter((column) => column !== name);
    updateRule(index, { columns: next, column: next[0] ?? dataset.columns[0]?.name ?? "" });
  }

  function updateComparisonColumn(index: number, position: 0 | 1, name: string) {
    const rule = rules[index];
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
    const current = rule.columns ?? dataset.columns.slice(0, 2).map((column) => column.name);
    const next = [...current];
    next[position] = name;
    updateRule(index, {
      columns: next,
      column: next[0] ?? dataset.columns[0]?.name ?? "",
    });
  }

  async function runQualityGate() {
    if (contract.kind !== "with_contract" || validationError) return;
    onContractAction({ kind: "gate_changed", gate: { kind: "loading" } });
    try {
      const result = await validateQualityRules(contract.rules);
      onContractAction({ kind: "gate_changed", gate: { kind: "ready", result } });
    } catch (error: unknown) {
      onContractAction({
        kind: "gate_changed",
        gate: {
          kind: "error",
          message: error instanceof Error ? error.message : String(error),
        },
      });
    }
  }

  async function importQualityRules() {
    setQualityFileState({ kind: "idle" });
    setMigrationState({ kind: "working" });
    try {
      const result = await pickQualityRulesMigration();
      if (result) {
        onContractAction({ kind: "rules_changed", rules: result.convertedRules });
        setMigrationState({ kind: "ready", result });
      } else {
        setMigrationState({ kind: "idle" });
      }
    } catch (error: unknown) {
      setMigrationState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function saveQualityContract() {
    if (contract.kind !== "with_contract" || validationError) return;
    setMigrationState({ kind: "idle" });
    setQualityFileState({ kind: "working" });
    try {
      const document = await saveQualityRulesDocument(contract.rules);
      setQualityFileState(document ? { kind: "ready", document } : { kind: "idle" });
    } catch (error: unknown) {
      setQualityFileState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  function requestExport(format: ExportFormat) {
    setOpenOutputState("idle");
    if (contract.kind === "with_contract") {
      onExport({ format, privacyMode, validation: { kind: "contract", rules: contract.rules } });
    } else if (contract.confirmation === "confirmed") {
      onExport({ format, privacyMode, validation: { kind: "explicitly_unvalidated" } });
    }
  }

  async function revealLastExport() {
    if (openOutputState === "working") return;
    setOpenOutputState("working");
    try {
      await openLastExport();
      setOpenOutputState("opened");
    } catch {
      setOpenOutputState("error");
    }
  }

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Entregar · Exportación local</p>
          <h2>Valida y crea una copia</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
          <p>Genera una copia del dataset preparado. El archivo original nunca se modifica.</p>
        </div>
      </header>
      <DatasetMetrics dataset={dataset} />
      <section className="quality-contract" aria-labelledby="quality-contract-title">
        <div className="quality-contract__header">
          <div>
            <p className="step">Control de entrega</p>
            <h3 id="quality-contract-title">Elige cómo validar la entrega</h3>
            <p>Usa reglas locales para comprobar el resultado o continúa sin validación de calidad.</p>
          </div>
        </div>

        <fieldset className="delivery-route">
          <legend>Ruta de entrega</legend>
          <label>
            <input
              type="radio"
              name="delivery-validation-route"
              checked={contract.kind === "with_contract"}
              disabled={busy || dataset.columns.length === 0}
              onChange={() => contract.kind !== "with_contract" && addRule()}
            />
            <span>
              <strong>Validar calidad</strong>
              <small>Recomendado · define hasta {MAX_QUALITY_RULES} comprobaciones locales</small>
            </span>
          </label>
          <label>
            <input
              type="radio"
              name="delivery-validation-route"
              checked={contract.kind === "without_contract"}
              disabled={busy}
              onChange={() => contract.kind !== "without_contract" && changeRules([])}
            />
            <span>
              <strong>Exportar sin validar</strong>
              <small>Requiere una confirmación explícita durante esta sesión</small>
            </span>
          </label>
        </fieldset>

        {contract.kind === "with_contract" ? (
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
                  <fieldset className="quality-rule" key={index} disabled={busy}>
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
                        <option value="distribution_drift">Drift de distribución</option>
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
                        aria-label={`Inválidos regla ${index + 1}`}
                        value={rule.maxInvalid ?? 0}
                        onChange={(event) => updateRule(index, { maxInvalid: Number(event.target.value) })} />
                    </label>}
                    {(toleranceMode === "percentage" || toleranceMode === "both") && <label>Porcentaje máximo
                      <input type="number" min="0" max="100" step="0.1"
                        aria-label={`Porcentaje regla ${index + 1}`}
                        value={rule.maxInvalidPct ?? 0}
                        onChange={(event) => updateRule(index, { maxInvalidPct: Number(event.target.value) })} />
                    </label>}
                    {(rule.kind === "numeric_range" || rule.kind === "row_count") && (
                      <>
                        <label>{rule.kind === "row_count" ? "Filas mínimas" : "Mínimo inclusivo"}
                          <input type="number" aria-label={`Mínimo regla ${index + 1}`}
                            value={rule.min ?? ""}
                            onChange={(event) => updateRule(index, { min: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                        <label>{rule.kind === "row_count" ? "Filas máximas" : "Máximo inclusivo"}
                          <input type="number" aria-label={`Máximo regla ${index + 1}`}
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
                        <textarea
                          rows={2}
                          aria-label={`Valores permitidos regla ${index + 1}`}
                          aria-describedby={`quality-values-help-${index}`}
                          value={rule.values?.join("\n") ?? ""}
                          onChange={(event) => updateRule(index, {
                            values: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
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
                          <textarea
                            rows={3}
                            aria-label={`Valores de referencia regla ${index + 1}`}
                            aria-describedby={`quality-reference-values-help-${index}`}
                            value={rule.referenceValues?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              referenceValues: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
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
                          <textarea
                            rows={2}
                            aria-label={`Referencias agregadas regla ${index + 1}`}
                            value={rule.referenceValues?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              referenceValues: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                              expected: undefined,
                            })}
                          />
                          <span className="quality-rule__help">Usa el valor esperado o estas referencias; se suman cuando la agregación es suma.</span>
                        </label>
                      </fieldset>
                    )}
                    {isDistributionDriftRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Drift de distribución</legend>
                        <label className="quality-rule__wide">Línea base numérica
                          <textarea
                            rows={3}
                            aria-label={`Línea base de distribución regla ${index + 1}`}
                            aria-describedby={`quality-drift-baseline-help-${index}`}
                            value={rule.baseline?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              baseline: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
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
                              <textarea
                                rows={2}
                                aria-label={`Valores permitidos then regla ${index + 1}`}
                                value={rule.then.values?.join("\n") ?? ""}
                                onChange={(event) => updateConditionalThen(index, {
                                  values: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
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
                          <textarea
                            rows={3}
                            aria-label={`Columnas requeridas esquema regla ${index + 1}`}
                            value={rule.columns?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              columns: event.target.value
                                .split(/\r?\n/)
                                .map((value) => value.trim())
                                .filter((value) => value.length > 0),
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
                          <textarea
                            rows={2}
                            aria-label={`Orden requerido esquema regla ${index + 1}`}
                            value={rule.requiredOrder?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              requiredOrder: event.target.value.trim().length === 0
                                ? undefined
                                : event.target.value
                                  .split(/\r?\n/)
                                  .map((value) => value.trim())
                                  .filter((value) => value.length > 0),
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
              <button type="button" onClick={() => void importQualityRules()} disabled={busy}>Importar contrato</button>
              <button type="button" onClick={() => void saveQualityContract()}
                disabled={busy || validationError !== null}>Guardar contrato</button>
              <button type="button" onClick={addRule} disabled={busy || rules.length >= MAX_QUALITY_RULES}>Añadir regla</button>
              <button type="button" className="primary-action" onClick={() => void runQualityGate()}
                disabled={busy || validationError !== null}>Validar contrato</button>
              <span>{rules.length}/{MAX_QUALITY_RULES} reglas</span>
            </div>
            {validationError && <p className="notice notice--error" role="alert">{validationError}</p>}
            {migrationState.kind === "working" && <p className="notice" role="status">Importando y comprobando compatibilidad…</p>}
            {migrationState.kind === "ready" && (
              <div className="notice quality-migration-result" role="status" aria-live="polite">
                <strong>Importación revisada</strong>
                <span>
                  {migrationState.result.report.convertedItems} reglas importadas · {migrationState.result.report.omittedItems} omitidas de {migrationState.result.report.totalItems} · origen {migrationState.result.sourceFormat === "columnia" ? "Columnia" : "legado"}
                  {migrationState.result.sourceVersion ? ` v${migrationState.result.sourceVersion}` : " sin versión"}
                </span>
                {migrationState.result.report.artifactSha256 && (
                  <small>SHA-256 del artefacto: {migrationState.result.report.artifactSha256}</small>
                )}
                {migrationState.result.report.manualActions.length > 0 && (
                  <div>
                    <strong>Acciones manuales</strong>
                    <ul>
                      {migrationState.result.report.manualActions.map((action) => <li key={action}>{action}</li>)}
                    </ul>
                  </div>
                )}
                {migrationState.result.warnings.length > 0 && (
                  <ul>
                    {migrationState.result.warnings.map((warning, index) => (
                      <li key={`${warning.ruleIndex}-${index}`}>
                        <strong>{warning.severity === "omitted" ? "Omitida" : "Advertencia"} · regla {warning.ruleIndex} · {warning.sourceKind}</strong>: {warning.message}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            {migrationState.kind === "error" && <p className="notice notice--error" role="alert">No se pudo importar el contrato: {migrationState.message}</p>}
            {qualityFileState.kind === "working" && <p className="notice" role="status">Guardando contrato versionado…</p>}
            {qualityFileState.kind === "ready" && (
              <p className="notice" role="status" aria-live="polite">
                Contrato guardado · formato Columnia v{qualityFileState.document.version} · {qualityFileState.document.rules.length} reglas.
              </p>
            )}
            {qualityFileState.kind === "error" && <p className="notice notice--error" role="alert">No se pudo guardar el contrato: {qualityFileState.message}</p>}
          </>
        ) : (
          <div className="quality-contract__unvalidated">
            <strong>Entrega no validada</strong>
            <p>No hay reglas activas. Confirma abajo para exportar sin validación durante esta sesión.</p>
            <label>
              <input
                type="checkbox"
                checked={contract.confirmation === "confirmed"}
                disabled={busy}
                onChange={(event) => onContractAction({
                  kind: "confirmation_changed",
                  confirmation: event.target.checked ? "confirmed" : "required",
                })}
              />
              Confirmo que quiero exportar sin validar la calidad
            </label>
          </div>
        )}

        {contract.gate.kind === "loading" && <p className="notice" role="status">Validando contrato localmente…</p>}
        {contract.gate.kind === "error" && <p className="notice notice--error" role="alert">No se pudo validar: {contract.gate.message}</p>}
        {(contract.gate.kind === "ready" || contract.gate.kind === "stale") && (
          <div className={`quality-gate quality-gate--${contract.gate.result.passed ? "passed" : "failed"}`} role="status">
            <strong>{contract.gate.kind === "stale"
              ? "Resultado desactualizado"
              : contract.gate.result.passed ? "Contrato aprobado" : "Contrato fallido"}</strong>
            <span>{contract.gate.result.failedRules} de {contract.gate.result.totalRules} reglas fallaron · {contract.gate.result.rowCount.toLocaleString()} filas comprobadas</span>
            <ul>
              {contract.gate.result.rules.map((result, index) => (
                <li key={index}>{result.column === QUALITY_DATASET_COLUMN ? "Dataset" : result.column}: {result.invalidCount.toLocaleString()} inválidos ({result.invalidPct.toFixed(2)}%) · {result.passed ? "aprobada" : "fallida"}</li>
              ))}
            </ul>
          </div>
        )}
      </section>
      <section className="export-panel" aria-labelledby="export-title">
        <div>
          <p className="step">Formato de entrega</p>
          <h3 id="export-title">Exportar dataset activo</h3>
          <p>Elige el formato y la protección antes de crear la copia.</p>
        </div>
        <div className="export-controls">
          <label className="export-format">
            Formato
            <select
              aria-label="Formato de exportación"
              value={exportFormat}
              onChange={(event) => setExportFormat(event.target.value as ExportFormat)}
              disabled={busy}
            >
              <option value="csv">CSV</option>
              <option value="json">JSON</option>
              <option value="parquet">Parquet</option>
              <option value="sql">SQL</option>
              <option value="excel">Excel</option>
              <option value="sqlite">SQLite</option>
              <option value="bundle">Paquete ZIP (dataset + diccionario + receta + calidad)</option>
            </select>
          </label>
          <label className="privacy-mode">
            Protección de datos personales
            <select
              aria-label="Protección de datos personales"
              value={privacyMode}
              onChange={(event) => setPrivacyMode(event.target.value as PrivacyMode)}
              disabled={busy}
            >
              <option value="none">Sin protección adicional</option>
              <option value="mask">Enmascarar columnas detectadas</option>
              <option value="hash">Aplicar hash SHA-256 a columnas detectadas</option>
            </select>
          </label>
          <button
            className="primary-action export-action"
            type="button"
            onClick={() => requestExport(exportFormat)}
            disabled={busy || !exportAllowed}
          >
            Exportar {exportFormatLabel}
          </button>
        </div>
        {exportFormat === "bundle" && (
          <p className="export-requirement" role="note">
            {recipeDraft
              ? "Este paquete incluirá recipe.json con la receta actual validada y su referencia en manifest.json."
              : "No hay una receta activa para incluir; el paquete contendrá dataset.csv, dictionary.json y manifest.json."}
          </p>
        )}
        {!exportAllowed && <p className="export-requirement">{exportRequirement}</p>}
      </section>
      {exportState.kind === "loading" && (
        <OperationProgressView
          progress={exportState.progress}
          cancellation={exportState.cancellation === "requested"
            ? { kind: "requested" }
            : { kind: "available", onCancel: onCancelExport }}
        />
      )}
      {exportState.kind === "success" && (
        <>
          <p className="notice notice--success" role="status">
            {exportState.result.format} exportado como {exportState.result.fileName} ({formatFileSize(exportState.result.fileSizeBytes)}).
            {exportState.result.protectedColumnCount > 0 && (
              <> Privacidad aplicada a {exportState.result.protectedColumnCount} columnas: {exportState.result.protectedColumns?.join(", ")}.</>
            )}
            {exportState.result.format === "Paquete Columnia" && (
              <> Incluye dataset.csv, dictionary.json, manifest.json{recipeDraft ? " y recipe.json validada" : ""}, además del reporte de calidad cuando hay reglas aprobadas.</>
            )}
          </p>
          <div className="notice__actions">
            <button
              type="button"
              className="secondary-action"
              onClick={() => void revealLastExport()}
              disabled={openOutputState === "working"}
            >
              {openOutputState === "working" ? "Abriendo carpeta…" : "Abrir carpeta de exportación"}
            </button>
          </div>
          {openOutputState === "opened" && (
            <p className="notice notice--success" role="status">Carpeta de exportación abierta.</p>
          )}
          {openOutputState === "error" && (
            <p className="notice notice--error" role="alert">No se pudo abrir la carpeta de exportación.</p>
          )}
        </>
      )}
      {exportState.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo exportar: {exportState.message}
        </p>
      )}
    </>
  );
}
