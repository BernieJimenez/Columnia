import { Channel, invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
}

export interface ResourceUsage {
  processCpuPercentage: number;
  systemCpuPercentage: number;
  processMemoryBytes: number;
  systemMemoryUsedBytes: number;
  systemMemoryTotalBytes: number;
}

export interface DatasetColumn {
  name: string;
  dataType: string;
}

export interface DatasetPreview {
  fileName: string;
  fileSizeBytes: number;
  rowCount: number;
  columnCount: number;
  columns: DatasetColumn[];
  rows: Array<Array<string | null>>;
}

export interface DatasetComparison {
  currentFileName: string;
  comparedFileName: string;
  currentRowCount: number;
  comparedRowCount: number;
  commonRowCount: number;
  currentOnlyRowCount: number;
  comparedOnlyRowCount: number;
  sharedColumns: string[];
  currentOnlyColumns: string[];
  comparedOnlyColumns: string[];
  schemaCompatible: boolean;
  keyColumns: string[];
  matchedKeyCount: number;
  currentOnlyKeyCount: number;
  comparedOnlyKeyCount: number;
  conflictingKeyCount: number;
  duplicateKeyCount: number;
  canConsolidate: boolean;
}

export type DatasetJoinType = "inner" | "left" | "full";

export type DatasetFormat = "csv" | "tsv" | "json" | "parquet" | "excel";

export interface WorkbookSheet {
  id: string;
  name: string;
}

export interface DatasetSourceInspection {
  selectionId: string;
  fileName: string;
  fileSizeBytes: number;
  format: DatasetFormat;
  sheets: WorkbookSheet[];
  defaultSheetId: string | null;
  isCompressedContainer: boolean;
}

export type SpreadsheetHeaderMode = "firstRow" | "generated";

export interface DatasetPage {
  offset: number;
  rows: Array<Array<string | null>>;
}

export interface DatasetQueryResult {
  columns: DatasetColumn[];
  rowCount: number;
  offset: number;
  rows: Array<Array<string | null>>;
  truncated: boolean;
}

export interface ColumnProfile {
  name: string;
  dataType: string;
  nullCount: number;
  completenessPercentage: number;
  uniqueCount: number;
  minimum: string | null;
  maximum: string | null;
  mean: number | null;
  emptyCount: number | null;
  minimumLength: number | null;
  maximumLength: number | null;
  averageLength: number | null;
  suggestedType: "boolean" | "integer" | "decimal" | "date" | null;
  typeMatchPercentage: number | null;
  invalidTypeCount: number | null;
  standardDeviation: number | null;
  firstQuartile: number | null;
  median: number | null;
  thirdQuartile: number | null;
  outlierCount: number | null;
}

export interface DatasetProfile {
  rowCount: number;
  duplicateRowCount: number;
  duplicatePercentage: number;
  columns: ColumnProfile[];
}

export interface DatasetMutation {
  dataset: DatasetPreview;
  affectedRowCount: number;
}

export interface ColumnRename {
  from: string;
  to: string;
}

export interface ColumnNormalizationResult {
  dataset: DatasetPreview;
  renamedColumnCount: number;
  renames: ColumnRename[];
}

export interface ChangedTextColumn {
  name: string;
  changedCellCount: number;
}

export interface TextCleaningResult {
  dataset: DatasetPreview;
  affectedRowCount: number;
  changedCellCount: number;
  changedColumns: ChangedTextColumn[];
}

export interface HistoryEntryState {
  index: number;
  label: string;
  isCurrent: boolean;
}

export interface HistoryState {
  canUndo: boolean;
  canRedo: boolean;
  currentIndex: number;
  entryCount: number;
  entries: HistoryEntryState[];
  snapshotsEnabled: boolean;
  degradedReason: string | null;
  maxEntries: number;
  diskBytes: number;
  diskBudgetBytes: number;
}

export interface HistoryResult {
  dataset: DatasetPreview;
  history: HistoryState;
  message: string;
}

export interface SafeCorrectionsResult {
  dataset: DatasetPreview;
  changedCellCount: number;
  affectedRowCount: number;
  renamedColumnCount: number;
  renames: ColumnRename[];
}

export type RecipeCastTarget = "string" | "integer" | "decimal" | "boolean";
export type RecipeDateFormat = "iso8601" | "ymd" | "dmy" | "mdy";
export type RecipeDateTarget = "date" | "datetime";
export type RecipeFilterOperator =
  | "eq" | "neq" | "gt" | "lt" | "gte" | "lte"
  | "contains" | "not_contains" | "is_null" | "not_null";
export type CalculatedOperation =
  | "add" | "subtract" | "multiply" | "divide" | "concat"
  | "year" | "month" | "day";
export type CalculatedOperandKind = "literal" | "column";
export type FindReplaceScope = "column" | "all_text_columns";
export type OutlierAction = "cap" | "drop";
export type SummaryOperation = "sum" | "mean" | "min" | "max" | "count" | "count_unique";
export type ContactKind = "email" | "phone" | "address";
export type ExtractionKind =
  | "first_token" | "last_token" | "digits" | "letters" | "before" | "after";

// Alias públicos históricos. Mantenerlos evita romper consumidores existentes.
export type TransformTarget = RecipeCastTarget;
export type DateInputFormat = RecipeDateFormat;
export type DateTarget = RecipeDateTarget;
export type FilterOperator = RecipeFilterOperator;
export type CalculationOperation = CalculatedOperation;

export interface RecipeRename {
  from: string;
  to: string;
}

export interface RecipeCast {
  column: string;
  target: RecipeCastTarget;
}

export interface RecipeDateParse {
  column: string;
  format: RecipeDateFormat;
  target: RecipeDateTarget;
}

export interface RecipeFilter {
  column: string;
  operator: RecipeFilterOperator;
  value: string | null;
}

export interface CalculatedOperand {
  kind: CalculatedOperandKind;
  value: string;
}

export type CalculationOperand = CalculatedOperand;

export interface CalculatedColumnRecipe {
  name: string;
  source: string;
  operation: CalculatedOperation;
  operand: CalculatedOperand | null;
}

export interface FindReplaceRecipe {
  scope: FindReplaceScope;
  column: string | null;
  find: string;
  replace: string;
}

export interface SplitColumnRecipe {
  source: string;
  delimiter: string;
  names: string[];
  dropSource: boolean;
}

export interface MergeColumnsRecipe {
  sources: string[];
  name: string;
  separator: string;
  dropSources: boolean;
}

export interface OutlierTreatment {
  column: string;
  action: OutlierAction;
}

export interface SummaryAggregation {
  column: string;
  operation: SummaryOperation;
}

export interface GroupSummaryRecipe {
  groupBy: string[];
  aggregations: SummaryAggregation[];
}

export interface ContactNormalization {
  column: string;
  kind: ContactKind;
}

export interface TextExtraction {
  source: string;
  kind: ExtractionKind;
  name: string;
  delimiter: string | null;
}

export interface TransformRecipe {
  renames: RecipeRename[];
  casts: RecipeCast[];
  dateParses: RecipeDateParse[];
  filters: RecipeFilter[];
  calculatedColumn: CalculatedColumnRecipe | null;
  findReplace: FindReplaceRecipe | null;
  keepColumns: string[] | null;
  splitColumn: SplitColumnRecipe | null;
  mergeColumns: MergeColumnsRecipe | null;
  outlierTreatments: OutlierTreatment[];
  groupSummary: GroupSummaryRecipe | null;
  contactNormalizations: ContactNormalization[];
  textExtractions: TextExtraction[];
}

export interface SavedRecipe {
  version: 1;
  name: string;
  savedAt: string;
  recipe: TransformRecipe;
}

export type LoadedRecipe = SavedRecipe;

export interface TransformRecipeResult {
  dataset: DatasetPreview;
  changed: boolean;
  renamedColumnCount: number;
  convertedColumnCount: number;
  parsedDateColumnCount: number;
  removedRowCount: number;
  calculatedColumnCount: number;
  replacedCellCount: number;
  droppedColumnCount: number;
  splitColumnCount: number;
  mergedColumnCount: number;
  droppedSourceColumnCount: number;
  adjustedOutlierCellCount: number;
  outlierRemovedRowCount: number;
  outlierColumnCount: number;
  groupCount: number;
  aggregatedColumnCount: number;
  collapsedRowCount: number;
  normalizedContactCellCount: number;
  normalizedContactColumnCount: number;
  extractedColumnCount: number;
}

export interface OperationProgress {
  operation: "load" | "profile" | "export";
  stage: string;
  percent: number;
}

export type CancellableOperation = OperationProgress["operation"];
export type ExportFormat = "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite";
export type PrivacyMode = "none" | "mask" | "hash";

export const QUALITY_DATASET_COLUMN = "__dataset__";

export type QualityRuleKind =
  | "not_null"
  | "non_empty"
  | "unique"
  | "numeric_range"
  | "allowed_values"
  | "regex"
  | "dtype"
  | "unique_together"
  | "row_count";

export interface QualityRule {
  column: string;
  kind: QualityRuleKind;
  maxInvalid?: number;
  maxInvalidPct?: number;
  min?: number;
  max?: number;
  values?: string[];
  pattern?: string;
  dtype?: string;
  columns?: string[];
}

export interface QualityRuleResult extends QualityRule {
  checkedCount: number;
  invalidCount: number;
  invalidPct: number;
  passed: boolean;
}

export interface QualityValidationResult {
  passed: boolean;
  rowCount: number;
  totalRules: number;
  failedRules: number;
  rules: QualityRuleResult[];
}

export interface QualityMigrationWarning {
  ruleIndex: number;
  sourceKind: string;
  severity: "warning" | "omitted";
  message: string;
}

export interface QualityMigrationResult {
  sourceVersion: string | null;
  convertedRules: QualityRule[];
  warnings: QualityMigrationWarning[];
  omittedRules: number;
}

export interface ExportResult {
  fileName: string;
  fileSizeBytes: number;
  format: "CSV" | "JSON" | "Parquet" | "SQL" | "Excel" | "SQLite";
}

export interface ProjectSummary {
  id: string;
  name: string;
  datasetFileName: string;
  rowCount: number;
  columnCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectWorkspace {
  qualityRules: QualityRule[];
  recipeDraft: SavedRecipe | null;
}

export interface ProjectOpenResult {
  project: ProjectSummary;
  dataset: DatasetPreview;
  workspace: ProjectWorkspace;
  profile: DatasetProfile | null;
}

type ProgressHandler = (progress: OperationProgress) => void;

function progressChannel(onProgress?: ProgressHandler): Channel<OperationProgress> {
  return new Channel<OperationProgress>((progress) => onProgress?.(progress));
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export function getResourceUsage(): Promise<ResourceUsage> {
  return invoke<ResourceUsage>("get_resource_usage");
}

export function pickDatasetSource(): Promise<DatasetSourceInspection | null> {
  return invoke<DatasetSourceInspection | null>("pick_dataset_source");
}

export function loadDatasetSelection(
  selectionId: string,
  sheetId: string | null,
  headerMode: SpreadsheetHeaderMode | null,
  onProgress?: ProgressHandler,
): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("load_dataset_selection", {
    selectionId,
    sheetId,
    headerMode,
    onProgress: progressChannel(onProgress),
  });
}

export function discardDatasetSelection(selectionId: string): Promise<void> {
  return invoke<void>("discard_dataset_selection", { selectionId });
}

export function compareDataset(keyColumns: string[] = []): Promise<DatasetComparison | null> {
  return invoke<DatasetComparison | null>("compare_dataset", { keyColumns });
}

export function joinDataset(
  keyColumns: string[],
  joinType: DatasetJoinType,
): Promise<DatasetPreview | null> {
  return invoke<DatasetPreview | null>("join_dataset", { keyColumns, joinType });
}

export function clearDatasetComparison(): Promise<void> {
  return invoke<void>("clear_dataset_comparison");
}

export function useConsolidatedDataset(): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("use_consolidated_dataset");
}

export function getDatasetPage(offset: number, limit: number): Promise<DatasetPage> {
  return invoke<DatasetPage>("get_dataset_page", { offset, limit });
}

export function queryDataset(query: string): Promise<DatasetQueryResult> {
  return invoke<DatasetQueryResult>("query_dataset", { query });
}

export function getDatasetProfile(onProgress?: ProgressHandler): Promise<DatasetProfile> {
  return invoke<DatasetProfile>("get_dataset_profile", {
    onProgress: progressChannel(onProgress),
  });
}

export function cancelOperation(operation: CancellableOperation): Promise<void> {
  return invoke<void>("cancel_operation", { operation });
}

export function exportDataset(
  format: ExportFormat,
  qualityRules: QualityRule[],
  allowUnvalidated: boolean,
  onProgress?: ProgressHandler,
  privacyMode: PrivacyMode = "none",
): Promise<ExportResult | null> {
  return invoke<ExportResult | null>("export_dataset", {
    format,
    qualityRules,
    allowUnvalidated,
    privacyMode,
    onProgress: progressChannel(onProgress),
  });
}

export function validateQualityRules(
  qualityRules: QualityRule[],
): Promise<QualityValidationResult> {
  return invoke<QualityValidationResult>("validate_quality_rules", { qualityRules });
}

export function pickQualityRulesMigration(): Promise<QualityMigrationResult | null> {
  return invoke<QualityMigrationResult | null>("pick_quality_rules_migration");
}

export function removeDuplicates(): Promise<DatasetMutation> {
  return invoke<DatasetMutation>("remove_duplicates");
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

export function applySafeCorrections(): Promise<SafeCorrectionsResult> {
  return invoke<SafeCorrectionsResult>("apply_safe_corrections");
}

export function applyTransformRecipe(recipe: TransformRecipe): Promise<TransformRecipeResult> {
  return invoke<TransformRecipeResult>("apply_transform_recipe", { recipe });
}

export function saveTransformRecipe(
  recipe: TransformRecipe,
  name: string,
): Promise<SavedRecipe | null> {
  return invoke<SavedRecipe | null>("save_transform_recipe", { recipe, name });
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

export function listProjects(): Promise<ProjectSummary[]> {
  return invoke<ProjectSummary[]>("list_projects");
}

export function getRecoveryCandidate(): Promise<ProjectSummary | null> {
  return invoke<ProjectSummary | null>("get_recovery_candidate");
}

export function saveProject(
  projectId: string | null,
  name: string,
  workspace: ProjectWorkspace,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("save_project", { projectId, name, workspace });
}

export function openProject(projectId: string): Promise<ProjectOpenResult> {
  return invoke<ProjectOpenResult>("open_project", { projectId });
}

export function deleteProject(projectId: string): Promise<void> {
  return invoke<void>("delete_project", { projectId });
}
