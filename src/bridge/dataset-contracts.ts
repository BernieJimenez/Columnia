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
