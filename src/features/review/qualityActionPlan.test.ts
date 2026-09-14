import { describe, expect, it } from "vitest";

import { buildQualityActionPlan, qualityActionTargetDomId } from "./qualityActionPlan";

describe("qualityActionPlan", () => {
  it("connects each measured signal with an explanation, impact, and focused correction", () => {
    const plan = buildQualityActionPlan({
      nullCount: 7,
      nullColumnCount: 2,
      duplicateCount: 3,
      invalidTypeCount: 4,
    });

    expect(plan.map(({ target, actionLabel }) => [target, actionLabel])).toEqual([
      ["missingValues", "Revisar opciones para nulos"],
      ["duplicates", "Revisar duplicados exactos"],
      ["incompatibleTypes", "Revisar tipos incompatibles"],
    ]);
    expect(plan[0].explanation).toContain("7 celdas sin valor en 2 columnas");
    expect(plan[0].impact).toContain("conserva los nulos");
    expect(plan[1].impact).toContain("doble conteo");
    expect(plan[2].impact).toContain("códigos o excepciones válidas");
    expect(plan.map(({ target }) => qualityActionTargetDomId(target))).toEqual([
      "prepare-quality-missingValues",
      "prepare-quality-duplicates",
      "prepare-quality-incompatibleTypes",
    ]);
  });

  it("no inventa una prioridad cuando el perfil no detecta estas señales", () => {
    expect(buildQualityActionPlan({
      nullCount: 0,
      nullColumnCount: 0,
      duplicateCount: 0,
      invalidTypeCount: 0,
    })).toEqual([]);
  });
});
