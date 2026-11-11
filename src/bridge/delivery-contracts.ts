export interface OperationProgress {
  operation: "load" | "profile" | "export" | "prepare";
  stage: string;
  percent: number;
}

export type CancellableOperation =
  | OperationProgress["operation"]
  | "prepare"
  | "query"
  | "datasetPage"
  | "temporal"
  | "snapshotComparison"
  | "datasetComparison"
  | "qualityValidation"
  | "databasePreflight"
  | "databaseConnection"
  | "updateCheck"
  | "reviewMutation"
  | "projectCatalog"
  | "projectVersions"
  | "projectDelete"
  | "projectSave"
  | "projectOpen"
  | "reusableTaskCatalog"
  | "deliveryPresetCatalog";
export type LocalExportFormat =
  | "csv" | "csv_excel" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle"
  | "sqlserver_script" | "sqlserver_bulk";
export type ExportFormat = LocalExportFormat | "postgresql" | "mysql" | "sqlserver";
export type PrivacyMode = "none" | "mask" | "hash";
export type ConflictSource = "current" | "compared";
export type DatabaseKind = "postgresql" | "mysql" | "sqlserver";
type DatabaseTablePolicy = "append" | "create_only" | "replace";
type DatabaseAuthentication = "windows" | "password";

/**
 * The fields of a remote connection. Rust builds the ODBC connection string
 * from them and checks every field; the interface never sends a string.
 * It lives only in the current React/Tauri call and is never persisted.
 */
export interface DatabaseConnection {
  /** The name of an installed ODBC driver. */
  driver: string;
  /** On SQL Server, host, host,port or host\instance. */
  server: string;
  /** PostgreSQL and MySQL only; empty is the engine's usual port. */
  port: string;
  database: string;
  /** Windows sign-in is SQL Server only. */
  authentication: DatabaseAuthentication;
  user: string;
  password: string;
  /** SQL Server only: accept any certificate of the server. */
  trustServerCertificate: boolean;
  /** PostgreSQL and MySQL only: connect without encryption if the server offers none. */
  allowUnencrypted: boolean;
}

type SqlServerLoadMode = "create" | "append" | "upsert";
type SqlServerTextSize = "fitted" | "max";

/**
 * «Script T-SQL» and «Paquete BULK INSERT» (2026-10-31): the table the script
 * fills and what it does with it. Empty names take `dbo` and the file name.
 */
export interface SqlServerScriptOptions {
  schema: string;
  table: string;
  mode: SqlServerLoadMode;
  /** Only for `upsert`: the columns that identify a row. */
  keyColumns: string[];
  /** `fitted` sizes text columns from the data; `max` keeps NVARCHAR(MAX). */
  textSize: SqlServerTextSize;
}

/** Credentials remain in the current React/Tauri call and are never persisted. */
export interface DatabaseTarget {
  kind: DatabaseKind;
  connection: DatabaseConnection;
  schema: string;
  table: string;
  tablePolicy: DatabaseTablePolicy;
}

export type ConflictResolution =
  | { action: "exclude"; conflictIndex: number }
  | {
      action: "useSource";
      conflictIndex: number;
      /** Omit only for compatibility with the legacy whole-row decision. */
      column?: string;
      source: ConflictSource;
    };

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

interface QualityCondition {
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

interface QualityRuleResult extends QualityRule {
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

interface QualityMigrationWarning {
  ruleIndex: number;
  sourceKind: string;
  severity: "warning" | "omitted";
  message: string;
}

interface QualityMigrationReport {
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
  /** Excel only: cells whose control characters became U+FFFD (RV19). */
  replacedControlCellCount: number;
  /** PROD-13: rows written. */
  rowCount?: number;
  /** PROD-13: the destination folder's own name, never its path. */
  folderName?: string;
  /** PROD-13: CSV cells quoted so a spreadsheet does not run them as formulas. */
  formulaProtectedCellCount?: number;
}

export interface DatabaseConnectionResult {
  kind: DatabaseKind;
  message: string;
}

interface RemotePreflightIssue {
  severity: "info" | "warning" | "blocking";
  category: "policy" | "type" | "nullability" | "length" | "value";
  column: string | null;
  message: string;
}

export interface RemoteExportPreflight {
  kind: DatabaseKind;
  schema: string;
  table: string;
  tablePolicy: DatabaseTablePolicy;
  tableExists: boolean;
  ready: boolean;
  issues: RemotePreflightIssue[];
}

/** Local preset: destination credentials and current-session overwrite approval are excluded. */
interface DeliveryPresetDatabaseTarget {
  kind: DatabaseKind;
  schema: string;
  table: string;
  tablePolicy: DatabaseTablePolicy;
}

export interface DeliveryPreset {
  version: 1;
  name: string;
  format: ExportFormat;
  selectedColumns: string[];
  privacyMode: PrivacyMode;
  databaseTarget?: DeliveryPresetDatabaseTarget;
  /** Only with «Script T-SQL» and «Paquete BULK INSERT». */
  sqlServerScript?: SqlServerScriptOptions;
}

export interface DeliveryPresetSummary {
  id: string;
  name: string;
  format: ExportFormat;
  updatedAt: string;
  selectedColumnCount: number;
  remote: boolean;
}
