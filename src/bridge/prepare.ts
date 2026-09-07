import { invoke } from "@tauri-apps/api/core";
import type {
  DatasetMutation,
  ColumnRemovalResult,
  ColumnNormalizationResult,
  TextCleaningResult,
  PersonalDataMaskResult,
  HistoryState,
  HistoryResult,
  SafeCorrectionsResult,
  TransformRecipe,
  SavedRecipe,
  LoadedRecipe,
  RecipeExportOptions,
  TransformRecipeResult,
} from "./contracts";

export function removeDuplicates(): Promise<DatasetMutation> {
  return invoke<DatasetMutation>("remove_duplicates");
}

export function removeNearDuplicates(): Promise<DatasetMutation> {
  return invoke<DatasetMutation>("remove_near_duplicates");
}

export function removeEmptyRows(): Promise<DatasetMutation> {
  return invoke<DatasetMutation>("remove_empty_rows");
}

export function enableRowAudit(): Promise<DatasetMutation> {
  return invoke<DatasetMutation>("enable_row_audit");
}

export function removeConstantColumns(): Promise<ColumnRemovalResult> {
  return invoke<ColumnRemovalResult>("remove_constant_columns");
}

export function removeEmptyColumns(): Promise<ColumnRemovalResult> {
  return invoke<ColumnRemovalResult>("remove_empty_columns");
}

export function removeHighNullColumns(): Promise<ColumnRemovalResult> {
  return invoke<ColumnRemovalResult>("remove_high_null_columns");
}

export function removeIdentifierColumns(): Promise<ColumnRemovalResult> {
  return invoke<ColumnRemovalResult>("remove_identifier_columns");
}

export function removePersonalColumns(): Promise<ColumnRemovalResult> {
  return invoke<ColumnRemovalResult>("remove_personal_columns");
}

export function maskPersonalValues(): Promise<PersonalDataMaskResult> {
  return invoke<PersonalDataMaskResult>("mask_personal_values");
}

export function normalizeColumnNames(): Promise<ColumnNormalizationResult> {
  return invoke<ColumnNormalizationResult>("normalize_column_names");
}

export function trimTextValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("trim_text_values");
}

export function normalizeTextValues(
  columns: string[],
  removeAccents: boolean,
): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("normalize_text_values", { columns, removeAccents });
}

export function parseDateValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("parse_date_values");
}

export function castNumericValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("cast_numeric_values");
}

export function normalizeSentinelValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("normalize_sentinel_values");
}

export function normalizeBooleanValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("normalize_boolean_values");
}

export function fixEncodingValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("fix_encoding_values");
}

export function nullifyInvalidTypeValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("nullify_invalid_type_values");
}

export function imputeMissingValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("impute_missing_values");
}

export function imputeCategoricalValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("impute_categorical_values");
}

export function imputeOutlierValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("impute_outlier_values");
}

export function capOutlierValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("cap_outlier_values");
}

export function dropOutlierValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("drop_outlier_values");
}

export function applySafeCorrections(): Promise<SafeCorrectionsResult> {
  return invoke<SafeCorrectionsResult>("apply_safe_corrections");
}

export function applyTransformRecipe(recipe: TransformRecipe): Promise<TransformRecipeResult> {
  return invoke<TransformRecipeResult>("apply_transform_recipe", { recipe });
}

export function saveTransformRecipe(
  recipe: TransformRecipe,
  name: string,
  exportOptions: RecipeExportOptions | null = null,
): Promise<SavedRecipe | null> {
  return invoke<SavedRecipe | null>("save_transform_recipe", {
    recipe,
    name,
    exportOptions,
  });
}

export function pickTransformRecipe(): Promise<LoadedRecipe | null> {
  return invoke<LoadedRecipe | null>("pick_transform_recipe");
}

export function undoLastChange(): Promise<HistoryResult> {
  return invoke<HistoryResult>("undo_last_change");
}

export function redoLastChange(): Promise<HistoryResult> {
  return invoke<HistoryResult>("redo_last_change");
}

export function getHistoryState(): Promise<HistoryState> {
  return invoke<HistoryState>("get_history_state");
}
