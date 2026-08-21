import { describe, expect, it } from "vitest";

import type { DatasetPreview, QualityRule, QualityValidationResult } from "../../bridge";
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
    { name: "estado", dataType: "String" },
  ],
  rows: [["10", "ok"], ["20", "ok"]],
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
  });

  it.each([
    [{ ...validRule, column: "ausente" }, /columna existente/],
    [{ ...validRule, maxInvalidPct: 0 }, /exactamente una tolerancia/],
    [{ ...validRule, maxInvalid: -1 }, /entero igual o mayor/],
    [{ ...validRule, maxInvalid: undefined, maxInvalidPct: 101 }, /entre 0 y 100/],
    [{ ...validRule, kind: "numeric_range", maxInvalid: 0 }, /al menos un límite/],
    [{ ...validRule, kind: "numeric_range", min: 2.5 }, /enteros seguros/],
    [{ ...validRule, kind: "numeric_range", min: 20, max: 10 }, /mínimo no puede superar/],
    [{ ...validRule, min: 10 }, /solo se permiten para rangos/],
  ] satisfies Array<[QualityRule, RegExp]>)("rechaza borradores inválidos", (rule, message) => {
    expect(validateQualityRuleDraft([rule], dataset)).toMatch(message);
  });

  it("centraliza el límite máximo de reglas", () => {
    const rules = Array.from({ length: MAX_QUALITY_RULES + 1 }, () => ({ ...validRule }));
    expect(validateQualityRuleDraft(rules, dataset)).toBe(
      `El contrato admite como máximo ${MAX_QUALITY_RULES} reglas.`,
    );
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
