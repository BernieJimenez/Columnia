import { describe, expect, it } from "vitest";

import { QUALITY_DATASET_COLUMN, type DatasetPreview, type QualityRule, type QualityValidationResult } from "../../bridge";
import {
  INITIAL_DELIVERY_CONTRACT,
  MAX_QUALITY_RULES,
  deliveryContractFromRules,
  deliveryRules,
  invalidateDeliveryContract,
  reduceDeliveryContract,
  validateQualityRuleDraft,
  type DeliveryContractState,
} from "./deliveryModel";

const dataset: DatasetPreview = {
  fileName: "ventas.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "total", dataType: "Int64" },
    { name: "limite", dataType: "Int64" },
    { name: "estado", dataType: "String" },
    { name: "fecha", dataType: "String" },
  ],
  rows: [["10", "12", "ok", "2024-01-01"], ["20", "20", "ok", "2024-06-01"]],
};

const validRule: QualityRule = {
  column: "total",
  kind: "not_null",
  maxInvalid: 0,
};

const passedResult: QualityValidationResult = {
  passed: true,
  rowCount: 2,
  totalRules: 1,
  failedRules: 0,
  rules: [{
    ...validRule,
    checkedCount: 2,
    invalidCount: 0,
    invalidPct: 0,
    passed: true,
  }],
};

describe("validateQualityRuleDraft", () => {
  it("acepta un contrato válido", () => {
    expect(validateQualityRuleDraft([validRule], dataset)).toBeNull();
    expect(validateQualityRuleDraft([{ ...validRule, maxInvalidPct: 0 }], dataset)).toBeNull();
  });

  it.each([
    [{ ...validRule, column: "ausente" }, /columna existente/],
    [{ ...validRule, maxInvalid: -1 }, /entero igual o mayor/],
    [{ ...validRule, maxInvalid: undefined, maxInvalidPct: 101 }, /entre 0 y 100/],
    [{ ...validRule, kind: "numeric_range", maxInvalid: 0 }, /al menos un límite/],
    [{ ...validRule, kind: "numeric_range", min: 2.5 }, /enteros seguros/],
    [{ ...validRule, kind: "numeric_range", min: 20, max: 10 }, /mínimo no puede superar/],
    [{ ...validRule, min: 10 }, /solo se permiten para rangos/],
    [{ ...validRule, kind: "column_compare", columns: ["total", "limite"], maxInvalid: 0 }, /operador de comparación/],
    [{ ...validRule, kind: "column_compare", columns: ["total", "estado"], operator: "lte", maxInvalid: 0 }, /compartir tipo físico/],
    [{ ...validRule, column: "fecha", kind: "date_range", maxInvalid: 0 }, /fecha mínima/],
    [{ ...validRule, column: "fecha", kind: "date_range", minDate: "2024-12-31", maxDate: "2024-01-01", maxInvalid: 0 }, /no puede superar/],
  ] satisfies Array<[QualityRule, RegExp]>)("rechaza borradores inválidos", (rule, message) => {
    expect(validateQualityRuleDraft([rule], dataset)).toMatch(message);
  });

  it("centraliza el límite máximo de reglas", () => {
    const rules = Array.from({ length: MAX_QUALITY_RULES + 1 }, () => ({ ...validRule }));
    expect(validateQualityRuleDraft(rules, dataset)).toBe(
      `El contrato admite como máximo ${MAX_QUALITY_RULES} reglas.`,
    );
  });

  it("acepta la primera extensión avanzada del contrato v3", () => {
    const advancedRules: QualityRule[] = [
      { column: "estado", kind: "allowed_values", values: ["ok", "pending"], maxInvalid: 0 },
      { column: "estado", kind: "regex", pattern: "^(ok|pending)$", maxInvalidPct: 0 },
      { column: "total", kind: "dtype", dtype: "integer", maxInvalid: 0 },
      { column: "total", kind: "unique_together", columns: ["total", "estado"], maxInvalid: 0 },
      { column: "total", kind: "column_compare", columns: ["total", "limite"], operator: "lte", maxInvalid: 0 },
      { column: "fecha", kind: "date_range", minDate: "2024-01-01", maxDate: "2024-12-31", maxInvalid: 0 },
      {
        column: "total",
        kind: "conditional",
        when: { column: "estado", operator: "eq", value: "ok" },
        then: { column: "total", kind: "numeric_range", min: 0, maxInvalid: 0 },
        maxInvalid: 0,
      },
      {
        column: QUALITY_DATASET_COLUMN,
        kind: "schema_contract",
        columns: ["total", "limite"],
        allowAdditional: true,
        requiredOrder: ["total", "limite"],
        maxInvalid: 0,
      },
      { column: QUALITY_DATASET_COLUMN, kind: "row_count", min: 1, max: 10, maxInvalid: 0 },
    ];

    expect(validateQualityRuleDraft(advancedRules, dataset)).toBeNull();
  });

  it.each([
    [{ column: "estado", kind: "allowed_values", maxInvalid: 0 }, /al menos un valor/],
    [{ column: "estado", kind: "regex", pattern: "[", maxInvalid: 0 }, /patrón regular no es válido/],
    [{ column: "total", kind: "unique_together", columns: ["total"], maxInvalid: 0 }, /al menos dos columnas/],
    [{ column: "total", kind: "conditional", maxInvalid: 0 }, /condición when/],
    [{
      column: "total",
      kind: "conditional",
      when: { column: "estado", operator: "eq", value: "ok" },
      then: { column: "total", kind: "unique", maxInvalid: 0 },
      maxInvalid: 0,
    }, /then solo admite/],
    [{ column: QUALITY_DATASET_COLUMN, kind: "schema_contract", maxInvalid: 0 }, /columna requerida/],
    [{
      column: QUALITY_DATASET_COLUMN,
      kind: "schema_contract",
      columns: ["total", "total"],
      maxInvalid: 0,
    }, /no repitas/],
    [{ column: QUALITY_DATASET_COLUMN, kind: "row_count", maxInvalid: 0 }, /al menos un límite/],
  ] satisfies Array<[QualityRule, RegExp]>)("rechaza reglas avanzadas incompletas", (rule, message) => {
    expect(validateQualityRuleDraft([rule], dataset)).toMatch(message);
  });
});

describe("estado de entrega", () => {
  it("invalida un gate aprobado cuando cambian las reglas", () => {
    const approved: DeliveryContractState = {
      kind: "with_contract",
      rules: [validRule],
      gate: { kind: "ready", result: passedResult },
    };
    const next = reduceDeliveryContract(approved, {
      kind: "rules_changed",
      rules: [{ ...validRule, maxInvalid: 1 }],
    });

    expect(next).toMatchObject({ kind: "with_contract", gate: { kind: "stale" } });
  });

  it("revoca la confirmación sin contrato cuando cambia el dataset", () => {
    const confirmed = reduceDeliveryContract(INITIAL_DELIVERY_CONTRACT, {
      kind: "confirmation_changed",
      confirmation: "confirmed",
    });
    expect(invalidateDeliveryContract(confirmed)).toEqual(INITIAL_DELIVERY_CONTRACT);
  });

  it("restaura reglas persistidas con el gate sin validar", () => {
    const restored = deliveryContractFromRules([validRule]);
    expect(restored).toEqual({ kind: "with_contract", rules: [validRule], gate: { kind: "idle" } });
    expect(deliveryRules(restored)).toEqual([validRule]);
    expect(deliveryContractFromRules([])).toEqual(INITIAL_DELIVERY_CONTRACT);
  });
});
