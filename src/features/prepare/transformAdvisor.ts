import type { DatasetPreview, RecipeFilter, TransformRecipe } from "../../bridge";

export type TransformRisk = "low" | "medium" | "high";

export interface TransformPreview {
  beforeRows: number;
  afterRows: number | null;
  beforeColumns: number;
  afterColumns: number;
  rowsDelta: number | null;
  columnsDelta: number;
  beforeColumnNames: string[];
  afterColumnNames: string[];
  operationLabels: string[];
  risk: TransformRisk;
  confidence: number;
  confidenceLabel: string;
  basis: string;
  recommendations: string[];
  recoveryOptions: string[];
}

const MAX_COLUMN_NAMES = 8;

function valueAt(row: Array<string | null>, dataset: DatasetPreview, column: string): string | null | undefined {
  const index = dataset.columns.findIndex((item) => item.name === column);
  return index < 0 ? undefined : row[index];
}

function matchesFilter(row: Array<string | null>, dataset: DatasetPreview, filter: RecipeFilter): boolean {
  const value = valueAt(row, dataset, filter.column);
  switch (filter.operator) {
    case "is_null":
      return value === null || value === undefined || value.trim() === "";
    case "not_null":
      return value !== null && value !== undefined && value.trim() !== "";
    case "eq":
      return value === filter.value;
    case "neq":
      return value !== filter.value;
    case "contains":
      return value?.toLocaleLowerCase().includes((filter.value ?? "").toLocaleLowerCase()) ?? false;
    case "not_contains":
      return !(value?.toLocaleLowerCase().includes((filter.value ?? "").toLocaleLowerCase()) ?? false);
    case "gt":
    case "lt":
    case "gte":
    case "lte": {
      const left = value === null || value === undefined ? Number.NaN : Number(value);
      const right = Number(filter.value);
      if (!Number.isFinite(left) || !Number.isFinite(right)) return false;
      if (filter.operator === "gt") return left > right;
      if (filter.operator === "lt") return left < right;
      if (filter.operator === "gte") return left >= right;
      return left <= right;
    }
  }
}

function operationLabels(recipe: TransformRecipe): string[] {
  const labels: string[] = [];
  if (recipe.renames.length > 0) labels.push(`${recipe.renames.length} renombre${recipe.renames.length === 1 ? "" : "s"}`);
  if (recipe.casts.length > 0) labels.push(`${recipe.casts.length} conversión${recipe.casts.length === 1 ? "" : "es"}`);
  if (recipe.dateParses.length > 0) labels.push(`${recipe.dateParses.length} fecha${recipe.dateParses.length === 1 ? "" : "s"}`);
  if (recipe.filters.length > 0) labels.push(`${recipe.filters.length} filtro${recipe.filters.length === 1 ? "" : "s"}`);
  if (recipe.calculatedColumn) labels.push("1 columna calculada");
  if (recipe.findReplace) labels.push("buscar y reemplazar");
  if (recipe.keepColumns) labels.push("selección de columnas");
  if (recipe.splitColumn) labels.push("división de columna");
  if (recipe.mergeColumns) labels.push("combinación de columnas");
  if (recipe.outlierTreatments.length > 0) labels.push(`${recipe.outlierTreatments.length} tratamiento${recipe.outlierTreatments.length === 1 ? "" : "s"} de outliers`);
  if (recipe.groupSummary) labels.push("resumen agrupado");
  if (recipe.contactNormalizations.length > 0) labels.push(`${recipe.contactNormalizations.length} normalización${recipe.contactNormalizations.length === 1 ? "" : "es"} de contacto`);
  if (recipe.textExtractions.length > 0) labels.push(`${recipe.textExtractions.length} extracción${recipe.textExtractions.length === 1 ? "" : "es"}`);
  return labels;
}

function previewColumnNames(dataset: DatasetPreview, recipe: TransformRecipe): { before: string[]; after: string[] } {
  const before = dataset.columns.map((column) => column.name);
  const renameMap = new Map<string, string>();
  recipe.renames.forEach((rename) => {
    const target = rename.to.trim();
    if (target) renameMap.set(rename.from, target);
  });
  const kept = recipe.keepColumns ?? before;
  const after = before
    .filter((name) => kept.includes(name))
    .map((name) => renameMap.get(name) ?? name);

  const append = (name: string) => {
    const trimmed = name.trim();
    if (trimmed && !after.includes(trimmed)) after.push(trimmed);
  };
  if (recipe.splitColumn?.dropSource) {
    const source = renameMap.get(recipe.splitColumn.source) ?? recipe.splitColumn.source;
    const index = after.indexOf(source);
    if (index >= 0) after.splice(index, 1);
  }
  if (recipe.splitColumn) recipe.splitColumn.names.forEach(append);
  if (recipe.mergeColumns?.dropSources) {
    for (const sourceName of recipe.mergeColumns.sources) {
      const source = renameMap.get(sourceName) ?? sourceName;
      const index = after.indexOf(source);
      if (index >= 0) after.splice(index, 1);
    }
  }
  if (recipe.mergeColumns) append(recipe.mergeColumns.name);
  if (recipe.calculatedColumn) append(recipe.calculatedColumn.name);
  recipe.textExtractions.forEach((extraction) => append(extraction.name));
  return { before, after };
}

function getRisk(recipe: TransformRecipe): TransformRisk {
  if (recipe.groupSummary || recipe.filters.length > 0 || recipe.keepColumns || recipe.outlierTreatments.some((item) => item.action === "drop")) return "high";
  if (recipe.splitColumn?.dropSource || recipe.mergeColumns?.dropSources || recipe.outlierTreatments.length > 0 || recipe.contactNormalizations.length > 0) return "medium";
  return "low";
}

function getRecommendations(recipe: TransformRecipe, risk: TransformRisk): string[] {
  const recommendations: string[] = [];
  if (risk === "high") recommendations.push("Revisa la estimación de filas y columnas antes de ejecutar cambios de alto impacto.");
  if (recipe.keepColumns || recipe.splitColumn?.dropSource || recipe.mergeColumns?.dropSources) {
    recommendations.push("Alternativa conservadora: conserva las columnas fuente y elimínalas en una segunda receta después de validar el resultado.");
  }
  if (recipe.filters.length > 0) recommendations.push("Si el filtro es crítico, aplícalo después de confirmar que el tipo de la columna coincide con la comparación.");
  if (recipe.groupSummary) recommendations.push("Guarda la receta y una copia del dataset antes de generar el resumen agrupado.");
  if (recommendations.length === 0) recommendations.push("La receta no elimina filas ni columnas; puedes validarla con bajo riesgo.");
  return recommendations.slice(0, 3);
}

function getRecoveryOptions(recipe: TransformRecipe): string[] {
  const options = [
    "Guarda la receta como respaldo para repetirla o inspeccionarla más tarde.",
    "La aplicación crea una entrada en Historial; puedes deshacer este paso inmediatamente.",
  ];
  if (recipe.filters.length > 0 || recipe.keepColumns || recipe.groupSummary || recipe.outlierTreatments.some((item) => item.action === "drop")) {
    options.push("Aplica primero una parte de la receta y verifica el perfil antes de continuar con las operaciones destructivas.");
  }
  return options;
}

export function buildTransformPreview(dataset: DatasetPreview, recipe: TransformRecipe): TransformPreview {
  const columns = previewColumnNames(dataset, recipe);
  const sampleRows = dataset.rows;
  let afterRows: number | null = dataset.rowCount;
  let confidence = 96;
  let basis = "El cambio de columnas se calcula sobre el esquema actual.";

  if (recipe.filters.length > 0) {
    const matchingRows = sampleRows.filter((row) => recipe.filters.every((filter) => matchesFilter(row, dataset, filter))).length;
    afterRows = sampleRows.length > 0 ? Math.round(dataset.rowCount * (matchingRows / sampleRows.length)) : null;
    confidence = sampleRows.length === 0 ? 20 : Math.min(78, 40 + sampleRows.length);
    basis = `Estimación de filas basada en ${sampleRows.length.toLocaleString()} filas visibles; el resultado real puede variar.`;
  }
  if (recipe.groupSummary || recipe.outlierTreatments.some((item) => item.action === "drop")) {
    afterRows = null;
    confidence = Math.min(confidence, 38);
    basis = "El número final de filas depende de valores que no se pueden inferir de forma segura desde la muestra.";
  }

  const risk = getRisk(recipe);
  const confidenceLabel = confidence >= 80 ? "alta" : confidence >= 55 ? "media" : "baja";
  return {
    beforeRows: dataset.rowCount,
    afterRows,
    beforeColumns: columns.before.length,
    afterColumns: columns.after.length,
    rowsDelta: afterRows === null ? null : afterRows - dataset.rowCount,
    columnsDelta: columns.after.length - columns.before.length,
    beforeColumnNames: columns.before,
    afterColumnNames: columns.after,
    operationLabels: operationLabels(recipe),
    risk,
    confidence,
    confidenceLabel,
    basis,
    recommendations: getRecommendations(recipe, risk),
    recoveryOptions: getRecoveryOptions(recipe),
  };
}

export function visibleColumnNames(names: string[]): string {
  if (names.length <= MAX_COLUMN_NAMES) return names.join(", ");
  return `${names.slice(0, MAX_COLUMN_NAMES).join(", ")} y ${names.length - MAX_COLUMN_NAMES} más`;
}
