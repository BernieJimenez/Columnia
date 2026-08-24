import type { HistoryState, LoadedRecipe, TransformRecipe } from "../../bridge";

export type ChangeStatus =
  | { kind: "idle" }
  | {
      kind: "working";
      action: "safe" | "duplicates" | "empty_rows" | "constant_columns" | "empty_columns" | "high_null_columns" | "sentinels" | "booleans" | "audit" | "columns" | "trim" | "text" | "transform" | "undo" | "redo";
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
    "mergeColumns" in recipe && "groupSummary" in recipe;
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
