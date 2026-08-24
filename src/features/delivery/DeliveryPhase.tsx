import { validateQualityRules, type DatasetPreview, type ExportFormat, type QualityRule, type QualityRuleKind } from "../../bridge";
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
                const percentageTolerance = rule.maxInvalidPct !== undefined;
                return (
                  <fieldset className="quality-rule" key={index} disabled={busy}>
                    <legend>Regla {index + 1}</legend>
                    <label>Columna
                      <select aria-label={`Columna regla ${index + 1}`} value={rule.column}
                        onChange={(event) => updateRule(index, { column: event.target.value })}>
                        {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                      </select>
                    </label>
                    <label>Comprobación
                      <select aria-label={`Comprobación regla ${index + 1}`} value={rule.kind}
                        onChange={(event) => {
                          const kind = event.target.value as QualityRuleKind;
                          updateRule(index, { kind, min: undefined, max: undefined });
                        }}>
                        <option value="not_null">Sin nulos</option>
                        <option value="non_empty">Texto no vacío</option>
                        <option value="unique">Valores únicos</option>
                        <option value="numeric_range">Rango numérico</option>
                      </select>
                    </label>
                    <label>Tolerancia
                      <select aria-label={`Tolerancia regla ${index + 1}`} value={percentageTolerance ? "percentage" : "count"}
                        onChange={(event) => updateRule(index, event.target.value === "percentage"
                          ? { maxInvalid: undefined, maxInvalidPct: 0 }
                          : { maxInvalid: 0, maxInvalidPct: undefined })}>
                        <option value="count">Máximo inválidos</option>
                        <option value="percentage">Máximo porcentaje</option>
                      </select>
                    </label>
                    <label>{percentageTolerance ? "Porcentaje máximo" : "Inválidos máximos"}
                      <input type="number" min="0" max={percentageTolerance ? "100" : undefined}
                        step={percentageTolerance ? "0.1" : "1"}
                        aria-label={`${percentageTolerance ? "Porcentaje" : "Inválidos"} regla ${index + 1}`}
                        value={percentageTolerance ? rule.maxInvalidPct ?? 0 : rule.maxInvalid ?? 0}
                        onChange={(event) => updateRule(index, percentageTolerance
                          ? { maxInvalidPct: Number(event.target.value) }
                          : { maxInvalid: Number(event.target.value) })} />
                    </label>
                    {rule.kind === "numeric_range" && (
                      <>
                        <label>Mínimo inclusivo
                          <input type="number" aria-label={`Mínimo regla ${index + 1}`}
                            value={rule.min ?? ""}
                            onChange={(event) => updateRule(index, { min: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                        <label>Máximo inclusivo
                          <input type="number" aria-label={`Máximo regla ${index + 1}`}
                            value={rule.max ?? ""}
                            onChange={(event) => updateRule(index, { max: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                      </>
                    )}
                    <button type="button" className="quality-rule__remove" aria-label={`Eliminar regla ${index + 1}`}
                      onClick={() => changeRules(rules.filter((_, ruleIndex) => ruleIndex !== index))}>Eliminar</button>
                  </fieldset>
                );
              })}
            </div>
            <div className="quality-contract__actions">
              <button type="button" onClick={addRule} disabled={busy || rules.length >= MAX_QUALITY_RULES}>Añadir regla</button>
              <button type="button" className="primary-action" onClick={() => void runQualityGate()}
                disabled={busy || validationError !== null}>Validar contrato</button>
              <span>{rules.length}/{MAX_QUALITY_RULES} reglas</span>
            </div>
            {validationError && <p className="notice notice--error" role="alert">{validationError}</p>}
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
                <li key={index}>{result.column}: {result.invalidCount.toLocaleString()} inválidos ({result.invalidPct.toFixed(2)}%) · {result.passed ? "aprobada" : "fallida"}</li>
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
