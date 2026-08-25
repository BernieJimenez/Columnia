import type {
  HistoryState,
  LoadedRecipe,
  RecipeExportOptions,
  RecipeMigrationReport,
  TransformRecipe,
} from "../../bridge";

export type ChangeStatus =
  | { kind: "idle" }
  | {
      kind: "working";
      action: "safe" | "duplicates" | "empty_rows" | "constant_columns" | "empty_columns" | "high_null_columns" | "sentinels" | "booleans" | "impute" | "audit" | "columns" | "trim" | "text" | "transform" | "undo" | "redo";
    }
  | { kind: "applied"; message: string }
  | { kind: "error"; message: string };

export type RecipeFileStatus =
  | { kind: "idle" }
  | { kind: "working"; action: "save" | "load" }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string };

export const EMPTY_HISTORY: HistoryState = {
  canUndo: false,
  canRedo: false,
  currentIndex: 0,
  entryCount: 0,
  entries: [],
  snapshotsEnabled: true,
  degradedReason: null,
  maxEntries: 0,
  diskBytes: 0,
  diskBudgetBytes: 0,
};

function isRecipeExportOptions(value: unknown): value is RecipeExportOptions {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<RecipeExportOptions>;
  return Array.isArray(candidate.formats) && candidate.formats.every((format) =>
    ["csv", "json", "parquet", "sql", "excel", "sqlite"].includes(format),
  ) && Array.isArray(candidate.selectedColumns) && candidate.selectedColumns.every((column) => typeof column === "string") &&
    ["none", "mask", "hash"].includes(candidate.privacyMode ?? "");
}

function isRecipeMigrationReport(value: unknown): value is RecipeMigrationReport {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<RecipeMigrationReport>;
  return (typeof candidate.artifactSha256 === "string" || candidate.artifactSha256 === null) &&
    ["dataprep", "legacy"].includes(candidate.sourceFormat ?? "") &&
    (typeof candidate.sourceVersion === "number" || candidate.sourceVersion === null) &&
    typeof candidate.convertedItems === "number" && typeof candidate.omittedItems === "number" &&
    typeof candidate.warningCount === "number" && Array.isArray(candidate.convertedOperations) &&
    candidate.convertedOperations.every((operation) => typeof operation === "string") &&
    Array.isArray(candidate.omittedOperations) && candidate.omittedOperations.every((operation) => typeof operation === "string") &&
    Array.isArray(candidate.warnings) && candidate.warnings.every((warning) =>
      !!warning && typeof warning === "object" && typeof warning.path === "string" &&
      ["warning", "omitted"].includes(warning.severity) && typeof warning.message === "string",
    ) && Array.isArray(candidate.manualActions) && candidate.manualActions.every((action) => typeof action === "string");
}

export function isLoadedRecipe(value: unknown): value is LoadedRecipe {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<LoadedRecipe>;
  const recipe = candidate.recipe as Partial<TransformRecipe> | undefined;
  return candidate.version === 1 && typeof candidate.name === "string" &&
    typeof candidate.savedAt === "string" && !!recipe &&
    Array.isArray(recipe.renames) && Array.isArray(recipe.casts) &&
    Array.isArray(recipe.dateParses) && Array.isArray(recipe.filters) &&
    Array.isArray(recipe.outlierTreatments) && Array.isArray(recipe.contactNormalizations) &&
    Array.isArray(recipe.textExtractions) && "calculatedColumn" in recipe &&
    "findReplace" in recipe && "keepColumns" in recipe && "splitColumn" in recipe &&
    "mergeColumns" in recipe && "groupSummary" in recipe &&
    (!candidate.exportOptions || isRecipeExportOptions(candidate.exportOptions)) &&
    (!candidate.migrationReport || isRecipeMigrationReport(candidate.migrationReport));
}

export function requiresImpactConfirmation(recipe: TransformRecipe): boolean {
  return recipe.filters.length > 0 ||
    recipe.keepColumns !== null ||
    Boolean(recipe.splitColumn?.dropSource) ||
    Boolean(recipe.mergeColumns?.dropSources) ||
    recipe.outlierTreatments.length > 0 ||
    recipe.groupSummary !== null ||
    recipe.contactNormalizations.length > 0;
}

export function changeProgressMessage(action: Extract<ChangeStatus, { kind: "working" }>["action"]): string {
  const messages: Record<typeof action, string> = {
    safe: "Aplicando correcciones recomendadas…",
    duplicates: "Eliminando duplicados…",
    empty_rows: "Eliminando filas vacías…",
    constant_columns: "Eliminando columnas constantes…",
    empty_columns: "Eliminando columnas vacías…",
    high_null_columns: "Eliminando columnas con alta nulidad…",
    sentinels: "Normalizando valores centinela…",
    booleans: "Normalizando booleanos…",
    impute: "Imputando nulos de forma conservadora…",
    audit: "Activando trazabilidad por fila…",
    columns: "Normalizando nombres de columnas…",
    trim: "Recortando espacios exteriores…",
    text: "Normalizando texto seleccionado…",
    transform: "Aplicando receta estructural…",
    undo: "Deshaciendo cambio…",
    redo: "Rehaciendo cambio…",
  };
  return messages[action];
}
