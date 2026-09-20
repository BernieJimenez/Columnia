import type {
  ImportProfileColumn,
  ReusableTaskExceptionPolicy,
  SavedRecipe,
} from "../../bridge";

/**
 * Reusable tasks carry only conversion choices that refer to input columns.
 * They remain a draft: opening a task never executes these choices.
 */
export function createReusableTaskExceptionPolicy(
  schema: ImportProfileColumn[],
  savedRecipe: SavedRecipe | null,
): ReusableTaskExceptionPolicy | undefined {
  if (!savedRecipe) return undefined;
  const inputColumns = new Set(schema.map(({ name }) => name));
  const conversionColumns = [
    ...savedRecipe.recipe.casts.map(({ column }) => column),
    ...savedRecipe.recipe.dateParses.map(({ column }) => column),
  ].filter((column) => inputColumns.has(column));
  // Ambiguous recipe rows must not make saving the whole reusable task fail.
  // Leave the existing recipe editable, but omit its derived exception policy.
  if (new Set(conversionColumns).size !== conversionColumns.length) return undefined;
  const conversions: ReusableTaskExceptionPolicy["conversions"] = [
    ...savedRecipe.recipe.casts
      .filter(({ column }) => inputColumns.has(column))
      .map(({ column, target }) => ({ kind: "cast" as const, column, target, onInvalid: "review" as const })),
    ...savedRecipe.recipe.dateParses
      .filter(({ column }) => inputColumns.has(column))
      .map(({ column, format, target }) => ({ kind: "date" as const, column, format, target, onInvalid: "review" as const })),
  ];
  if (conversions.length === 0) return undefined;
  return {
    version: 1,
    baseline: "lexical",
    schema: schema.map((column) => ({ ...column })),
    conversions,
  };
}

/** Keeps decisions only when an edited recipe still has the same conversion. */
export function refreshReusableTaskExceptionPolicy(
  policy: ReusableTaskExceptionPolicy,
  savedRecipe: SavedRecipe | null,
): ReusableTaskExceptionPolicy | undefined {
  const next = createReusableTaskExceptionPolicy(policy.schema, savedRecipe);
  if (!next) return undefined;
  return {
    ...next,
    conversions: next.conversions.map((conversion) => {
      const previous = policy.conversions.find((candidate) => {
        if (candidate.kind !== conversion.kind || candidate.column !== conversion.column) return false;
        if (candidate.kind === "cast" && conversion.kind === "cast") {
          return candidate.target === conversion.target;
        }
        return candidate.kind === "date" && conversion.kind === "date" &&
          candidate.target === conversion.target && candidate.format === conversion.format;
      });
      return previous ? { ...conversion, onInvalid: previous.onInvalid } : conversion;
    }),
  };
}

/** The saved decisions cannot be reused after any schema change, including order. */
export function exceptionPolicyMatchesSchema(
  policy: ReusableTaskExceptionPolicy | undefined,
  schema: ImportProfileColumn[],
): boolean {
  return policy === undefined || (
    policy.version === 1 &&
    policy.baseline === "lexical" &&
    policy.schema.length === schema.length &&
    policy.schema.every((column, index) =>
      column.name === schema[index]?.name && column.dataType === schema[index]?.dataType)
  );
}
