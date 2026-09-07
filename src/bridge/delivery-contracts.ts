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
