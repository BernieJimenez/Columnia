import type { DatasetColumn, DatasetPreview, TransformRecipe } from "../../bridge";

type RequiredColumnKind = "text" | "numeric" | "date";

export interface RecipeSchemaIssue {
  column: string;
  kind: "missing" | "incompatible";
  reasons: string[];
  compatibleColumns: string[];
}

interface RecipeColumnUse {
  operations: Set<string>;
  acceptedKinds: RequiredColumnKind[][];
}

function usesFor(recipe: TransformRecipe): Map<string, RecipeColumnUse> {
  const uses = new Map<string, RecipeColumnUse>();
  const add = (column: string | null | undefined, operation: string, acceptedKinds?: RequiredColumnKind[]) => {
    if (!column) return;
    const use = uses.get(column) ?? { operations: new Set<string>(), acceptedKinds: [] };
    use.operations.add(operation);
    if (acceptedKinds) use.acceptedKinds.push(acceptedKinds);
    uses.set(column, use);
  };

  recipe.renames.forEach((item) => add(item.from, "renombrar"));
  recipe.casts.forEach((item) => add(item.column, "convertir tipo"));
  recipe.dateParses.forEach((item) => add(item.column, "interpretar fecha", ["date"]));
  recipe.filters.forEach((item) => add(
    item.column,
    `filtro ${item.operator}`,
    ["gt", "lt", "gte", "lte"].includes(item.operator)
      ? ["numeric", "date"]
      : undefined,
  ));
  if (recipe.calculatedColumn) {
    const { operation, source, operand } = recipe.calculatedColumn;
    const acceptedKinds = ["add", "subtract", "multiply", "divide"].includes(operation)
      ? ["numeric"] as RequiredColumnKind[]
      : ["year", "month", "day"].includes(operation) ? ["date"] as RequiredColumnKind[] : undefined;
    add(source, `cálculo ${operation}`, acceptedKinds);
    if (operand?.kind === "column") add(operand.value, `operando de cálculo ${operation}`, acceptedKinds);
  }
  if (recipe.findReplace?.scope === "column") add(recipe.findReplace.column, "buscar y reemplazar", ["text"]);
  recipe.keepColumns?.forEach((column) => add(column, "conservar columna"));
  if (recipe.splitColumn) add(recipe.splitColumn.source, "dividir texto", ["text"]);
  recipe.mergeColumns?.sources.forEach((column) => add(column, "combinar columnas"));
  recipe.outlierTreatments.forEach((item) => add(item.column, `outliers ${item.action}`, ["numeric"]));
  if (recipe.groupSummary) {
    recipe.groupSummary.groupBy.forEach((column) => add(column, "agrupar filas"));
    recipe.groupSummary.aggregations.forEach((item) => add(
      item.column,
      `agregación ${item.operation}`,
      ["sum", "mean"].includes(item.operation) ? ["numeric"] : undefined,
    ));
  }
  recipe.contactNormalizations.forEach((item) => add(item.column, `normalizar ${item.kind}`, ["text"]));
  recipe.textExtractions.forEach((item) => add(item.source, `extraer texto ${item.kind}`, ["text"]));
  return uses;
}

export function recipeSourceSchema(recipe: TransformRecipe, dataset: DatasetPreview): DatasetColumn[] {
  const referencedColumns = usesFor(recipe);
  return dataset.columns.filter((column) => referencedColumns.has(column.name));
}

function effectiveKind(recipe: TransformRecipe, recipeColumn: string, datasetColumn: string, dataset: DatasetPreview): string {
  const cast = [...recipe.casts].reverse().find((item) => item.column === recipeColumn);
  if (cast) {
    if (cast.target === "integer") return "numeric";
    if (cast.target === "decimal") return "numeric";
    if (cast.target === "string") return "text";
    return "boolean";
  }
  const dateParse = recipe.dateParses.find((item) => item.column === recipeColumn);
  if (dateParse) return "date";
  const dataType = dataset.columns.find((item) => item.name === datasetColumn)?.dataType;
  if (dataType === "Int64" || dataType === "Float64") return "numeric";
  if (dataType === "String") return "text";
  if (dataType === "Date" || dataType === "Datetime") return "date";
  return dataType ?? "unknown";
}

function supportsRequirements(recipe: TransformRecipe, recipeColumn: string, datasetColumn: string, dataset: DatasetPreview, acceptedKinds: RequiredColumnKind[][]): boolean {
  const kind = effectiveKind(recipe, recipeColumn, datasetColumn, dataset);
  return acceptedKinds.every((accepted) => accepted.includes(kind as RequiredColumnKind));
}

function expectedKinds(acceptedKinds: RequiredColumnKind[][]): string {
  if (!acceptedKinds.length) return "tipo compatible con sus operaciones";
  const label = (kind: RequiredColumnKind) => ({ text: "texto", numeric: "numérico", date: "fecha" })[kind];
  return acceptedKinds.map((accepted) => accepted.map(label).join(" o ")).join(" y ");
}

function normalizedDataType(dataType: string): string {
  const normalized = dataType.toLowerCase().replaceAll(" ", "");
  if (["str", "string", "utf8", "largeutf8"].includes(normalized)) return "string";
  if (["i8", "i16", "i32", "i64", "int8", "int16", "int32", "int64"].includes(normalized)) return "integer";
  if (["f32", "f64", "float32", "float64"].includes(normalized)) return "float";
  if (normalized.startsWith("datetime(")) return "datetime";
  return normalized;
}

/**
 * Finds recipe references that need review against the active dataset. Legacy v1
 * recipes have no source schema, so those still receive existence and semantic checks.
 */
export function inspectRecipeSchema(
  recipe: TransformRecipe,
  dataset: DatasetPreview,
  sourceSchema?: DatasetColumn[],
): RecipeSchemaIssue[] {
  const uses = usesFor(recipe);
  const issueColumns = new Set<string>();
  const preliminary = [...uses.entries()].flatMap(([column, use]) => {
    const currentColumn = dataset.columns.find((item) => item.name === column);
    const exists = currentColumn !== undefined;
    const originalColumn = sourceSchema?.find((item) => item.name === column);
    const typeChanged = originalColumn !== undefined && currentColumn !== undefined &&
      normalizedDataType(originalColumn.dataType) !== normalizedDataType(currentColumn.dataType);
    if (!exists || typeChanged || !supportsRequirements(recipe, column, column, dataset, use.acceptedKinds)) {
      issueColumns.add(column);
      return [{ column, use, exists, originalType: originalColumn?.dataType, currentType: currentColumn?.dataType }];
    }
    return [];
  });
  const reservedColumns = new Set([...uses.keys()].filter((column) => !issueColumns.has(column)));

  return preliminary.map(({ column, use, exists, originalType, currentType }) => {
    const operationNames = [...use.operations].join(", ");
    const reasons = !exists
      ? [`No existe en el esquema actual; se usa en ${operationNames}.`]
      : [
          ...(originalType && currentType && normalizedDataType(originalType) !== normalizedDataType(currentType)
            ? [`El tipo cambió desde ${originalType} hasta ${currentType} desde que se guardó la receta.`]
            : []),
          ...(!supportsRequirements(recipe, column, column, dataset, use.acceptedKinds)
            ? [`${operationNames} requiere ${expectedKinds(use.acceptedKinds)}; el tipo actual es ${currentType ?? "desconocido"}.`]
            : []),
        ];
    const compatibleColumns = dataset.columns
      .filter((candidate) => !reservedColumns.has(candidate.name))
      .filter((candidate) => supportsRequirements(recipe, column, candidate.name, dataset, use.acceptedKinds))
      .map((candidate) => candidate.name);
    return { column, kind: exists ? "incompatible" as const : "missing" as const, reasons, compatibleColumns };
  });
}

export function canMapRecipeSchema(issues: RecipeSchemaIssue[], mappings: Record<string, string>): boolean {
  if (issues.length === 0 || issues.some((issue) => !issue.compatibleColumns.includes(mappings[issue.column] ?? ""))) return false;
  const targets = issues.map((issue) => mappings[issue.column]);
  return new Set(targets).size === targets.length;
}

/** Rewrites every source-column reference; output column names remain unchanged. */
export function mapRecipeColumns(recipe: TransformRecipe, issues: RecipeSchemaIssue[], mappings: Record<string, string>): TransformRecipe | null {
  if (!canMapRecipeSchema(issues, mappings)) return null;
  const remap = (column: string) => mappings[column] ?? column;
  return {
    ...recipe,
    renames: recipe.renames.map((item) => ({ ...item, from: remap(item.from) })),
    casts: recipe.casts.map((item) => ({ ...item, column: remap(item.column) })),
    dateParses: recipe.dateParses.map((item) => ({ ...item, column: remap(item.column) })),
    filters: recipe.filters.map((item) => ({ ...item, column: remap(item.column) })),
    calculatedColumn: recipe.calculatedColumn ? {
      ...recipe.calculatedColumn,
      source: remap(recipe.calculatedColumn.source),
      operand: recipe.calculatedColumn.operand?.kind === "column"
        ? { ...recipe.calculatedColumn.operand, value: remap(recipe.calculatedColumn.operand.value) }
        : recipe.calculatedColumn.operand,
    } : null,
    findReplace: recipe.findReplace?.scope === "column" && recipe.findReplace.column
      ? { ...recipe.findReplace, column: remap(recipe.findReplace.column) }
      : recipe.findReplace,
    keepColumns: recipe.keepColumns?.map(remap) ?? null,
    splitColumn: recipe.splitColumn ? { ...recipe.splitColumn, source: remap(recipe.splitColumn.source) } : null,
    mergeColumns: recipe.mergeColumns ? { ...recipe.mergeColumns, sources: recipe.mergeColumns.sources.map(remap) } : null,
    outlierTreatments: recipe.outlierTreatments.map((item) => ({ ...item, column: remap(item.column) })),
    groupSummary: recipe.groupSummary ? {
      ...recipe.groupSummary,
      groupBy: recipe.groupSummary.groupBy.map(remap),
      aggregations: recipe.groupSummary.aggregations.map((item) => ({ ...item, column: remap(item.column) })),
    } : null,
    contactNormalizations: recipe.contactNormalizations.map((item) => ({ ...item, column: remap(item.column) })),
    textExtractions: recipe.textExtractions.map((item) => ({ ...item, source: remap(item.source) })),
  };
}
