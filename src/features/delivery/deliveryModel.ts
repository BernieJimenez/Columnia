import type {
  DatasetPreview,
  ExportFormat,
  ExportResult,
  OperationProgress,
  QualityRule,
  PrivacyMode,
  QualityValidationResult,
} from "../../bridge";
import { QUALITY_DATASET_COLUMN } from "../../bridge";

export const MAX_QUALITY_RULES = 16;
const MAX_QUALITY_VALUES = 128;
const SUPPORTED_QUALITY_DTYPES = new Set([
  "string",
  "integer",
  "float",
  "boolean",
  "date",
  "datetime",
]);

export type QualityGateState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; result: QualityValidationResult }
  | { kind: "stale"; result: QualityValidationResult }
  | { kind: "error"; message: string };

export type DeliveryContractState =
  | {
      kind: "without_contract";
      confirmation: "required" | "confirmed";
      gate: QualityGateState;
    }
  | {
      kind: "with_contract";
      rules: QualityRule[];
      gate: QualityGateState;
    };

export type DeliveryContractAction =
  | { kind: "rules_changed"; rules: QualityRule[] }
  | { kind: "gate_changed"; gate: QualityGateState }
  | { kind: "confirmation_changed"; confirmation: "required" | "confirmed" };

export type DeliveryExportState =
  | { kind: "idle" }
  | {
      kind: "loading";
      format: ExportFormat;
      progress: OperationProgress;
      cancellation: "available" | "requested";
    }
  | { kind: "success"; result: ExportResult }
  | { kind: "error"; message: string };

export type DeliveryExportRequest =
  | {
      format: ExportFormat;
      privacyMode: PrivacyMode;
      validation: { kind: "contract"; rules: QualityRule[] };
    }
  | {
      format: ExportFormat;
      privacyMode: PrivacyMode;
      validation: { kind: "explicitly_unvalidated" };
    };

export const INITIAL_DELIVERY_CONTRACT: DeliveryContractState = {
  kind: "without_contract",
  confirmation: "required",
  gate: { kind: "idle" },
};

export function deliveryContractFromRules(rules: QualityRule[]): DeliveryContractState {
  return rules.length === 0
    ? INITIAL_DELIVERY_CONTRACT
    : { kind: "with_contract", rules, gate: { kind: "idle" } };
}

export function deliveryRules(state: DeliveryContractState): QualityRule[] {
  return state.kind === "with_contract" ? state.rules : [];
}

function invalidateGate(gate: QualityGateState): QualityGateState {
  return gate.kind === "ready" || gate.kind === "stale"
    ? { kind: "stale", result: gate.result }
    : { kind: "idle" };
}

export function reduceDeliveryContract(
  state: DeliveryContractState,
  action: DeliveryContractAction,
): DeliveryContractState {
  if (action.kind === "gate_changed") return { ...state, gate: action.gate };

  if (action.kind === "confirmation_changed") {
    return state.kind === "without_contract"
      ? { ...state, confirmation: action.confirmation }
      : state;
  }

  const gate = invalidateGate(state.gate);
  return action.rules.length === 0
    ? { kind: "without_contract", confirmation: "required", gate }
    : { kind: "with_contract", rules: action.rules, gate };
}

export function invalidateDeliveryContract(state: DeliveryContractState): DeliveryContractState {
  const gate = invalidateGate(state.gate);
  return state.kind === "without_contract"
    ? { ...state, confirmation: "required", gate }
    : { ...state, gate };
}

export function validateQualityRuleDraft(
  rules: QualityRule[],
  dataset: DatasetPreview,
): string | null {
  if (rules.length > MAX_QUALITY_RULES) {
    return `El contrato admite como máximo ${MAX_QUALITY_RULES} reglas.`;
  }

  const columns = new Map(dataset.columns.map((column) => [column.name, column]));
  for (const [index, rule] of rules.entries()) {
    const label = `Regla ${index + 1}`;
    const isDatasetRule = rule.kind === "row_count";
    const column = columns.get(rule.column);
    if (!isDatasetRule && !column) return `${label}: selecciona una columna existente.`;
    if (isDatasetRule && rule.column !== QUALITY_DATASET_COLUMN) {
      return `${label}: la comprobación de filas debe usar el dataset completo.`;
    }

    const hasCount = rule.maxInvalid !== undefined;
    const hasPercentage = rule.maxInvalidPct !== undefined;
    if (!hasCount && !hasPercentage) return `${label}: activa al menos una tolerancia.`;
    if (hasCount && (!Number.isInteger(rule.maxInvalid) || (rule.maxInvalid ?? -1) < 0)) {
      return `${label}: el máximo de inválidos debe ser un entero igual o mayor que cero.`;
    }
    if (hasPercentage && (
      !Number.isFinite(rule.maxInvalidPct) ||
      (rule.maxInvalidPct ?? -1) < 0 ||
      (rule.maxInvalidPct ?? 101) > 100
    )) {
      return `${label}: el porcentaje debe estar entre 0 y 100.`;
    }

    const usesBounds = rule.kind === "numeric_range" || isDatasetRule;
    if (usesBounds) {
      if (rule.min === undefined && rule.max === undefined) {
        return `${label}: indica al menos un límite.`;
      }
      if (rule.min !== undefined && !Number.isFinite(rule.min)) {
        return `${label}: el mínimo debe ser finito.`;
      }
      if (rule.max !== undefined && !Number.isFinite(rule.max)) {
        return `${label}: el máximo debe ser finito.`;
      }
      if (rule.kind === "numeric_range" && column?.dataType.toLowerCase().includes("int") && (
        (rule.min !== undefined && !Number.isSafeInteger(rule.min)) ||
        (rule.max !== undefined && !Number.isSafeInteger(rule.max))
      )) {
        return `${label}: los límites de una columna entera deben ser enteros seguros.`;
      }
      if (rule.min !== undefined && rule.max !== undefined && rule.min > rule.max) {
        return `${label}: el mínimo no puede superar el máximo.`;
      }
    } else if (rule.min !== undefined || rule.max !== undefined) {
      return `${label}: los límites solo se permiten para rangos numéricos o conteo de filas.`;
    }

    if (rule.kind === "allowed_values") {
      if (!rule.values || rule.values.length === 0) {
        return `${label}: indica al menos un valor permitido.`;
      }
      if (rule.values.length > MAX_QUALITY_VALUES) {
        return `${label}: admite como máximo ${MAX_QUALITY_VALUES} valores permitidos.`;
      }
      if (column?.dataType !== "String") {
        return `${label}: los valores permitidos solo aplican a columnas de texto.`;
      }
    } else if (rule.values !== undefined) {
      return `${label}: los valores permitidos solo aplican a allowed_values.`;
    }

    if (rule.kind === "regex") {
      if (!rule.pattern) return `${label}: indica un patrón regular.`;
      try {
        new RegExp(rule.pattern);
      } catch {
        return `${label}: el patrón regular no es válido.`;
      }
      if (column?.dataType !== "String") {
        return `${label}: las expresiones regulares solo aplican a columnas de texto.`;
      }
    } else if (rule.pattern !== undefined) {
      return `${label}: el patrón solo aplica a regex.`;
    }

    if (rule.kind === "dtype") {
      if (!rule.dtype || !SUPPORTED_QUALITY_DTYPES.has(rule.dtype)) {
        return `${label}: selecciona un tipo esperado válido.`;
      }
    } else if (rule.dtype !== undefined) {
      return `${label}: el tipo esperado solo aplica a dtype.`;
    }

    if (rule.kind === "unique_together") {
      if (!rule.columns || rule.columns.length < 2) {
        return `${label}: selecciona al menos dos columnas.`;
      }
      if (rule.columns.some((name) => !columns.has(name))) {
        return `${label}: todas las columnas compuestas deben existir.`;
      }
    } else if (rule.columns !== undefined) {
      return `${label}: la selección múltiple solo aplica a unique_together.`;
    }
  }

  return null;
}
