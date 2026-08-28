import { Channel, invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  updaterConfigured?: boolean;
}

export interface UpdateInfo {
  currentVersion: string;
  version: string;
  notes: string | null;
  date: string | null;
  sizeBytes: number | null;
}

export interface UpdaterProgress {
  phase: "started" | "progress" | "finished" | "cancelled";
  downloadedBytes: number;
  contentLength: number | null;
}

export interface ResourceUsage {
  processCpuPercentage: number;
  systemCpuPercentage: number;
  logicalCpuCount: number;
  processMemoryBytes: number;
  systemMemoryUsedBytes: number;
  systemMemoryTotalBytes: number;
  /** Available RAM is optional for compatibility with older desktop builds. */
  systemMemoryAvailableBytes?: number;
  /** GPU values are null when the runtime has no GPU probe. */
  gpu?: GpuUsage;
}

export type PerformanceProfile = "conservative" | "balanced" | "maximum";

export interface PerformanceSettings {
  requestedProfile: PerformanceProfile;
  activeProfile: PerformanceProfile | null;
  requestedThreads: number;
  activeThreads: number | null;
  applied: boolean;
  locked: boolean;
  reason: string | null;
}

export interface GpuUsage {
  status: "available" | "unavailable";
  usagePercentage: number | null;
  memoryUsedBytes: number | null;
  memoryTotalBytes: number | null;
  reason: string | null;
}

export interface DatasetColumn {
  name: string;
  dataType: string;
}

export interface DatasetConflictCell {
  column: string;
  current: string | null;
  compared: string | null;
}

export interface DatasetConflict {
  key: Array<string | null>;
  cells: DatasetConflictCell[];
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
  conflicts: DatasetConflict[];
  conflictOffset: number;
  conflictsTruncated: boolean;
  canConsolidate: boolean;
}

export interface DatasetConflictPage {
  offset: number;
  conflicts: DatasetConflict[];
  hasNext: boolean;
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

export interface HistogramBucket {
  lower: number;
  upper: number;
  count: number;
}

export interface NumericCorrelation {
  firstColumn: string;
  secondColumn: string;
  coefficient: number | null;
  sampleCount: number;
}

export interface NumericCorrelationMatrix {
  columns: string[];
  pairs: NumericCorrelation[];
  sampledRowCount: number;
  truncated: boolean;
}

export interface CategoricalGroup {
  label: string;
  rowCount: number;
  percentage: number;
  isOther: boolean;
}

export interface CategoricalGroupSummary {
  column: string;
  groups: CategoricalGroup[];
  distinctCount: number;
  truncated: boolean;
}

export interface TemporalPeriod {
  period: string;
  rowCount: number;
  percentage: number;
}

export interface TemporalSeriesSummary {
  column: string;
  granularity: "day" | "month" | "year";
  periods: TemporalPeriod[];
  parsedRowCount: number;
  unparsedRowCount: number;
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
  sentinelCount: number | null;
  privacySignal: "email" | "phone" | "address" | "identifier" | "name" | null;
  standardDeviation: number | null;
  firstQuartile: number | null;
  median: number | null;
  thirdQuartile: number | null;
  outlierCount: number | null;
  histogram: HistogramBucket[] | null;
}

export interface DatasetProfile {
  rowCount: number;
  duplicateRowCount: number;
  nearDuplicateRowCount: number;
  duplicatePercentage: number;
  columns: ColumnProfile[];
  numericCorrelations?: NumericCorrelationMatrix;
  categoricalGroupSummaries?: CategoricalGroupSummary[];
  temporalSeries?: TemporalSeriesSummary[];
}

export interface DatasetMutation {
  dataset: DatasetPreview;
  affectedRowCount: number;
}

export interface ColumnRemovalResult {
  dataset: DatasetPreview;
  removedColumnCount: number;
  removedColumns: string[];
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
  exportOptions?: RecipeExportOptions;
  migrationReport?: RecipeMigrationReport;
}

export type LoadedRecipe = SavedRecipe;

export interface RecipeExportOptions {
  formats: ExportFormat[];
  selectedColumns: string[];
  privacyMode: PrivacyMode;
}

export interface RecipeMigrationWarning {
  path: string;
  severity: "warning" | "omitted";
  message: string;
}

export interface SessionMigrationMetadata {
  hasSourceReference: boolean;
  hasSnapshotReference: boolean;
  sheetName: string | null;
  stageLabel: string | null;
  appliedOperationCount: number;
  qualityRuleCount: number;
  analysisCheckCount: number;
}

export type SessionReferenceStatus = "not_provided" | "available" | "missing" | "unsupported";

export interface SessionMigrationReferenceReport {
  status: SessionReferenceStatus;
  available: boolean;
}

export interface SessionMigrationReport {
  schemaVersion: number;
  command: "session-migration-report";
  artifactSha256: string;
  origin: {
    source: SessionMigrationReferenceReport;
    snapshot: SessionMigrationReferenceReport;
    sourceFileName: string | null;
  };
  session: {
    sourceVersion: string | null;
    sheetName: string | null;
    stageLabel: string | null;
    appliedOperationCount: number;
    analysisCheckCount: number;
  };
  recipeSummary: {
    operationCount: number;
    convertedOperationCount: number;
    omittedOperationCount: number;
    warningCount: number;
    convertedOperations: string[];
    omittedOperations: string[];
  };
  quality: {
    totalRules: number;
    convertedRules: number;
    omittedRules: number;
    warningCount: number;
  };
  missingReferences: string[];
  collisions: string[];
  canCreateProject: boolean;
  requiresManualReview: boolean;
  manualActions: string[];
}

export interface DataprepSessionMigrationPlan {
  name: string;
  sourceFileName: string | null;
  sourceStatus: SessionReferenceStatus;
  snapshotStatus: SessionReferenceStatus;
  sheetName: string | null;
  stageLabel: string | null;
  recipe: SavedRecipe;
  qualityRules: QualityRule[];
  qualityReport: QualityMigrationReport | null;
  missingReferences: string[];
  collisions: string[];
  canCreateProject: boolean;
}

export interface RecipeMigrationReport {
  artifactSha256: string | null;
  sourceFormat: "dataprep" | "legacy";
  sourceVersion: number | null;
  convertedItems: number;
  omittedItems: number;
  warningCount: number;
  convertedOperations: string[];
  omittedOperations: string[];
  warnings: RecipeMigrationWarning[];
  manualActions: string[];
  session?: SessionMigrationMetadata;
}

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

export type CancellableOperation = OperationProgress["operation"] | "query";
export type ExportFormat = "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle";
export type PrivacyMode = "none" | "mask" | "hash";
export type ConflictSource = "current" | "compared";

export interface ConflictResolution {
  conflictIndex: number;
  /** Omit only for compatibility with the legacy whole-row decision. */
  column?: string;
  source: ConflictSource;
}

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
  | "column_compare"
  | "referential_integrity"
  | "monotonic"
  | "aggregate_check"
  | "aggregate_reconciliation"
  | "distribution_drift"
  | "date_range"
  | "conditional"
  | "schema_contract"
  | "row_count";

export type QualityComparison = "eq" | "ne" | "lt" | "lte" | "gt" | "gte";
export type QualityMonotonicDirection = "increasing" | "decreasing";
export type QualityAggregate = "count" | "sum" | "min" | "max";

export interface QualityCondition {
  column: string;
  operator?: QualityComparison;
  value?: string;
}

export interface QualityRule {
  column: string;
  kind: QualityRuleKind;
  maxInvalid?: number;
  maxInvalidPct?: number;
  min?: number;
  max?: number;
  values?: string[];
  referenceValues?: string[];
  baseline?: string[];
  direction?: QualityMonotonicDirection;
  expected?: number;
  aggregate?: QualityAggregate;
  toleranceAbs?: number;
  toleranceRel?: number;
  threshold?: number;
  pattern?: string;
  dtype?: string;
  columns?: string[];
  operator?: QualityComparison;
  minDate?: string;
  maxDate?: string;
  when?: QualityCondition;
  then?: QualityRule;
  allowAdditional?: boolean;
  requiredOrder?: string[];
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

export interface QualityMigrationReport {
  artifactSha256: string | null;
  totalItems: number;
  convertedItems: number;
  omittedItems: number;
  warningCount: number;
  manualActions: string[];
}

export interface QualityMigrationResult {
  sourceFormat: "columnia" | "dataprep" | "legacy";
  sourceVersion: string | null;
  convertedRules: QualityRule[];
  warnings: QualityMigrationWarning[];
  omittedRules: number;
  report: QualityMigrationReport;
}

export interface QualityRulesDocument {
  format: string;
  version: number;
  rules: QualityRule[];
}

export interface ExportResult {
  fileName: string;
  fileSizeBytes: number;
  format: "CSV" | "JSON" | "Parquet" | "SQL" | "Excel" | "SQLite" | "Paquete Columnia";
  protectedColumnCount: number;
  protectedColumns: string[];
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

export function checkForUpdate(): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("check_for_update");
}

export function downloadUpdate(onProgress?: (progress: UpdaterProgress) => void): Promise<void> {
  const channel = new Channel<UpdaterProgress>((progress) => onProgress?.(progress));
  return invoke<void>("download_update", { onProgress: channel });
}

export function cancelUpdateDownload(): Promise<void> {
  return invoke<void>("cancel_update_download");
}

export function installUpdate(): Promise<void> {
  return invoke<void>("install_update");
}

export function getResourceUsage(): Promise<ResourceUsage> {
  return invoke<ResourceUsage>("get_resource_usage");
}

export function getPerformanceSettings(): Promise<PerformanceSettings> {
  return invoke<PerformanceSettings>("get_performance_settings");
}

export function setPerformanceProfile(profile: PerformanceProfile): Promise<PerformanceSettings> {
  return invoke<PerformanceSettings>("set_performance_profile", { profile });
}

export function pickDatasetSource(): Promise<DatasetSourceInspection | null> {
  return invoke<DatasetSourceInspection | null>("pick_dataset_source");
}

/** Inspects the path captured by Tauri's native drag/drop event without exposing it to React. */
export function inspectDroppedDataset(): Promise<DatasetSourceInspection | null> {
  return invoke<DatasetSourceInspection | null>("inspect_dropped_dataset");
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

export function getDatasetConflictPage(
  offset: number,
  limit: number,
): Promise<DatasetConflictPage | null> {
  return invoke<DatasetConflictPage | null>("get_dataset_conflict_page", { offset, limit });
}

export function joinDataset(
  keyColumns: string[],
  joinType: DatasetJoinType,
): Promise<DatasetPreview | null> {
  return invoke<DatasetPreview | null>("join_dataset", { keyColumns, joinType });
}

export function resolveDatasetConflicts(
  decisions: ConflictResolution[],
): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("resolve_dataset_conflicts", { decisions });
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

/**
 * Runs the bounded local SELECT contract. JOIN can read only the comparison
 * already loaded by Review through the logical table `compared`.
 */
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
  recipe?: SavedRecipe | null,
): Promise<ExportResult | null> {
  return invoke<ExportResult | null>("export_dataset", {
    format,
    qualityRules,
    allowUnvalidated,
    privacyMode,
    onProgress: progressChannel(onProgress),
    recipe: recipe ?? null,
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

export function pickDataprepSessionMigration(): Promise<DataprepSessionMigrationPlan | null> {
  return invoke<DataprepSessionMigrationPlan | null>("pick_dataprep_session_migration");
}

export function saveQualityRulesDocument(
  qualityRules: QualityRule[],
): Promise<QualityRulesDocument | null> {
  return invoke<QualityRulesDocument | null>("save_quality_rules_document", { qualityRules });
}

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

export function normalizeSentinelValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("normalize_sentinel_values");
}

export function normalizeBooleanValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("normalize_boolean_values");
}

export function imputeMissingValues(): Promise<TextCleaningResult> {
  return invoke<TextCleaningResult>("impute_missing_values");
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
  migrationReport: RecipeMigrationReport | null = null,
  exportOptions: RecipeExportOptions | null = null,
): Promise<SavedRecipe | null> {
  return invoke<SavedRecipe | null>("save_transform_recipe", {
    recipe,
    name,
    migrationReport,
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

export function importDataprepSessionProject(
  name: string | null = null,
  sheetName: string | null = null,
  headerMode: SpreadsheetHeaderMode | null = null,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("import_dataprep_session_project", {
    name,
    sheetName,
    headerMode,
  });
}

export function previewDataprepSessionMigration(): Promise<SessionMigrationReport | null> {
  return invoke<SessionMigrationReport | null>("preview_dataprep_session_migration");
}
