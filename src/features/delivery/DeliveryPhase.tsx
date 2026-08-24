import { useState } from "react";

import {
  QUALITY_DATASET_COLUMN,
  pickQualityRulesMigration,
  validateQualityRules,
  type DatasetPreview,
  type ExportFormat,
  type QualityMigrationResult,
  type QualityRule,
  type QualityRuleKind,
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
  contract: DeliveryContractState;
  exportState: DeliveryExportState;
  onContractAction: (action: DeliveryContractAction) => void;
  onExport: (request: DeliveryExportRequest) => void;
  onCancelExport: () => void;
}

export function DeliveryPhase({
  dataset,
  contract,
  exportState,
  onContractAction,
  onExport,
  onCancelExport,
}: DeliveryPhaseProps) {
  const [migrationState, setMigrationState] = useState<
    | { kind: "idle" }
    | { kind: "ready"; result: QualityMigrationResult }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const rules = contract.kind === "with_contract" ? contract.rules : [];
  const validationError = validateQualityRuleDraft(rules, dataset);
  const gatePassed = contract.gate.kind === "ready" && contract.gate.result.passed;
  const exportAllowed = contract.kind === "with_contract"
    ? gatePassed
    : contract.confirmation === "confirmed";
  const busy = exportState.kind === "loading" || contract.gate.kind === "loading";

  function changeRules(nextRules: QualityRule[]) {
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
    const nextColumns = kind === "unique_together"
      ? rule.columns?.filter((name) => dataset.columns.some((column) => column.name === name))
        ?? dataset.columns.slice(0, 2).map((column) => column.name)
      : undefined;
    const nextColumn = kind === "row_count"
      ? QUALITY_DATASET_COLUMN
      : kind === "unique_together"
        ? nextColumns?.[0] ?? firstColumn
        : rule.column === QUALITY_DATASET_COLUMN ? firstColumn : rule.column;
    updateRule(index, {
      kind,
      column: nextColumn,
      min: undefined,
      max: undefined,
      values: kind === "allowed_values" ? rule.values ?? [] : undefined,
      pattern: kind === "regex" ? rule.pattern ?? "" : undefined,
      dtype: kind === "dtype" ? rule.dtype ?? "string" : undefined,
      columns: kind === "unique_together" ? nextColumns : undefined,
    });
  }

  function updateTogetherColumns(index: number, name: string, checked: boolean) {
    const current = rules[index].columns ?? [];
    const next = checked
      ? [...current, name]
      : current.filter((column) => column !== name);
    updateRule(index, { columns: next, column: next[0] ?? dataset.columns[0]?.name ?? "" });
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
    setMigrationState({ kind: "idle" });
    try {
      const result = await pickQualityRulesMigration();
      if (result) {
        onContractAction({ kind: "rules_changed", rules: result.convertedRules });
        setMigrationState({ kind: "ready", result });
      }
    } catch (error: unknown) {
      setMigrationState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  function requestExport(format: ExportFormat) {
    if (contract.kind === "with_contract") {
      onExport({ format, validation: { kind: "contract", rules: contract.rules } });
    } else if (contract.confirmation === "confirmed") {
      onExport({ format, validation: { kind: "explicitly_unvalidated" } });
    }
  }

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Entregar · Exportación local</p>
          <h2>{dataset.fileName}</h2>
          <p>Genera una copia del dataset preparado. El archivo original nunca se modifica.</p>
        </div>
      </header>
      <DatasetMetrics dataset={dataset} />
      <section className="quality-contract" aria-labelledby="quality-contract-title">
        <div className="quality-contract__header">
          <div>
            <p className="step">Control de entrega</p>
            <h3 id="quality-contract-title">Contrato de calidad</h3>
            <p>Define hasta {MAX_QUALITY_RULES} comprobaciones locales. Los resultados solo muestran conteos.</p>
          </div>
          <label className="quality-contract__toggle">
            <input
              type="checkbox"
              checked={contract.kind === "with_contract"}
              disabled={busy || dataset.columns.length === 0}
              onChange={(event) => event.target.checked ? addRule() : changeRules([])}
            />
            Validar antes de exportar
          </label>
        </div>

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
                return (
                  <fieldset className="quality-rule" key={index} disabled={busy}>
                    <legend>Regla {index + 1}</legend>
                    {!isDatasetRule && !isTogetherRule && <label>Columna
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
                    <button type="button" className="quality-rule__remove" aria-label={`Eliminar regla ${index + 1}`}
                      onClick={() => changeRules(rules.filter((_, ruleIndex) => ruleIndex !== index))}>Eliminar</button>
                  </fieldset>
                );
              })}
            </div>
            <div className="quality-contract__actions">
              <button type="button" onClick={() => void importQualityRules()} disabled={busy}>Importar reglas DataPrep</button>
              <button type="button" onClick={addRule} disabled={busy || rules.length >= MAX_QUALITY_RULES}>Añadir regla</button>
              <button type="button" className="primary-action" onClick={() => void runQualityGate()}
                disabled={busy || validationError !== null}>Validar contrato</button>
              <span>{rules.length}/{MAX_QUALITY_RULES} reglas</span>
            </div>
            {validationError && <p className="notice notice--error" role="alert">{validationError}</p>}
            {migrationState.kind === "ready" && (
              <div className="notice quality-migration-result" role="status" aria-live="polite">
                <strong>Importación revisada</strong>
                <span>{migrationState.result.convertedRules.length} reglas convertidas · {migrationState.result.omittedRules} omitidas{migrationState.result.sourceVersion ? ` · versión ${migrationState.result.sourceVersion}` : ""}</span>
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
          </>
        ) : (
          <div className="quality-contract__unvalidated">
            <strong>Entrega no validada</strong>
            <p>No hay reglas activas. Confirma explícitamente esta decisión para habilitar la exportación durante esta sesión.</p>
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
              Entiendo y deseo exportar sin contrato de calidad
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
          <p>El destino solo aparece cuando el archivo está completo.</p>
        </div>
        <div className="export-actions">
          <button type="button" onClick={() => requestExport("csv")} disabled={busy || !exportAllowed}>
            Exportar CSV
          </button>
          <button type="button" onClick={() => requestExport("json")} disabled={busy || !exportAllowed}>
            Exportar JSON
          </button>
          <button type="button" onClick={() => requestExport("parquet")} disabled={busy || !exportAllowed}>
            Exportar Parquet
          </button>
          <button type="button" onClick={() => requestExport("sql")} disabled={busy || !exportAllowed}>
            Exportar SQL
          </button>
        </div>
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
        <p className="notice notice--success" role="status">
          {exportState.result.format} exportado como {exportState.result.fileName} ({formatFileSize(exportState.result.fileSizeBytes)}).
        </p>
      )}
      {exportState.kind === "error" && (
        <p className="notice notice--error" role="alert">
          No se pudo exportar: {exportState.message}
        </p>
      )}
    </>
  );
}
