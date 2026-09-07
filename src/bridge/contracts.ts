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

export interface SampleDatasetDescriptor {
  id: string;
  name: string;
  format: string;
  description: string;
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

export type DatasetQueryEngine = "polars" | "duckdb";

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
  encodingIssueCount: number | null;
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

export interface PersonalDataMaskResult {
  dataset: DatasetPreview;
  changedCellCount: number;
  changedColumnCount: number;
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
export type OutlierAction = "cap" | "drop" | "impute";
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
  regex: boolean;
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
}

export type LoadedRecipe = SavedRecipe;

export interface RecipeExportOptions {
  formats: LocalExportFormat[];
  selectedColumns: string[];
  privacyMode: PrivacyMode;
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
export type LocalExportFormat = "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle";
export type ExportFormat = LocalExportFormat | "postgresql" | "mysql" | "sqlserver";
export type PrivacyMode = "none" | "mask" | "hash";
export type ConflictSource = "current" | "compared";
export type DatabaseKind = "postgresql" | "mysql" | "sqlserver";
export type DatabaseTablePolicy = "append" | "create_only" | "replace";

/** Credentials remain in the current React/Tauri call and are never persisted. */
export interface DatabaseTarget {
  kind: DatabaseKind;
  connectionString: string;
  schema: string;
  table: string;
  tablePolicy: DatabaseTablePolicy;
}

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
  sourceFormat: "columnia" | "legacy";
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
  format: "CSV" | "JSON" | "Parquet" | "SQL" | "Excel" | "SQLite" | "Paquete Columnia" | "PostgreSQL" | "MySQL" | "SQL Server";
  protectedColumnCount: number;
  protectedColumns: string[];
}

export interface DatabaseConnectionResult {
  kind: DatabaseKind;
  message: string;
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

export interface ProjectOpenResult {
  project: ProjectSummary;
  dataset: DatasetPreview;
  workspace: ProjectWorkspace;
  profile: DatasetProfile | null;
}

export interface SqlQueryHistoryEntry {
  id: number;
  outcome: "success" | "error" | "cancelled";
  durationMs: number;
  rowCount: number | null;
}

export interface ProjectWorkspace {
  qualityRules: QualityRule[];
  recipeDraft: SavedRecipe | null;
  sqlHistory?: SqlQueryHistoryEntry[];
  reviewTab?: ProjectReviewTab;
  previewOffset?: number;
  activePhase?: ProjectActivePhase;
  queryEngine?: DatasetQueryEngine;
  analysisSampleRows?: 10_000 | 50_000 | 100_000;
  performanceProfile?: PerformanceProfile;
  exportFormat?: ExportFormat;
  privacyMode?: PrivacyMode;
  comparisonKeyColumns?: string[];
  joinType?: DatasetJoinType;
}

export type ProjectReviewTab = "diagnosis" | "preview";
export type ProjectActivePhase = "load" | "review" | "prepare" | "deliver";
