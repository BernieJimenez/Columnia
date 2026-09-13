import { describe, expect, it } from "vitest";

import type { DatasetPreview, TransformRecipe } from "../../bridge";
import { canMapRecipeSchema, inspectRecipeSchema, mapRecipeColumns } from "./recipeSchema";

const dataset: DatasetPreview = {
  fileName: "actual.csv",
  fileSizeBytes: 100,
  rowCount: 1,
  columnCount: 7,
  columns: [
    { name: "name", dataType: "String" },
    { name: "label", dataType: "String" },
    { name: "detail", dataType: "String" },
    { name: "key", dataType: "String" },
    { name: "value", dataType: "Int64" },
    { name: "operand", dataType: "Float64" },
    { name: "started", dataType: "Date" },
  ],
  rows: [["Ana", "A", "x", "1", "2", "3", "2026-01-01"]],
};

const emptyRecipe: TransformRecipe = {
  renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
  findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
  outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
};

describe("recipe schema preflight", () => {
  it("reports missing and semantically incompatible references with compatible destinations", () => {
    const recipe: TransformRecipe = {
      ...emptyRecipe,
      renames: [{ from: "former_name", to: "client" }],
      outlierTreatments: [{ column: "name", action: "cap" }],
    };

    const issues = inspectRecipeSchema(recipe, dataset);
    expect(issues).toEqual([
      expect.objectContaining({ column: "former_name", kind: "missing", compatibleColumns: ["name", "label", "detail", "key", "value", "operand", "started"] }),
      expect.objectContaining({ column: "name", kind: "incompatible", compatibleColumns: ["value", "operand"] }),
    ]);
    expect(issues[1].reasons[0]).toContain("tipo actual es String");
  });

  it("accepts temporal ordering and concatenation over non-text columns", () => {
    const recipe: TransformRecipe = {
      ...emptyRecipe,
      filters: [
        { column: "started", operator: "gte", value: "2026-01-01" },
        { column: "value", operator: "contains", value: "2" },
      ],
      calculatedColumn: {
        name: "combined",
        source: "value",
        operation: "concat",
        operand: { kind: "column", value: "started" },
      },
    };

    expect(inspectRecipeSchema(recipe, dataset)).toEqual([]);
  });

  it("requires every explicit mapping and never selects a name by itself", () => {
    const issues = inspectRecipeSchema({
      ...emptyRecipe,
      renames: [{ from: "old_name", to: "customer" }],
    }, dataset);
    expect(canMapRecipeSchema(issues, {})).toBe(false);
    expect(canMapRecipeSchema(issues, { old_name: "name" })).toBe(true);
    expect(canMapRecipeSchema(issues, { old_name: "value" })).toBe(true);
  });

  it("rewrites every recipe source reference and leaves generated names and literals intact", () => {
    const recipe: TransformRecipe = {
      ...emptyRecipe,
      renames: [{ from: "old_text", to: "renamed" }],
      casts: [{ column: "old_any", target: "string" }],
      dateParses: [{ column: "old_date", format: "iso8601", target: "date" }],
      filters: [{ column: "old_any", operator: "eq", value: "literal" }],
      calculatedColumn: { name: "derived", source: "old_num", operation: "add", operand: { kind: "column", value: "old_operand" } },
      findReplace: { scope: "column", column: "old_text", find: "literal", replace: "", regex: false },
      keepColumns: ["old_keep", "old_text"],
      splitColumn: { source: "old_text2", delimiter: ",", names: ["left", "right"], dropSource: false },
      mergeColumns: { sources: ["old_keep", "old_text2"], name: "merged", separator: "|", dropSources: false },
      outlierTreatments: [{ column: "old_num", action: "impute" }],
      groupSummary: { groupBy: ["old_keep"], aggregations: [{ column: "old_num", operation: "sum" }] },
      contactNormalizations: [{ column: "old_text", kind: "email" }],
      textExtractions: [{ source: "old_text2", kind: "digits", name: "digits", delimiter: null }],
    };
    const issues = inspectRecipeSchema(recipe, dataset);
    const mapping = {
      old_text: "name",
      old_any: "key",
      old_date: "started",
      old_num: "value",
      old_operand: "operand",
      old_keep: "label",
      old_text2: "detail",
    };

    expect(canMapRecipeSchema(issues, mapping)).toBe(true);
    const mapped = mapRecipeColumns(recipe, issues, mapping);
    expect(mapped).toMatchObject({
      renames: [{ from: "name", to: "renamed" }],
      casts: [{ column: "key", target: "string" }],
      dateParses: [{ column: "started" }],
      filters: [{ column: "key", value: "literal" }],
      calculatedColumn: { name: "derived", source: "value", operand: { kind: "column", value: "operand" } },
      findReplace: { column: "name", find: "literal" },
      keepColumns: ["label", "name"],
      splitColumn: { source: "detail", names: ["left", "right"] },
      mergeColumns: { sources: ["label", "detail"], name: "merged" },
      outlierTreatments: [{ column: "value" }],
      groupSummary: { groupBy: ["label"], aggregations: [{ column: "value", operation: "sum" }] },
      contactNormalizations: [{ column: "name" }],
      textExtractions: [{ source: "detail", name: "digits" }],
    });
  });

  it("rejects duplicate and type-incompatible explicit destinations", () => {
    const recipe: TransformRecipe = {
      ...emptyRecipe,
      renames: [{ from: "old_name", to: "customer" }],
      outlierTreatments: [{ column: "old_value", action: "cap" }],
    };
    const issues = inspectRecipeSchema(recipe, dataset);
    expect(canMapRecipeSchema(issues, { old_name: "name", old_value: "name" })).toBe(false);
    expect(canMapRecipeSchema(issues, { old_name: "label", old_value: "name" })).toBe(false);
    expect(mapRecipeColumns(recipe, issues, { old_name: "name" })).toBeNull();
  });
});
