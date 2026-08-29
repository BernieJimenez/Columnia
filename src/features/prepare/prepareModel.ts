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
       action: "safe" | "duplicates" | "near_duplicates" | "empty_rows" | "constant_columns" | "empty_columns" | "high_null_columns" | "identifier_columns" | "personal_columns" | "personal_mask" | "sentinels" | "booleans" | "encoding" | "invalid_types" | "impute" | "categorical_impute" | "parse_dates" | "outlier_impute" | "outlier_cap" | "outlier_drop" | "audit" | "columns" | "trim" | "text" | "transform" | "undo" | "redo";
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
    ["csv", "json", "parquet", "sql", "excel", "sqlite", "bundle"].includes(format),
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
    ) && Array.isArray(candidate.manualActions) && candidate.manualActions.every((action) => typeof action === "string") &&
    (!candidate.session || (
      typeof candidate.session === "object" &&
      typeof candidate.session.hasSourceReference === "boolean" &&
      typeof candidate.session.hasSnapshotReference === "boolean" &&
      (typeof candidate.session.sheetName === "string" || candidate.session.sheetName === null) &&
      (typeof candidate.session.stageLabel === "string" || candidate.session.stageLabel === null) &&
      typeof candidate.session.appliedOperationCount === "number" &&
      typeof candidate.session.qualityRuleCount === "number" &&
      typeof candidate.session.analysisCheckCount === "number" &&
      (candidate.session.appliedOperations === undefined || (
        Array.isArray(candidate.session.appliedOperations) &&
        candidate.session.appliedOperations.every((operation) => typeof operation === "string")
      )) &&
      (candidate.session.analysisChecks === undefined || (
        Array.isArray(candidate.session.analysisChecks) &&
        candidate.session.analysisChecks.every((check) => typeof check === "string")
      ))
    ));
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
    near_duplicates: "Eliminando duplicados parecidos…",
    empty_rows: "Eliminando filas vacías…",
    constant_columns: "Eliminando columnas constantes…",
    empty_columns: "Eliminando columnas vacías…",
    high_null_columns: "Eliminando columnas con alta nulidad…",
    identifier_columns: "Retirando columnas identificadoras…",
    personal_columns: "Retirando datos personales detectados…",
    personal_mask: "Protegiendo valores personales detectados…",
    sentinels: "Normalizando valores centinela…",
    booleans: "Normalizando booleanos…",
    encoding: "Corrigiendo doble codificación UTF-8…",
    invalid_types: "Apartando valores incompatibles…",
    impute: "Imputando nulos de forma conservadora…",
    categorical_impute: "Completando nulos textuales…",
    parse_dates: "Interpretando fechas detectadas…",
    outlier_impute: "Imputando outliers por mediana…",
    outlier_cap: "Limitando outliers con IQR…",
    outlier_drop: "Eliminando filas atípicas…",
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
