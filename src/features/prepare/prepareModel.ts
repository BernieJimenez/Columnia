import type {
  ChangedTextColumn,
  DatasetProfile,
  HistoryState,
  LoadedRecipe,
  RecipeExportOptions,
  SafeCorrectionOptions,
  SafeCorrectionsResult,
  TransformRecipe,
} from "../../bridge";
import { isTextType } from "../../dataTypes";

export type ChangeStatus =
  | { kind: "idle" }
  | {
      kind: "working";
      action: "safe" | "duplicates" | "near_duplicates" | "empty_rows" | "constant_columns" | "empty_columns" | "high_null_columns" | "identifier_columns" | "personal_columns" | "personal_mask" | "sentinels" | "booleans" | "encoding" | "invalid_types" | "impute" | "categorical_impute" | "parse_dates" | "cast_numeric" | "outlier_impute" | "outlier_cap" | "outlier_drop" | "audit" | "columns" | "trim" | "text" | "transform" | "undo" | "redo";
      cancelRequested?: boolean;
    }
  /** `changes` lists what a proposal applied, one line each, for its result screen. */
  | { kind: "applied"; message: string; changes?: string[] }
  | { kind: "cancelled"; message: string }
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

export function isLoadedRecipe(value: unknown): value is LoadedRecipe {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<LoadedRecipe>;
  const recipe = candidate.recipe as Partial<TransformRecipe> | undefined;
  const sourceSchema = candidate.sourceSchema;
  const sourceSchemaValid = sourceSchema === undefined || (Array.isArray(sourceSchema) && sourceSchema.every((column) =>
    !!column && typeof column.name === "string" && column.name.length > 0 &&
    typeof column.dataType === "string" && column.dataType.length > 0,
  ));
  return (candidate.version === 1 || candidate.version === 2) && sourceSchemaValid &&
    !(candidate.version === 1 && sourceSchema !== undefined) &&
    typeof candidate.name === "string" &&
    typeof candidate.savedAt === "string" && !!recipe &&
    Array.isArray(recipe.renames) && Array.isArray(recipe.casts) &&
    Array.isArray(recipe.dateParses) && Array.isArray(recipe.filters) &&
    Array.isArray(recipe.outlierTreatments) && Array.isArray(recipe.contactNormalizations) &&
    Array.isArray(recipe.textExtractions) && "calculatedColumn" in recipe &&
    "findReplace" in recipe && "keepColumns" in recipe && "splitColumn" in recipe &&
    "mergeColumns" in recipe && "groupSummary" in recipe &&
    (!candidate.exportOptions || isRecipeExportOptions(candidate.exportOptions));
}

export function requiresImpactConfirmation(recipe: TransformRecipe): boolean {
  return recipe.filters.length > 0 ||
    recipe.keepColumns !== null ||
    Boolean(recipe.splitColumn?.dropSource) ||
    Boolean(recipe.mergeColumns?.dropSources) ||
    recipe.outlierTreatments.length > 0 ||
    recipe.groupSummary !== null ||
    // Find-and-replace rewrites every matching cell (FUN-25).
    recipe.findReplace !== null ||
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
    cast_numeric: "Convirtiendo números detectados…",
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

function counted(count: number, one: string, many: string): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

/**
 * What a proposal changed, one line per kind of change, so its result screen
 * says what happened even when rows, gaps and duplicates stay the same.
 */
export function appliedPlanChanges(
  options: SafeCorrectionOptions,
  result: Pick<SafeCorrectionsResult, "changedCellCount" | "renamedColumnCount" | "removedRowCount" | "typedColumnCount" | "datedColumnCount" | "imputedCellCount">,
): string[] {
  const [cleanedOne, cleanedMany] = options.trimText && options.normalizeSentinels
    ? ["celda limpiada (espacios y marcadores «sin dato»)", "celdas limpiadas (espacios y marcadores «sin dato»)"]
    : options.trimText
      ? ["celda con espacios recortados", "celdas con espacios recortados"]
      : ["marcador «sin dato» convertido en vacío", "marcadores «sin dato» convertidos en vacíos"];
  return [
    (options.trimText || options.normalizeSentinels) && result.changedCellCount > 0
      ? counted(result.changedCellCount, cleanedOne, cleanedMany)
      : null,
    options.normalizeColumnNames && result.renamedColumnCount > 0
      ? counted(result.renamedColumnCount, "columna renombrada", "columnas renombradas")
      : null,
    options.removeDuplicates && result.removedRowCount > 0
      ? counted(result.removedRowCount, "fila duplicada quitada", "filas duplicadas quitadas")
      : null,
    result.typedColumnCount > 0
      ? counted(result.typedColumnCount, "columna convertida a número", "columnas convertidas a número")
      : null,
    result.datedColumnCount > 0
      ? counted(result.datedColumnCount, "columna convertida a fecha", "columnas convertidas a fecha")
      : null,
    options.imputeMissing && result.imputedCellCount > 0
      ? counted(result.imputedCellCount, "valor vacío rellenado", "valores vacíos rellenados")
      : null,
  ].filter((change): change is string => change !== null);
}

export type TypeConversionKind = "numeric" | "dates";

/** Text columns that «Convertir números detectados» would consider. */
export function numericConversionCandidates(profile: DatasetProfile) {
  return profile.columns.filter(
    (column) => column.name !== "_cambios" && isTextType(column.dataType) &&
      (column.suggestedType === "integer" || column.suggestedType === "decimal") &&
      (column.typeMatchPercentage ?? 0) > 90 &&
      column.privacySignal !== "identifier",
  );
}

/** Text columns that «Interpretar fechas detectadas» would consider. */
export function dateConversionCandidates(profile: DatasetProfile) {
  return profile.columns.filter(
    (column) => column.name !== "_cambios" && isTextType(column.dataType) && column.suggestedType === "date",
  );
}

/**
 * Cells that the profile counted as not matching the detected type: a
 * conversion leaves them empty. Only columns with at least one are listed.
 */
export function conversionNullEstimate(profile: DatasetProfile, kind: TypeConversionKind) {
  const candidates = kind === "numeric" ? numericConversionCandidates(profile) : dateConversionCandidates(profile);
  return candidates
    .filter((column) => (column.invalidTypeCount ?? 0) > 0)
    .map((column) => ({ name: column.name, count: column.invalidTypeCount ?? 0 }));
}

export function cellCount(count: number): string {
  return count === 1 ? "1 celda" : `${count.toLocaleString()} celdas`;
}

/** «2 celdas quedaron vacías (importe: 2).» or "" when nothing was emptied. */
export function nullifiedCellsSentence(columns: ChangedTextColumn[]): string {
  const emptied = columns.filter((column) => (column.nullifiedCellCount ?? 0) > 0);
  const total = emptied.reduce((sum, column) => sum + (column.nullifiedCellCount ?? 0), 0);
  if (total === 0) return "";
  const detail = emptied
    .map((column) => `${column.name}: ${(column.nullifiedCellCount ?? 0).toLocaleString()}`)
    .join(", ");
  return ` ${cellCount(total)} que no encajaban ${total === 1 ? "quedó vacía" : "quedaron vacías"} (${detail}).`;
}
