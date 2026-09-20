import { describe, expect, it } from "vitest";

import type { ImportProfileColumn, SavedRecipe } from "../../bridge";
import {
  createReusableTaskExceptionPolicy,
  exceptionPolicyMatchesSchema,
} from "./reusableTaskExceptions";

const schema: ImportProfileColumn[] = [
  { name: "amount", dataType: "String" },
  { name: "started", dataType: "String" },
];

const recipe: SavedRecipe = {
  version: 1,
  name: "Cierre mensual",
  savedAt: "2026-09-01T10:00:00Z",
  recipe: {
    renames: [],
    casts: [
      { column: "amount", target: "decimal" },
      { column: "generated", target: "integer" },
    ],
    dateParses: [{ column: "started", format: "dmy", target: "date" }],
    filters: [],
    calculatedColumn: null,
    findReplace: null,
    keepColumns: null,
    splitColumn: null,
    mergeColumns: null,
    outlierTreatments: [],
    groupSummary: null,
    contactNormalizations: [],
    textExtractions: [],
  },
};

describe("ReusableTaskExceptionPolicy", () => {
  it("saves input-column casts and date formats as review-only decisions", () => {
    const policy = createReusableTaskExceptionPolicy(schema, recipe);

    expect(policy).toEqual({
      version: 1,
      baseline: "lexical",
      schema,
      conversions: [
        { kind: "cast", column: "amount", target: "decimal", onInvalid: "review" },
        { kind: "date", column: "started", format: "dmy", target: "date", onInvalid: "review" },
      ],
    });
    expect(JSON.stringify(policy)).not.toMatch(/generated|path|value|sample/i);
  });

  it("omits a policy when the recipe has no conversion for an input column", () => {
    expect(createReusableTaskExceptionPolicy(schema, null)).toBeUndefined();
  });

  it("omits an ambiguous policy instead of making the full task unsaveable", () => {
    const duplicate = {
      ...recipe,
      recipe: {
        ...recipe.recipe,
        casts: [
          { column: "amount", target: "decimal" as const },
          { column: "amount", target: "integer" as const },
        ],
      },
    };
    expect(createReusableTaskExceptionPolicy(schema, duplicate)).toBeUndefined();
  });

  it("invalidates decisions for any schema difference, including column order", () => {
    const policy = createReusableTaskExceptionPolicy(schema, recipe);
    expect(exceptionPolicyMatchesSchema(policy, schema)).toBe(true);
    expect(exceptionPolicyMatchesSchema(policy, [...schema].reverse())).toBe(false);
    expect(exceptionPolicyMatchesSchema(policy, [schema[0], { ...schema[1], dataType: "Date" }])).toBe(false);
  });
});
