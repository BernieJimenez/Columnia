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
const MAX_QUALITY_COLUMNS_PER_RULE = 16;
const SUPPORTED_QUALITY_DTYPES = new Set([
  "string",
  "integer",
  "float",
  "boolean",
  "date",
  "datetime",
]);
const QUALITY_COMPARISONS = new Set(["eq", "ne", "lt", "lte", "gt", "gte"]);
const ORDERING_COMPARISONS = new Set(["lt", "lte", "gt", "gte"]);

function parseQualityDateBound(value: string): number | null {
  const normalized = value.trim();
  const dayFirst = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(normalized);
  if (dayFirst) {
    const parsed = Date.parse(`${dayFirst[3]}-${dayFirst[2]}-${dayFirst[1]}T00:00:00Z`);
    return Number.isFinite(parsed) ? parsed : null;
  }
  const parsed = Date.parse(normalized);
  return Number.isFinite(parsed) ? parsed : null;
}

function supportsQualityOrdering(dataType: string): boolean {
  const normalized = dataType.toLowerCase();
  return normalized === "string"
    || normalized.includes("int")
    || normalized.includes("float")
    || normalized.includes("decimal")
    || normalized.includes("number");
}

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
    const isTogetherRule = rule.kind === "unique_together";
    const isCompareRule = rule.kind === "column_compare";
    const isReferentialRule = rule.kind === "referential_integrity";
    const isDateRangeRule = rule.kind === "date_range";
    const isConditionalRule = rule.kind === "conditional";
    const isSchemaRule = rule.kind === "schema_contract";
    const column = columns.get(rule.column);
    if (!isDatasetRule && !isSchemaRule && !isTogetherRule && !column) {
      return `${label}: selecciona una columna existente.`;
    }
    if ((isDatasetRule || isSchemaRule) && rule.column !== QUALITY_DATASET_COLUMN) {
      return `${label}: ${isSchemaRule ? "el esquema" : "la comprobación de filas"} debe usar el dataset completo.`;
    }

    if (isReferentialRule) {
      const referenceColumns = rule.columns ?? [];
      if (referenceColumns.length === 0) {
        return `${label}: selecciona al menos una columna para la referencia.`;
      }
      if (referenceColumns.length > MAX_QUALITY_COLUMNS_PER_RULE) {
        return `${label}: admite como máximo ${MAX_QUALITY_COLUMNS_PER_RULE} columnas de referencia.`;
      }
      if (referenceColumns[0] !== rule.column) {
        return `${label}: la primera columna referenciada debe coincidir con la columna principal.`;
      }
      if (new Set(referenceColumns).size !== referenceColumns.length) {
        return `${label}: no repitas columnas en la clave referencial.`;
      }
      const referenceColumnDefinitions = referenceColumns.map((name) => columns.get(name));
      if (referenceColumnDefinitions.some((value) => !value)) {
        return `${label}: todas las columnas referenciadas deben existir.`;
      }
      if (referenceColumnDefinitions.some((value) => {
        const type = value?.dataType.toLowerCase() ?? "";
        return type !== "string"
          && type !== "boolean"
          && !type.includes("int")
          && !type.includes("float")
          && !type.includes("decimal")
          && !type.includes("number");
      })) {
        return `${label}: las columnas referenciadas deben ser texto, booleanas o numéricas.`;
      }
      if (!rule.referenceValues || rule.referenceValues.length === 0) {
        return `${label}: indica al menos una referencia permitida.`;
      }
      if (rule.referenceValues.length > MAX_QUALITY_VALUES) {
        return `${label}: admite como máximo ${MAX_QUALITY_VALUES} referencias permitidas.`;
      }
      if (new Set(rule.referenceValues).size !== rule.referenceValues.length) {
        return `${label}: no repitas referencias permitidas.`;
      }
      if (referenceColumns.length === 1 && rule.referenceValues.some((value) => value.length === 0)) {
        return `${label}: las referencias de una columna no pueden estar vacías.`;
      }
      if (referenceColumns.length > 1) {
        for (const reference of rule.referenceValues) {
          let parsed: unknown;
          try {
            parsed = JSON.parse(reference);
          } catch {
            return `${label}: cada referencia compuesta debe ser un arreglo JSON.`;
          }
          if (!Array.isArray(parsed) || parsed.length !== referenceColumns.length
            || parsed.some((value) => value === null || typeof value === "object")) {
            return `${label}: cada referencia compuesta debe contener ${referenceColumns.length} valores escalares.`;
          }
        }
      }
    } else if (isCompareRule) {
      const compareColumns = rule.columns ?? [];
      if (compareColumns.length !== 2) {
        return `${label}: selecciona exactamente dos columnas para comparar.`;
      }
      if (compareColumns[0] !== rule.column) {
        return `${label}: la primera columna comparada debe coincidir con la columna principal.`;
      }
      if (compareColumns[0] === compareColumns[1]) {
        return `${label}: selecciona dos columnas distintas para comparar.`;
      }
      const left = columns.get(compareColumns[0]);
      const right = columns.get(compareColumns[1]);
      if (!left || !right) return `${label}: ambas columnas comparadas deben existir.`;
      if (left.dataType !== right.dataType) {
        return `${label}: las columnas comparadas deben compartir tipo físico.`;
      }
      if (!rule.operator || !QUALITY_COMPARISONS.has(rule.operator)) {
        return `${label}: selecciona un operador de comparación válido.`;
      }
      if (ORDERING_COMPARISONS.has(rule.operator) && !supportsQualityOrdering(left.dataType)) {
        return `${label}: ese operador solo aplica a texto o columnas numéricas.`;
      }
    } else if (rule.operator !== undefined) {
      return `${label}: el operador solo aplica a column_compare.`;
    }

    if (!isConditionalRule && (rule.when !== undefined || rule.then !== undefined)) {
      return `${label}: when y then solo aplican a conditional.`;
    }
    if (!isSchemaRule && (rule.allowAdditional !== undefined || rule.requiredOrder !== undefined)) {
      return `${label}: allowAdditional y requiredOrder solo aplican a schema_contract.`;
    }
    if (isConditionalRule) {
      const condition = rule.when;
      const then = rule.then;
      if (!condition) return `${label}: indica la condición when.`;
      if (!columns.has(condition.column)) {
        return `${label}: la columna de when debe existir.`;
      }
      const normalizedConditionType = (columns.get(condition.column)?.dataType ?? "").toLowerCase();
      if (!new Set(["string", "int64", "float64", "boolean"]).has(normalizedConditionType)) {
        return `${label}: when solo admite columnas String, Int64, Float64 o Boolean.`;
      }
      if (!condition.operator || !QUALITY_COMPARISONS.has(condition.operator)) {
        return `${label}: selecciona un operador válido para when.`;
      }
      const conditionType = columns.get(condition.column)?.dataType ?? "";
      if (!supportsQualityOrdering(conditionType) && ORDERING_COMPARISONS.has(condition.operator)) {
        return `${label}: ese operador de when solo aplica a texto o columnas numéricas.`;
      }
      if (condition.value === undefined) {
        return `${label}: indica el valor de when.`;
      }
      if (!then) return `${label}: indica la comprobación then.`;
      const conditionalKinds = new Set([
        "not_null",
        "non_empty",
        "numeric_range",
        "allowed_values",
        "regex",
        "dtype",
      ]);
      if (!conditionalKinds.has(then.kind)) {
        return `${label}: then solo admite sin nulos, texto no vacío, rango numérico, valores permitidos, regex o tipo esperado.`;
      }
      if (then.maxInvalid !== 0 || then.maxInvalidPct !== undefined) {
        return `${label}: la tolerancia de then debe ser exactamente 0 inválidos.`;
      }
      const thenError = validateQualityRuleDraft([then], dataset);
      if (thenError) return `${label}: la comprobación then no es válida (${thenError}).`;
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

    if (isDateRangeRule) {
      if (rule.minDate === undefined && rule.maxDate === undefined) {
        return `${label}: indica una fecha mínima, máxima o ambas.`;
      }
      const minimum = rule.minDate === undefined ? null : parseQualityDateBound(rule.minDate);
      const maximum = rule.maxDate === undefined ? null : parseQualityDateBound(rule.maxDate);
      if (rule.minDate !== undefined && minimum === null) {
        return `${label}: la fecha mínima no es válida.`;
      }
      if (rule.maxDate !== undefined && maximum === null) {
        return `${label}: la fecha máxima no es válida.`;
      }
      if (minimum !== null && maximum !== null && minimum > maximum) {
        return `${label}: la fecha mínima no puede superar la máxima.`;
      }
      const normalizedType = column?.dataType.toLowerCase();
      if (normalizedType !== "string" && normalizedType !== "date" && normalizedType !== "datetime") {
        return `${label}: date_range solo aplica a texto, date o datetime.`;
      }
    } else if (rule.minDate !== undefined || rule.maxDate !== undefined) {
      return `${label}: las fechas límite solo aplican a date_range.`;
    }

    if (isSchemaRule) {
      const required = rule.columns ?? [];
      if (required.length === 0) return `${label}: indica al menos una columna requerida.`;
      if (required.length > MAX_QUALITY_COLUMNS_PER_RULE) {
        return `${label}: admite como máximo ${MAX_QUALITY_COLUMNS_PER_RULE} columnas requeridas.`;
      }
      if (required.some((name) => name.trim().length === 0)) {
        return `${label}: las columnas requeridas no pueden estar vacías.`;
      }
      if (new Set(required).size !== required.length) {
        return `${label}: no repitas columnas requeridas.`;
      }
      if (rule.requiredOrder !== undefined) {
        if (rule.requiredOrder.length === 0) return `${label}: el orden requerido no puede estar vacío.`;
        if (rule.requiredOrder.length > MAX_QUALITY_COLUMNS_PER_RULE) {
          return `${label}: el orden requerido supera el máximo permitido.`;
        }
        if (rule.requiredOrder.some((name) => name.trim().length === 0)) {
          return `${label}: el orden requerido contiene una columna vacía.`;
        }
        if (new Set(rule.requiredOrder).size !== rule.requiredOrder.length) {
          return `${label}: no repitas columnas en el orden requerido.`;
        }
      }
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

    if (!isReferentialRule && rule.referenceValues !== undefined) {
      return `${label}: las referencias solo aplican a referential_integrity.`;
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

    if (isTogetherRule || isCompareRule || isReferentialRule) {
      if (!rule.columns || rule.columns.length < (isReferentialRule ? 1 : 2)) {
        return isCompareRule
          ? `${label}: selecciona exactamente dos columnas para comparar.`
          : isReferentialRule
            ? `${label}: selecciona al menos una columna para la referencia.`
            : `${label}: selecciona al menos dos columnas.`;
      }
      if (rule.columns.some((name) => !columns.has(name))) {
        return `${label}: todas las columnas compuestas deben existir.`;
      }
    } else if (!isSchemaRule && rule.columns !== undefined) {
      return `${label}: la selección múltiple solo aplica a unique_together, column_compare o referential_integrity.`;
    }
  }

  return null;
}
