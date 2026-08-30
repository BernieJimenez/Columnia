use std::{
    collections::HashMap,
    collections::HashSet,
    collections::VecDeque,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex,
    },
};

use ::zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
use calamine::{open_workbook_auto, Data, DataType as CalamineDataType, Range, Reader};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use polars::io::json::{JsonFormat, JsonWriter};
use polars::lazy::dsl::{col, len, lit};
use polars::prelude::*;
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::dataset_fingerprints::{
    normalized_fingerprint_columns, normalized_row_fingerprint, row_fingerprint,
    NormalizedRowFingerprint,
};

const PREVIEW_ROW_LIMIT: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const MAX_QUERY_CHARS: usize = 2 * 1024;
const MAX_CONFLICT_PREVIEW: usize = 50;
const HIGH_NULL_COLUMN_THRESHOLD_PERCENTAGE: usize = 80;
const SENTINEL_VALUES: &[&str] = &[
    "",
    "na",
    "n/a",
    "nan",
    "n.a.",
    "n.a",
    "null",
    "(null)",
    "none",
    "nil",
    "-",
    "--",
    "?",
    "??",
    "unknown",
    "unk",
    "missing",
    "not available",
    "not applicable",
    "sin dato",
    "sin datos",
    "s/d",
    "n/d",
    "no disponible",
    "desconocido",
    "desconocida",
    "#n/a",
    "(blank)",
    "(vacio)",
];
const MOJIBAKE_MARKERS: &[&str] = &[
    "â€™", "â€œ", "â€", "Ã©", "Ã¨", "Ã ", "Ã¢", "Ã®", "Ã´", "Ã³", "Ã±", "Ã¼", "Ã¡", "Ã\u{AD}",
    "Ãº", "â€”", "â€¦",
];
const OPERATION_CANCELLED_MESSAGE: &str = "Operación cancelada por el usuario.";
const REDACTED_VALUE: &str = "[REDACTED]";
const DELIMITED_SAMPLE_BYTES: u64 = 64 * 1024;
const HISTORY_MAX_ENTRIES: usize = 12;
const HISTORY_DISK_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_AUDIT_CELL_CHARS: usize = 2048;
const RECIPE_FILE_VERSION: u32 = 1;
const RECIPE_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const MAX_SESSION_METADATA_NAMES: usize = 64;
const MAX_SESSION_METADATA_NAME_CHARS: usize = 96;
const MAX_SESSION_SAMPLE_ROWS: usize = 3_000_000;
const MAX_SESSION_EXECUTION_HISTORY_ENTRIES: usize = 5;
const MAX_SESSION_EXECUTION_DURATION_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_RECIPE_TEXT_FIELD_CHARS: usize = 4 * 1024;
const MAX_RECIPE_TOTAL_TEXT_CHARS: usize = 64 * 1024;
const NORMALIZED_DUPLICATE_CHUNK_ROWS: usize = 262_144;
// Fingerprints are spilled into fixed buckets before sorting. Equal values
// always land in the same bucket, while the in-memory sort only holds one
// bucket instead of one entry per dataset row.
const NORMALIZED_DUPLICATE_BUCKETS: usize = 256;
const NORMALIZED_FINGERPRINT_BYTES: usize = std::mem::size_of::<NormalizedRowFingerprint>();
const NUMERIC_HISTOGRAM_BUCKETS: usize = 12;
const MAX_NUMERIC_CORRELATION_COLUMNS: usize = 12;
const MAX_NUMERIC_CORRELATION_SAMPLE_ROWS: usize = 100_000;
const MAX_CATEGORICAL_GROUP_COLUMNS: usize = 4;
const MAX_CATEGORICAL_GROUPS: usize = 8;
const MAX_GROUP_CANDIDATES: usize = 2_048;
const MAX_GROUP_LABEL_CHARS: usize = 120;
const MIN_GROUP_COUNT: usize = 3;
const MAX_TEMPORAL_COLUMNS: usize = 4;
const MAX_TEMPORAL_PERIODS: usize = 48;
const MAX_TEMPORAL_DAY_SPAN: i64 = 90;
const MAX_TEMPORAL_MONTH_SPAN: i64 = 36;
const LOCAL_QUERY_BLOCK_ROWS: usize = 16 * 1024;
const LOCAL_QUERY_CANCEL_CHECK_ROWS: usize = 4096;
const LOCAL_QUERY_JOIN_MAX_INPUT_ROWS: usize = 2_000_000;
const LOCAL_QUERY_JOIN_MAX_RESULT_ROWS: usize = 2_000_000;
const LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS: usize = 2_000_000;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    operation: &'static str,
    stage: &'static str,
    percent: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Parquet,
    Sql,
    Excel,
    Sqlite,
    Bundle,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Parquet => "parquet",
            Self::Sql => "sql",
            Self::Excel => "xlsx",
            Self::Sqlite => "sqlite",
            Self::Bundle => "zip",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Parquet => "Parquet",
            Self::Sql => "SQL",
            Self::Excel => "Excel",
            Self::Sqlite => "SQLite",
            Self::Bundle => "Paquete Columnia",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyMode {
    None,
    Mask,
    Hash,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub(crate) file_name: String,
    pub(crate) file_size_bytes: u64,
    pub(crate) format: &'static str,
    pub(crate) protected_column_count: usize,
    pub(crate) protected_columns: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleFileManifest {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleManifest {
    format: String,
    version: u8,
    dataset_file: String,
    dataset_format: String,
    row_count: usize,
    column_count: usize,
    dictionary_file: String,
    quality_report_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recipe_file: Option<String>,
    files: Vec<BundleFileManifest>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleDictionary {
    format: String,
    version: u8,
    columns: Vec<BundleDictionaryColumn>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleDictionaryColumn {
    name: String,
    data_type: String,
    null_count: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleQualityReport {
    format: String,
    version: u8,
    passed: bool,
    row_count: usize,
    total_rules: usize,
    failed_rules: usize,
    rules: Vec<BundleQualityRuleReport>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BundleQualityRuleReport {
    column: String,
    kind: QualityRuleKind,
    checked_count: usize,
    invalid_count: usize,
    invalid_pct: f64,
    passed: bool,
}

const MAX_QUALITY_RULES: usize = 16;
const MAX_QUALITY_COLUMN_CHARS: usize = 256;
const MAX_QUALITY_TOTAL_TEXT_CHARS: usize = 2 * 1024;
const MAX_QUALITY_VALUES: usize = 128;
const MAX_QUALITY_COLUMNS_PER_RULE: usize = 16;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const QUALITY_DATASET_COLUMN: &str = "__dataset__";
const QUALITY_MIGRATION_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const QUALITY_RULES_DOCUMENT_FORMAT: &str = "columnia-quality-rules";
const QUALITY_RULES_DOCUMENT_VERSION: u8 = 1;
const DATAPREP_QUALITY_DOCUMENT_MAX_VERSION: u8 = 3;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityRuleKind {
    NotNull,
    NonEmpty,
    Unique,
    NumericRange,
    AllowedValues,
    Regex,
    Dtype,
    UniqueTogether,
    ColumnCompare,
    ReferentialIntegrity,
    Monotonic,
    AggregateCheck,
    AggregateReconciliation,
    DistributionDrift,
    DateRange,
    Conditional,
    SchemaContract,
    RowCount,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityComparison {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityMonotonicDirection {
    Increasing,
    Decreasing,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityAggregate {
    Count,
    Sum,
    Min,
    Max,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityCondition {
    column: String,
    operator: Option<QualityComparison>,
    value: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityRule {
    column: String,
    kind: QualityRuleKind,
    max_invalid: Option<usize>,
    max_invalid_pct: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    values: Option<Vec<String>>,
    reference_values: Option<Vec<String>>,
    baseline: Option<Vec<String>>,
    direction: Option<QualityMonotonicDirection>,
    expected: Option<f64>,
    aggregate: Option<QualityAggregate>,
    tolerance_abs: Option<f64>,
    tolerance_rel: Option<f64>,
    threshold: Option<f64>,
    pattern: Option<String>,
    dtype: Option<String>,
    columns: Option<Vec<String>>,
    operator: Option<QualityComparison>,
    min_date: Option<String>,
    max_date: Option<String>,
    when: Option<QualityCondition>,
    then: Option<Box<QualityRule>>,
    allow_additional: Option<bool>,
    required_order: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityRuleResult {
    column: String,
    kind: QualityRuleKind,
    max_invalid: Option<usize>,
    max_invalid_pct: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    values: Option<Vec<String>>,
    reference_values: Option<Vec<String>>,
    baseline: Option<Vec<String>>,
    direction: Option<QualityMonotonicDirection>,
    expected: Option<f64>,
    aggregate: Option<QualityAggregate>,
    tolerance_abs: Option<f64>,
    tolerance_rel: Option<f64>,
    threshold: Option<f64>,
    pattern: Option<String>,
    dtype: Option<String>,
    columns: Option<Vec<String>>,
    operator: Option<QualityComparison>,
    min_date: Option<String>,
    max_date: Option<String>,
    when: Option<QualityCondition>,
    then: Option<Box<QualityRule>>,
    allow_additional: Option<bool>,
    required_order: Option<Vec<String>>,
    checked_count: usize,
    invalid_count: usize,
    invalid_pct: f64,
    passed: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityValidationResult {
    pub(crate) passed: bool,
    pub(crate) row_count: usize,
    pub(crate) total_rules: usize,
    pub(crate) failed_rules: usize,
    rules: Vec<QualityRuleResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationWarning {
    rule_index: usize,
    source_kind: String,
    severity: &'static str,
    message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationReport {
    artifact_sha256: Option<String>,
    total_items: usize,
    converted_items: usize,
    omitted_items: usize,
    warning_count: usize,
    manual_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationResult {
    source_format: &'static str,
    source_version: Option<String>,
    converted_rules: Vec<QualityRule>,
    warnings: Vec<QualityMigrationWarning>,
    omitted_rules: usize,
    report: QualityMigrationReport,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityRulesDocument {
    format: String,
    version: u8,
    rules: Vec<QualityRule>,
}

impl QualityValidationResult {
    pub(crate) fn total_invalid_count(&self) -> usize {
        self.rules.iter().map(|rule| rule.invalid_count).sum()
    }
}

fn send_progress(
    channel: &Channel<OperationProgress>,
    operation: &'static str,
    stage: &'static str,
    percent: u8,
) {
    let _ = channel.send(OperationProgress {
        operation,
        stage,
        percent,
    });
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetColumn {
    pub(crate) name: String,
    pub(crate) data_type: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetPreview {
    pub(crate) file_name: String,
    file_size_bytes: u64,
    pub(crate) row_count: usize,
    pub(crate) column_count: usize,
    pub(crate) columns: Vec<DatasetColumn>,
    rows: Vec<Vec<Option<String>>>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetQueryResult {
    columns: Vec<DatasetColumn>,
    row_count: usize,
    offset: usize,
    rows: Vec<Vec<Option<String>>>,
    truncated: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetComparison {
    pub(crate) current_file_name: String,
    pub(crate) compared_file_name: String,
    pub(crate) current_row_count: usize,
    pub(crate) compared_row_count: usize,
    pub(crate) common_row_count: usize,
    pub(crate) current_only_row_count: usize,
    pub(crate) compared_only_row_count: usize,
    pub(crate) shared_columns: Vec<String>,
    pub(crate) current_only_columns: Vec<String>,
    pub(crate) compared_only_columns: Vec<String>,
    pub(crate) schema_compatible: bool,
    pub(crate) key_columns: Vec<String>,
    pub(crate) matched_key_count: usize,
    pub(crate) current_only_key_count: usize,
    pub(crate) compared_only_key_count: usize,
    pub(crate) conflicting_key_count: usize,
    pub(crate) duplicate_key_count: usize,
    pub(crate) conflicts: Vec<DatasetConflict>,
    pub(crate) conflict_offset: usize,
    pub(crate) conflicts_truncated: bool,
    pub(crate) can_consolidate: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetConflict {
    pub(crate) key: Vec<Option<String>>,
    pub(crate) cells: Vec<DatasetConflictCell>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetConflictCell {
    pub(crate) column: String,
    pub(crate) current: Option<String>,
    pub(crate) compared: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetConflictPage {
    pub(crate) offset: usize,
    pub(crate) conflicts: Vec<DatasetConflict>,
    pub(crate) has_next: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictSource {
    Current,
    Compared,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictResolution {
    conflict_index: usize,
    #[serde(default)]
    column: Option<String>,
    source: ConflictSource,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatasetJoinType {
    Inner,
    Left,
    Full,
}

impl DatasetJoinType {
    fn polars_type(self) -> JoinType {
        match self {
            Self::Inner => JoinType::Inner,
            Self::Left => JoinType::Left,
            Self::Full => JoinType::Full,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Left => "left",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookSheet {
    id: String,
    name: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSourceInspection {
    selection_id: String,
    file_name: String,
    file_size_bytes: u64,
    format: &'static str,
    sheets: Vec<WorkbookSheet>,
    default_sheet_id: Option<String>,
    is_compressed_container: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SampleDatasetDescriptor {
    id: String,
    name: String,
    format: String,
    description: String,
}

struct SampleDatasetDefinition {
    id: &'static str,
    name: &'static str,
    format: &'static str,
    description: &'static str,
    file_name: &'static str,
    content: &'static str,
}

const SAMPLE_DATASETS: &[SampleDatasetDefinition] = &[
    SampleDatasetDefinition {
        id: "quality",
        name: "Clientes · señales de calidad",
        format: "csv",
        description: "Nulos, identificadores y fechas para probar Revisar y Preparar.",
        file_name: "clientes_calidad.csv",
        content: "cliente_id,nombre,segmento,alta,monto\n1001,Ana Torres,Pyme,2025-01-14,1250.50\n1002,Luis Pérez,Empresa,2025-02-03,980.00\n,Cuenta sin identificador,Pyme,,450.00\n1004,María Rojas,Pyme,2025-02-28,\n",
    },
    SampleDatasetDefinition {
        id: "temporal",
        name: "Ventas · serie temporal",
        format: "tsv",
        description: "Una serie temporal pequeña para probar análisis y tendencias.",
        file_name: "ventas_mensuales.tsv",
        content: "fecha\tregion\tproducto\tunidades\tingresos\n2025-01-01\tNorte\tLicencia\t18\t2160\n2025-02-01\tNorte\tLicencia\t24\t2880\n2025-03-01\tSur\tSoporte\t15\t1800\n2025-04-01\tSur\tLicencia\t31\t3720\n2025-05-01\tNorte\tSoporte\t22\t2640\n",
    },
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpreadsheetHeaderMode {
    FirstRow,
    Generated,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetPage {
    offset: usize,
    rows: Vec<Vec<Option<String>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistogramBucket {
    lower: f64,
    upper: f64,
    count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericCorrelation {
    first_column: String,
    second_column: String,
    coefficient: Option<f64>,
    sample_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericCorrelationMatrix {
    columns: Vec<String>,
    pairs: Vec<NumericCorrelation>,
    sampled_row_count: usize,
    truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoricalGroup {
    label: String,
    row_count: usize,
    percentage: f64,
    is_other: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoricalGroupSummary {
    column: String,
    groups: Vec<CategoricalGroup>,
    distinct_count: usize,
    truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemporalPeriod {
    period: String,
    row_count: usize,
    percentage: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemporalSeriesSummary {
    column: String,
    granularity: String,
    periods: Vec<TemporalPeriod>,
    parsed_row_count: usize,
    unparsed_row_count: usize,
    truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnProfile {
    name: String,
    data_type: String,
    null_count: usize,
    completeness_percentage: f64,
    unique_count: usize,
    minimum: Option<String>,
    maximum: Option<String>,
    mean: Option<f64>,
    empty_count: Option<usize>,
    minimum_length: Option<usize>,
    maximum_length: Option<usize>,
    average_length: Option<f64>,
    suggested_type: Option<String>,
    type_match_percentage: Option<f64>,
    invalid_type_count: Option<usize>,
    #[serde(default)]
    sentinel_count: Option<usize>,
    #[serde(default)]
    encoding_issue_count: Option<usize>,
    #[serde(default)]
    privacy_signal: Option<String>,
    standard_deviation: Option<f64>,
    first_quartile: Option<f64>,
    median: Option<f64>,
    third_quartile: Option<f64>,
    outlier_count: Option<usize>,
    #[serde(default)]
    histogram: Option<Vec<HistogramBucket>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetProfile {
    row_count: usize,
    duplicate_row_count: usize,
    #[serde(default)]
    near_duplicate_row_count: usize,
    duplicate_percentage: f64,
    columns: Vec<ColumnProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    numeric_correlations: Option<NumericCorrelationMatrix>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    categorical_group_summaries: Option<Vec<CategoricalGroupSummary>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temporal_series: Option<Vec<TemporalSeriesSummary>>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetMutation {
    dataset: DatasetPreview,
    affected_row_count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRemovalResult {
    dataset: DatasetPreview,
    removed_column_count: usize,
    removed_columns: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRename {
    from: String,
    to: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnNormalizationResult {
    dataset: DatasetPreview,
    renamed_column_count: usize,
    renames: Vec<ColumnRename>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedTextColumn {
    name: String,
    changed_cell_count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextCleaningResult {
    dataset: DatasetPreview,
    affected_row_count: usize,
    changed_cell_count: usize,
    changed_columns: Vec<ChangedTextColumn>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDataMaskResult {
    dataset: DatasetPreview,
    changed_cell_count: usize,
    changed_column_count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResult {
    dataset: DatasetPreview,
    history: HistoryState,
    message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryState {
    index: usize,
    label: String,
    is_current: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    can_undo: bool,
    can_redo: bool,
    current_index: usize,
    entry_count: usize,
    entries: Vec<HistoryEntryState>,
    snapshots_enabled: bool,
    degraded_reason: Option<String>,
    max_entries: usize,
    disk_bytes: u64,
    disk_budget_bytes: u64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SafeCorrectionsResult {
    dataset: DatasetPreview,
    changed_cell_count: usize,
    affected_row_count: usize,
    renamed_column_count: usize,
    renames: Vec<ColumnRename>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeRename {
    from: String,
    to: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeCastTarget {
    String,
    Integer,
    Decimal,
    Boolean,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeCast {
    column: String,
    target: RecipeCastTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeDateFormat {
    Ymd,
    Dmy,
    Mdy,
    Iso8601,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeDateTarget {
    Date,
    Datetime,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeDateParse {
    column: String,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeFilterOperator {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    NotContains,
    IsNull,
    NotNull,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeFilter {
    column: String,
    operator: RecipeFilterOperator,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CalculatedOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Concat,
    Year,
    Month,
    Day,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CalculatedOperandKind {
    Literal,
    Column,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculatedOperand {
    kind: CalculatedOperandKind,
    value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculatedColumnRecipe {
    name: String,
    source: String,
    operation: CalculatedOperation,
    #[serde(default)]
    operand: Option<CalculatedOperand>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindReplaceScope {
    Column,
    AllTextColumns,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindReplaceRecipe {
    scope: FindReplaceScope,
    column: Option<String>,
    find: String,
    replace: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SplitColumnRecipe {
    source: String,
    delimiter: String,
    names: Vec<String>,
    drop_source: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeColumnsRecipe {
    sources: Vec<String>,
    name: String,
    separator: String,
    drop_sources: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutlierAction {
    Cap,
    Drop,
    Impute,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutlierTreatment {
    column: String,
    action: OutlierAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SummaryOperation {
    Sum,
    Mean,
    Min,
    Max,
    Count,
    CountUnique,
}

impl SummaryOperation {
    fn suffix(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Mean => "mean",
            Self::Min => "min",
            Self::Max => "max",
            Self::Count => "count",
            Self::CountUnique => "count_unique",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SummaryAggregation {
    column: String,
    operation: SummaryOperation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSummaryRecipe {
    group_by: Vec<String>,
    aggregations: Vec<SummaryAggregation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContactKind {
    Email,
    Phone,
    Address,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContactNormalization {
    column: String,
    kind: ContactKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionKind {
    FirstToken,
    LastToken,
    Digits,
    Letters,
    Before,
    After,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextExtraction {
    source: String,
    kind: ExtractionKind,
    name: String,
    delimiter: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransformRecipe {
    #[serde(default)]
    renames: Vec<RecipeRename>,
    #[serde(default)]
    casts: Vec<RecipeCast>,
    #[serde(default)]
    date_parses: Vec<RecipeDateParse>,
    #[serde(default)]
    filters: Vec<RecipeFilter>,
    #[serde(default)]
    calculated_column: Option<CalculatedColumnRecipe>,
    #[serde(default)]
    find_replace: Option<FindReplaceRecipe>,
    #[serde(default)]
    keep_columns: Option<Vec<String>>,
    #[serde(default)]
    split_column: Option<SplitColumnRecipe>,
    #[serde(default)]
    merge_columns: Option<MergeColumnsRecipe>,
    #[serde(default)]
    outlier_treatments: Vec<OutlierTreatment>,
    #[serde(default)]
    group_summary: Option<GroupSummaryRecipe>,
    #[serde(default)]
    contact_normalizations: Vec<ContactNormalization>,
    #[serde(default)]
    text_extractions: Vec<TextExtraction>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeExportOptions {
    formats: Vec<ExportFormat>,
    selected_columns: Vec<String>,
    privacy_mode: PrivacyMode,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeMigrationWarning {
    path: String,
    severity: String,
    message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionMigrationMetadata {
    has_source_reference: bool,
    has_snapshot_reference: bool,
    sheet_name: Option<String>,
    stage_label: Option<String>,
    applied_operation_count: usize,
    quality_rule_count: usize,
    analysis_check_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    applied_operations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    analysis_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    analysis_sampled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    analysis_sample_row_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    analysis_total_row_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    non_portable_artifacts: Vec<String>,
}

type SessionSampleMetadataValues = (Option<bool>, Option<usize>, Option<usize>);

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionReferenceStatus {
    NotProvided,
    Available,
    Missing,
    Unsupported,
}

/// A read-only migration decision. It deliberately contains no resolved paths so it can
/// cross the IPC boundary without leaking the user's filesystem layout.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DataprepSessionMigrationPlan {
    pub(crate) name: String,
    pub(crate) source_file_name: Option<String>,
    pub(crate) source_status: SessionReferenceStatus,
    pub(crate) snapshot_status: SessionReferenceStatus,
    pub(crate) sheet_name: Option<String>,
    pub(crate) stage_label: Option<String>,
    pub(crate) recipe: StoredTransformRecipe,
    pub(crate) quality_rules: Vec<QualityRule>,
    pub(crate) quality_report: Option<QualityMigrationReport>,
    pub(crate) missing_references: Vec<String>,
    pub(crate) collisions: Vec<String>,
    pub(crate) can_create_project: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionExecutionHistoryEntry {
    pub(crate) outcome: String,
    pub(crate) duration_ms: u64,
    pub(crate) row_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeMigrationReport {
    artifact_sha256: Option<String>,
    source_format: String,
    source_version: Option<u64>,
    converted_items: usize,
    omitted_items: usize,
    warning_count: usize,
    converted_operations: Vec<String>,
    omitted_operations: Vec<String>,
    warnings: Vec<RecipeMigrationWarning>,
    manual_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session: Option<SessionMigrationMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredTransformRecipe {
    pub version: u32,
    pub name: String,
    pub saved_at: String,
    pub recipe: TransformRecipe,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_options: Option<RecipeExportOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_report: Option<RecipeMigrationReport>,
}

impl StoredTransformRecipe {
    pub(crate) fn session_applied_operations(&self) -> Vec<String> {
        self.migration_report
            .as_ref()
            .and_then(|report| report.session.as_ref())
            .map(|session| session.applied_operations.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformRecipeResult {
    dataset: DatasetPreview,
    renamed_column_count: usize,
    converted_column_count: usize,
    parsed_date_column_count: usize,
    removed_row_count: usize,
    calculated_column_count: usize,
    replaced_cell_count: usize,
    dropped_column_count: usize,
    split_column_count: usize,
    merged_column_count: usize,
    dropped_source_column_count: usize,
    adjusted_outlier_cell_count: usize,
    outlier_removed_row_count: usize,
    outlier_column_count: usize,
    group_count: usize,
    aggregated_column_count: usize,
    collapsed_row_count: usize,
    normalized_contact_cell_count: usize,
    normalized_contact_column_count: usize,
    extracted_column_count: usize,
    changed: bool,
}

struct LoadedDataset {
    source_path: Option<PathBuf>,
    file_name: String,
    file_size_bytes: u64,
    frame: DataFrame,
    profile: Option<DatasetProfile>,
    history: HistoryManager,
}

#[derive(Debug)]
struct HistoryEntry {
    label: String,
    path: PathBuf,
    bytes: u64,
}

#[derive(Debug)]
struct HistoryManager {
    directory: tempfile::TempDir,
    entries: Vec<HistoryEntry>,
    cursor: usize,
    snapshots_enabled: bool,
    degraded_reason: Option<String>,
    current_label: String,
    next_id: u64,
    max_entries: usize,
    disk_budget_bytes: u64,
}

impl HistoryManager {
    fn new(frame: &DataFrame) -> Result<Self, String> {
        Self::with_limits(frame, HISTORY_MAX_ENTRIES, HISTORY_DISK_BUDGET_BYTES)
    }

    fn with_limits(
        frame: &DataFrame,
        max_entries: usize,
        disk_budget_bytes: u64,
    ) -> Result<Self, String> {
        let directory = tempfile::tempdir()
            .map_err(|error| format!("No se pudo crear el historial temporal: {error}"))?;
        let mut manager = Self {
            directory,
            entries: Vec::new(),
            cursor: 0,
            snapshots_enabled: true,
            degraded_reason: None,
            current_label: "Dataset original".to_owned(),
            next_id: 0,
            max_entries: max_entries.max(1),
            disk_budget_bytes,
        };
        manager.record(frame, "Dataset original")?;
        Ok(manager)
    }

    fn disk_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    fn state(&self) -> HistoryState {
        let entries = if self.snapshots_enabled {
            self.entries
                .iter()
                .enumerate()
                .map(|(index, entry)| HistoryEntryState {
                    index,
                    label: entry.label.clone(),
                    is_current: index == self.cursor,
                })
                .collect()
        } else {
            vec![HistoryEntryState {
                index: 0,
                label: self.current_label.clone(),
                is_current: true,
            }]
        };
        let entry_count = entries.len();
        HistoryState {
            can_undo: self.snapshots_enabled && self.cursor > 0,
            can_redo: self.snapshots_enabled && self.cursor + 1 < self.entries.len(),
            current_index: if self.snapshots_enabled {
                self.cursor
            } else {
                0
            },
            entry_count,
            entries,
            snapshots_enabled: self.snapshots_enabled,
            degraded_reason: self.degraded_reason.clone(),
            max_entries: self.max_entries,
            disk_bytes: self.disk_bytes(),
            disk_budget_bytes: self.disk_budget_bytes,
        }
    }

    fn disable_for_size(&mut self, label: &str, bytes: u64) {
        for entry in self.entries.drain(..) {
            let _ = fs::remove_file(entry.path);
        }
        self.cursor = 0;
        self.snapshots_enabled = false;
        self.current_label = label.to_owned();
        self.degraded_reason = Some(format!(
            "El snapshot requiere {bytes} bytes y supera el límite local de {} bytes. El cambio se aplicó sin historial reversible.",
            self.disk_budget_bytes
        ));
    }

    fn record(&mut self, frame: &DataFrame, label: &str) -> Result<(), String> {
        self.current_label = label.to_owned();
        if !self.snapshots_enabled {
            return Ok(());
        }

        let temporary = tempfile::NamedTempFile::new_in(self.directory.path())
            .map_err(|error| format!("No se pudo preparar el snapshot del historial: {error}"))?;
        let mut snapshot = frame.clone();
        ParquetWriter::new(temporary.as_file())
            .finish(&mut snapshot)
            .map_err(|error| format!("No se pudo escribir el snapshot del historial: {error}"))?;
        temporary.as_file().sync_all().map_err(|error| {
            format!("No se pudo sincronizar el snapshot del historial: {error}")
        })?;
        let bytes = temporary
            .as_file()
            .metadata()
            .map_err(|error| format!("No se pudo verificar el snapshot del historial: {error}"))?
            .len();
        if bytes > self.disk_budget_bytes {
            drop(temporary);
            self.disable_for_size(label, bytes);
            return Ok(());
        }

        let destination = self
            .directory
            .path()
            .join(format!("snapshot-{:020}.parquet", self.next_id));
        temporary.persist(&destination).map_err(|error| {
            format!(
                "No se pudo publicar el snapshot del historial: {}",
                error.error
            )
        })?;

        // Solo después de publicar el snapshot se descarta una posible rama de rehacer.
        let branch_start = self.cursor.saturating_add(1).min(self.entries.len());
        let removed = self.entries.split_off(branch_start);
        for entry in removed {
            let _ = fs::remove_file(entry.path);
        }
        self.entries.push(HistoryEntry {
            label: label.to_owned(),
            path: destination,
            bytes,
        });
        self.cursor = self.entries.len() - 1;
        self.next_id = self.next_id.wrapping_add(1);

        while self.entries.len() > self.max_entries || self.disk_bytes() > self.disk_budget_bytes {
            let entry = self.entries.remove(0);
            let _ = fs::remove_file(entry.path);
            self.cursor = self.cursor.saturating_sub(1);
        }
        Ok(())
    }

    fn restore(&self, index: usize) -> Result<DataFrame, String> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| "La revisión solicitada ya no está disponible.".to_owned())?;
        read_parquet_frame(&entry.path)
            .map_err(|error| format!("No se pudo restaurar el snapshot del historial: {error}"))
    }
}

fn publish_candidate(
    dataset: &mut LoadedDataset,
    mut candidate: DataFrame,
    label: &str,
) -> Result<DatasetPreview, String> {
    let audit_active = dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios");
    if audit_active && label != "Activar trazabilidad por fila" {
        append_audit_label(&mut candidate, label)?;
    }
    let preview = loaded_dataset_preview(dataset, &candidate)?;
    dataset.history.record(&candidate, label)?;
    dataset.frame = candidate;
    dataset.profile = None;
    Ok(preview)
}

fn append_audit_label(frame: &mut DataFrame, label: &str) -> Result<(), String> {
    let has_audit = frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios");
    if !has_audit {
        frame
            .with_column(Column::new(
                "_cambios".into(),
                vec![Some(label.to_owned()); frame.height()],
            ))
            .map_err(|error| format!("No se pudo conservar la trazabilidad _cambios: {error}"))?;
        return Ok(());
    }

    let audit = frame
        .column("_cambios")
        .map_err(|_| "La columna de trazabilidad _cambios no está disponible.".to_owned())?
        .str()
        .map_err(|_| "La columna de trazabilidad _cambios debe ser texto.".to_owned())?;
    let values = audit
        .iter()
        .map(|previous| {
            let next = match previous {
                Some(previous) if !previous.trim().is_empty() => format!("{previous}; {label}"),
                _ => label.to_owned(),
            };
            Some(next.chars().take(MAX_AUDIT_CELL_CHARS).collect::<String>())
        })
        .collect::<Vec<_>>();
    frame
        .replace("_cambios", Column::new("_cambios".into(), values))
        .map_err(|error| format!("No se pudo actualizar la trazabilidad _cambios: {error}"))?;
    Ok(())
}

fn add_audit_column_to_frame(frame: &DataFrame) -> Result<(DataFrame, bool), String> {
    if frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios")
    {
        return Ok((frame.clone(), false));
    }

    let mut candidate = frame.clone();
    candidate
        .with_column(Column::new(
            "_cambios".into(),
            vec![None::<String>; frame.height()],
        ))
        .map_err(|error| format!("No se pudo activar la trazabilidad _cambios: {error}"))?;
    Ok((candidate, true))
}

#[derive(Clone)]
struct PendingSelection {
    id: String,
    path: PathBuf,
    file_size_bytes: u64,
    sheets: Vec<String>,
}

struct PendingComparison {
    file_name: String,
    file_size_bytes: u64,
    frame: DataFrame,
    key_columns: Vec<String>,
}

#[derive(Default)]
pub struct DatasetState {
    current: Mutex<Option<LoadedDataset>>,
    pending_selection: Mutex<Option<PendingSelection>>,
    pending_drop: Mutex<Option<PathBuf>>,
    comparison: Mutex<Option<PendingComparison>>,
    last_export_path: Mutex<Option<PathBuf>>,
    load_generation: AtomicU64,
    profile_generation: AtomicU64,
    export_generation: AtomicU64,
    query_generation: AtomicU64,
}

impl DatasetState {
    pub(crate) fn queue_dropped_path(&self, path: PathBuf) {
        if let Ok(mut pending) = self.pending_drop.lock() {
            *pending = Some(path);
        }
    }

    pub(crate) fn take_dropped_path(&self) -> Result<Option<PathBuf>, String> {
        self.pending_drop
            .lock()
            .map_err(|_| "La selección arrastrada quedó bloqueada inesperadamente.".to_owned())
            .map(|mut pending| pending.take())
    }

    fn begin_load(&self) -> u64 {
        self.load_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    fn begin_profile(&self) -> u64 {
        self.profile_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    fn begin_export(&self) -> u64 {
        self.export_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    fn begin_query(&self) -> u64 {
        self.query_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    fn load_was_cancelled(&self, generation: u64) -> bool {
        self.load_generation.load(Ordering::SeqCst) != generation
    }

    fn profile_was_cancelled(&self, generation: u64) -> bool {
        self.profile_generation.load(Ordering::SeqCst) != generation
    }

    fn export_was_cancelled(&self, generation: u64) -> bool {
        self.export_generation.load(Ordering::SeqCst) != generation
    }

    fn query_was_cancelled(&self, generation: u64) -> bool {
        self.query_generation.load(Ordering::SeqCst) != generation
    }

    fn remember_last_export(&self, path: PathBuf) {
        if let Ok(mut last_export_path) = self.last_export_path.lock() {
            *last_export_path = Some(path);
        }
    }

    fn last_export(&self) -> Result<PathBuf, String> {
        self.last_export_path
            .lock()
            .map_err(|_| "La salida exportada no está disponible.".to_owned())?
            .clone()
            .ok_or_else(|| "Todavía no hay una exportación disponible para abrir.".to_owned())
    }

    fn cancel(&self, operation: &str) -> Result<(), String> {
        let generation = match operation {
            "load" => &self.load_generation,
            "profile" => &self.profile_generation,
            "export" => &self.export_generation,
            "query" => &self.query_generation,
            _ => return Err("La operación indicada no admite cancelación.".to_owned()),
        };
        generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn probe_seed_dataset(state: State<'_, DatasetState>) -> Result<DatasetPreview, String> {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), [1_i64, 2_i64]).into_column(),
            Series::new("value".into(), ["probe-a", "probe-b"]).into_column(),
        ],
    )
    .map_err(|error| format!("No se pudo preparar el dataset nativo de prueba: {error}"))?;
    let history = HistoryManager::new(&frame)?;
    let preview = dataset_preview_with_size("native-probe.csv", 96, &frame)?;
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos no está disponible.".to_owned())?;
    if current.is_some() {
        return Err("El probe nativo requiere una sesión de datos vacía.".to_owned());
    }
    *current = Some(LoadedDataset {
        source_path: None,
        file_name: "native-probe.csv".to_owned(),
        file_size_bytes: 96,
        frame,
        profile: None,
        history,
    });
    Ok(preview)
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn probe_save_transform_recipe(
    recipe: TransformRecipe,
    name: String,
) -> Result<StoredTransformRecipe, String> {
    let document = build_stored_recipe(recipe, name)?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el almacén temporal de receta: {error}")
        })?;
        let destination = recipe_path_with_extension(
            directory
                .path()
                .join(recipe_suggested_file_name(&document.name)),
        );
        save_recipe_atomic(&document, &destination)?;
        load_recipe_file(&destination)
    })
    .await
    .map_err(|error| format!("El guardado nativo de la receta se interrumpió: {error}"))?
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn probe_export_dataset(
    app: AppHandle,
    format: ExportFormat,
    quality_rules: Vec<QualityRule>,
    allow_unvalidated: bool,
) -> Result<ExportResult, String> {
    validate_quality_rules_payload(&quality_rules)?;
    let (frame, suggested_name) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos no está disponible.".to_owned())?;
        let dataset = current
            .as_ref()
            .ok_or_else(|| "No hay un dataset activo para el probe nativo.".to_owned())?;
        let stem = Path::new(&dataset.file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("dataset");
        (
            dataset.frame.clone(),
            format!("{stem}-native-probe.{}", format.extension()),
        )
    };

    let generation = app.state::<DatasetState>().begin_export();
    tauri::async_runtime::spawn_blocking(move || {
        enforce_export_quality_with_cancel(&frame, &quality_rules, allow_unvalidated, || {
            app.state::<DatasetState>().export_was_cancelled(generation)
        })?;
        ensure_not_cancelled(app.state::<DatasetState>().export_was_cancelled(generation))?;
        let directory = tempfile::tempdir()
            .map_err(|error| format!("No se pudo preparar el destino temporal: {error}"))?;
        let destination = path_with_extension(directory.path().join(suggested_name), format);
        export_frame_atomic(
            &frame,
            &destination,
            format,
            |_, _| {},
            || app.state::<DatasetState>().export_was_cancelled(generation),
        )
    })
    .await
    .map_err(|error| format!("La exportación nativa de prueba se interrumpió: {error}"))?
}

#[cfg(test)]
impl DatasetState {
    pub(crate) fn project_test_record(&self, frame: DataFrame, label: &str) -> Result<(), String> {
        let mut current = self.current.lock().map_err(|_| "lock".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        publish_candidate(dataset, frame, label).map(|_| ())
    }

    pub(crate) fn project_test_undo(&self) -> Result<DataFrame, String> {
        let mut current = self.current.lock().map_err(|_| "lock".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        undo_dataset(dataset)?;
        Ok(dataset.frame.clone())
    }

    pub(crate) fn project_test_redo(&self) -> Result<DataFrame, String> {
        let mut current = self.current.lock().map_err(|_| "lock".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        redo_dataset(dataset)?;
        Ok(dataset.frame.clone())
    }

    pub(crate) fn project_test_cache_profile(&self) -> Result<DatasetProfile, String> {
        let mut current = self.current.lock().map_err(|_| "lock".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        let profile = profile_dataset(&dataset.frame)?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    }

    pub(crate) fn project_test_degrade_history(&self) -> Result<(), String> {
        let mut current = self.current.lock().map_err(|_| "lock".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        dataset.history = HistoryManager::with_limits(&dataset.frame, HISTORY_MAX_ENTRIES, 0)?;
        Ok(())
    }
}

fn ensure_not_cancelled(cancelled: bool) -> Result<(), String> {
    if cancelled {
        Err(OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

fn dataset_extension(path: &Path) -> Result<String, String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|extension| {
            matches!(
                extension.as_str(),
                "csv"
                    | "tsv"
                    | "txt"
                    | "json"
                    | "jsonl"
                    | "ndjson"
                    | "parquet"
                    | "xlsx"
                    | "xls"
                    | "xlsb"
                    | "ods"
            )
        })
        .ok_or_else(|| {
            "Columnia admite CSV, TSV, TXT delimitado, JSON, Parquet y libros Excel/ODS en esta versión.".to_owned()
        })
}

#[cfg(not(windows))]
fn is_symbolic_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_symbolic_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn canonicalize_existing_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("No se pudo verificar {label}: {error}"))?;
    if is_symbolic_link_or_reparse_point(&metadata) {
        return Err(format!(
            "{label} no puede ser un enlace simbólico o punto de reanálisis."
        ));
    }
    if !metadata.is_file() {
        return Err(format!("{label} no existe o no es un archivo regular."));
    }

    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("No se pudo resolver la ruta real de {label}: {error}"))?;
    let canonical_metadata = fs::metadata(&canonical)
        .map_err(|error| format!("No se pudo verificar la ruta real de {label}: {error}"))?;
    if !canonical_metadata.is_file() {
        return Err(format!("{label} no apunta a un archivo regular."));
    }
    Ok(canonical)
}

fn canonicalize_write_destination(path: &Path, label: &str) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("No se pudo resolver el nombre de {label}."))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("No se pudo resolver la carpeta de {label}: {error}"))?;
    if !canonical_parent.is_dir() {
        return Err(format!("La carpeta de {label} no es un directorio válido."));
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) => {
            return Err(format!(
                "El destino de {label} no puede ser un enlace simbólico o punto de reanálisis."
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!("El destino de {label} no es un archivo regular."));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "No se pudo verificar el destino de {label}: {error}"
            ));
        }
    }

    Ok(canonical_parent.join(file_name))
}

fn validate_dataset_file(path: &Path) -> Result<(PathBuf, u64, String), String> {
    let canonical = canonicalize_existing_file(path, "el dataset seleccionado")?;
    let extension = dataset_extension(&canonical)?;

    let size = fs::metadata(&canonical)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();

    Ok((canonical, size, extension))
}

pub(crate) fn preview_value(value: AnyValue<'_>) -> Option<String> {
    match value {
        AnyValue::Null => None,
        AnyValue::String(value) => Some(value.to_owned()),
        AnyValue::StringOwned(value) => Some(value.as_str().to_owned()),
        value => Some(value.to_string()),
    }
}

fn dataset_page(frame: &DataFrame, offset: usize, limit: usize) -> Result<DatasetPage, String> {
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(format!(
            "El tamaño de página debe estar entre 1 y {MAX_PAGE_SIZE} filas."
        ));
    }

    if offset > frame.height() {
        return Err("La página solicitada está fuera del dataset activo.".into());
    }

    let end = offset.saturating_add(limit).min(frame.height());
    let rows = (offset..end)
        .map(|row_index| {
            frame
                .columns()
                .iter()
                .map(|column| {
                    column
                        .get(row_index)
                        .map_err(|error| format!("No se pudo preparar la vista previa: {error}"))
                        .map(preview_value)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DatasetPage { offset, rows })
}

#[derive(Clone, Copy, Debug)]
enum LocalAggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
}

#[derive(Clone, Debug)]
enum LocalProjection {
    Column {
        name: String,
        output_name: String,
    },
    Aggregate {
        function: LocalAggregate,
        column: Option<String>,
        output_name: String,
    },
}

#[derive(Clone, Copy, Debug)]
enum LocalPredicateOperator {
    IsNull,
    IsNotNull,
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Debug)]
struct LocalPredicate {
    column: String,
    operator: LocalPredicateOperator,
    value: Option<String>,
}

#[derive(Clone, Debug)]
struct LocalQueryPlan {
    projections: Vec<LocalProjection>,
    predicates: Vec<LocalPredicate>,
    group_by: Option<String>,
    offset: usize,
    limit: usize,
    aggregate: bool,
}

type LocalGroupRows = Vec<(Option<String>, Vec<usize>)>;

fn local_identifier(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let value = value[1..value.len() - 1].replace("\"\"", "\"");
        if !value.is_empty() {
            return Ok(value);
        }
    } else if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_' || character == '.')
    {
        return Ok(value.to_owned());
    }
    Err("La consulta solo permite nombres de columnas simples o entre comillas dobles.".to_owned())
}

fn split_local_sql_list(value: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quoted = false;
    let characters = value.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().enumerate() {
        match character {
            '\'' => {
                if quoted && characters.get(index + 1) == Some(&'\'') {
                    continue;
                }
                quoted = !quoted;
            }
            '(' if !quoted => depth = depth.saturating_add(1),
            ')' if !quoted => {
                if depth == 0 {
                    return Err("La lista de columnas tiene paréntesis desbalanceados.".to_owned());
                }
                depth -= 1;
            }
            ',' if !quoted && depth == 0 => {
                let part = characters[start..index].iter().collect::<String>();
                if part.trim().is_empty() {
                    return Err("La proyección contiene una expresión vacía.".to_owned());
                }
                parts.push(part);
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err("La consulta tiene comillas o paréntesis desbalanceados.".to_owned());
    }
    let part = characters[start..].iter().collect::<String>();
    if part.trim().is_empty() {
        return Err("La proyección contiene una expresión vacía.".to_owned());
    }
    parts.push(part);
    Ok(parts)
}

fn split_local_predicates(value: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut quoted = false;
    let characters = value.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < characters.len() {
        match characters[index] {
            '\'' => {
                if quoted && characters.get(index + 1) == Some(&'\'') {
                    index += 2;
                    continue;
                }
                quoted = !quoted;
                index += 1;
            }
            _ if !quoted
                && index + 3 <= characters.len()
                && characters[index..index + 3]
                    .iter()
                    .map(|character| character.to_ascii_lowercase())
                    .eq(['a', 'n', 'd'])
                && (index == 0 || characters[index - 1].is_whitespace())
                && (index + 3 == characters.len() || characters[index + 3].is_whitespace()) =>
            {
                let part = characters[start..index].iter().collect::<String>();
                if part.trim().is_empty() {
                    return Err("El filtro contiene una condición vacía.".to_owned());
                }
                parts.push(part);
                index += 3;
                start = index;
            }
            _ => index += 1,
        }
    }
    if quoted {
        return Err("El filtro contiene comillas desbalanceadas.".to_owned());
    }
    let part = characters[start..].iter().collect::<String>();
    if part.trim().is_empty() {
        return Err("El filtro contiene una condición vacía.".to_owned());
    }
    parts.push(part);
    Ok(parts)
}

fn parse_local_literal(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Ok(value[1..value.len() - 1].replace("''", "'"));
    }
    let numeric = Regex::new(r"^-?(?:\d+(?:\.\d*)?|\.\d+)$")
        .expect("el patrón numérico local debe ser válido");
    if numeric.is_match(value) || matches!(value.to_ascii_lowercase().as_str(), "true" | "false") {
        return Ok(value.to_owned());
    }
    Err("Los filtros solo permiten literales entre comillas, números o booleanos.".to_owned())
}

fn parse_local_projection(
    projection: &str,
    frame: &DataFrame,
    group_by: Option<&str>,
) -> Result<(Vec<LocalProjection>, bool), String> {
    if projection.trim() == "*" {
        return Ok((
            frame
                .get_column_names()
                .iter()
                .map(|name| LocalProjection::Column {
                    name: name.to_string(),
                    output_name: name.to_string(),
                })
                .collect(),
            false,
        ));
    }

    let aggregate_pattern = Regex::new(
        r#"(?is)^\s*(count|sum|avg|average|min|max)\s*\(\s*(\*|(?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*\)(?:\s+as\s+((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+)))?\s*$"#,
    )
    .expect("el patrón de agregaciones locales debe ser válido");
    let column_pattern = Regex::new(r#"(?is)^\s*((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*$"#)
        .expect("el patrón de columnas locales debe ser válido");
    let mut projections = Vec::new();
    let mut aggregate = false;
    let mut column_projection = false;

    for expression in split_local_sql_list(projection)? {
        if let Some(captures) = aggregate_pattern.captures(&expression) {
            aggregate = true;
            if column_projection
                && (group_by.is_none()
                    || projections.iter().any(|projection| {
                        matches!(projection, LocalProjection::Column { name, .. } if Some(name.as_str()) != group_by)
                    }))
            {
                return Err("No mezcles columnas y agregaciones salvo la clave GROUP BY.".to_owned());
            }
            let function = match captures
                .get(1)
                .expect("la función agregada debe existir")
                .as_str()
                .to_ascii_lowercase()
                .as_str()
            {
                "count" => LocalAggregate::Count,
                "sum" => LocalAggregate::Sum,
                "avg" | "average" => LocalAggregate::Average,
                "min" => LocalAggregate::Minimum,
                "max" => LocalAggregate::Maximum,
                _ => unreachable!("la expresión ya fue validada por el patrón"),
            };
            let argument = captures
                .get(2)
                .expect("el argumento agregado debe existir")
                .as_str();
            let column = if argument == "*" {
                if !matches!(function, LocalAggregate::Count) {
                    return Err("COUNT es la única agregación que permite '*'.".to_owned());
                }
                None
            } else {
                Some(local_identifier(argument)?)
            };
            if let Some(column) = &column {
                frame.column(column).map_err(|_| {
                    format!("La columna '{column}' no existe en el dataset activo.")
                })?;
            }
            let default_name = match (&function, &column) {
                (LocalAggregate::Count, None) => "count".to_owned(),
                (LocalAggregate::Count, Some(column)) => format!("count_{column}"),
                (LocalAggregate::Sum, Some(column)) => format!("sum_{column}"),
                (LocalAggregate::Average, Some(column)) => format!("avg_{column}"),
                (LocalAggregate::Minimum, Some(column)) => format!("min_{column}"),
                (LocalAggregate::Maximum, Some(column)) => format!("max_{column}"),
                _ => unreachable!("las agregaciones no válidas ya fueron rechazadas"),
            };
            let output_name = captures
                .get(3)
                .map(|value| local_identifier(value.as_str()))
                .transpose()?
                .unwrap_or(default_name);
            projections.push(LocalProjection::Aggregate {
                function,
                column,
                output_name,
            });
        } else if let Some(captures) = column_pattern.captures(&expression) {
            column_projection = true;
            let name =
                local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
            if aggregate && (group_by.is_none() || Some(name.as_str()) != group_by) {
                return Err("No mezcles columnas y agregaciones en una misma consulta.".to_owned());
            }
            frame
                .column(&name)
                .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))?;
            projections.push(LocalProjection::Column {
                name: name.clone(),
                output_name: name,
            });
        } else {
            return Err("La proyección solo permite columnas o COUNT/SUM/AVG/MIN/MAX.".to_owned());
        }
    }
    Ok((projections, aggregate))
}

#[derive(Clone, Debug)]
struct LocalJoinOperand {
    table: Option<String>,
    column: String,
}

fn parse_local_join_operand(value: &str) -> Result<LocalJoinOperand, String> {
    let identifier = local_identifier(value)?;
    let mut parts = identifier.split('.');
    let first = parts.next().unwrap_or_default();
    let second = parts.next();
    if parts.next().is_some() {
        return Err("El JOIN solo permite columnas de dataset o compared.".to_owned());
    }
    if let Some(column) = second {
        if !matches!(first, "dataset" | "compared") || column.is_empty() {
            return Err("El JOIN solo permite columnas de dataset o compared.".to_owned());
        }
        Ok(LocalJoinOperand {
            table: Some(first.to_owned()),
            column: column.to_owned(),
        })
    } else {
        Ok(LocalJoinOperand {
            table: None,
            column: first.to_owned(),
        })
    }
}

fn parse_local_join_query_with_cancel<C>(
    query: &str,
    current: &DataFrame,
    compared: Option<&DataFrame>,
    is_cancelled: &C,
) -> Result<Option<(DataFrame, String)>, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let pattern = Regex::new(
        r#"(?is)^\s*select\s+(.+?)\s+from\s+dataset\s+(?:(inner|left|full)\s+)?join\s+compared\s+on\s+((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))\s*=\s*((?:\"(?:\"\"|[^\"])+\"|[[:alnum:]_.]+))(.*)$"#,
    )
    .expect("el patrón de JOIN local debe ser válido");
    let Some(captures) = pattern.captures(query) else {
        if query.to_ascii_lowercase().contains("join") {
            return Err(
                "El JOIN local debe usar FROM dataset JOIN compared ON columna = columna."
                    .to_owned(),
            );
        }
        return Ok(None);
    };
    let compared = compared.ok_or_else(|| {
        "No hay un dataset comparado cargado. Selecciona una fuente en Comparar datasets antes de usar JOIN."
            .to_owned()
    })?;
    if current.height().saturating_add(compared.height()) > LOCAL_QUERY_JOIN_MAX_INPUT_ROWS {
        return Err(format!(
            "El JOIN local limita las entradas a {LOCAL_QUERY_JOIN_MAX_INPUT_ROWS} filas para proteger la memoria."
        ));
    }

    let left = parse_local_join_operand(
        captures
            .get(3)
            .expect("la clave izquierda debe existir")
            .as_str(),
    )?;
    let right = parse_local_join_operand(
        captures
            .get(4)
            .expect("la clave derecha debe existir")
            .as_str(),
    )?;
    let left_table = left.table.as_deref().unwrap_or("dataset");
    let right_table = right.table.as_deref().unwrap_or("compared");
    if left_table == right_table {
        return Err(
            "El JOIN debe relacionar una columna de dataset con una de compared.".to_owned(),
        );
    }
    let (current_key, compared_key) = if left_table == "dataset" {
        (left.column, right.column)
    } else {
        (right.column, left.column)
    };
    let join_type = match captures
        .get(2)
        .map(|value| value.as_str().to_ascii_lowercase())
    {
        Some(value) if value == "left" => DatasetJoinType::Left,
        Some(value) if value == "full" => DatasetJoinType::Full,
        _ => DatasetJoinType::Inner,
    };
    let joined = join_frames_on_keys_with_cancel(
        current,
        compared,
        &[current_key],
        &[compared_key],
        join_type,
        is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let normalized_query = format!(
        "SELECT {} FROM dataset{}",
        captures
            .get(1)
            .expect("la proyección debe existir")
            .as_str(),
        captures
            .get(5)
            .map(|value| value.as_str())
            .unwrap_or_default()
    );
    Ok(Some((joined, normalized_query)))
}

fn parse_local_predicates(
    where_clause: Option<&str>,
    frame: &DataFrame,
) -> Result<Vec<LocalPredicate>, String> {
    let Some(where_clause) = where_clause else {
        return Ok(Vec::new());
    };
    let null_pattern = Regex::new(r"(?is)^\s*(.+?)\s+is\s+(not\s+)?null\s*$")
        .expect("el patrón de nulos local debe ser válido");
    let comparison_pattern = Regex::new(r"(?is)^\s*(.+?)\s*(<>|!=|>=|<=|=|>|<)\s*(.+?)\s*$")
        .expect("el patrón de comparación local debe ser válido");
    split_local_predicates(where_clause)?
        .into_iter()
        .map(|condition| {
            if let Some(captures) = null_pattern.captures(&condition) {
                let column =
                    local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
                frame.column(&column).map_err(|_| {
                    format!("La columna '{column}' no existe en el dataset activo.")
                })?;
                return Ok(LocalPredicate {
                    column,
                    operator: if captures.get(2).is_some() {
                        LocalPredicateOperator::IsNotNull
                    } else {
                        LocalPredicateOperator::IsNull
                    },
                    value: None,
                });
            }
            let captures = comparison_pattern.captures(&condition).ok_or_else(|| {
                "El filtro solo permite comparaciones simples o IS NULL.".to_owned()
            })?;
            let column =
                local_identifier(captures.get(1).expect("la columna debe existir").as_str())?;
            frame
                .column(&column)
                .map_err(|_| format!("La columna '{column}' no existe en el dataset activo."))?;
            let operator = match captures.get(2).expect("el operador debe existir").as_str() {
                "=" => LocalPredicateOperator::Eq,
                "!=" | "<>" => LocalPredicateOperator::Neq,
                ">" => LocalPredicateOperator::Gt,
                ">=" => LocalPredicateOperator::Gte,
                "<" => LocalPredicateOperator::Lt,
                "<=" => LocalPredicateOperator::Lte,
                _ => unreachable!("el operador ya fue validado por el patrón"),
            };
            Ok(LocalPredicate {
                column,
                operator,
                value: Some(parse_local_literal(
                    captures.get(3).expect("el literal debe existir").as_str(),
                )?),
            })
        })
        .collect()
}

fn parse_local_query(query: &str, frame: &DataFrame) -> Result<LocalQueryPlan, String> {
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!(
            "La consulta supera el límite local de {MAX_QUERY_CHARS} caracteres."
        ));
    }
    if query.contains(';') || query.contains("--") || query.contains("/*") || query.contains("*/") {
        return Err(
            "La consulta solo permite una sentencia SELECT sin comentarios ni separadores."
                .to_owned(),
        );
    }
    let pattern = Regex::new(
        r"(?is)^\s*select\s+(.+?)\s+from\s+dataset(?:\s+where\s+(.+?))?(?:\s+group\s+by\s+(.+?))?(?:\s+limit\s+(\d+))?(?:\s+offset\s+(\d+))?\s*$",
    )
    .map_err(|_| "No se pudo preparar el analizador SQL local.".to_owned())?;
    let captures = pattern.captures(query).ok_or_else(|| {
        "Usa SELECT columnas FROM dataset con LIMIT y OFFSET opcionales.".to_owned()
    })?;
    let projection = captures
        .get(1)
        .map(|value| value.as_str().trim())
        .unwrap_or_default();
    let limit = captures
        .get(4)
        .map(|value| value.as_str().parse::<usize>())
        .transpose()
        .map_err(|_| "LIMIT debe ser un entero válido.".to_owned())?
        .unwrap_or(50);
    let offset = captures
        .get(5)
        .map(|value| value.as_str().parse::<usize>())
        .transpose()
        .map_err(|_| "OFFSET debe ser un entero válido.".to_owned())?
        .unwrap_or(0);
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(format!("LIMIT debe estar entre 1 y {MAX_PAGE_SIZE}."));
    }

    let group_by = captures
        .get(3)
        .map(|value| local_identifier(value.as_str()))
        .transpose()?;
    if let Some(group_by) = &group_by {
        frame
            .column(group_by)
            .map_err(|_| format!("La columna '{group_by}' no existe en el dataset activo."))?;
    }
    let (projections, aggregate) = parse_local_projection(projection, frame, group_by.as_deref())?;
    if group_by.is_some() && !aggregate {
        return Err("GROUP BY necesita al menos una agregación.".to_owned());
    }
    let predicates = parse_local_predicates(captures.get(2).map(|value| value.as_str()), frame)?;
    Ok(LocalQueryPlan {
        projections,
        predicates,
        group_by,
        offset,
        limit,
        aggregate,
    })
}

fn local_compare(left: AnyValue<'_>, right: &str, operator: LocalPredicateOperator) -> bool {
    let Some(left) = preview_value(left) else {
        return false;
    };
    let ordering = match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => left.partial_cmp(&right),
        _ => Some(left.as_str().cmp(right)),
    };
    match operator {
        LocalPredicateOperator::Eq => ordering == Some(std::cmp::Ordering::Equal),
        LocalPredicateOperator::Neq => ordering != Some(std::cmp::Ordering::Equal),
        LocalPredicateOperator::Gt => ordering == Some(std::cmp::Ordering::Greater),
        LocalPredicateOperator::Gte => matches!(
            ordering,
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ),
        LocalPredicateOperator::Lt => ordering == Some(std::cmp::Ordering::Less),
        LocalPredicateOperator::Lte => matches!(
            ordering,
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ),
        LocalPredicateOperator::IsNull | LocalPredicateOperator::IsNotNull => false,
    }
}

fn local_predicate_matches(
    frame: &DataFrame,
    row_index: usize,
    predicate: &LocalPredicate,
) -> Result<bool, String> {
    let value = frame
        .column(&predicate.column)
        .map_err(|_| {
            format!(
                "La columna '{}' no existe en el dataset activo.",
                predicate.column
            )
        })?
        .get(row_index)
        .map_err(|error| format!("No se pudo evaluar el filtro local: {error}"))?;
    Ok(match predicate.operator {
        LocalPredicateOperator::IsNull => matches!(value, AnyValue::Null),
        LocalPredicateOperator::IsNotNull => !matches!(value, AnyValue::Null),
        operator => local_compare(
            value,
            predicate.value.as_deref().unwrap_or_default(),
            operator,
        ),
    })
}

fn local_row_matches(
    frame: &DataFrame,
    row_index: usize,
    predicates: &[LocalPredicate],
) -> Result<bool, String> {
    for predicate in predicates {
        if !local_predicate_matches(frame, row_index, predicate)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn local_query_block_flags_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<Vec<bool>, String>
where
    C: Fn() -> bool + Sync,
{
    (start..end)
        .into_par_iter()
        .map(|row_index| {
            if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            local_row_matches(frame, row_index, predicates)
        })
        .collect::<Result<Vec<_>, _>>()
}

fn local_query_block_match_count_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    Ok(
        local_query_block_flags_with_cancel(frame, predicates, start, end, is_cancelled)?
            .into_iter()
            .filter(|matches| *matches)
            .count(),
    )
}

fn local_query_block_rows_with_cancel<C>(
    frame: &DataFrame,
    predicates: &[LocalPredicate],
    start: usize,
    end: usize,
    is_cancelled: &C,
) -> Result<Vec<usize>, String>
where
    C: Fn() -> bool + Sync,
{
    Ok(
        local_query_block_flags_with_cancel(frame, predicates, start, end, is_cancelled)?
            .into_iter()
            .enumerate()
            .filter_map(|(offset, matches)| matches.then_some(start + offset))
            .collect(),
    )
}

fn aggregate_result_with_cancel<C>(
    frame: &DataFrame,
    projection: &LocalProjection,
    rows: &[usize],
    is_cancelled: &C,
) -> Result<Option<String>, String>
where
    C: Fn() -> bool + Sync,
{
    let LocalProjection::Aggregate {
        function, column, ..
    } = projection
    else {
        return Ok(None);
    };
    if matches!(function, LocalAggregate::Count) && column.is_none() {
        ensure_not_cancelled(is_cancelled())?;
        return Ok(Some(rows.len().to_string()));
    }
    let column_name = column
        .as_deref()
        .ok_or_else(|| "La agregación necesita una columna válida.".to_owned())?;
    let series = frame
        .column(column_name)
        .map_err(|_| format!("La columna '{column_name}' no existe en el dataset activo."))?;
    if matches!(function, LocalAggregate::Count) {
        let mut count = 0usize;
        for (position, row) in rows.iter().enumerate() {
            if position % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            if series
                .get(*row)
                .map_err(|error| format!("No se pudo leer la agregación local: {error}"))?
                != AnyValue::Null
            {
                count += 1;
            }
        }
        ensure_not_cancelled(is_cancelled())?;
        return Ok(Some(count.to_string()));
    }
    let mut values = Vec::with_capacity(rows.len());
    for (position, row) in rows.iter().enumerate() {
        if position % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        if let Some(value) = preview_value(
            series
                .get(*row)
                .map_err(|error| format!("No se pudo leer la agregación local: {error}"))?,
        ) {
            values.push(value);
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    if values.is_empty() {
        return Ok(None);
    }
    match function {
        LocalAggregate::Sum | LocalAggregate::Average => {
            let numbers = values
                .iter()
                .filter_map(|value| value.parse::<f64>().ok())
                .collect::<Vec<_>>();
            if numbers.len() != values.len() {
                return Err(format!(
                    "La columna '{column_name}' debe ser numérica para SUM/AVG."
                ));
            }
            let total = numbers.iter().sum::<f64>();
            if matches!(function, LocalAggregate::Average) {
                Ok(Some((total / numbers.len() as f64).to_string()))
            } else {
                Ok(Some(total.to_string()))
            }
        }
        LocalAggregate::Minimum | LocalAggregate::Maximum => {
            let numbers = values
                .iter()
                .map(|value| value.parse::<f64>())
                .collect::<Result<Vec<_>, _>>();
            if let Ok(numbers) = numbers {
                let value = if matches!(function, LocalAggregate::Minimum) {
                    numbers.iter().copied().fold(f64::INFINITY, f64::min)
                } else {
                    numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                };
                Ok(Some(value.to_string()))
            } else {
                let value = if matches!(function, LocalAggregate::Minimum) {
                    values.iter().min()
                } else {
                    values.iter().max()
                };
                Ok(value.cloned())
            }
        }
        LocalAggregate::Count => unreachable!("COUNT se resuelve antes"),
    }
}

fn grouped_local_rows_with_cancel<C>(
    frame: &DataFrame,
    rows: &[usize],
    group_by: &str,
    is_cancelled: &C,
) -> Result<LocalGroupRows, String>
where
    C: Fn() -> bool + Sync,
{
    let group_column = frame
        .column(group_by)
        .map_err(|_| format!("La columna '{group_by}' no existe en el dataset activo."))?;
    let mut groups = Vec::<(Option<String>, Vec<usize>)>::new();
    let mut positions = HashMap::<Option<String>, usize>::new();
    for (position, row_index) in rows.iter().enumerate() {
        if position % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let key = preview_value(
            group_column
                .get(*row_index)
                .map_err(|error| format!("No se pudo leer la clave GROUP BY: {error}"))?,
        );
        if let Some(position) = positions.get(&key) {
            groups[*position].1.push(*row_index);
        } else {
            positions.insert(key.clone(), groups.len());
            groups.push((key, vec![*row_index]));
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(groups)
}

fn local_query_row_with_cancel<C>(
    frame: &DataFrame,
    projections: &[LocalProjection],
    rows: &[usize],
    group_value: Option<&Option<String>>,
    is_cancelled: &C,
) -> Result<Vec<Option<String>>, String>
where
    C: Fn() -> bool + Sync,
{
    projections
        .iter()
        .map(|projection| match projection {
            LocalProjection::Column { .. } => Ok(group_value.cloned().flatten()),
            LocalProjection::Aggregate { .. } => {
                aggregate_result_with_cancel(frame, projection, rows, is_cancelled)
            }
        })
        .collect()
}

#[cfg(test)]
fn execute_local_query(frame: &DataFrame, query: &str) -> Result<DatasetQueryResult, String> {
    let never_cancelled = || false;
    execute_local_query_with_cancel(frame, query, &never_cancelled)
}

fn execute_local_query_with_cancel<C>(
    frame: &DataFrame,
    query: &str,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    let plan = parse_local_query(query, frame)?;
    ensure_not_cancelled(is_cancelled())?;
    let block_count = frame.height().div_ceil(LOCAL_QUERY_BLOCK_ROWS);
    let (matching_count, matching_rows) = if plan.aggregate {
        // Count first so aggregate queries never collect matching row indexes
        // beyond the local materialization budget.
        let block_counts = (0..block_count)
            .into_par_iter()
            .map(|block_index| {
                let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
                let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
                local_query_block_match_count_with_cancel(
                    frame,
                    &plan.predicates,
                    start,
                    end,
                    is_cancelled,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_count = block_counts.iter().sum::<usize>();
        if matching_count > LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS {
            return Err(format!(
                "La agregación local limita las filas coincidentes a {LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS} para proteger la memoria."
            ));
        }
        ensure_not_cancelled(is_cancelled())?;
        let block_rows = (0..block_count)
            .into_par_iter()
            .map(|block_index| {
                let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
                let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
                local_query_block_rows_with_cancel(
                    frame,
                    &plan.predicates,
                    start,
                    end,
                    is_cancelled,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_rows = block_rows.into_iter().flatten().collect();
        (matching_count, matching_rows)
    } else {
        // A paged query first counts each block in parallel. Only the block(s)
        // containing the requested window are scanned a second time, so a
        // query over millions of matching rows never indexes every match.
        let block_counts = (0..block_count)
            .into_par_iter()
            .map(|block_index| {
                let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
                let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
                local_query_block_match_count_with_cancel(
                    frame,
                    &plan.predicates,
                    start,
                    end,
                    is_cancelled,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matching_count = block_counts.iter().sum();
        ensure_not_cancelled(is_cancelled())?;
        if plan.offset > matching_count {
            return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
        }

        let mut offset_in_matches = plan.offset;
        let mut remaining = plan.limit;
        let mut matching_rows = Vec::with_capacity(plan.limit.min(matching_count));
        for (block_index, block_match_count) in block_counts.iter().copied().enumerate() {
            if remaining == 0 {
                break;
            }
            if offset_in_matches >= block_match_count {
                offset_in_matches -= block_match_count;
                continue;
            }

            let start = block_index * LOCAL_QUERY_BLOCK_ROWS;
            let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
            let block_rows = local_query_block_rows_with_cancel(
                frame,
                &plan.predicates,
                start,
                end,
                is_cancelled,
            )?;
            let take = remaining.min(block_match_count - offset_in_matches);
            matching_rows.extend(block_rows.into_iter().skip(offset_in_matches).take(take));
            remaining -= take;
            offset_in_matches = 0;
        }
        (matching_count, matching_rows)
    };
    ensure_not_cancelled(is_cancelled())?;
    let columns = plan
        .projections
        .iter()
        .map(|projection| match projection {
            LocalProjection::Column { name, output_name } => frame
                .column(name)
                .map(|column| DatasetColumn {
                    name: output_name.clone(),
                    data_type: column.dtype().to_string(),
                })
                .map_err(|_| format!("La columna '{name}' no existe en el dataset activo.")),
            LocalProjection::Aggregate {
                function,
                column,
                output_name,
            } => Ok(DatasetColumn {
                name: output_name.clone(),
                data_type: match function {
                    LocalAggregate::Count => "UInt64".to_owned(),
                    LocalAggregate::Sum | LocalAggregate::Average => "Float64".to_owned(),
                    LocalAggregate::Minimum | LocalAggregate::Maximum => column
                        .as_deref()
                        .and_then(|name| frame.column(name).ok())
                        .map(|column| column.dtype().to_string())
                        .unwrap_or_else(|| "String".to_owned()),
                },
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (row_count, rows, offset, truncated) = if plan.aggregate {
        let aggregate_rows = if let Some(group_by) = &plan.group_by {
            grouped_local_rows_with_cancel(frame, &matching_rows, group_by, is_cancelled)?
                .iter()
                .map(|(group_value, group_rows)| {
                    local_query_row_with_cancel(
                        frame,
                        &plan.projections,
                        group_rows,
                        Some(group_value),
                        is_cancelled,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![local_query_row_with_cancel(
                frame,
                &plan.projections,
                &matching_rows,
                None,
                is_cancelled,
            )?]
        };
        ensure_not_cancelled(is_cancelled())?;
        let row_count = aggregate_rows.len();
        if plan.offset > row_count {
            return Err("La página solicitada está fuera del resultado agregado.".to_owned());
        }
        let end = plan.offset.saturating_add(plan.limit).min(row_count);
        (
            row_count,
            aggregate_rows[plan.offset..end].to_vec(),
            plan.offset,
            plan.offset.saturating_add(plan.limit) < row_count,
        )
    } else {
        if plan.offset > matching_count {
            return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
        }
        let rows = matching_rows
            .iter()
            .map(|row_index| {
                plan.projections
                    .iter()
                    .map(|projection| {
                        let LocalProjection::Column { name, .. } = projection else {
                            return Ok(None);
                        };
                        frame
                            .column(name)
                            .map_err(|error| {
                                format!("No se pudo preparar la consulta local: {error}")
                            })?
                            .get(*row_index)
                            .map_err(|error| {
                                format!("No se pudo preparar la consulta local: {error}")
                            })
                            .map(preview_value)
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        ensure_not_cancelled(is_cancelled())?;
        (
            matching_count,
            rows,
            plan.offset,
            plan.offset.saturating_add(plan.limit) < matching_count,
        )
    };
    Ok(DatasetQueryResult {
        columns,
        row_count,
        offset,
        rows,
        truncated,
    })
}

#[cfg(test)]
fn execute_local_query_with_comparison(
    current: &DataFrame,
    compared: Option<&DataFrame>,
    query: &str,
) -> Result<DatasetQueryResult, String> {
    let never_cancelled = || false;
    execute_local_query_with_comparison_and_cancel(current, compared, query, &never_cancelled)
}

fn execute_local_query_with_comparison_and_cancel<C>(
    current: &DataFrame,
    compared: Option<&DataFrame>,
    query: &str,
    is_cancelled: &C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Sync,
{
    if let Some((joined, normalized_query)) =
        parse_local_join_query_with_cancel(query, current, compared, is_cancelled)?
    {
        execute_local_query_with_cancel(&joined, &normalized_query, is_cancelled)
    } else {
        execute_local_query_with_cancel(current, query, is_cancelled)
    }
}

fn numeric_value(value: AnyValue<'_>) -> Option<f64> {
    let value = match value {
        AnyValue::UInt8(value) => Some(value.into()),
        AnyValue::UInt16(value) => Some(value.into()),
        AnyValue::UInt32(value) => Some(value.into()),
        AnyValue::UInt64(value) => Some(value as f64),
        AnyValue::UInt128(value) => Some(value as f64),
        AnyValue::Int8(value) => Some(value.into()),
        AnyValue::Int16(value) => Some(value.into()),
        AnyValue::Int32(value) => Some(value.into()),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::Int128(value) => Some(value as f64),
        AnyValue::Float32(value) => Some(value.into()),
        AnyValue::Float64(value) => Some(value),
        _ => None,
    };
    value.filter(|number| number.is_finite())
}

struct NumericStatistics {
    minimum: Option<f64>,
    maximum: Option<f64>,
    mean: Option<f64>,
    standard_deviation: Option<f64>,
    first_quartile: Option<f64>,
    median: Option<f64>,
    third_quartile: Option<f64>,
    outlier_count: usize,
    histogram: Option<Vec<HistogramBucket>>,
}

fn has_identifier_leading_zero(value: &str) -> bool {
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mut characters = unsigned.chars();
    matches!((characters.next(), characters.next()), (Some('0'), Some(next)) if next.is_ascii_digit())
}

fn semantic_numeric_value(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() || has_identifier_leading_zero(value) {
        return None;
    }
    if !value.contains('.') && !value.contains('e') && !value.contains('E') {
        let integer = value.parse::<i128>().ok()?;
        if integer.unsigned_abs() > (1_u128 << 53) {
            return None;
        }
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn stable_histogram_boundary(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    // Reparse a fixed-precision scientific representation so the value is
    // identical before and after serde_json serialization.
    format!("{value:.15e}").parse().unwrap_or(value)
}

fn numeric_histogram(values: &Float64Chunked, minimum: f64, maximum: f64) -> Vec<HistogramBucket> {
    if minimum == maximum {
        let count = values
            .iter()
            .flatten()
            .filter(|value| value.is_finite())
            .count();
        return vec![HistogramBucket {
            lower: minimum,
            upper: maximum,
            count,
        }];
    }

    // Normalize before calculating positions so ranges such as -1e308..1e308 do not
    // overflow when the difference between the endpoints is computed.
    let scale = minimum.abs().max(maximum.abs());
    let normalized_minimum = minimum / scale;
    let normalized_maximum = maximum / scale;
    let normalized_span = normalized_maximum - normalized_minimum;
    let mut counts = vec![0usize; NUMERIC_HISTOGRAM_BUCKETS];

    for value in values.iter().flatten().filter(|value| value.is_finite()) {
        let position = ((value / scale - normalized_minimum) / normalized_span).clamp(0.0, 1.0);
        let index = ((position * NUMERIC_HISTOGRAM_BUCKETS as f64).floor() as usize)
            .min(NUMERIC_HISTOGRAM_BUCKETS - 1);
        counts[index] += 1;
    }

    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            let lower_fraction = index as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
            let upper_fraction = (index + 1) as f64 / NUMERIC_HISTOGRAM_BUCKETS as f64;
            HistogramBucket {
                lower: if index == 0 {
                    minimum
                } else {
                    stable_histogram_boundary(
                        minimum * (1.0 - lower_fraction) + maximum * lower_fraction,
                    )
                },
                upper: if index + 1 == NUMERIC_HISTOGRAM_BUCKETS {
                    maximum
                } else {
                    stable_histogram_boundary(
                        minimum * (1.0 - upper_fraction) + maximum * upper_fraction,
                    )
                },
                count,
            }
        })
        .collect()
}

fn numeric_statistics(
    column: &Column,
    text_profile: Option<&TextStatistics>,
) -> Result<Option<NumericStatistics>, String> {
    let source = if column.dtype().is_primitive_numeric() {
        column.clone()
    } else if column.dtype() == &DataType::String {
        // The text pass already validates semantic numeric candidates for normal-sized
        // columns. Reuse that result so a 5 GB CSV is not parsed twice.
        let validated_numeric_profile = text_profile.is_some_and(|profile| {
            matches!(profile.suggested_type, Some("integer") | Some("decimal"))
                && profile.invalid_type_count == Some(0)
        });
        let needs_validation = !validated_numeric_profile;
        if needs_validation {
            if text_profile.is_some_and(|profile| profile.suggested_type.is_some()) {
                return Ok(None);
            }
            let text = column
                .str()
                .map_err(|error| format!("No se pudo analizar texto numérico: {error}"))?;
            let mut has_value = false;
            for value in text.iter().flatten() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    continue;
                }
                has_value = true;
                if semantic_numeric_value(trimmed).is_none() {
                    return Ok(None);
                }
            }
            if !has_value {
                return Ok(None);
            }
        }
        column
            .cast(&DataType::Float64)
            .map_err(|error| format!("No se pudo convertir texto numérico: {error}"))?
    } else {
        return Ok(None);
    };

    let value_count = source.len().saturating_sub(source.null_count());
    if value_count == 0 {
        return Ok(None);
    }

    let minimum = numeric_value(
        source
            .min_reduce()
            .map_err(|error| format!("No se pudo calcular el mínimo: {error}"))?
            .into_value(),
    );
    let maximum = numeric_value(
        source
            .max_reduce()
            .map_err(|error| format!("No se pudo calcular el máximo: {error}"))?
            .into_value(),
    );
    let mean = numeric_value(
        source
            .mean_reduce()
            .map_err(|error| format!("No se pudo calcular el promedio: {error}"))?
            .into_value(),
    );
    let standard_deviation = if value_count < 2 {
        None
    } else {
        numeric_value(
            source
                .std_reduce(1)
                .map_err(|error| format!("No se pudo calcular la desviación estándar: {error}"))?
                .into_value(),
        )
    };
    let quartile_values = source
        .quantiles_reduce(&[0.25, 0.5, 0.75], QuantileMethod::Linear)
        .map_err(|error| format!("No se pudieron calcular los cuantiles: {error}"))?
        .into_value();
    let mut quartiles = [None; 3];
    if let AnyValue::List(values) = quartile_values {
        let mut values = values.iter();
        for quartile in &mut quartiles {
            *quartile = values.next().and_then(numeric_value);
        }
    }
    let first_quartile = quartiles[0];
    let median = quartiles[1];
    let third_quartile = quartiles[2];
    // Reuse one Float64 view for the histogram and outlier pass. Integer
    // columns used to be cast twice, which increased peak memory on wide files.
    let floating_source = if source.dtype() == &DataType::Float64 {
        source.clone()
    } else {
        source
            .cast(&DataType::Float64)
            .map_err(|error| format!("No se pudo preparar las estadísticas numéricas: {error}"))?
    };
    let floating_values = floating_source
        .f64()
        .map_err(|error| format!("No se pudieron leer las estadísticas numéricas: {error}"))?;
    let histogram = match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => {
            Some(numeric_histogram(floating_values, minimum, maximum))
        }
        _ => None,
    };
    let outlier_count = if value_count < 4 {
        0
    } else {
        let q1 = first_quartile.expect("cuatro valores siempre producen Q1");
        let q3 = third_quartile.expect("cuatro valores siempre producen Q3");
        let interquartile_range = q3 - q1;
        let lower_bound = q1 - 1.5 * interquartile_range;
        let upper_bound = q3 + 1.5 * interquartile_range;
        floating_values
            .iter()
            .flatten()
            .filter(|value| *value < lower_bound || *value > upper_bound)
            .count()
    };

    Ok(Some(NumericStatistics {
        minimum,
        maximum,
        mean,
        standard_deviation,
        first_quartile,
        median,
        third_quartile,
        outlier_count,
        histogram,
    }))
}

struct TextStatistics {
    empty_count: usize,
    sentinel_count: usize,
    encoding_issue_count: usize,
    minimum_length: Option<usize>,
    maximum_length: Option<usize>,
    average_length: Option<f64>,
    suggested_type: Option<&'static str>,
    type_match_percentage: Option<f64>,
    invalid_type_count: Option<usize>,
}

#[derive(Clone, Copy)]
enum DataprepDateFormat {
    Ymd,
    DmySlash,
    MdySlash,
    DmyDash,
    YmdSlash,
    DmyShort,
    DmyLong,
    CompactYmd,
    DmyShortDash,
    MdyShort,
    MdyLong,
    YmdTime,
    YmdSpaceTime,
    DmySlashTime,
    Iso8601,
}

const DATAPREP_DATE_FORMATS: &[DataprepDateFormat] = &[
    DataprepDateFormat::Ymd,
    DataprepDateFormat::DmySlash,
    DataprepDateFormat::MdySlash,
    DataprepDateFormat::DmyDash,
    DataprepDateFormat::YmdSlash,
    DataprepDateFormat::DmyShort,
    DataprepDateFormat::DmyLong,
    DataprepDateFormat::CompactYmd,
    DataprepDateFormat::DmyShortDash,
    DataprepDateFormat::MdyShort,
    DataprepDateFormat::MdyLong,
    DataprepDateFormat::YmdTime,
    DataprepDateFormat::YmdSpaceTime,
    DataprepDateFormat::DmySlashTime,
    DataprepDateFormat::Iso8601,
];

fn parse_dataprep_datetime(value: &str, format: DataprepDateFormat) -> Option<NaiveDateTime> {
    let value = value.trim();
    let parse_date = |format| {
        NaiveDate::parse_from_str(value, format)
            .ok()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
    };
    let parse_datetime = |formats: &[&str]| {
        formats
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
    };

    match format {
        DataprepDateFormat::Ymd => parse_date("%Y-%m-%d"),
        DataprepDateFormat::DmySlash => parse_date("%d/%m/%Y"),
        DataprepDateFormat::MdySlash => parse_date("%m/%d/%Y"),
        DataprepDateFormat::DmyDash => parse_date("%d-%m-%Y"),
        DataprepDateFormat::YmdSlash => parse_date("%Y/%m/%d"),
        DataprepDateFormat::DmyShort => parse_date("%d %b %Y"),
        DataprepDateFormat::DmyLong => parse_date("%d %B %Y"),
        DataprepDateFormat::CompactYmd => parse_date("%Y%m%d"),
        DataprepDateFormat::DmyShortDash => parse_date("%d-%b-%Y"),
        DataprepDateFormat::MdyShort => parse_date("%b %d, %Y"),
        DataprepDateFormat::MdyLong => parse_date("%B %d, %Y"),
        DataprepDateFormat::YmdTime => {
            parse_datetime(&["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"])
        }
        DataprepDateFormat::YmdSpaceTime => {
            parse_datetime(&["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"])
        }
        DataprepDateFormat::DmySlashTime => parse_datetime(&["%d/%m/%Y %H:%M"]),
        DataprepDateFormat::Iso8601 => parse_recipe_datetime(value, RecipeDateFormat::Iso8601).ok(),
    }
}

fn is_supported_date(value: &str) -> bool {
    DATAPREP_DATE_FORMATS
        .iter()
        .copied()
        .any(|format| parse_dataprep_datetime(value, format).is_some())
}

fn is_missing_sentinel(value: &str) -> bool {
    let normalized = normalize_text_value(value, true);
    SENTINEL_VALUES.contains(&normalized.as_str())
}

fn mojibake_byte(character: char) -> Option<u8> {
    match character {
        '\u{20ac}' => Some(0x80),
        '\u{201a}' => Some(0x82),
        '\u{192}' => Some(0x83),
        '\u{201e}' => Some(0x84),
        '\u{2026}' => Some(0x85),
        '\u{2020}' => Some(0x86),
        '\u{2021}' => Some(0x87),
        '\u{2c6}' => Some(0x88),
        '\u{2030}' => Some(0x89),
        '\u{160}' => Some(0x8a),
        '\u{2039}' => Some(0x8b),
        '\u{152}' => Some(0x8c),
        '\u{17d}' => Some(0x8e),
        '\u{2018}' => Some(0x91),
        '\u{2019}' => Some(0x92),
        '\u{201c}' => Some(0x93),
        '\u{201d}' => Some(0x94),
        '\u{2022}' => Some(0x95),
        '\u{2013}' => Some(0x96),
        '\u{2014}' => Some(0x97),
        '\u{2dc}' => Some(0x98),
        '\u{2122}' => Some(0x99),
        '\u{161}' => Some(0x9a),
        '\u{203a}' => Some(0x9b),
        '\u{153}' => Some(0x9c),
        '\u{17e}' => Some(0x9e),
        '\u{178}' => Some(0x9f),
        character if (character as u32) <= 0xff => Some(character as u8),
        _ => None,
    }
}

fn repair_mojibake(value: &str) -> Option<String> {
    if !MOJIBAKE_MARKERS.iter().any(|marker| value.contains(marker)) {
        return None;
    }
    let bytes = value
        .chars()
        .map(mojibake_byte)
        .collect::<Option<Vec<_>>>()?;
    let repaired = String::from_utf8(bytes).ok()?;
    (repaired != value && !repaired.contains('\u{fffd}')).then_some(repaired)
}

fn boolean_token(value: &str) -> Option<&'static str> {
    match normalize_text_value(value, true).as_str() {
        "true" | "yes" | "si" => Some("true"),
        "false" | "no" => Some("false"),
        _ => None,
    }
}

fn is_valid_suggested_type(value: &str, suggested_type: &str) -> bool {
    let value = value.trim();
    match suggested_type {
        "boolean" => boolean_token(value).is_some(),
        "integer" => value.parse::<i64>().is_ok() && semantic_numeric_value(value).is_some(),
        "decimal" => semantic_numeric_value(value).is_some(),
        "date" => is_supported_date(value),
        _ => false,
    }
}

fn is_boolean_candidate(column: &Column) -> Result<bool, String> {
    if column.dtype() != &DataType::String {
        return Ok(false);
    }
    let values = column
        .str()
        .map_err(|error| format!("No se pudo analizar una columna booleana: {error}"))?;
    let non_empty = values
        .iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if non_empty.len() < 3 {
        return Ok(false);
    }
    let recognized = non_empty
        .iter()
        .filter(|value| boolean_token(value).is_some())
        .count();
    Ok(recognized * 100 >= non_empty.len() * 90)
}

fn privacy_signal(column_name: &str) -> Option<&'static str> {
    let normalized = normalize_text_value(column_name, true);
    let tokens = normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let compact = tokens.join("");
    let has_token = |values: &[&str]| {
        values
            .iter()
            .any(|value| tokens.iter().any(|token| token == value))
    };
    let has_text = |values: &[&str]| values.iter().any(|value| compact.contains(value));

    if has_text(&["email", "correo", "mail"]) {
        Some("email")
    } else if has_text(&["phone", "telefono", "tel", "movil", "celular"]) {
        Some("phone")
    } else if has_text(&["address", "direccion", "domicilio"]) {
        Some("address")
    } else if has_token(&[
        "id",
        "uuid",
        "identifier",
        "identificador",
        "codigo",
        "code",
        "clave",
        "key",
        "dni",
        "cedula",
        "pasaporte",
        "passport",
        "ssn",
    ]) {
        Some("identifier")
    } else if has_text(&["name", "nombre", "apellido", "surname"]) {
        Some("name")
    } else {
        None
    }
}

fn suggest_text_type(
    value_count: usize,
    boolean_count: usize,
    integer_count: usize,
    decimal_count: usize,
    date_count: usize,
) -> (Option<&'static str>, Option<f64>, Option<usize>) {
    if value_count < 3 {
        return (None, None, None);
    }

    let mut best = ("boolean", boolean_count);
    for candidate in [
        ("integer", integer_count),
        ("decimal", decimal_count),
        ("date", date_count),
    ] {
        if candidate.1 > best.1 {
            best = candidate;
        }
    }

    let match_percentage = (best.1 as f64 / value_count as f64) * 100.0;
    if match_percentage < 90.0 {
        return (None, None, None);
    }

    (
        Some(best.0),
        Some(match_percentage),
        Some(value_count - best.1),
    )
}

fn text_statistics(column: &Column) -> Result<Option<TextStatistics>, String> {
    if column.dtype() != &DataType::String {
        return Ok(None);
    }

    let values = column
        .str()
        .map_err(|error| format!("No se pudo analizar la columna de texto: {error}"))?;
    let mut empty_count = 0;
    let mut sentinel_count = 0;
    let mut encoding_issue_count = 0;
    let mut value_count: usize = 0;
    let mut boolean_count: usize = 0;
    let mut integer_count: usize = 0;
    let mut decimal_count: usize = 0;
    let mut date_count: usize = 0;
    let mut total_length: usize = 0;
    let mut minimum_length: Option<usize> = None;
    let mut maximum_length: Option<usize> = None;

    for value in values.iter().flatten() {
        let length = value.chars().count();
        let trimmed = value.trim();
        empty_count += usize::from(trimmed.is_empty());
        sentinel_count += usize::from(is_missing_sentinel(trimmed));
        encoding_issue_count += usize::from(repair_mojibake(value).is_some());
        if !trimmed.is_empty() {
            boolean_count += usize::from(boolean_token(trimmed).is_some());
            integer_count += usize::from(
                trimmed.parse::<i64>().is_ok() && semantic_numeric_value(trimmed).is_some(),
            );
            decimal_count += usize::from(semantic_numeric_value(trimmed).is_some());
            date_count += usize::from(is_supported_date(trimmed));
        }
        value_count += 1;
        total_length += length;
        minimum_length = Some(minimum_length.map_or(length, |current| current.min(length)));
        maximum_length = Some(maximum_length.map_or(length, |current| current.max(length)));
    }

    let average_length = (value_count > 0).then(|| total_length as f64 / value_count as f64);
    let non_empty_count = value_count.saturating_sub(empty_count);
    let (suggested_type, type_match_percentage, invalid_type_count) = suggest_text_type(
        non_empty_count,
        boolean_count,
        integer_count,
        decimal_count,
        date_count,
    );
    Ok(Some(TextStatistics {
        empty_count,
        sentinel_count,
        encoding_issue_count,
        minimum_length,
        maximum_length,
        average_length,
        suggested_type,
        type_match_percentage,
        invalid_type_count,
    }))
}

enum ProfileColumnEvent {
    Progress {
        index: usize,
        stage: &'static str,
        phase: u8,
    },
    Completed {
        index: usize,
        result: Box<Result<ColumnProfile, String>>,
    },
}

fn profile_column<C, F>(
    column: &Column,
    row_count: usize,
    is_cancelled: &C,
    mut report: F,
) -> Result<ColumnProfile, String>
where
    C: Fn() -> bool,
    F: FnMut(&'static str, u8),
{
    ensure_not_cancelled(is_cancelled())?;
    let null_count = column.null_count();
    report("Contando valores únicos", 25);
    let unique_count = column
        .n_unique()
        .map_err(|error| format!("No se pudieron contar los valores únicos: {error}"))?
        .saturating_sub(usize::from(null_count > 0));
    let completeness_percentage = if row_count == 0 {
        100.0
    } else {
        ((row_count - null_count) as f64 / row_count as f64) * 100.0
    };

    ensure_not_cancelled(is_cancelled())?;
    report("Analizando texto", 50);
    let text_statistics = text_statistics(column)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Calculando estadísticas numéricas", 75);
    let numeric_statistics = numeric_statistics(column, text_statistics.as_ref())?;
    let (minimum, maximum, mean) = if column.dtype().is_primitive_numeric() {
        let minimum = column
            .min_reduce()
            .map_err(|error| format!("No se pudo calcular el mínimo: {error}"))?
            .into_value();
        let maximum = column
            .max_reduce()
            .map_err(|error| format!("No se pudo calcular el máximo: {error}"))?
            .into_value();
        (
            preview_value(minimum),
            preview_value(maximum),
            numeric_statistics
                .as_ref()
                .and_then(|statistics| statistics.mean),
        )
    } else if let Some(statistics) = numeric_statistics.as_ref() {
        (
            statistics.minimum.map(|value| value.to_string()),
            statistics.maximum.map(|value| value.to_string()),
            statistics.mean,
        )
    } else {
        (None, None, None)
    };

    Ok(ColumnProfile {
        name: column.name().to_string(),
        data_type: column.dtype().to_string(),
        null_count,
        completeness_percentage,
        unique_count,
        minimum,
        maximum,
        mean,
        empty_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.empty_count),
        minimum_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.minimum_length),
        maximum_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.maximum_length),
        average_length: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.average_length),
        suggested_type: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.suggested_type)
            .map(str::to_owned),
        type_match_percentage: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.type_match_percentage),
        invalid_type_count: text_statistics
            .as_ref()
            .and_then(|statistics| statistics.invalid_type_count),
        sentinel_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.sentinel_count),
        encoding_issue_count: text_statistics
            .as_ref()
            .map(|statistics| statistics.encoding_issue_count),
        privacy_signal: privacy_signal(column.name()).map(str::to_owned),
        standard_deviation: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.standard_deviation),
        first_quartile: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.first_quartile),
        median: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.median),
        third_quartile: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.third_quartile),
        outlier_count: numeric_statistics
            .as_ref()
            .map(|statistics| statistics.outlier_count),
        histogram: numeric_statistics
            .as_ref()
            .and_then(|statistics| statistics.histogram.clone()),
    })
}

fn count_distinct_rows(frame: &DataFrame) -> Result<usize, String> {
    let streaming_result = frame
        .clone()
        .lazy()
        .unique(None, UniqueKeepStrategy::First)
        .select([len().alias("__distinct_rows")])
        .collect_with_engine(Engine::Streaming);

    match streaming_result {
        Ok(result) => {
            let result = result.unwrap_single();
            let value = result
                .column("__distinct_rows")
                .map_err(|error| format!("No se pudo leer el conteo de filas distintas: {error}"))?
                .get(0)
                .map_err(|error| format!("No se pudo leer el conteo de filas distintas: {error}"))?;
            match value {
                AnyValue::UInt8(value) => Ok(value.into()),
                AnyValue::UInt16(value) => Ok(value.into()),
                AnyValue::UInt32(value) => Ok(value as usize),
                AnyValue::UInt64(value) => usize::try_from(value).map_err(|_| {
                    "El conteo de filas distintas excede la capacidad local.".to_owned()
                }),
                AnyValue::UInt128(value) => usize::try_from(value).map_err(|_| {
                    "El conteo de filas distintas excede la capacidad local.".to_owned()
                }),
                _ => Err(
                    "El motor devolvió un tipo inesperado para el conteo de filas distintas."
                        .to_owned(),
                ),
            }
        }
        Err(streaming_error) => frame
            .unique::<Vec<String>, String>(None, UniqueKeepStrategy::First, None)
            .map(|distinct| distinct.height())
            .map_err(|fallback_error| {
                format!(
                    "No se pudieron detectar las filas duplicadas: {fallback_error} (streaming: {streaming_error})"
                )
            }),
    }
}

fn correlation_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    if let Some(value) = numeric_value(value.clone()) {
        return Some(value);
    }
    match value {
        AnyValue::String(value) => semantic_numeric_value(value),
        AnyValue::StringOwned(value) => semantic_numeric_value(value.as_str()),
        _ => None,
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum GroupKey {
    Missing,
    Value(String),
}

fn categorical_group_key(value: AnyValue<'_>) -> Option<GroupKey> {
    match value {
        AnyValue::Null => Some(GroupKey::Missing),
        AnyValue::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                Some(GroupKey::Missing)
            } else if value.chars().count() <= MAX_GROUP_LABEL_CHARS {
                Some(GroupKey::Value(value.to_owned()))
            } else {
                None
            }
        }
        AnyValue::StringOwned(value) => {
            let value = value.as_str().trim();
            if value.is_empty() {
                Some(GroupKey::Missing)
            } else if value.chars().count() <= MAX_GROUP_LABEL_CHARS {
                Some(GroupKey::Value(value.to_owned()))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn categorical_group_label(key: &GroupKey) -> String {
    match key {
        GroupKey::Missing => "Sin valor".to_owned(),
        GroupKey::Value(value) => value.clone(),
    }
}

fn retain_group_candidate(counts: &mut HashMap<GroupKey, usize>, key: GroupKey) {
    if let Some(count) = counts.get_mut(&key) {
        *count = (*count).saturating_add(1);
        return;
    }
    if counts.len() < MAX_GROUP_CANDIDATES {
        counts.insert(key, 1);
        return;
    }

    let Some((least_key, least_count)) = counts
        .iter()
        .min_by_key(|(_, count)| **count)
        .map(|(key, count)| (key.clone(), *count))
    else {
        return;
    };
    counts.remove(&least_key);
    counts.insert(key, least_count.saturating_add(1));
}

fn categorical_group_summary<C>(
    frame: &DataFrame,
    column: &Column,
    profile: &ColumnProfile,
    is_cancelled: &C,
) -> Result<Option<CategoricalGroupSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut candidates = HashMap::with_capacity(MAX_GROUP_CANDIDATES);
    for row_index in 0..frame.height() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column
            .get(row_index)
            .map_err(|error| format!("No se pudo resumir la columna {}: {error}", profile.name))?;
        if let Some(key) = categorical_group_key(value) {
            retain_group_candidate(&mut candidates, key);
        }
    }
    ensure_not_cancelled(is_cancelled())?;

    let mut selected_counts = HashMap::with_capacity(candidates.len());
    for row_index in 0..frame.height() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column
            .get(row_index)
            .map_err(|error| format!("No se pudo resumir la columna {}: {error}", profile.name))?;
        let Some(key) = categorical_group_key(value) else {
            continue;
        };
        if candidates.contains_key(&key) {
            let count = selected_counts.entry(key).or_insert(0usize);
            *count = (*count).saturating_add(1);
        }
    }

    let mut groups = selected_counts
        .into_iter()
        .filter(|(_, count)| *count >= MIN_GROUP_COUNT)
        .map(|(key, row_count)| CategoricalGroup {
            label: categorical_group_label(&key),
            row_count,
            percentage: if frame.height() == 0 {
                0.0
            } else {
                (row_count as f64 / frame.height() as f64) * 100.0
            },
            is_other: false,
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right
            .row_count
            .cmp(&left.row_count)
            .then_with(|| left.label.cmp(&right.label))
    });
    groups.truncate(MAX_CATEGORICAL_GROUPS);

    let displayed_count = groups.iter().map(|group| group.row_count).sum::<usize>();
    let other_count = frame.height().saturating_sub(displayed_count);
    if other_count > 0 {
        groups.push(CategoricalGroup {
            label: "Resto".to_owned(),
            row_count: other_count,
            percentage: if frame.height() == 0 {
                0.0
            } else {
                (other_count as f64 / frame.height() as f64) * 100.0
            },
            is_other: true,
        });
    }

    if groups.is_empty() {
        return Ok(None);
    }
    Ok(Some(CategoricalGroupSummary {
        column: profile.name.clone(),
        groups,
        distinct_count: profile.unique_count,
        truncated: other_count > 0,
    }))
}

fn categorical_group_summaries<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
) -> Result<Option<Vec<CategoricalGroupSummary>>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut summaries = Vec::new();
    for (column, profile) in frame.columns().iter().zip(profiles) {
        if summaries.len() >= MAX_CATEGORICAL_GROUP_COLUMNS {
            break;
        }
        // A category summary is useful for text dimensions, but raw identifiers,
        // contact fields and semantic numbers belong to other profile views.
        if column.dtype() != &DataType::String
            || profile.empty_count.is_none()
            || profile.suggested_type.is_some()
            || profile.privacy_signal.is_some()
            || profile.unique_count < 2
        {
            continue;
        }
        if let Some(summary) = categorical_group_summary(frame, column, profile, is_cancelled)? {
            summaries.push(summary);
        }
    }
    Ok((!summaries.is_empty()).then_some(summaries))
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Ord, PartialOrd)]
struct TemporalPeriodKey {
    year: i32,
    month: u32,
    day: u32,
}

fn temporal_period_key(value: NaiveDateTime) -> TemporalPeriodKey {
    TemporalPeriodKey {
        year: value.year(),
        month: value.month(),
        day: value.day(),
    }
}

fn temporal_period_label(key: TemporalPeriodKey, granularity: &str) -> String {
    match granularity {
        "year" => format!("{:04}", key.year),
        "day" => format!("{:04}-{:02}-{:02}", key.year, key.month, key.day),
        _ => format!("{:04}-{:02}", key.year, key.month),
    }
}

fn temporal_periods_between(
    first: TemporalPeriodKey,
    last: TemporalPeriodKey,
    granularity: &str,
    counts: &HashMap<TemporalPeriodKey, usize>,
) -> Vec<(String, usize)> {
    if granularity == "day" {
        let Some(first_date) = NaiveDate::from_ymd_opt(first.year, first.month, first.day) else {
            return Vec::new();
        };
        let Some(last_date) = NaiveDate::from_ymd_opt(last.year, last.month, last.day) else {
            return Vec::new();
        };
        let span = (last_date - first_date).num_days();
        return (0..=span.max(0))
            .map(|offset| {
                let date = first_date + chrono::Duration::days(offset);
                let key = TemporalPeriodKey {
                    year: date.year(),
                    month: date.month(),
                    day: date.day(),
                };
                (
                    temporal_period_label(key, granularity),
                    counts.get(&key).copied().unwrap_or(0),
                )
            })
            .collect();
    }

    if granularity == "month" {
        let span = (i64::from(last.year) - i64::from(first.year)) * 12 + i64::from(last.month)
            - i64::from(first.month);
        return (0..=span.max(0))
            .map(|offset| {
                let absolute_month =
                    i64::from(first.year) * 12 + i64::from(first.month.saturating_sub(1)) + offset;
                let year = absolute_month.div_euclid(12) as i32;
                let month = absolute_month.rem_euclid(12) as u32 + 1;
                let key = TemporalPeriodKey {
                    year,
                    month,
                    day: 1,
                };
                let count = counts
                    .iter()
                    .filter(|(period, _)| period.year == year && period.month == month)
                    .map(|(_, count)| *count)
                    .sum();
                (temporal_period_label(key, granularity), count)
            })
            .collect();
    }

    let year_span = i64::from(last.year) - i64::from(first.year);
    if (0..=MAX_TEMPORAL_PERIODS as i64).contains(&year_span) {
        return (0..=year_span)
            .map(|offset| {
                let key = TemporalPeriodKey {
                    year: first.year.saturating_add(offset as i32),
                    month: 1,
                    day: 1,
                };
                let count = counts
                    .iter()
                    .filter(|(period, _)| period.year == key.year)
                    .map(|(_, count)| *count)
                    .sum();
                (temporal_period_label(key, granularity), count)
            })
            .collect();
    }

    let mut years = counts.keys().map(|key| key.year).collect::<Vec<_>>();
    years.sort_unstable();
    years.dedup();
    years
        .into_iter()
        .map(|year| {
            let key = TemporalPeriodKey {
                year,
                month: 1,
                day: 1,
            };
            let count = counts
                .iter()
                .filter(|(period, _)| period.year == year)
                .map(|(_, count)| *count)
                .sum();
            (temporal_period_label(key, granularity), count)
        })
        .collect()
}

fn temporal_series_summary<C>(
    frame: &DataFrame,
    column: &Column,
    profile: &ColumnProfile,
    is_cancelled: &C,
) -> Result<Option<TemporalSeriesSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut counts = HashMap::<TemporalPeriodKey, usize>::new();
    let mut first = None;
    let mut last = None;
    let mut parsed_row_count = 0usize;

    for row_index in 0..frame.height() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column.get(row_index).map_err(|error| {
            format!(
                "No se pudo resumir la tendencia temporal de {}: {error}",
                profile.name
            )
        })?;
        let Some(datetime) = quality_datetime_value(value) else {
            continue;
        };
        let key = temporal_period_key(datetime);
        first = Some(first.map_or(key, |current: TemporalPeriodKey| current.min(key)));
        last = Some(last.map_or(key, |current: TemporalPeriodKey| current.max(key)));
        *counts.entry(key).or_insert(0) += 1;
        parsed_row_count = parsed_row_count.saturating_add(1);
    }
    ensure_not_cancelled(is_cancelled())?;

    let (Some(first), Some(last)) = (first, last) else {
        return Ok(None);
    };
    let first_date = NaiveDate::from_ymd_opt(first.year, first.month, first.day)
        .ok_or_else(|| "No se pudo interpretar el inicio de la tendencia temporal.".to_owned())?;
    let last_date = NaiveDate::from_ymd_opt(last.year, last.month, last.day)
        .ok_or_else(|| "No se pudo interpretar el fin de la tendencia temporal.".to_owned())?;
    let day_span = (last_date - first_date).num_days();
    let month_span = (i64::from(last.year) - i64::from(first.year)) * 12 + i64::from(last.month)
        - i64::from(first.month);
    let granularity = if day_span <= MAX_TEMPORAL_DAY_SPAN {
        "day"
    } else if month_span <= MAX_TEMPORAL_MONTH_SPAN {
        "month"
    } else {
        "year"
    };
    let mut raw_periods = temporal_periods_between(first, last, granularity, &counts);
    raw_periods.retain(|(_, count)| *count > 0 || matches!(granularity, "month" | "day"));

    let truncated = raw_periods.len() > MAX_TEMPORAL_PERIODS;
    if truncated {
        let split = raw_periods.len() - (MAX_TEMPORAL_PERIODS - 1);
        let previous_count = raw_periods[..split]
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>();
        let mut retained = vec![("Periodos anteriores".to_owned(), previous_count)];
        retained.extend(raw_periods.into_iter().skip(split));
        raw_periods = retained;
    }
    let denominator = parsed_row_count.max(1) as f64;
    let periods = raw_periods
        .into_iter()
        .map(|(period, row_count)| TemporalPeriod {
            period,
            row_count,
            percentage: (row_count as f64 / denominator) * 100.0,
        })
        .collect::<Vec<_>>();

    Ok(Some(TemporalSeriesSummary {
        column: profile.name.clone(),
        granularity: granularity.to_owned(),
        periods,
        parsed_row_count,
        unparsed_row_count: frame.height().saturating_sub(parsed_row_count),
        truncated,
    }))
}

fn temporal_series_summaries<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
) -> Result<Option<Vec<TemporalSeriesSummary>>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut summaries = Vec::new();
    for (column, profile) in frame.columns().iter().zip(profiles) {
        if summaries.len() >= MAX_TEMPORAL_COLUMNS
            || profile.privacy_signal.is_some()
            || (!matches!(column.dtype(), DataType::Date | DataType::Datetime(_, _))
                && profile.suggested_type.as_deref() != Some("date"))
        {
            continue;
        }
        if let Some(summary) = temporal_series_summary(frame, column, profile, is_cancelled)? {
            summaries.push(summary);
        }
    }
    Ok((!summaries.is_empty()).then_some(summaries))
}

fn numeric_correlation_matrix<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
) -> Result<Option<NumericCorrelationMatrix>, String>
where
    C: Fn() -> bool + Sync,
{
    let numeric_columns = frame
        .columns()
        .iter()
        .zip(profiles)
        .filter(|(_, profile)| profile.outlier_count.is_some())
        .take(MAX_NUMERIC_CORRELATION_COLUMNS)
        .map(|(column, profile)| (column, profile.name.as_str()))
        .collect::<Vec<_>>();

    if numeric_columns.len() < 2 {
        return Ok(None);
    }

    let row_count = frame.height();
    if row_count == 0 {
        return Ok(None);
    }
    let sampled_row_count = row_count.min(MAX_NUMERIC_CORRELATION_SAMPLE_ROWS);
    let mut values = Vec::with_capacity(numeric_columns.len());
    for (column_index, (column, _)) in numeric_columns.iter().enumerate() {
        let mut column_values = Vec::with_capacity(sampled_row_count);
        for sample_index in 0..sampled_row_count {
            if sample_index % 4096 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            // Evenly sample the frame so a large file is not represented only by its header.
            let row_index = sample_index.saturating_mul(row_count) / sampled_row_count;
            let value = column.get(row_index).map_err(|error| {
                format!(
                    "No se pudieron calcular correlaciones para la columna {}: {error}",
                    numeric_columns[column_index].1
                )
            })?;
            column_values.push(correlation_numeric_value(value));
        }
        values.push(column_values);
    }

    let mut pairs =
        Vec::with_capacity(numeric_columns.len().saturating_mul(numeric_columns.len()) / 2);
    for first_index in 0..numeric_columns.len() {
        for second_index in (first_index + 1)..numeric_columns.len() {
            let (coefficient, sample_count) =
                pearson_correlation(&values[first_index], &values[second_index]);
            pairs.push(NumericCorrelation {
                first_column: numeric_columns[first_index].1.to_owned(),
                second_column: numeric_columns[second_index].1.to_owned(),
                coefficient,
                sample_count,
            });
        }
    }

    Ok(Some(NumericCorrelationMatrix {
        columns: numeric_columns
            .into_iter()
            .map(|(_, name)| name.to_owned())
            .collect(),
        pairs,
        sampled_row_count,
        truncated: profiles
            .iter()
            .filter(|profile| profile.outlier_count.is_some())
            .count()
            > MAX_NUMERIC_CORRELATION_COLUMNS,
    }))
}

fn pearson_correlation(first: &[Option<f64>], second: &[Option<f64>]) -> (Option<f64>, usize) {
    let mut paired = Vec::new();
    for (first, second) in first.iter().zip(second) {
        if let (Some(first), Some(second)) = (first, second) {
            paired.push((*first, *second));
        }
    }

    let sample_count = paired.len();
    if sample_count < 2 {
        return (None, sample_count);
    }
    let first_mean = paired.iter().map(|(first, _)| first).sum::<f64>() / sample_count as f64;
    let second_mean = paired.iter().map(|(_, second)| second).sum::<f64>() / sample_count as f64;
    let mut covariance = 0.0;
    let mut first_variance = 0.0;
    let mut second_variance = 0.0;
    for (first, second) in paired {
        let first_delta = first - first_mean;
        let second_delta = second - second_mean;
        covariance += first_delta * second_delta;
        first_variance += first_delta * first_delta;
        second_variance += second_delta * second_delta;
    }
    let denominator = (first_variance * second_variance).sqrt();
    let coefficient = if denominator > 0.0 && denominator.is_finite() {
        Some((covariance / denominator).clamp(-1.0, 1.0))
    } else {
        None
    };
    (coefficient, sample_count)
}

fn profile_dataset_with_progress<F, C>(
    frame: &DataFrame,
    mut report: F,
    is_cancelled: C,
) -> Result<DatasetProfile, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let row_count = frame.height();
    report("Detectando filas duplicadas", 10);
    let distinct_row_count = count_distinct_rows(frame)?;
    let duplicate_row_count = row_count.saturating_sub(distinct_row_count);
    let duplicate_percentage = if row_count == 0 {
        0.0
    } else {
        (duplicate_row_count as f64 / row_count as f64) * 100.0
    };
    report("Normalizando filas parecidas", 15);
    let near_duplicate_row_count =
        count_normalized_duplicate_rows(frame, duplicate_row_count, &is_cancelled, &mut report)?;
    ensure_not_cancelled(is_cancelled())?;
    let source_columns = frame.columns();
    let mut columns = Vec::with_capacity(source_columns.len());
    if source_columns.is_empty() {
        report("Perfil completado", 100);
    } else {
        report("Analizando columnas", 40);
        let column_count = source_columns.len();
        let worker_count = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get().min(4))
            .unwrap_or(2)
            .min(column_count)
            .max(1);
        let work_queue =
            std::sync::Arc::new(Mutex::new((0..column_count).collect::<VecDeque<_>>()));
        let (sender, receiver) = mpsc::channel();
        let mut profiles = (0..column_count)
            .map(|_| None)
            .collect::<Vec<Option<ColumnProfile>>>();
        let mut progress = vec![0_u16; column_count];
        let mut completed_columns = 0usize;
        let mut first_error = None;
        let mut last_percent = 40_u8;

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let work_queue = std::sync::Arc::clone(&work_queue);
                let sender = sender.clone();
                let cancellation = &is_cancelled;
                scope.spawn(move || {
                    while let Some(index) = work_queue
                        .lock()
                        .ok()
                        .and_then(|mut queue| queue.pop_front())
                    {
                        let column = &source_columns[index];
                        let progress_sender = sender.clone();
                        let result =
                            profile_column(column, row_count, cancellation, |stage, phase| {
                                let _ = progress_sender.send(ProfileColumnEvent::Progress {
                                    index,
                                    stage,
                                    phase,
                                });
                            });
                        let _ = sender.send(ProfileColumnEvent::Completed {
                            index,
                            result: Box::new(result),
                        });
                    }
                });
            }
            drop(sender);

            while let Ok(event) = receiver.recv() {
                match event {
                    ProfileColumnEvent::Progress {
                        index,
                        stage,
                        phase,
                    } => {
                        progress[index] = progress[index].max(u16::from(phase));
                        let weighted_progress = progress
                            .iter()
                            .map(|value| usize::from(*value))
                            .sum::<usize>();
                        let percent = 40 + ((weighted_progress * 50) / (column_count * 100)) as u8;
                        last_percent = last_percent.max(percent);
                        report(stage, last_percent);
                    }
                    ProfileColumnEvent::Completed { index, result } => {
                        progress[index] = 100;
                        completed_columns += 1;
                        match *result {
                            Ok(profile) => profiles[index] = Some(profile),
                            Err(error) => {
                                first_error.get_or_insert(error);
                            }
                        }
                        let weighted_progress = progress
                            .iter()
                            .map(|value| usize::from(*value))
                            .sum::<usize>();
                        let percent = 40 + ((weighted_progress * 50) / (column_count * 100)) as u8;
                        last_percent = last_percent.max(percent);
                        report("Analizando columnas", last_percent);
                        if completed_columns == column_count && last_percent < 90 {
                            report("Analizando columnas", 90);
                        }
                    }
                }
            }
        });

        if let Some(error) = first_error {
            return Err(error);
        }
        columns = profiles
            .into_iter()
            .map(|profile| profile.expect("cada columna debe producir un perfil"))
            .collect();
    }

    let categorical_group_summaries = if columns.is_empty() {
        None
    } else {
        report("Resumiendo categorías", 93);
        categorical_group_summaries(frame, &columns, &is_cancelled)?
    };

    let temporal_series = if columns.is_empty() {
        None
    } else {
        report("Resumiendo tendencia temporal", 95);
        temporal_series_summaries(frame, &columns, &is_cancelled)?
    };

    let numeric_correlations = if columns.is_empty() {
        None
    } else {
        report("Calculando correlaciones", 97);
        let correlations = numeric_correlation_matrix(frame, &columns, &is_cancelled)?;
        report("Analizando columnas", 100);
        correlations
    };

    Ok(DatasetProfile {
        row_count,
        duplicate_row_count,
        near_duplicate_row_count,
        duplicate_percentage,
        columns,
        numeric_correlations,
        categorical_group_summaries,
        temporal_series,
    })
}

fn count_normalized_duplicate_rows<C, F>(
    frame: &DataFrame,
    exact_duplicate_row_count: usize,
    is_cancelled: &C,
    report: &mut F,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
    F: FnMut(&'static str, u8),
{
    if frame.height() == 0 {
        return Ok(0);
    }

    let fingerprint_columns = normalized_fingerprint_columns(frame.columns())?;
    let chunk_count = frame.height().div_ceil(NORMALIZED_DUPLICATE_CHUNK_ROWS);
    let spill_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}")
    })?;
    let bucket_paths = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|bucket| {
            spill_directory
                .path()
                .join(format!("fingerprints-{bucket:03}.bin"))
        })
        .collect::<Vec<_>>();
    let mut bucket_writers = (0..NORMALIZED_DUPLICATE_BUCKETS)
        .map(|_| None::<BufWriter<File>>)
        .collect::<Vec<_>>();
    let (progress_sender, progress_receiver) = mpsc::sync_channel(2);
    let producer_result = std::thread::scope(|scope| {
        let producer = scope.spawn(move || {
            (0..chunk_count)
                .into_par_iter()
                .try_for_each(|chunk_index| {
                    if is_cancelled() {
                        return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                    }

                    let start = chunk_index * NORMALIZED_DUPLICATE_CHUNK_ROWS;
                    let end = (start + NORMALIZED_DUPLICATE_CHUNK_ROWS).min(frame.height());
                    let mut fingerprints = Vec::with_capacity(end - start);
                    for row_index in start..end {
                        if row_index % 4096 == 0 && is_cancelled() {
                            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
                        }
                        let key = normalized_row_fingerprint(&fingerprint_columns, row_index)?;
                        fingerprints.push(key);
                    }
                    progress_sender
                        .send((chunk_index, fingerprints))
                        .map_err(|_| OPERATION_CANCELLED_MESSAGE.to_owned())
                })
        });

        let mut completed_chunks = 0_usize;
        let mut spill_error = None;
        while let Ok((_, mut partial)) = progress_receiver.recv() {
            if spill_error.is_none() && !is_cancelled() {
                for (fingerprint_index, fingerprint) in partial.drain(..).enumerate() {
                    if fingerprint_index % 4096 == 0 && is_cancelled() {
                        break;
                    }
                    let bucket = (fingerprint >> 120) as usize;
                    let writer = if let Some(writer) = bucket_writers[bucket].as_mut() {
                        writer
                    } else {
                        let file = match File::create(&bucket_paths[bucket]) {
                            Ok(file) => file,
                            Err(error) => {
                                spill_error = Some(format!(
                                    "No se pudo preparar el almacenamiento temporal para duplicados parecidos: {error}"
                                ));
                                break;
                            }
                        };
                        bucket_writers[bucket]
                            .get_or_insert_with(|| BufWriter::with_capacity(64 * 1024, file))
                    };
                    if let Err(error) = writer.write_all(&fingerprint.to_le_bytes()) {
                        spill_error = Some(format!(
                            "No se pudieron guardar las huellas temporales de duplicados parecidos: {error}"
                        ));
                        break;
                    }
                }
            }
            completed_chunks += 1;
            let percent = 15 + ((completed_chunks.saturating_mul(25) / chunk_count).min(25)) as u8;
            report("Normalizando filas parecidas", percent);
        }

        let producer_result = producer
            .join()
            .map_err(|_| "El perfilado paralelo se interrumpió inesperadamente.".to_owned())?;
        producer_result?;
        ensure_not_cancelled(is_cancelled())?;
        if let Some(error) = spill_error {
            return Err(error);
        }

        for writer in bucket_writers.iter_mut().flatten() {
            writer.flush().map_err(|error| {
                format!(
                    "No se pudieron sincronizar las huellas temporales de duplicados parecidos: {error}"
                )
            })?;
        }
        drop(bucket_writers);
        Ok(())
    });
    producer_result?;

    report("Ordenando filas parecidas", 40);
    let mut normalized_duplicate_row_count = 0usize;
    for bucket_path in &bucket_paths {
        ensure_not_cancelled(is_cancelled())?;
        if !bucket_path.exists() {
            continue;
        }
        let bytes = fs::metadata(bucket_path)
            .map_err(|error| {
                format!("No se pudo inspeccionar el almacenamiento temporal: {error}")
            })?
            .len();
        if bytes % NORMALIZED_FINGERPRINT_BYTES as u64 != 0 {
            return Err(
                "El almacenamiento temporal de duplicados parecidos quedó incompleto.".to_owned(),
            );
        }
        let fingerprint_count = usize::try_from(bytes / NORMALIZED_FINGERPRINT_BYTES as u64)
            .map_err(|_| "El conteo de huellas temporales excede la capacidad local.".to_owned())?;
        let file = File::open(bucket_path).map_err(|error| {
            format!("No se pudo leer el almacenamiento temporal de duplicados parecidos: {error}")
        })?;
        let mut reader = BufReader::new(file);
        let mut fingerprints = Vec::with_capacity(fingerprint_count);
        let mut encoded = [0_u8; NORMALIZED_FINGERPRINT_BYTES];
        for index in 0..fingerprint_count {
            if index % 4096 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            reader.read_exact(&mut encoded).map_err(|error| {
                format!("No se pudo leer una huella temporal de duplicados parecidos: {error}")
            })?;
            fingerprints.push(u128::from_le_bytes(encoded));
        }
        fingerprints.sort_unstable();
        normalized_duplicate_row_count = normalized_duplicate_row_count.saturating_add(
            fingerprints
                .windows(2)
                .filter(|pair| pair[0] == pair[1])
                .count(),
        );
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(normalized_duplicate_row_count.saturating_sub(exact_duplicate_row_count))
}

#[cfg(test)]
fn profile_dataset(frame: &DataFrame) -> Result<DatasetProfile, String> {
    profile_dataset_with_progress(frame, |_, _| {}, || false)
}

fn dataset_preview(path: &Path, frame: &DataFrame) -> Result<DatasetPreview, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset.csv");
    dataset_preview_named(path, file_name, frame)
}

fn dataset_preview_named(
    storage_path: &Path,
    file_name: &str,
    frame: &DataFrame,
) -> Result<DatasetPreview, String> {
    let file_size_bytes = fs::metadata(storage_path)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();
    dataset_preview_with_size(file_name, file_size_bytes, frame)
}

fn dataset_preview_with_size(
    file_name: &str,
    file_size_bytes: u64,
    frame: &DataFrame,
) -> Result<DatasetPreview, String> {
    let columns = frame
        .columns()
        .iter()
        .map(|column| DatasetColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
        })
        .collect();
    let rows = dataset_page(frame, 0, PREVIEW_ROW_LIMIT)?.rows;

    Ok(DatasetPreview {
        file_name: file_name.to_owned(),
        file_size_bytes,
        row_count: frame.height(),
        column_count: frame.width(),
        columns,
        rows,
    })
}

fn loaded_dataset_preview(
    dataset: &LoadedDataset,
    frame: &DataFrame,
) -> Result<DatasetPreview, String> {
    match dataset.source_path.as_deref() {
        Some(path) => dataset_preview_named(path, &dataset.file_name, frame),
        None => dataset_preview_with_size(&dataset.file_name, dataset.file_size_bytes, frame),
    }
}

fn remove_duplicate_rows(frame: &DataFrame) -> Result<(DataFrame, usize), String> {
    let cleaned = frame
        .unique_stable(None, UniqueKeepStrategy::First, None)
        .map_err(|error| format!("No se pudieron eliminar las filas duplicadas: {error}"))?;
    let affected_row_count = frame.height().saturating_sub(cleaned.height());
    Ok((cleaned, affected_row_count))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NearDuplicateFingerprint {
    normalized: NormalizedRowFingerprint,
    exact: u128,
    row_index: usize,
}

fn remove_near_duplicate_rows(frame: &DataFrame) -> Result<(DataFrame, usize), String> {
    if frame.height() < 2 {
        return Ok((frame.clone(), 0));
    }

    let fingerprint_columns = normalized_fingerprint_columns(frame.columns())?;
    let columns = frame.columns();
    let mut fingerprints = (0..frame.height())
        .into_par_iter()
        .map(|row_index| {
            Ok(NearDuplicateFingerprint {
                normalized: normalized_row_fingerprint(&fingerprint_columns, row_index)?,
                exact: row_fingerprint(columns, row_index, false)?,
                row_index,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    fingerprints
        .par_sort_unstable_by_key(|fingerprint| (fingerprint.normalized, fingerprint.row_index));

    let mut keep = vec![true; frame.height()];
    let mut removed_count = 0;
    let mut group_start = 0;
    while group_start < fingerprints.len() {
        let normalized = fingerprints[group_start].normalized;
        let mut group_end = group_start + 1;
        while group_end < fingerprints.len() && fingerprints[group_end].normalized == normalized {
            group_end += 1;
        }

        // Keep the earliest normalized row. Exact repeats in the same normalized
        // group stay untouched so this operation only handles the near-duplicate delta.
        let mut seen_exact = HashSet::with_capacity(group_end - group_start);
        for (position, fingerprint) in fingerprints[group_start..group_end].iter().enumerate() {
            if position == 0 {
                seen_exact.insert(fingerprint.exact);
                continue;
            }
            if !seen_exact.insert(fingerprint.exact) {
                continue;
            }
            keep[fingerprint.row_index] = false;
            removed_count += 1;
        }

        group_start = group_end;
    }

    if removed_count == 0 {
        return Ok((frame.clone(), 0));
    }

    let cleaned = frame
        .filter(&BooleanChunked::from_slice(
            "near_duplicate_row".into(),
            &keep,
        ))
        .map_err(|error| {
            format!("No se pudieron eliminar las filas duplicadas parecidas: {error}")
        })?;
    Ok((cleaned, removed_count))
}

fn remove_empty_rows_from_frame(frame: &DataFrame) -> Result<(DataFrame, usize), String> {
    let keep = (0..frame.height())
        .map(|row_index| {
            frame.columns().iter().any(|column| {
                column
                    .get(row_index)
                    .ok()
                    .and_then(preview_value)
                    .is_some_and(|value| !value.trim().is_empty())
            })
        })
        .collect::<Vec<_>>();
    let cleaned = frame
        .filter(&BooleanChunked::from_slice("non_empty_row".into(), &keep))
        .map_err(|error| format!("No se pudieron eliminar las filas vacías: {error}"))?;
    let affected_row_count = frame.height().saturating_sub(cleaned.height());
    Ok((cleaned, affected_row_count))
}

fn remove_null_only_rows_from_frame(frame: &DataFrame) -> Result<(DataFrame, usize), String> {
    let keep = (0..frame.height())
        .map(|row_index| {
            frame
                .columns()
                .iter()
                .map(|column| {
                    column
                        .get(row_index)
                        .map(|value| !matches!(value, AnyValue::Null))
                        .map_err(|error| format!("No se pudo leer la fila vacía: {error}"))
                })
                .try_fold(false, |has_value, value| {
                    value.map(|value| has_value || value)
                })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let cleaned = frame
        .filter(&BooleanChunked::from_slice("non_null_row".into(), &keep))
        .map_err(|error| format!("No se pudieron eliminar las filas nulas: {error}"))?;
    let affected_row_count = frame.height().saturating_sub(cleaned.height());
    Ok((cleaned, affected_row_count))
}

fn remove_dataprep_columns_from_frame(
    frame: &DataFrame,
    candidates: Vec<String>,
    error_message: &str,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }
    let removed_columns = candidates
        .into_iter()
        .take(frame.width().saturating_sub(1))
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }
    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("{error_message}: {error}"))?;
    Ok((cleaned, removed_columns))
}

fn dataprep_numeric_text(value: &str) -> bool {
    let value = value.trim();
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    !integer.is_empty()
        && integer.chars().all(|character| character.is_ascii_digit())
        && fraction.is_none_or(|fraction| {
            !fraction.is_empty() && fraction.chars().all(|character| character.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn dataprep_leading_zero_text(value: &str) -> bool {
    let value = value.trim().trim_start_matches(['+', '-']);
    value.starts_with('0')
        && value
            .chars()
            .nth(1)
            .is_some_and(|character| character.is_ascii_digit())
}

fn dataprep_numeric_or_date_text_column(column: &Column) -> Result<bool, String> {
    if matches!(
        column.dtype(),
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
            | DataType::Date
            | DataType::Datetime(_, _)
    ) {
        return Ok(true);
    }
    if column.dtype() != &DataType::String {
        return Ok(false);
    }
    let values = column
        .str()
        .map_err(|error| format!("No se pudo inferir la columna '{}': {error}", column.name()))?
        .iter()
        .flatten()
        .take(50)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Ok(false);
    }
    let numeric_count = values
        .iter()
        .filter(|value| dataprep_numeric_text(value))
        .count();
    if numeric_count * 10 > values.len() * 8
        && !values.iter().any(|value| dataprep_leading_zero_text(value))
    {
        return Ok(true);
    }
    let date_count = values
        .iter()
        .filter(|value| is_supported_date(value.trim()))
        .count();
    Ok(date_count * 10 > values.len() * 8)
}

fn remove_dataprep_high_null_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    let candidates = frame
        .columns()
        .iter()
        .filter(|column| {
            let null_count = column.null_count();
            null_count > 0 && null_count.saturating_mul(100) > frame.height().saturating_mul(80)
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    remove_dataprep_columns_from_frame(
        frame,
        candidates,
        "No se pudieron eliminar columnas DataPrep con alta nulidad",
    )
}

fn remove_dataprep_identifier_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    let mut candidates = Vec::new();
    for column in frame.columns() {
        if column.null_count() != 0
            || column.n_unique().map_err(|error| {
                format!("No se pudo contar la columna '{}': {error}", column.name())
            })? != frame.height()
        {
            continue;
        }
        if !dataprep_numeric_or_date_text_column(column)? {
            candidates.push(column.name().to_string());
        }
    }
    remove_dataprep_columns_from_frame(
        frame,
        candidates,
        "No se pudieron eliminar columnas identificadoras de DataPrep",
    )
}

fn remove_constant_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() <= 1 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter_map(|column| {
            let null_count = column.null_count();
            let unique_count = column
                .n_unique()
                .ok()?
                .saturating_sub(usize::from(null_count > 0));
            (unique_count <= 1 && null_count < frame.height()).then(|| column.name().to_string())
        })
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas constantes: {error}"))?;
    Ok((cleaned, removed_columns))
}

fn remove_empty_columns_from_frame(frame: &DataFrame) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| column.null_count() == frame.height())
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas vacías: {error}"))?;
    Ok((cleaned, removed_columns))
}

fn remove_high_null_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| {
            let null_count = column.null_count();
            null_count > 0
                && null_count < frame.height()
                && null_count.saturating_mul(100)
                    >= frame
                        .height()
                        .saturating_mul(HIGH_NULL_COLUMN_THRESHOLD_PERCENTAGE)
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas con alta nulidad: {error}"))?;
    Ok((cleaned, removed_columns))
}

fn remove_identifier_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| matches!(privacy_signal(column.name()), Some("identifier")))
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron retirar columnas identificadoras: {error}"))?;
    Ok((cleaned, removed_columns))
}

fn remove_personal_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| {
            column.name() != "_cambios"
                && matches!(
                    privacy_signal(column.name()),
                    Some("email" | "phone" | "address" | "name")
                )
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame.select(&remaining_columns).map_err(|error| {
        format!("No se pudieron retirar columnas con datos personales: {error}")
    })?;
    Ok((cleaned, removed_columns))
}

fn is_personal_privacy_signal(column_name: &str) -> bool {
    matches!(
        privacy_signal(column_name),
        Some("email" | "phone" | "address" | "name")
    )
}

fn mask_personal_values_from_frame(frame: &DataFrame) -> Result<(DataFrame, usize, usize), String> {
    let mut masked = frame.clone();
    let mut changed_cell_count = 0;
    let mut changed_column_count = 0;

    for column in frame
        .columns()
        .iter()
        .filter(|column| column.name() != "_cambios" && is_personal_privacy_signal(column.name()))
    {
        let mut column_changed_cell_count = 0;
        let values = (0..column.len())
            .map(|row_index| {
                column
                    .get(row_index)
                    .map_err(|_| {
                        "No se pudo leer una columna personal para proteger sus valores.".to_owned()
                    })
                    .map(|value| match value {
                        AnyValue::Null => None,
                        value => {
                            let already_redacted = match &value {
                                AnyValue::String(current) => *current == REDACTED_VALUE,
                                AnyValue::StringOwned(current) => {
                                    current.as_str() == REDACTED_VALUE
                                }
                                _ => false,
                            };
                            if !already_redacted {
                                column_changed_cell_count += 1;
                            }
                            Some(REDACTED_VALUE.to_owned())
                        }
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;

        if column_changed_cell_count > 0 {
            changed_cell_count += column_changed_cell_count;
            changed_column_count += 1;
        }
        masked
            .replace(
                column.name().as_str(),
                Column::new(column.name().clone(), values),
            )
            .map_err(|_| "No se pudo proteger una columna de datos personales.".to_owned())?;
    }

    Ok((masked, changed_cell_count, changed_column_count))
}

fn normalize_column_name(name: &str) -> String {
    let decomposed = name
        .nfd()
        .filter(|character| !is_combining_mark(*character));
    let mut normalized = String::new();
    let mut pending_separator = false;

    for character in decomposed.flat_map(char::to_lowercase) {
        if character.is_whitespace() || character == '-' {
            pending_separator = true;
            continue;
        }
        if character.is_alphanumeric() || character == '_' {
            if pending_separator {
                normalized.push('_');
                pending_separator = false;
            }
            normalized.push(character);
        }
    }

    if normalized.is_empty() {
        normalized.push_str("unnamed");
    } else if normalized
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        normalized.insert_str(0, "col_");
    }
    normalized
}

fn normalized_column_names(frame: &DataFrame) -> (Vec<String>, Vec<ColumnRename>) {
    let mut unique = Vec::with_capacity(frame.width());
    let mut counts = std::collections::HashMap::<String, usize>::new();
    let mut renames = Vec::new();

    for column in frame.get_column_names() {
        let original = column.as_str();
        let base = normalize_column_name(original);
        let count = counts.entry(base.clone()).or_insert(0);
        *count += 1;
        let mut candidate = if *count == 1 {
            base.clone()
        } else {
            format!("{base}_{}", *count)
        };
        while unique.contains(&candidate) {
            *count += 1;
            candidate = format!("{base}_{}", *count);
        }
        if original != candidate {
            renames.push(ColumnRename {
                from: original.to_owned(),
                to: candidate.clone(),
            });
        }
        unique.push(candidate);
    }

    (unique, renames)
}

#[derive(Clone, Copy)]
enum TextCleaningMode {
    Trim,
    Normalize { remove_accents: bool },
    Sentinels,
    Booleans,
    FixEncoding,
    NullifyInvalidTypes,
}

pub(crate) fn normalize_text_value(value: &str, remove_accents: bool) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let lowered = collapsed.chars().flat_map(char::to_lowercase);
    if remove_accents {
        lowered
            .collect::<String>()
            .nfd()
            .filter(|character| !is_combining_mark(*character))
            .collect()
    } else {
        lowered.collect()
    }
}

fn clean_text_columns(
    frame: &DataFrame,
    selected_columns: Option<&[String]>,
    mode: TextCleaningMode,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    if selected_columns.is_some_and(|columns| columns.is_empty()) {
        return Err("Selecciona al menos una columna de texto.".to_owned());
    }

    let column_names = match selected_columns {
        Some(columns) => columns.to_vec(),
        None => frame
            .columns()
            .iter()
            .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
            .filter(|column| {
                !matches!(mode, TextCleaningMode::Booleans)
                    || is_boolean_candidate(column).unwrap_or(false)
            })
            .map(|column| column.name().to_string())
            .collect(),
    };

    for name in column_names {
        if name == "_cambios" {
            return Err("La columna de trazabilidad _cambios no se modifica.".to_owned());
        }
        let column = frame
            .column(&name)
            .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))?;
        if column.dtype() != &DataType::String {
            return Err(format!("La columna '{name}' no es de texto."));
        }
        let values = column
            .str()
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        let suggested_type = if matches!(mode, TextCleaningMode::NullifyInvalidTypes) {
            text_statistics(column)?.and_then(|statistics| statistics.suggested_type)
        } else {
            None
        };
        let mut column_changes = 0;
        let transformed: Vec<Option<String>> = values
            .iter()
            .enumerate()
            .map(|(row_index, value)| {
                value.and_then(|original| {
                    if matches!(mode, TextCleaningMode::NullifyInvalidTypes)
                        && suggested_type.is_some_and(|kind| {
                            !original.trim().is_empty() && !is_valid_suggested_type(original, kind)
                        })
                    {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                        return None;
                    }
                    if matches!(mode, TextCleaningMode::Sentinels) && is_missing_sentinel(original)
                    {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                        return None;
                    }

                    let next = match mode {
                        TextCleaningMode::Trim => original.trim().to_owned(),
                        TextCleaningMode::Normalize { remove_accents } => {
                            normalize_text_value(original, remove_accents)
                        }
                        TextCleaningMode::Sentinels => original.to_owned(),
                        TextCleaningMode::Booleans => {
                            boolean_token(original).unwrap_or(original).to_owned()
                        }
                        TextCleaningMode::FixEncoding => {
                            repair_mojibake(original).unwrap_or_else(|| original.to_owned())
                        }
                        TextCleaningMode::NullifyInvalidTypes => original.to_owned(),
                    };
                    if next != original {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                    }
                    Some(next)
                })
            })
            .collect();

        if column_changes > 0 {
            cleaned
                .replace(&name, Column::new(name.clone().into(), transformed))
                .map_err(|error| format!("No se pudo actualizar la columna '{name}': {error}"))?;
            changed_columns.push(ChangedTextColumn {
                name,
                changed_cell_count: column_changes,
            });
        }
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn dataprep_boolean_token(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "si" | "sí" | "yes" | "y" | "true" | "verdadero" | "1" => Some(true),
        "no" | "n" | "false" | "falso" | "0" => Some(false),
        _ => None,
    }
}

fn normalize_dataprep_boolean_columns(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || column.dtype() != &DataType::String {
            continue;
        }
        let values = column
            .str()
            .map_err(|error| format!("No se pudo leer la columna booleana '{name}': {error}"))?;
        let tokens = values
            .iter()
            .flatten()
            .map(|value| value.trim().to_lowercase())
            .collect::<Vec<_>>();
        if tokens.is_empty()
            || !tokens
                .iter()
                .all(|value| dataprep_boolean_token(value).is_some())
            || !tokens
                .iter()
                .any(|value| dataprep_boolean_token(value) == Some(true))
            || !tokens
                .iter()
                .any(|value| dataprep_boolean_token(value) == Some(false))
        {
            continue;
        }

        let mut transformed = Vec::with_capacity(values.len());
        for (row_index, value) in values.iter().enumerate() {
            let Some(value) = value else {
                transformed.push(None);
                continue;
            };
            let next = dataprep_boolean_token(value).ok_or_else(|| {
                format!("La columna booleana '{name}' contiene un token no reconocido.")
            })?;
            changed_rows[row_index] = true;
            changed_cell_count += 1;
            transformed.push(Some(next));
        }
        let column_changes = transformed.iter().filter(|value| value.is_some()).count();
        cleaned
            .replace(&name, Column::new(name.clone().into(), transformed))
            .map_err(|error| {
                format!("No se pudo normalizar la columna booleana '{name}': {error}")
            })?;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: column_changes,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn parse_dataprep_date_columns(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    const INFERENCE_THRESHOLD_PERCENTAGE: usize = 80;
    const MAX_EXTRA_NULL_PERCENTAGE: usize = 1;

    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios"
            || !matches!(
                column.dtype(),
                DataType::String | DataType::Date | DataType::Datetime(_, _)
            )
        {
            continue;
        }
        if matches!(column.dtype(), DataType::Date | DataType::Datetime(_, _)) {
            continue;
        }

        let values = strict_column_text(column)?;
        let non_null_count = values.iter().filter(|value| value.is_some()).count();
        if non_null_count == 0 {
            continue;
        }
        let sample = values
            .iter()
            .filter_map(Option::as_deref)
            .take(50)
            .collect::<Vec<_>>();
        if sample.is_empty() {
            continue;
        }

        let inferred_format = DATAPREP_DATE_FORMATS.iter().copied().find(|format| {
            let parsed = sample
                .iter()
                .filter_map(|value| parse_dataprep_datetime(value, *format))
                .collect::<Vec<_>>();
            parsed.len() * 100 > sample.len() * INFERENCE_THRESHOLD_PERCENTAGE
                && parsed
                    .iter()
                    .all(|value| (1900..=2100).contains(&value.year()))
        });
        let Some(inferred_format) = inferred_format else {
            continue;
        };

        let parse_value = |value: &str| parse_dataprep_datetime(value, inferred_format);
        let parsed = values
            .iter()
            .map(|value| value.as_deref().and_then(parse_value))
            .collect::<Vec<_>>();
        let parsed_count = parsed.iter().filter(|value| value.is_some()).count();
        let extra_null_count = non_null_count.saturating_sub(parsed_count);
        if parsed_count * 100 <= sample.len() * INFERENCE_THRESHOLD_PERCENTAGE
            || extra_null_count * 100 > values.len() * MAX_EXTRA_NULL_PERCENTAGE
            || parsed
                .iter()
                .flatten()
                .any(|value| !(1900..=2100).contains(&value.year()))
        {
            continue;
        }

        let milliseconds = parsed
            .into_iter()
            .enumerate()
            .map(|(row_index, value)| {
                let parsed = value.map(|value| value.and_utc().timestamp_millis());
                if parsed.is_some() {
                    changed_rows[row_index] = true;
                }
                parsed
            })
            .collect::<Vec<_>>();
        let converted = Series::new(column.name().clone(), milliseconds)
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
            .map_err(|error| {
                format!("No se pudo interpretar la columna de fechas '{name}': {error}")
            })?
            .into_column();
        cleaned.replace(&name, converted).map_err(|error| {
            format!("No se pudo actualizar la columna de fechas '{name}': {error}")
        })?;
        changed_cell_count += parsed_count;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: parsed_count,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn impute_missing_values_in_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || column.null_count() == 0 {
            continue;
        }

        if column.dtype() == &DataType::String {
            let values = column
                .str()
                .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
            let mut counts = HashMap::<String, usize>::new();
            for value in values
                .iter()
                .flatten()
                .filter(|value| !value.trim().is_empty())
            {
                *counts.entry((*value).to_owned()).or_insert(0) += 1;
            }
            let mut mode = None;
            let mut mode_count = 0;
            for value in values
                .iter()
                .flatten()
                .filter(|value| !value.trim().is_empty())
            {
                let count = counts.get(value).copied().unwrap_or(0);
                if count > mode_count {
                    mode = Some((*value).to_owned());
                    mode_count = count;
                }
            }
            let Some(mode) = mode.filter(|_| mode_count >= 2) else {
                continue;
            };

            let mut column_changes = 0;
            let transformed = values
                .iter()
                .enumerate()
                .map(|(row_index, value)| match value {
                    Some(value) => Some((*value).to_owned()),
                    None => {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                        Some(mode.clone())
                    }
                })
                .collect::<Vec<_>>();
            cleaned
                .replace(&name, Column::new(name.clone().into(), transformed))
                .map_err(|error| format!("No se pudo imputar la columna '{name}': {error}"))?;
            changed_columns.push(ChangedTextColumn {
                name,
                changed_cell_count: column_changes,
            });
            continue;
        }

        if !column.dtype().is_primitive_numeric() {
            continue;
        }

        let numeric = column.cast(&DataType::Float64).map_err(|error| {
            format!("No se pudo preparar la columna numérica '{name}': {error}")
        })?;
        let values = numeric
            .f64()
            .map_err(|error| format!("No se pudo leer la columna numérica '{name}': {error}"))?
            .iter()
            .collect::<Vec<_>>();
        let mut observed = values
            .iter()
            .flatten()
            .copied()
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        if observed.is_empty() {
            continue;
        }
        observed
            .sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let replacement = observed[(observed.len() - 1) / 2];
        let mut column_changes = 0;
        let transformed = values
            .into_iter()
            .enumerate()
            .map(|(row_index, value)| match value {
                Some(value) => Some(value),
                None => {
                    column_changes += 1;
                    changed_cell_count += 1;
                    changed_rows[row_index] = true;
                    Some(replacement)
                }
            })
            .collect::<Vec<_>>();
        let replacement_column = Column::new(name.clone().into(), transformed)
            .cast(column.dtype())
            .map_err(|error| format!("No se pudo conservar el tipo de '{name}': {error}"))?;
        cleaned
            .replace(&name, replacement_column)
            .map_err(|error| format!("No se pudo imputar la columna '{name}': {error}"))?;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: column_changes,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn impute_dataprep_numeric_values_in_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios"
            || !matches!(column.dtype(), DataType::Int64 | DataType::Float64)
            || column.null_count() == 0
        {
            continue;
        }

        let values = physical_numeric_values(column)?;
        let mut observed = values.iter().flatten().copied().collect::<Vec<_>>();
        if observed.is_empty() {
            continue;
        }
        observed.sort_by(f64::total_cmp);
        let middle = observed.len() / 2;
        let replacement = if observed.len() % 2 == 0 {
            (observed[middle - 1] + observed[middle]) / 2.0
        } else {
            observed[middle]
        };
        if !replacement.is_finite() {
            return Err(format!(
                "La mediana de '{name}' excede el rango numérico finito."
            ));
        }

        let mut column_changes = 0;
        let transformed = values
            .into_iter()
            .enumerate()
            .map(|(row_index, value)| match value {
                Some(value) => Some(value),
                None => {
                    column_changes += 1;
                    changed_cell_count += 1;
                    changed_rows[row_index] = true;
                    Some(replacement)
                }
            })
            .collect::<Vec<_>>();
        let replacement_column = Column::new(name.clone().into(), transformed);
        let replacement_column = if column.dtype() == &DataType::Int64
            && replacement.fract() == 0.0
            && replacement.abs() <= (1_u64 << 53) as f64
        {
            replacement_column
                .cast(&DataType::Int64)
                .map_err(|error| format!("No se pudo conservar el tipo de '{name}': {error}"))?
        } else {
            replacement_column
        };
        cleaned
            .replace(&name, replacement_column)
            .map_err(|error| format!("No se pudo imputar la columna numérica '{name}': {error}"))?;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: column_changes,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn impute_categorical_values_in_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || column.dtype() != &DataType::String || column.null_count() == 0 {
            continue;
        }
        let values = column
            .str()
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        let mut column_changes = 0;
        let transformed = values
            .iter()
            .enumerate()
            .map(|(row_index, value)| match value {
                Some(value) => Some((*value).to_owned()),
                None => {
                    column_changes += 1;
                    changed_cell_count += 1;
                    changed_rows[row_index] = true;
                    Some("Desconocido".to_owned())
                }
            })
            .collect::<Vec<_>>();
        cleaned
            .replace(&name, Column::new(name.clone().into(), transformed))
            .map_err(|error| format!("No se pudo imputar la columna '{name}': {error}"))?;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: column_changes,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn cast_dataprep_numeric_columns(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_columns = Vec::new();
    let mut changed_cell_count = 0;

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || column.dtype() != &DataType::String {
            continue;
        }
        let values = column
            .str()
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?
            .iter()
            .map(|value| value.map(str::to_owned))
            .collect::<Vec<_>>();
        let non_null_count = values.iter().flatten().count();
        if non_null_count == 0 {
            continue;
        }
        if privacy_signal(&name) == Some("identifier")
            || values
                .iter()
                .flatten()
                .any(|value| dataprep_leading_zero_text(value))
        {
            continue;
        }

        let integer_values = values
            .iter()
            .map(|value| {
                value
                    .as_deref()
                    .and_then(|value| value.trim().parse::<i64>().ok())
            })
            .collect::<Vec<_>>();
        let integer_count = integer_values.iter().flatten().count();
        let has_decimal_token = values.iter().flatten().any(|value| {
            value
                .trim()
                .chars()
                .any(|character| matches!(character, '.' | 'e' | 'E'))
        });

        if !has_decimal_token && integer_count * 10 > non_null_count * 9 {
            for (row_index, value) in integer_values.iter().enumerate() {
                if value.is_some() {
                    changed_rows[row_index] = true;
                }
            }
            let converted = Column::new(name.clone().into(), integer_values);
            cleaned.replace(&name, converted).map_err(|error| {
                format!("No se pudo convertir la columna numérica '{name}': {error}")
            })?;
            changed_cell_count += integer_count;
            changed_columns.push(ChangedTextColumn {
                name,
                changed_cell_count: integer_count,
            });
            continue;
        }

        let float_values = values
            .iter()
            .map(|value| {
                value.as_deref().and_then(|value| {
                    let parsed = value.trim().parse::<f64>().ok()?;
                    parsed.is_finite().then_some(parsed)
                })
            })
            .collect::<Vec<_>>();
        let float_count = float_values.iter().flatten().count();
        if float_count * 10 <= non_null_count * 9 {
            continue;
        }
        if float_values
            .iter()
            .flatten()
            .any(|value| value.fract() == 0.0 && value.abs() > (1_u64 << 53) as f64)
        {
            return Err(format!(
                "La columna '{name}' contiene números que perderían precisión al convertirse."
            ));
        }
        for (row_index, value) in float_values.iter().enumerate() {
            if value.is_some() {
                changed_rows[row_index] = true;
            }
        }
        let converted = Column::new(name.clone().into(), float_values);
        cleaned.replace(&name, converted).map_err(|error| {
            format!("No se pudo convertir la columna numérica '{name}': {error}")
        })?;
        changed_cell_count += float_count;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: float_count,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

fn impute_outlier_values_in_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut cleaned = frame.clone();
    let mut changed_rows = vec![false; frame.height()];
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || !matches!(column.dtype(), DataType::Int64 | DataType::Float64) {
            continue;
        }

        let values = physical_numeric_values(column)?;
        let mut observed = values.iter().flatten().copied().collect::<Vec<_>>();
        if observed.len() < 4 {
            continue;
        }
        observed.sort_by(f64::total_cmp);
        let q1 = outlier_linear_quantile(&observed, 0.25);
        let q3 = outlier_linear_quantile(&observed, 0.75);
        let lower = q1 - 1.5 * (q3 - q1);
        let upper = q3 + 1.5 * (q3 - q1);
        if ![q1, q3, lower, upper].into_iter().all(f64::is_finite) {
            return Err(format!(
                "Los umbrales IQR de '{name}' exceden el rango numérico finito."
            ));
        }
        let replacement = if column.dtype() == &DataType::Int64 {
            observed[(observed.len() - 1) / 2]
        } else {
            outlier_linear_quantile(&observed, 0.5)
        };
        let mut column_changes = 0;
        let transformed = values
            .into_iter()
            .enumerate()
            .map(|(row_index, value)| {
                value.map(|value| {
                    if value < lower || value > upper {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                        replacement
                    } else {
                        value
                    }
                })
            })
            .collect::<Vec<_>>();
        if column_changes == 0 {
            continue;
        }

        let replacement_column = Column::new(name.clone().into(), transformed)
            .cast(column.dtype())
            .map_err(|error| format!("No se pudo imputar outliers en '{name}': {error}"))?;
        cleaned
            .replace(&name, replacement_column)
            .map_err(|error| format!("No se pudo actualizar la columna '{name}': {error}"))?;
        changed_columns.push(ChangedTextColumn {
            name,
            changed_cell_count: column_changes,
        });
    }

    Ok((
        cleaned,
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
        changed_columns,
    ))
}

#[derive(Clone, Copy)]
enum DataprepOutlierMode {
    Cap,
    Drop,
}

fn apply_dataprep_outlier_mode(
    frame: &DataFrame,
    mode: DataprepOutlierMode,
) -> Result<(DataFrame, usize, usize, Vec<ChangedTextColumn>), String> {
    let mut candidate = frame.clone();
    let mut drop_mask = vec![false; frame.height()];
    let mut changed_rows = vec![false; frame.height()];
    let mut affected_row_count = 0;
    let mut changed_cell_count = 0;
    let mut changed_columns = Vec::new();

    for column in frame.columns() {
        let name = column.name().to_string();
        if name == "_cambios" || !matches!(column.dtype(), DataType::Int64 | DataType::Float64) {
            continue;
        }

        let values = physical_numeric_values(column)?;
        let mut observed = values.iter().flatten().copied().collect::<Vec<_>>();
        if observed.len() < 4 {
            continue;
        }
        observed.sort_by(f64::total_cmp);
        let q1 = outlier_linear_quantile(&observed, 0.25);
        let q3 = outlier_linear_quantile(&observed, 0.75);
        let iqr = q3 - q1;
        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;
        if ![q1, q3, iqr, lower, upper].into_iter().all(f64::is_finite) {
            return Err(format!(
                "Los umbrales IQR de '{name}' exceden el rango numérico finito."
            ));
        }

        let mut column_changes = 0;
        match mode {
            DataprepOutlierMode::Cap => {
                let transformed = values
                    .into_iter()
                    .enumerate()
                    .map(|(row_index, value)| {
                        value.map(|value| {
                            let capped = value.clamp(lower, upper);
                            if capped != value {
                                column_changes += 1;
                                changed_cell_count += 1;
                                changed_rows[row_index] = true;
                            }
                            capped
                        })
                    })
                    .collect::<Vec<_>>();
                if column_changes > 0 {
                    candidate
                        .replace(&name, Column::new(name.clone().into(), transformed))
                        .map_err(|error| format!("No se pudo limitar '{name}': {error}"))?;
                }
            }
            DataprepOutlierMode::Drop => {
                for (row, value) in values.into_iter().enumerate() {
                    if value.is_some_and(|value| value < lower || value > upper) {
                        drop_mask[row] = true;
                        changed_rows[row] = true;
                        column_changes += 1;
                    }
                }
                changed_cell_count += column_changes;
            }
        }
        if column_changes > 0 {
            changed_columns.push(ChangedTextColumn {
                name,
                changed_cell_count: column_changes,
            });
        }
    }

    if matches!(mode, DataprepOutlierMode::Drop) {
        affected_row_count = drop_mask.iter().filter(|drop| **drop).count();
        if affected_row_count > 0 {
            let keep = drop_mask.iter().map(|drop| !drop).collect::<Vec<_>>();
            candidate = candidate
                .filter(&BooleanChunked::from_slice("outliers".into(), &keep))
                .map_err(|error| format!("No se pudieron retirar filas atípicas: {error}"))?;
        }
    }
    if matches!(mode, DataprepOutlierMode::Cap) {
        affected_row_count = changed_rows.iter().filter(|changed| **changed).count();
    }

    Ok((
        candidate,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    ))
}

fn safe_corrected_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize, Vec<ColumnRename>), String> {
    let (mut candidate, affected_row_count, changed_cell_count, _) =
        clean_text_columns(frame, None, TextCleaningMode::Trim)?;
    let (names, renames) = normalized_column_names(&candidate);
    if !renames.is_empty() {
        candidate
            .set_column_names(&names)
            .map_err(|error| format!("No se pudieron normalizar las columnas: {error}"))?;
    }
    Ok((candidate, affected_row_count, changed_cell_count, renames))
}

fn spreadsheet_extensions(extension: &str) -> bool {
    matches!(extension, "xlsx" | "xls" | "xlsb" | "ods")
}

fn inspect_workbook(path: &Path) -> Result<Vec<String>, String> {
    let workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    let sheets = workbook.sheet_names();
    if sheets.is_empty() {
        return Err("El libro no contiene hojas disponibles.".to_owned());
    }
    Ok(sheets)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpreadsheetColumnKind {
    Null,
    Boolean,
    Int64,
    Float64,
    Datetime,
    Duration,
    String,
}

fn spreadsheet_cell_kind(cell: &Data) -> SpreadsheetColumnKind {
    match cell {
        Data::Empty => SpreadsheetColumnKind::Null,
        Data::Bool(_) => SpreadsheetColumnKind::Boolean,
        Data::Int(_) => SpreadsheetColumnKind::Int64,
        Data::Float(_) => SpreadsheetColumnKind::Float64,
        Data::DateTime(value) if value.is_duration() => SpreadsheetColumnKind::Duration,
        Data::DateTime(_) => SpreadsheetColumnKind::Datetime,
        Data::DateTimeIso(_) if cell.as_datetime().is_some() => SpreadsheetColumnKind::Datetime,
        Data::DurationIso(_) if cell.as_duration().is_some() => SpreadsheetColumnKind::Duration,
        Data::String(_) | Data::DateTimeIso(_) | Data::DurationIso(_) | Data::Error(_) => {
            SpreadsheetColumnKind::String
        }
    }
}

fn merge_spreadsheet_kinds(
    left: SpreadsheetColumnKind,
    right: SpreadsheetColumnKind,
) -> SpreadsheetColumnKind {
    use SpreadsheetColumnKind::*;
    match (left, right) {
        (Null, kind) | (kind, Null) => kind,
        (left, right) if left == right => left,
        (Int64, Float64) | (Float64, Int64) => Float64,
        _ => String,
    }
}

fn unique_spreadsheet_headers(headers: Vec<String>) -> Vec<String> {
    let mut occurrences = HashMap::<String, usize>::new();
    headers
        .into_iter()
        .enumerate()
        .map(|(index, header)| {
            let trimmed = header.trim();
            let base = if trimmed.is_empty() {
                format!("column_{}", index + 1)
            } else {
                trimmed.to_owned()
            };
            let count = occurrences.entry(base.clone()).or_default();
            *count += 1;
            if *count == 1 {
                base
            } else {
                format!("{base}_{}", *count)
            }
        })
        .collect()
}

fn spreadsheet_cells_to_column(name: &str, cells: &[&Data]) -> Result<Column, String> {
    let mut kind = cells
        .iter()
        .fold(SpreadsheetColumnKind::Null, |kind, cell| {
            merge_spreadsheet_kinds(kind, spreadsheet_cell_kind(cell))
        });
    if kind == SpreadsheetColumnKind::Float64
        && cells
            .iter()
            .any(|cell| matches!(cell, Data::Int(value) if value.unsigned_abs() > (1_u64 << 53)))
    {
        kind = SpreadsheetColumnKind::String;
    }
    let column_name = name.into();
    let incompatible =
        |value: &Data| format!("La columna '{name}' contiene un valor incompatible: {value}");

    match kind {
        SpreadsheetColumnKind::Null => Ok(Column::full_null(
            column_name,
            cells.len(),
            &polars::prelude::DataType::Null,
        )),
        SpreadsheetColumnKind::Boolean => {
            let values = cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Bool(value) => Ok(Some(*value)),
                    other => Err(incompatible(other)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Int64 => {
            let values = cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Int(value) => Ok(Some(*value)),
                    other => Err(incompatible(other)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Float64 => {
            let values = cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => Ok(None),
                    Data::Int(value) if value.unsigned_abs() <= (1_u64 << 53) => {
                        Ok(Some(*value as f64))
                    }
                    Data::Int(_) => Err(format!(
                        "La columna '{name}' mezcla decimales con enteros que perderían precisión."
                    )),
                    Data::Float(value) => Ok(Some(*value)),
                    other => Err(incompatible(other)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(column_name, values).into_column())
        }
        SpreadsheetColumnKind::Datetime => {
            let values = cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => Ok(None),
                    Data::DateTime(value) if !value.is_duration() => value
                        .as_datetime()
                        .map(|value| Some(value.and_utc().timestamp_millis()))
                        .ok_or_else(|| incompatible(cell)),
                    Data::DateTimeIso(_) => cell
                        .as_datetime()
                        .map(|value| Some(value.and_utc().timestamp_millis()))
                        .ok_or_else(|| incompatible(cell)),
                    other => Err(incompatible(other)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(column_name, values)
                .cast(&polars::prelude::DataType::Datetime(
                    TimeUnit::Milliseconds,
                    None,
                ))
                .map_err(|error| format!("No se pudo conservar una fecha de '{name}': {error}"))?
                .into_column())
        }
        SpreadsheetColumnKind::Duration => {
            let values = cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => Ok(None),
                    Data::DateTime(value) if value.is_duration() => value
                        .as_duration()
                        .map(|value| Some(value.num_milliseconds()))
                        .ok_or_else(|| incompatible(cell)),
                    Data::DurationIso(_) => cell
                        .as_duration()
                        .map(|value| Some(value.num_milliseconds()))
                        .ok_or_else(|| incompatible(cell)),
                    other => Err(incompatible(other)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(column_name, values)
                .cast(&polars::prelude::DataType::Duration(TimeUnit::Milliseconds))
                .map_err(|error| format!("No se pudo conservar una duración de '{name}': {error}"))?
                .into_column())
        }
        SpreadsheetColumnKind::String => Ok(Series::new(
            column_name,
            cells
                .iter()
                .map(|cell| match cell {
                    Data::Empty => None,
                    value => Some(value.to_string()),
                })
                .collect::<Vec<_>>(),
        )
        .into_column()),
    }
}

fn spreadsheet_range_to_frame(
    range: &Range<Data>,
    header_mode: SpreadsheetHeaderMode,
) -> Result<DataFrame, String> {
    if range.is_empty() {
        return Err("La hoja seleccionada está vacía.".to_owned());
    }
    let width = range.width();
    let height = range.height();
    let data_start = usize::from(header_mode == SpreadsheetHeaderMode::FirstRow);
    let headers = match header_mode {
        SpreadsheetHeaderMode::FirstRow => unique_spreadsheet_headers(
            (0..width)
                .map(|column| {
                    range
                        .get((0, column))
                        .map(ToString::to_string)
                        .unwrap_or_default()
                })
                .collect(),
        ),
        SpreadsheetHeaderMode::Generated => {
            (1..=width).map(|index| format!("column_{index}")).collect()
        }
    };
    let mut columns = Vec::with_capacity(width);
    for (column_index, name) in headers.iter().enumerate() {
        let cells = (data_start..height)
            .map(|row| range.get((row, column_index)).expect("rango rectangular"))
            .collect::<Vec<_>>();
        columns.push(spreadsheet_cells_to_column(name, &cells)?);
    }
    DataFrame::new(height.saturating_sub(data_start), columns)
        .map_err(|error| format!("No se pudo construir el dataset desde la hoja: {error}"))
}

fn load_spreadsheet_sheet(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
) -> Result<DataFrame, String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    if !workbook.sheet_names().iter().any(|name| name == sheet_name) {
        return Err("La hoja seleccionada ya no está disponible en el libro.".to_owned());
    }
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
    spreadsheet_range_to_frame(&range, header_mode)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JsonColumnKind {
    Null,
    Boolean,
    Int64,
    Float64,
    String,
}

fn json_value_kind(value: &JsonValue) -> JsonColumnKind {
    match value {
        JsonValue::Null => JsonColumnKind::Null,
        JsonValue::Bool(_) => JsonColumnKind::Boolean,
        JsonValue::Number(number) if number.as_i64().is_some() => JsonColumnKind::Int64,
        JsonValue::Number(number)
            if number
                .to_string()
                .chars()
                .any(|character| matches!(character, '.' | 'e' | 'E'))
                && number.as_f64().is_some_and(f64::is_finite) =>
        {
            JsonColumnKind::Float64
        }
        JsonValue::Number(_)
        | JsonValue::String(_)
        | JsonValue::Array(_)
        | JsonValue::Object(_) => JsonColumnKind::String,
    }
}

fn merge_json_kinds(left: JsonColumnKind, right: JsonColumnKind) -> JsonColumnKind {
    use JsonColumnKind::*;
    match (left, right) {
        (Null, kind) | (kind, Null) => kind,
        (left, right) if left == right => left,
        (Int64, Float64) | (Float64, Int64) => Float64,
        _ => String,
    }
}

fn json_value_as_text(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::Null => None,
        JsonValue::String(value) => Some(value.clone()),
        other => Some(other.to_string()),
    }
}

fn json_records_to_frame(records: &[JsonMap<String, JsonValue>]) -> Result<DataFrame, String> {
    if records.is_empty() {
        return Err("El JSON no contiene registros.".to_owned());
    }
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for record in records {
        for name in record.keys() {
            if seen.insert(name.clone()) {
                names.push(name.clone());
            }
        }
    }
    if names.is_empty() {
        return Err("Los registros JSON no contienen campos.".to_owned());
    }

    let mut columns = Vec::with_capacity(names.len());
    for name in names {
        let values = records
            .iter()
            .map(|record| record.get(&name).unwrap_or(&JsonValue::Null))
            .collect::<Vec<_>>();
        let mut kind = values.iter().fold(JsonColumnKind::Null, |kind, value| {
            merge_json_kinds(kind, json_value_kind(value))
        });
        if kind == JsonColumnKind::Float64
            && values.iter().any(|value| {
                value
                    .as_i64()
                    .is_some_and(|number| number.unsigned_abs() > (1_u64 << 53))
            })
        {
            kind = JsonColumnKind::String;
        }
        let column_name = name.as_str().into();
        let column = match kind {
            JsonColumnKind::Null => {
                Column::full_null(column_name, values.len(), &polars::prelude::DataType::Null)
            }
            JsonColumnKind::Boolean => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_bool())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::Int64 => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_i64())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::Float64 => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| value.as_f64())
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            JsonColumnKind::String => Series::new(
                column_name,
                values
                    .iter()
                    .map(|value| json_value_as_text(value))
                    .collect::<Vec<_>>(),
            )
            .into_column(),
        };
        columns.push(column);
    }
    DataFrame::new(records.len(), columns)
        .map_err(|error| format!("No se pudo construir el dataset JSON: {error}"))
}

fn load_json_records(path: &Path) -> Result<DataFrame, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("No se pudo abrir el JSON: {error}"))?;
    let mut values =
        serde_json::Deserializer::from_reader(BufReader::new(file)).into_iter::<JsonValue>();
    let first = values
        .next()
        .transpose()
        .map_err(|error| format!("El JSON no es válido: {error}"))?
        .ok_or_else(|| "El archivo JSON está vacío.".to_owned())?;

    let records = match first {
        JsonValue::Array(items) => {
            if values.next().is_some() {
                return Err("Un arreglo JSON debe ser el único valor del archivo.".to_owned());
            }
            items
                .into_iter()
                .enumerate()
                .map(|(index, value)| match value {
                    JsonValue::Object(record) => Ok(record),
                    _ => Err(format!("El registro JSON {} no es un objeto.", index + 1)),
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        JsonValue::Object(record) => {
            let mut records = vec![record];
            for (index, value) in values.enumerate() {
                let value = value.map_err(|error| format!("JSON Lines inválido: {error}"))?;
                match value {
                    JsonValue::Object(record) => records.push(record),
                    _ => {
                        return Err(format!(
                            "La línea JSON {} no contiene un objeto.",
                            index + 2
                        ))
                    }
                }
            }
            records
        }
        _ => {
            return Err(
                "El JSON debe ser un arreglo de objetos o contener un objeto por línea.".to_owned(),
            )
        }
    };
    json_records_to_frame(&records)
}

fn read_utf8_delimited_sample(path: &Path) -> Result<(String, bool), String> {
    let file_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar el archivo delimitado: {error}"))?
        .len();
    let mut bytes = Vec::with_capacity(DELIMITED_SAMPLE_BYTES as usize);
    fs::File::open(path)
        .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))?
        .take(DELIMITED_SAMPLE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo inspeccionar el archivo delimitado: {error}"))?;
    let complete = file_size <= bytes.len() as u64;

    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    match std::str::from_utf8(bytes) {
        Ok(sample) => Ok((sample.to_owned(), complete)),
        Err(error) if !complete && error.error_len().is_none() => {
            // La muestra puede terminar a mitad de un carácter UTF-8. Solo se
            // descarta esa cola incompleta; un byte inválido interior se rechaza.
            let valid = &bytes[..error.valid_up_to()];
            Ok((std::str::from_utf8(valid).unwrap_or_default().to_owned(), false))
        }
        Err(_) => Err(
            "El archivo delimitado no contiene UTF-8 válido. Columnia no sustituye caracteres ni aplica codificaciones heredadas automáticamente."
                .to_owned(),
        ),
    }
}

fn delimited_field_counts(sample: &str, delimiter: char, complete: bool) -> Vec<usize> {
    let mut counts = Vec::new();
    let mut fields = 1usize;
    let mut in_quotes = false;
    let mut has_content = false;
    let mut chars = sample.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                has_content = true;
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
                has_content = true;
            }
            value if value == delimiter && !in_quotes => {
                fields += 1;
                has_content = true;
            }
            '\n' if !in_quotes => {
                if has_content {
                    counts.push(fields);
                    if counts.len() == 24 {
                        break;
                    }
                }
                fields = 1;
                has_content = false;
            }
            '\r' if !in_quotes => {}
            value => has_content |= !value.is_whitespace(),
        }
    }

    if complete && !in_quotes && has_content && counts.len() < 24 {
        counts.push(fields);
    }
    counts
}

fn detect_delimiter(path: &Path, extension: &str) -> Result<u8, String> {
    if extension == "tsv" {
        // La extensión explícita tiene prioridad sobre cualquier carácter que
        // aparezca dentro de los valores.
        read_utf8_delimited_sample(path)?;
        return Ok(b'\t');
    }

    let (sample, complete) = read_utf8_delimited_sample(path)?;
    let candidates = [(b',', ','), (b';', ';'), (b'\t', '\t'), (b'|', '|')];
    let mut valid = candidates
        .into_iter()
        .filter_map(|(byte, delimiter)| {
            let counts = delimited_field_counts(&sample, delimiter, complete);
            let first = *counts.first()?;
            (counts.len() >= 2 && first > 1 && counts.iter().all(|count| *count == first))
                .then_some((byte, first))
        })
        .collect::<Vec<_>>();
    valid.sort_unstable_by_key(|(_, field_count)| std::cmp::Reverse(*field_count));

    match valid.as_slice() {
        [(delimiter, _)] => Ok(*delimiter),
        [(delimiter, best), (_, second), ..] if best > second => Ok(*delimiter),
        _ => Ok(b','),
    }
}

fn collect_lazy_frame_streaming(plan: LazyFrame, context: &str) -> Result<DataFrame, String> {
    plan.collect_with_engine(Engine::Streaming)
        .map(|result| result.unwrap_single())
        .map_err(|error| format!("{context}: {error}"))
}

fn read_delimited_frame(path: &Path, extension: &str) -> Result<DataFrame, String> {
    let separator = detect_delimiter(path, extension)?;
    let source = PlRefPath::try_from_path(path)
        .map_err(|error| format!("No se pudo preparar el lector delimitado: {error}"))?;
    let plan = LazyCsvReader::new(source)
        .with_has_header(true)
        .with_infer_schema_length(Some(0))
        .with_low_memory(true)
        .with_rechunk(false)
        .with_separator(separator)
        .finish()
        .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))?;
    collect_lazy_frame_streaming(
        plan,
        "No se pudo interpretar el archivo delimitado como UTF-8",
    )
}

fn read_parquet_frame(path: &Path) -> Result<DataFrame, String> {
    let source = PlRefPath::try_from_path(path)
        .map_err(|error| format!("No se pudo preparar el lector Parquet: {error}"))?;
    let options = ScanArgsParquet {
        parallel: ParallelStrategy::None,
        low_memory: true,
        rechunk: false,
        ..Default::default()
    };
    let plan = LazyFrame::scan_parquet(source, options)
        .map_err(|error| format!("No se pudo abrir el Parquet: {error}"))?;
    collect_lazy_frame_streaming(plan, "No se pudo interpretar el Parquet")
}

#[cfg(test)]
fn load_csv_with_progress<F, C>(
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    report("Validando archivo", 10);
    let (_, _, extension) = validate_dataset_file(path)?;
    if extension != "csv" {
        return Err("El lector CSV recibió un formato diferente.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    report("Leyendo y detectando columnas", 25);
    let frame = read_delimited_frame(path, &extension)?;

    ensure_not_cancelled(is_cancelled())?;
    report("Preparando vista previa", 85);
    let preview = dataset_preview(path, &frame)?;
    report("Preparando sesión", 95);

    Ok((frame, preview))
}

fn load_dataset_with_progress<F, C>(
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    report("Validando archivo", 10);
    let (_, _, extension) = validate_dataset_file(path)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Leyendo y detectando columnas", 25);

    let frame = match extension.as_str() {
        "csv" | "tsv" | "txt" => read_delimited_frame(path, &extension)?,
        "parquet" => read_parquet_frame(path)?,
        "json" | "jsonl" | "ndjson" => load_json_records(path)?,
        extension if spreadsheet_extensions(extension) => {
            return Err("Selecciona primero una hoja del libro.".to_owned())
        }
        _ => unreachable!("la extensión fue validada"),
    };

    ensure_not_cancelled(is_cancelled())?;
    report("Preparando vista previa", 85);
    let preview = dataset_preview(path, &frame)?;
    report("Preparando sesión", 95);
    Ok((frame, preview))
}

#[cfg(test)]
fn load_csv(path: &Path) -> Result<(DataFrame, DatasetPreview), String> {
    load_csv_with_progress(path, |_, _| {}, || false)
}

fn path_with_extension(mut path: PathBuf, format: ExportFormat) -> PathBuf {
    let has_expected_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()));
    if !has_expected_extension {
        path.set_extension(format.extension());
    }
    path
}

fn recipe_path_with_extension(mut path: PathBuf) -> PathBuf {
    let is_json = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    if !is_json {
        path.set_extension("json");
    }
    path
}

fn validate_recipe_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name != name.trim() {
        return Err(
            "El nombre de la receta no puede estar vacío ni tener espacios externos.".to_owned(),
        );
    }
    if name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err(
            "El nombre de la receta debe tener hasta 120 caracteres imprimibles.".to_owned(),
        );
    }
    Ok(())
}

fn recipe_suggested_file_name(name: &str) -> String {
    let safe_name: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            _ => character,
        })
        .collect();
    format!("{safe_name}.json")
}

fn current_recipe_timestamp() -> String {
    DateTime::<chrono::Utc>::from(std::time::SystemTime::now()).to_rfc3339()
}

fn build_stored_recipe(
    recipe: TransformRecipe,
    name: String,
) -> Result<StoredTransformRecipe, String> {
    validate_recipe_name(&name)?;
    let document = StoredTransformRecipe {
        version: RECIPE_FILE_VERSION,
        name,
        saved_at: current_recipe_timestamp(),
        recipe,
        export_options: None,
        migration_report: None,
    };
    validate_stored_recipe(&document)?;
    Ok(document)
}

fn validate_recipe_export_options(options: &RecipeExportOptions) -> Result<(), String> {
    if options.formats.is_empty() || options.formats.len() > 6 {
        return Err("Las opciones de entrega deben incluir entre 1 y 6 formatos.".to_owned());
    }
    if options.selected_columns.len() > 512 {
        return Err("La entrega puede seleccionar como máximo 512 columnas.".to_owned());
    }
    validate_semantic_text_budget(
        "opciones de entrega",
        options
            .selected_columns
            .iter()
            .map(|column| ("columna seleccionada", column.as_str())),
        MAX_RECIPE_TEXT_FIELD_CHARS,
        MAX_RECIPE_TOTAL_TEXT_CHARS,
    )
}

fn validate_stored_recipe(document: &StoredTransformRecipe) -> Result<(), String> {
    if document.version != RECIPE_FILE_VERSION {
        return Err(format!(
            "La receta usa la versión {}, pero Columnia admite exactamente la versión {}.",
            document.version, RECIPE_FILE_VERSION
        ));
    }
    validate_recipe_name(&document.name)?;
    DateTime::parse_from_rfc3339(&document.saved_at)
        .map_err(|_| "La receta no incluye una fecha de guardado RFC 3339 válida.".to_owned())?;
    validate_recipe_structure(&document.recipe)?;
    if let Some(export_options) = &document.export_options {
        validate_recipe_export_options(export_options)?;
    }
    let encoded = serde_json::to_vec(document)
        .map_err(|error| format!("No se pudo validar la receta: {error}"))?;
    if encoded.len() as u64 > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    Ok(())
}

fn validate_semantic_text_budget<'a, I>(
    payload: &str,
    fields: I,
    maximum_field_chars: usize,
    maximum_total_chars: usize,
) -> Result<(), String>
where
    I: IntoIterator<Item = (&'static str, &'a str)>,
{
    let mut total_chars = 0_usize;
    for (field, value) in fields {
        let field_chars = value.chars().count();
        if field_chars > maximum_field_chars {
            return Err(format!(
                "El campo {field} del {payload} supera el límite de {maximum_field_chars} caracteres."
            ));
        }
        total_chars = total_chars.saturating_add(field_chars);
        if total_chars > maximum_total_chars {
            return Err(format!(
                "El {payload} supera el presupuesto semántico de {maximum_total_chars} caracteres."
            ));
        }
    }
    Ok(())
}

fn validate_recipe_text_budget(recipe: &TransformRecipe) -> Result<(), String> {
    let mut fields: Vec<(&'static str, &str)> = Vec::new();
    for rename in &recipe.renames {
        fields.extend([
            ("from de renombre", rename.from.as_str()),
            ("to de renombre", rename.to.as_str()),
        ]);
    }
    for cast in &recipe.casts {
        fields.push(("column de conversión", cast.column.as_str()));
    }
    for date_parse in &recipe.date_parses {
        fields.push(("column de fecha", date_parse.column.as_str()));
    }
    for filter in &recipe.filters {
        fields.push(("column de filtro", filter.column.as_str()));
        if let Some(value) = &filter.value {
            fields.push(("value de filtro", value.as_str()));
        }
    }
    if let Some(calculation) = &recipe.calculated_column {
        fields.extend([
            ("name de cálculo", calculation.name.as_str()),
            ("source de cálculo", calculation.source.as_str()),
        ]);
        if let Some(operand) = &calculation.operand {
            fields.push(("value de operando", operand.value.as_str()));
        }
    }
    if let Some(replacement) = &recipe.find_replace {
        if let Some(column) = &replacement.column {
            fields.push(("column de reemplazo", column.as_str()));
        }
        fields.extend([
            ("find de reemplazo", replacement.find.as_str()),
            ("replace de reemplazo", replacement.replace.as_str()),
        ]);
    }
    if let Some(columns) = &recipe.keep_columns {
        fields.extend(
            columns
                .iter()
                .map(|column| ("keepColumns", column.as_str())),
        );
    }
    if let Some(split) = &recipe.split_column {
        fields.extend([
            ("source de división", split.source.as_str()),
            ("delimiter de división", split.delimiter.as_str()),
        ]);
        fields.extend(
            split
                .names
                .iter()
                .map(|name| ("names de división", name.as_str())),
        );
    }
    if let Some(merge) = &recipe.merge_columns {
        fields.extend(
            merge
                .sources
                .iter()
                .map(|source| ("sources de combinación", source.as_str())),
        );
        fields.extend([
            ("name de combinación", merge.name.as_str()),
            ("separator de combinación", merge.separator.as_str()),
        ]);
    }
    fields.extend(
        recipe
            .outlier_treatments
            .iter()
            .map(|treatment| ("column de atípicos", treatment.column.as_str())),
    );
    if let Some(summary) = &recipe.group_summary {
        fields.extend(
            summary
                .group_by
                .iter()
                .map(|column| ("groupBy", column.as_str())),
        );
        fields.extend(
            summary
                .aggregations
                .iter()
                .map(|aggregation| ("column de agregación", aggregation.column.as_str())),
        );
    }
    fields.extend(
        recipe
            .contact_normalizations
            .iter()
            .map(|normalization| ("column de contacto", normalization.column.as_str())),
    );
    for extraction in &recipe.text_extractions {
        fields.extend([
            ("source de extracción", extraction.source.as_str()),
            ("name de extracción", extraction.name.as_str()),
        ]);
        if let Some(delimiter) = &extraction.delimiter {
            fields.push(("delimiter de extracción", delimiter.as_str()));
        }
    }

    validate_semantic_text_budget(
        "payload de receta",
        fields,
        MAX_RECIPE_TEXT_FIELD_CHARS,
        MAX_RECIPE_TOTAL_TEXT_CHARS,
    )
}

fn validate_recipe_structure(recipe: &TransformRecipe) -> Result<(), String> {
    validate_recipe_text_budget(recipe)?;
    let bounded = [
        (recipe.renames.len(), 256, "renombres"),
        (recipe.casts.len(), 256, "conversiones"),
        (recipe.date_parses.len(), 256, "fechas"),
        (recipe.filters.len(), 3, "filtros"),
        (
            recipe.outlier_treatments.len(),
            16,
            "tratamientos de atípicos",
        ),
        (
            recipe.contact_normalizations.len(),
            16,
            "normalizaciones de contacto",
        ),
        (recipe.text_extractions.len(), 16, "extracciones de texto"),
    ];
    for (count, maximum, operation) in bounded {
        if count > maximum {
            return Err(format!(
                "La receta contiene demasiados {operation}: máximo {maximum}."
            ));
        }
    }
    if recipe
        .keep_columns
        .as_ref()
        .is_some_and(|columns| columns.len() > 512)
    {
        return Err("La receta puede conservar como máximo 512 columnas.".to_owned());
    }
    if recipe
        .split_column
        .as_ref()
        .is_some_and(|split| split.names.len() > 16)
    {
        return Err("Una división puede crear como máximo 16 columnas.".to_owned());
    }
    if recipe
        .merge_columns
        .as_ref()
        .is_some_and(|merge| merge.sources.len() > 16)
    {
        return Err("Una combinación puede usar como máximo 16 columnas.".to_owned());
    }
    if let Some(summary) = &recipe.group_summary {
        if summary.group_by.len() > 8 || summary.aggregations.len() > 32 {
            return Err(
                "Un resumen admite hasta 8 columnas de grupo y 32 agregaciones.".to_owned(),
            );
        }
    }
    Ok(())
}

fn save_recipe_atomic(document: &StoredTransformRecipe, destination: &Path) -> Result<(), String> {
    validate_stored_recipe(document)?;
    let destination = canonicalize_write_destination(destination, "la receta")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de la receta.".to_owned())?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo preparar el archivo de receta: {error}"))?;
    serde_json::to_writer_pretty(temporary.as_file(), document)
        .map_err(|error| format!("No se pudo escribir la receta: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la receta: {error}"))?;
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar la receta: {}", error.error))?;
    Ok(())
}

fn load_recipe_file(path: &Path) -> Result<StoredTransformRecipe, String> {
    let path = canonicalize_existing_file(path, "la receta seleccionada")?;
    let file = File::open(path).map_err(|error| format!("No se pudo abrir la receta: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("No se pudo verificar la receta: {error}"))?
        .len();
    if size > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }

    let mut bytes = Vec::with_capacity(size as usize);
    file.take(RECIPE_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer la receta: {error}"))?;
    if bytes.len() as u64 > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    let json = std::str::from_utf8(&bytes)
        .map_err(|_| "La receta no contiene texto UTF-8 válido.".to_owned())?;
    let raw: JsonValue = serde_json::from_str(json)
        .map_err(|error| format!("La receta JSON no es válida: {error}"))?;

    if raw.get("recipe").is_some() {
        let document: StoredTransformRecipe = serde_json::from_value(raw)
            .map_err(|error| format!("La receta Columnia no es válida: {error}"))?;
        validate_stored_recipe(&document)?;
        return Ok(document);
    }

    if let Ok(document) = serde_json::from_value::<StoredTransformRecipe>(raw.clone()) {
        validate_stored_recipe(&document)?;
        return Ok(document);
    }

    let mut document = migration_dataprep_recipe(&raw)?;
    if let Some(report) = &mut document.migration_report {
        report.artifact_sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    }
    Ok(document)
}

fn migration_recipe_transform(
    root: &JsonMap<String, JsonValue>,
) -> Result<&JsonMap<String, JsonValue>, String> {
    if let Some(value) = root
        .get("transform")
        .or_else(|| root.get("transformConfig"))
        .or_else(|| root.get("transform_config"))
    {
        return value.as_object().ok_or_else(|| {
            "La configuración de transformación de DataPrep debe ser un objeto.".to_owned()
        });
    }

    let known_field = [
        "rename_text",
        "dtype_col",
        "parse_date_cols",
        "filters",
        "find_replace",
        "keep_columns",
        "calc",
        "outliers",
        "split_column",
        "merge_columns",
        "group_summary",
        "normalize_contacts",
        "extract_text",
    ]
    .iter()
    .any(|key| root.contains_key(*key));

    if known_field {
        Ok(root)
    } else {
        Err("El JSON no es una receta Columnia ni contiene una transformación DataPrep reconocible.".to_owned())
    }
}

fn migration_scalar_text(value: &JsonValue, key: &str) -> Result<String, String> {
    match value {
        JsonValue::String(value) => Ok(value.clone()),
        JsonValue::Bool(value) => Ok(value.to_string()),
        JsonValue::Number(value) => Ok(value.to_string()),
        JsonValue::Null => Err(format!("El campo '{key}' no puede ser nulo.")),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            Err(format!("El campo '{key}' debe ser un valor escalar."))
        }
    }
}

fn migration_text_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
    label: &str,
) -> Result<Option<String>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(migration_scalar_text(
        value,
        &format!("{label}/{key}"),
    )?))
}

fn migration_required_text(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
    label: &str,
) -> Result<String, String> {
    migration_text_field(map, keys, label)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("La operación DataPrep '{label}' necesita un valor de texto."))
}

fn migration_filter_operator(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "eq" | "equal" | "equals" | "=" => Some("eq"),
        "ne" | "neq" | "not_equal" | "!=" => Some("neq"),
        "gt" | ">" => Some("gt"),
        "lt" | "<" => Some("lt"),
        "gte" | "ge" | ">=" => Some("gte"),
        "lte" | "le" | "<=" => Some("lte"),
        "contains" => Some("contains"),
        "not_contains" | "not contains" => Some("not_contains"),
        "is_null" | "null" => Some("is_null"),
        "not_null" | "not null" => Some("not_null"),
        _ => None,
    }
}

fn migration_cast_target(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "string" | "str" | "text" | "object" => Some("string"),
        "integer" | "int" | "int64" => Some("integer"),
        "decimal" | "float" | "float64" | "number" => Some("decimal"),
        "boolean" | "bool" => Some("boolean"),
        _ => None,
    }
}

fn migration_date_format(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "iso8601" | "iso" | "auto" => Some("iso8601"),
        "ymd" | "%y-%m-%d" => Some("ymd"),
        "dmy" | "%d/%m/%y" => Some("dmy"),
        "mdy" | "%m/%d/%y" => Some("mdy"),
        _ => None,
    }
}

fn migration_calculated_operation(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "add" | "+" => Some("add"),
        "subtract" | "sub" | "-" => Some("subtract"),
        "multiply" | "mul" | "*" => Some("multiply"),
        "divide" | "div" | "/" => Some("divide"),
        "concat" => Some("concat"),
        "year" => Some("year"),
        "month" => Some("month"),
        "day" => Some("day"),
        _ => None,
    }
}

fn migration_summary_operation(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "sum" | "total" => Some("sum"),
        "mean" | "avg" | "average" => Some("mean"),
        "min" | "minimum" => Some("min"),
        "max" | "maximum" => Some("max"),
        "count" | "counts" => Some("count"),
        "count_unique" | "nunique" | "unique" => Some("count_unique"),
        _ => None,
    }
}

fn push_migration_operation(operations: &mut Vec<String>, operation: impl Into<String>) {
    let operation = operation.into();
    if !operations.contains(&operation) {
        operations.push(operation);
    }
}

fn recipe_migration_warning(
    path: impl Into<String>,
    severity: &str,
    message: impl Into<String>,
) -> RecipeMigrationWarning {
    RecipeMigrationWarning {
        path: path.into(),
        severity: severity.to_owned(),
        message: message.into(),
    }
}

fn build_recipe_migration_report(
    source_format: &str,
    source_version: Option<u64>,
    mut converted_operations: Vec<String>,
    mut omitted_operations: Vec<String>,
    warnings: Vec<RecipeMigrationWarning>,
    session: Option<SessionMigrationMetadata>,
) -> RecipeMigrationReport {
    converted_operations.sort();
    omitted_operations.sort();
    let mut manual_actions = vec![
        "Revisar la receta y sus opciones de entrega antes de aplicarla o exportarla.".to_owned(),
    ];
    if !omitted_operations.is_empty() {
        manual_actions.push(
            "Revisar las operaciones y opciones omitidas; deben recrearse manualmente si siguen siendo necesarias."
                .to_owned(),
        );
    }
    if !warnings.is_empty() {
        manual_actions.push(
            "Confirmar las advertencias de compatibilidad frente al pipeline original de DataPrep."
                .to_owned(),
        );
    }
    RecipeMigrationReport {
        artifact_sha256: None,
        source_format: source_format.to_owned(),
        source_version,
        converted_items: converted_operations.len(),
        omitted_items: omitted_operations.len(),
        warning_count: warnings.len(),
        converted_operations,
        omitted_operations,
        warnings,
        manual_actions,
        session,
    }
}

fn migration_session_field<'a>(
    root: &'a JsonMap<String, JsonValue>,
    key: &str,
) -> Option<&'a JsonValue> {
    let camel_case = match key {
        "source_path" => "sourcePath",
        "snapshot_path" => "snapshotPath",
        "sheet_name" => "sheetName",
        "stage_label" => "stageLabel",
        "applied_ops" => "appliedOps",
        "selected_cleaning_operations" => "selectedCleaningOperations",
        "quality_rules" => "qualityRules",
        "analysis_checks" => "analysisChecks",
        "execution_history" => "executionHistory",
        "file_name" => "fileName",
        _ => return root.get(key),
    };
    let session = root.get("session").and_then(JsonValue::as_object);
    root.get(key)
        .filter(|value| !value.is_null())
        .or_else(|| root.get(camel_case).filter(|value| !value.is_null()))
        .or_else(|| {
            session
                .and_then(|map| map.get(key))
                .filter(|value| !value.is_null())
        })
        .or_else(|| {
            session
                .and_then(|map| map.get(camel_case))
                .filter(|value| !value.is_null())
        })
}

fn migration_cleaning_operation(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "drop_duplicates" | "remove_duplicates" | "deduplicate" => Some("drop_duplicates"),
        "drop_high_null_cols" | "drop_high_null_columns" | "remove_high_null_columns" => {
            Some("drop_high_null_cols")
        }
        "drop_id_cols" | "drop_id_columns" | "drop_identifier_columns" => Some("drop_id_cols"),
        "drop_empty_cols" | "drop_empty_columns" | "remove_empty_columns" => {
            Some("drop_empty_cols")
        }
        "drop_constant_cols" | "drop_constant_columns" | "remove_constant_columns" => {
            Some("drop_constant_cols")
        }
        "drop_empty_rows" | "remove_empty_rows" => Some("drop_empty_rows"),
        "normalize_sentinels" | "sentinels" => Some("normalize_sentinels"),
        "impute_numeric" | "impute_missing_numeric" => Some("impute_numeric"),
        "impute_categorical" | "impute_missing_categorical" => Some("impute_categorical"),
        "parse_dates" | "parse_date_columns" | "parse_datetime" => Some("parse_dates"),
        "trim_text" | "trim_text_values" => Some("trim_text"),
        "normalize_text" | "normalize_text_values" => Some("normalize_text"),
        "fix_encoding" | "repair_encoding" => Some("fix_encoding"),
        "cast_numeric" | "cast_numeric_columns" => Some("cast_numeric"),
        "cap_outliers" | "cap_outlier_values" => Some("cap_outliers"),
        "impute_outliers" | "impute_outlier_values" => Some("impute_outliers"),
        "drop_outliers" | "remove_outliers" => Some("drop_outliers"),
        "normalize_booleans" | "normalize_boolean_values" => Some("normalize_booleans"),
        "mask_pii" | "mask_personal_data" | "mask_personal_values" => Some("mask_pii"),
        "drop_fuzzy_duplicates" | "remove_near_duplicates" | "deduplicate_fuzzy" => {
            Some("drop_fuzzy_duplicates")
        }
        "normalize_columns" | "normalize_column_names" => Some("normalize_columns"),
        "add_cambios_col" | "enable_row_audit" => Some("add_cambios_col"),
        _ => None,
    }
}

fn migration_metadata_name(value: &JsonValue) -> Option<String> {
    let candidate = value.as_str().or_else(|| {
        value.as_object().and_then(|map| {
            ["name", "kind", "operation", "id"]
                .iter()
                .find_map(|key| map.get(*key).and_then(JsonValue::as_str))
        })
    })?;
    let candidate = candidate.trim();
    if candidate.is_empty()
        || candidate.chars().count() > MAX_SESSION_METADATA_NAME_CHARS
        || candidate
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return None;
    }
    Some(candidate.to_owned())
}

fn migration_session_artifact_names(root: &JsonMap<String, JsonValue>) -> Vec<String> {
    let session = root.get("session").and_then(JsonValue::as_object);
    let fields: [(&str, &[&str]); 3] = [
        (
            "analysis_results",
            &[
                "analysis_results",
                "analysisResults",
                "analysis_result",
                "analysisResult",
            ],
        ),
        (
            "history",
            &["history", "execution_history", "executionHistory"],
        ),
        (
            "caches",
            &[
                "cache",
                "cache_path",
                "cachePath",
                "cache_dir",
                "cacheDir",
                "cached_derived",
                "cachedDerived",
            ],
        ),
    ];
    let mut artifacts = fields
        .iter()
        .filter_map(|(name, aliases)| {
            aliases
                .iter()
                .any(|key| {
                    root.get(*key)
                        .or_else(|| session.and_then(|map| map.get(*key)))
                        .is_some_and(|value| !value.is_null())
                })
                .then_some((*name).to_owned())
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

fn migration_sample_metadata_map(
    root: &JsonMap<String, JsonValue>,
) -> Option<&JsonMap<String, JsonValue>> {
    let session = root.get("session").and_then(JsonValue::as_object);
    let mut containers = vec![root];
    if let Some(session) = session {
        containers.push(session);
    }
    let container_keys = [
        "analysis_results",
        "analysisResults",
        "analysis_result",
        "analysisResult",
        "analysis",
        "resource_info",
        "resourceInfo",
    ];
    let direct_keys = [
        "is_sampled",
        "isSampled",
        "sampled",
        "profile_sampled",
        "profileSampled",
        "sample_rows",
        "sampleRows",
        "sample_rows_count",
        "sampleRowsCount",
        "profile_sample_rows",
        "profileSampleRows",
    ];
    let nested_keys = [
        "sample",
        "analysis_sample",
        "analysisSample",
        "sample_metadata",
        "sampleMetadata",
        "profile",
        "profile_metadata",
        "profileMetadata",
    ];

    for container in containers {
        if direct_keys.iter().any(|key| container.contains_key(*key)) {
            return Some(container);
        }
        for key in container_keys {
            let Some(object) = container.get(key).and_then(JsonValue::as_object) else {
                continue;
            };
            if direct_keys.iter().any(|key| object.contains_key(*key)) {
                return Some(object);
            }
            if let Some(nested) = nested_keys
                .iter()
                .find_map(|key| object.get(*key).and_then(JsonValue::as_object))
            {
                return Some(nested);
            }
        }
    }
    None
}

fn migration_bool_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<bool>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| format!("El campo '{key}' debe ser booleano."))
}

fn migration_session_sample_metadata(
    root: &JsonMap<String, JsonValue>,
) -> Result<Option<SessionSampleMetadataValues>, String> {
    let Some(map) = migration_sample_metadata_map(root) else {
        return Ok(None);
    };
    let sampled = migration_bool_field(
        map,
        &[
            "is_sampled",
            "isSampled",
            "sampled",
            "profile_sampled",
            "profileSampled",
        ],
    )?;
    let sample_row_count = migration_usize_field(
        map,
        &[
            "sample_rows",
            "sampleRows",
            "sample_rows_count",
            "sampleRowsCount",
            "profile_sample_rows",
            "profileSampleRows",
        ],
    )?;
    let total_row_count = migration_usize_field(
        map,
        &[
            "n_total_rows",
            "nTotalRows",
            "total_rows",
            "totalRows",
            "row_count",
            "rowCount",
        ],
    )?;
    if sampled.is_none() && sample_row_count.is_none() && total_row_count.is_none() {
        return Ok(None);
    }
    if let Some(sample_row_count) = sample_row_count {
        if sample_row_count > MAX_SESSION_SAMPLE_ROWS {
            return Err(format!(
                "La muestra de análisis supera el límite local de {MAX_SESSION_SAMPLE_ROWS} filas."
            ));
        }
        if let Some(total_row_count) = total_row_count {
            if sample_row_count > total_row_count {
                return Err(
                    "La muestra de análisis no puede superar el total de filas registrado."
                        .to_owned(),
                );
            }
        }
    }
    Ok(Some((sampled, sample_row_count, total_row_count)))
}

fn migration_session_metadata(
    root: &JsonMap<String, JsonValue>,
) -> Result<Option<SessionMigrationMetadata>, String> {
    let non_portable_artifacts = migration_session_artifact_names(root);
    let has_session_fields = [
        "source_path",
        "snapshot_path",
        "sheet_name",
        "stage_label",
        "applied_ops",
        "selected_cleaning_operations",
        "quality_rules",
        "analysis_checks",
    ]
    .iter()
    .any(|key| migration_session_field(root, key).is_some_and(|value| !value.is_null()))
        || !non_portable_artifacts.is_empty();
    if !has_session_fields {
        return Ok(None);
    }

    let optional_text = |key: &str| -> Result<Option<String>, String> {
        let Some(value) = migration_session_field(root, key) else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        value
            .as_str()
            .map(str::to_owned)
            .map(Some)
            .ok_or_else(|| format!("El metadato de sesión '{key}' debe ser texto."))
    };
    let count_array = |key: &str| -> Result<usize, String> {
        let Some(value) = migration_session_field(root, key) else {
            return Ok(0);
        };
        if value.is_null() {
            return Ok(0);
        }
        value
            .as_array()
            .map(Vec::len)
            .ok_or_else(|| format!("El metadato de sesión '{key}' debe ser un arreglo."))
    };
    let analysis_check_count = match migration_session_field(root, "analysis_checks") {
        None | Some(JsonValue::Null) => 0,
        Some(value) => value
            .as_object()
            .map(JsonMap::len)
            .or_else(|| value.as_array().map(Vec::len))
            .ok_or_else(|| {
                "El metadato de sesión 'analysis_checks' debe ser un objeto o arreglo.".to_owned()
            })?,
    };
    let (analysis_sampled, analysis_sample_row_count, analysis_total_row_count) =
        migration_session_sample_metadata(root)?.unwrap_or_default();

    let metadata_names = |key: &str| -> Result<Vec<String>, String> {
        let Some(value) = migration_session_field(root, key) else {
            return Ok(Vec::new());
        };
        let mut names = match value {
            JsonValue::Array(values) => values.iter().filter_map(migration_metadata_name).collect(),
            JsonValue::Object(values) if key == "analysis_checks" => values
                .keys()
                .filter_map(|value| migration_metadata_name(&JsonValue::String(value.clone())))
                .collect(),
            JsonValue::Null => Vec::new(),
            _ => {
                return Err(format!(
                    "El metadato de sesión '{key}' debe ser un objeto o arreglo."
                ));
            }
        };
        names.truncate(MAX_SESSION_METADATA_NAMES);
        Ok(names)
    };

    let explicit_applied_operations = metadata_names("applied_ops")?;
    let selected_cleaning_operations = metadata_names("selected_cleaning_operations")?;
    let mut applied_operations = explicit_applied_operations
        .into_iter()
        .map(|operation| {
            migration_cleaning_operation(&operation)
                .unwrap_or(operation.as_str())
                .to_owned()
        })
        .collect::<Vec<_>>();
    applied_operations.extend(
        selected_cleaning_operations
            .into_iter()
            .filter_map(|operation| migration_cleaning_operation(&operation).map(str::to_owned)),
    );
    let mut seen_operations = HashSet::new();
    applied_operations.retain(|operation| seen_operations.insert(operation.clone()));

    Ok(Some(SessionMigrationMetadata {
        has_source_reference: migration_session_field(root, "source_path")
            .is_some_and(|value| !value.is_null()),
        has_snapshot_reference: migration_session_field(root, "snapshot_path")
            .is_some_and(|value| !value.is_null()),
        sheet_name: optional_text("sheet_name")?,
        stage_label: optional_text("stage_label")?,
        applied_operation_count: applied_operations.len(),
        quality_rule_count: count_array("quality_rules")?,
        analysis_check_count,
        applied_operations,
        analysis_checks: metadata_names("analysis_checks")?,
        analysis_sampled,
        analysis_sample_row_count,
        analysis_total_row_count,
        non_portable_artifacts,
    }))
}

fn session_execution_history(
    root: &JsonMap<String, JsonValue>,
) -> Vec<SessionExecutionHistoryEntry> {
    let Some(JsonValue::Array(entries)) = migration_session_field(root, "execution_history") else {
        return Vec::new();
    };

    entries
        .iter()
        .rev()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            let outcome = object
                .get("outcome")
                .or_else(|| object.get("status"))
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .and_then(|value| match value.as_str() {
                    "success" => Some("success".to_owned()),
                    "error" => Some("error".to_owned()),
                    "cancelled" | "canceled" => Some("cancelled".to_owned()),
                    _ => None,
                })?;
            let duration_ms = object
                .get("duration_ms")
                .or_else(|| object.get("durationMs"))
                .and_then(|value| {
                    value
                        .as_u64()
                        .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
                })
                .filter(|duration| *duration <= MAX_SESSION_EXECUTION_DURATION_MS)?;
            let row_count = object
                .get("row_count")
                .or_else(|| object.get("rowCount"))
                .and_then(|value| {
                    value
                        .as_u64()
                        .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
                })
                .and_then(|value| usize::try_from(value).ok());

            Some(SessionExecutionHistoryEntry {
                outcome,
                duration_ms,
                row_count,
            })
        })
        .take(MAX_SESSION_EXECUTION_HISTORY_ENTRIES)
        .collect()
}

pub(crate) fn load_dataprep_session_execution_history(
    path: &Path,
) -> Vec<SessionExecutionHistoryEntry> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(size) = file.metadata().map(|metadata| metadata.len()) else {
        return Vec::new();
    };
    if size > RECIPE_FILE_LIMIT_BYTES {
        return Vec::new();
    }
    let mut bytes = Vec::with_capacity(size as usize);
    if file
        .take(RECIPE_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > RECIPE_FILE_LIMIT_BYTES
    {
        return Vec::new();
    }
    let Ok(JsonValue::Object(root)) = serde_json::from_slice::<JsonValue>(&bytes) else {
        return Vec::new();
    };
    session_execution_history(&root)
}

fn session_source_reference(root: &JsonMap<String, JsonValue>) -> Option<&JsonValue> {
    migration_session_field(root, "source_path")
        .or_else(|| migration_session_field(root, "file_name"))
}

fn session_quality_rules(root: &JsonMap<String, JsonValue>) -> Option<&JsonValue> {
    root.get("quality_rules")
        .filter(|value| !value.is_null())
        .or_else(|| root.get("qualityRules").filter(|value| !value.is_null()))
        .or_else(|| root.get("quality").filter(|value| !value.is_null()))
        .or_else(|| {
            root.get("session")
                .and_then(JsonValue::as_object)
                .and_then(|session| {
                    session
                        .get("quality_rules")
                        .filter(|value| !value.is_null())
                        .or_else(|| session.get("qualityRules").filter(|value| !value.is_null()))
                })
        })
}

fn session_reference_status(
    session_path: &Path,
    value: Option<&JsonValue>,
    require_dataset_extension: bool,
) -> (SessionReferenceStatus, Option<PathBuf>) {
    let Some(JsonValue::String(reference)) = value else {
        return (
            if value.is_some() {
                SessionReferenceStatus::Unsupported
            } else {
                SessionReferenceStatus::NotProvided
            },
            None,
        );
    };
    let reference = reference.trim();
    if reference.is_empty() || reference.contains("://") {
        return (SessionReferenceStatus::Unsupported, None);
    }
    let path = Path::new(reference);
    let candidate = if path.is_absolute() {
        path.to_owned()
    } else {
        session_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    let Ok(metadata) = fs::symlink_metadata(&candidate) else {
        return (SessionReferenceStatus::Missing, None);
    };
    if is_symbolic_link_or_reparse_point(&metadata) {
        return (SessionReferenceStatus::Unsupported, None);
    }
    if !metadata.is_file() {
        return (SessionReferenceStatus::Missing, None);
    }
    let Ok(canonical) = canonicalize_existing_file(&candidate, "la referencia de sesión") else {
        return (SessionReferenceStatus::Unsupported, None);
    };
    if require_dataset_extension && dataset_extension(&canonical).is_err() {
        return (SessionReferenceStatus::Unsupported, Some(canonical));
    }
    (SessionReferenceStatus::Available, Some(canonical))
}

fn session_display_file_name(
    root: &JsonMap<String, JsonValue>,
    source_path: Option<&Path>,
) -> Option<String> {
    let raw = migration_session_field(root, "file_name")
        .or_else(|| migration_session_field(root, "source_path"))
        .and_then(JsonValue::as_str)
        .or_else(|| source_path.and_then(|path| path.file_name().and_then(OsStr::to_str)))?;
    let file_name = raw.rsplit(['/', '\\']).next()?.trim();
    if file_name.is_empty() || file_name.chars().any(char::is_control) {
        return None;
    }
    Some(file_name.to_owned())
}

fn session_recipe_collisions(recipe: &TransformRecipe) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut collisions = Vec::new();
    let mut record = |name: &str| {
        let name = name.trim();
        if !name.is_empty() && !seen.insert(name.to_owned()) {
            collisions.push("recipe.output_names".to_owned());
        }
    };
    for rename in &recipe.renames {
        record(&rename.to);
    }
    if let Some(calculated) = &recipe.calculated_column {
        record(&calculated.name);
    }
    if let Some(split) = &recipe.split_column {
        for name in &split.names {
            record(name);
        }
    }
    if let Some(merge) = &recipe.merge_columns {
        record(&merge.name);
    }
    for extraction in &recipe.text_extractions {
        record(&extraction.name);
    }
    collisions.sort();
    collisions.dedup();
    collisions
}

/// Reads a DataPrep session and produces a sanitized, read-only import decision.
/// No project catalog or snapshot is touched by this operation.
pub(crate) fn load_dataprep_session_migration_plan(
    path: &Path,
) -> Result<DataprepSessionMigrationPlan, String> {
    let path = canonicalize_existing_file(path, "la sesión DataPrep seleccionada")?;
    if !path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err("La sesión DataPrep debe ser un archivo JSON.".to_owned());
    }
    let file = File::open(&path)
        .map_err(|error| format!("No se pudo abrir la sesión DataPrep: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("No se pudo verificar la sesión DataPrep: {error}"))?
        .len();
    if size > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La sesión DataPrep supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(RECIPE_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer la sesión DataPrep: {error}"))?;
    if bytes.len() as u64 > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La sesión DataPrep supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    let raw = serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("La sesión DataPrep no es JSON válido: {error}"))?;
    let root = raw
        .as_object()
        .ok_or_else(|| "La sesión DataPrep debe ser un objeto JSON.".to_owned())?;
    let mut recipe = migration_dataprep_recipe(&raw)?;
    if let Some(report) = &mut recipe.migration_report {
        report.artifact_sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    }

    let (source_status, source_path) =
        session_reference_status(&path, session_source_reference(root), true);
    let (snapshot_status, snapshot_path) =
        session_reference_status(&path, migration_session_field(root, "snapshot_path"), true);
    let source_file_name =
        session_display_file_name(root, source_path.as_deref().or(snapshot_path.as_deref()));
    let mut missing_references = Vec::new();
    if matches!(source_status, SessionReferenceStatus::Missing) {
        missing_references.push("source".to_owned());
    } else if matches!(source_status, SessionReferenceStatus::Unsupported) {
        missing_references.push("source.unsupported".to_owned());
    }
    if matches!(snapshot_status, SessionReferenceStatus::Missing) {
        missing_references.push("snapshot".to_owned());
    } else if matches!(snapshot_status, SessionReferenceStatus::Unsupported) {
        missing_references.push("snapshot.unsupported".to_owned());
    }

    let mut collisions = session_recipe_collisions(&recipe.recipe);
    if let (Some(source_path), Some(snapshot_path)) = (&source_path, &snapshot_path) {
        if source_path == snapshot_path {
            collisions.push("source_snapshot".to_owned());
        }
    }
    collisions.sort();
    collisions.dedup();

    let (quality_rules, quality_report) = if let Some(raw_rules) = session_quality_rules(root) {
        let migrated = migrate_quality_rules_document(raw_rules.clone())?;
        (migrated.converted_rules, Some(migrated.report))
    } else {
        (Vec::new(), None)
    };
    let name = recipe.name.clone();
    // A source is preferred because its recipe can be replayed. If it disappeared,
    // an available compatible snapshot is still a valid temporal dataset to import.
    let has_usable_input = matches!(source_status, SessionReferenceStatus::Available)
        || matches!(snapshot_status, SessionReferenceStatus::Available);
    let can_create_project = has_usable_input && collisions.is_empty();
    Ok(DataprepSessionMigrationPlan {
        name,
        source_file_name,
        source_status,
        snapshot_status,
        sheet_name: recipe
            .migration_report
            .as_ref()
            .and_then(|report| report.session.as_ref())
            .and_then(|session| session.sheet_name.clone()),
        stage_label: recipe
            .migration_report
            .as_ref()
            .and_then(|report| report.session.as_ref())
            .and_then(|session| session.stage_label.clone()),
        recipe,
        quality_rules,
        quality_report,
        missing_references,
        collisions,
        can_create_project,
    })
}

fn migration_export_format(value: &str) -> Option<ExportFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "csv" => Some(ExportFormat::Csv),
        "json" => Some(ExportFormat::Json),
        "parquet" => Some(ExportFormat::Parquet),
        "sql" => Some(ExportFormat::Sql),
        "xlsx" | "excel" => Some(ExportFormat::Excel),
        "sqlite" => Some(ExportFormat::Sqlite),
        "bundle" | "zip" => Some(ExportFormat::Bundle),
        _ => None,
    }
}

fn migration_export_options(
    root: &JsonMap<String, JsonValue>,
    converted_operations: &mut Vec<String>,
    omitted_operations: &mut Vec<String>,
    warnings: &mut Vec<RecipeMigrationWarning>,
) -> Result<Option<RecipeExportOptions>, String> {
    let Some(value) = root
        .get("export")
        .or_else(|| root.get("exportOptions"))
        .or_else(|| root.get("export_options"))
    else {
        return Ok(None);
    };
    let export = value
        .as_object()
        .ok_or_else(|| "La configuración de exportación DataPrep debe ser un objeto.".to_owned())?;

    let mut formats = Vec::new();
    if let Some(raw_formats) = export.get("formats") {
        let raw_formats = raw_formats
            .as_array()
            .ok_or_else(|| "export.formats de DataPrep debe ser un arreglo.".to_owned())?;
        for raw_format in raw_formats {
            let raw_format = raw_format
                .as_str()
                .ok_or_else(|| "Cada formato de exportación DataPrep debe ser texto.".to_owned())?;
            if let Some(format) = migration_export_format(raw_format) {
                if !formats.contains(&format) {
                    formats.push(format);
                }
            } else {
                push_migration_operation(omitted_operations, "export.formats");
                warnings.push(recipe_migration_warning(
                    "export.formats",
                    "omitted",
                    "Se omitió un formato de entrega DataPrep sin equivalente local seguro.",
                ));
            }
        }
    } else {
        formats.push(ExportFormat::Csv);
        push_migration_operation(converted_operations, "export.formats.default");
    }
    if formats.is_empty() {
        formats.push(ExportFormat::Csv);
    }
    if export.get("formats").is_some() && !formats.is_empty() {
        push_migration_operation(converted_operations, "export.formats");
    }

    let selected_columns = if let Some(raw_columns) = export.get("selected_columns") {
        let raw_columns = raw_columns
            .as_array()
            .ok_or_else(|| "export.selected_columns de DataPrep debe ser un arreglo.".to_owned())?;
        let mut columns = Vec::with_capacity(raw_columns.len());
        for raw_column in raw_columns {
            let column = raw_column.as_str().ok_or_else(|| {
                "Cada columna seleccionada de exportación debe ser texto.".to_owned()
            })?;
            if !column.trim().is_empty() {
                columns.push(column.to_owned());
            }
        }
        push_migration_operation(converted_operations, "export.selected_columns");
        columns
    } else {
        Vec::new()
    };

    let privacy_mode = if let Some(raw_privacy) = export.get("privacy_mode") {
        let raw_privacy = raw_privacy
            .as_str()
            .ok_or_else(|| "export.privacy_mode de DataPrep debe ser texto.".to_owned())?;
        match raw_privacy.trim().to_ascii_lowercase().as_str() {
            "none" => {
                push_migration_operation(converted_operations, "export.privacy_mode");
                PrivacyMode::None
            }
            "mask" => {
                push_migration_operation(converted_operations, "export.privacy_mode");
                PrivacyMode::Mask
            }
            "hash" => {
                push_migration_operation(converted_operations, "export.privacy_mode");
                PrivacyMode::Hash
            }
            _ => {
                push_migration_operation(omitted_operations, "export.privacy_mode");
                warnings.push(recipe_migration_warning(
                    "export.privacy_mode",
                    "omitted",
                    "La política de privacidad DataPrep no se reconoce; se usará revisión manual.",
                ));
                PrivacyMode::None
            }
        }
    } else {
        PrivacyMode::None
    };

    let unsupported_fields = [
        (
            "report_format",
            "Los reportes de entrega DataPrep requieren una exportación manual.",
        ),
        (
            "csv_separator",
            "El separador CSV DataPrep no forma parte del contrato de receta local.",
        ),
        (
            "csv_encoding",
            "La codificación CSV DataPrep no forma parte del contrato de receta local.",
        ),
        (
            "date_format",
            "El formato de fecha de entrega DataPrep requiere revisión manual.",
        ),
        (
            "package_zip",
            "El empaquetado ZIP DataPrep no tiene equivalente local.",
        ),
        (
            "sql_table_name",
            "El nombre de tabla SQL DataPrep requiere configuración en el destino.",
        ),
        (
            "sql_dialect",
            "El dialecto SQL DataPrep requiere configuración en el destino.",
        ),
        (
            "sql_if_exists",
            "La política SQL DataPrep requiere configuración en el destino.",
        ),
    ];
    for (field, message) in unsupported_fields {
        let Some(raw_value) = export.get(field) else {
            continue;
        };
        if field == "package_zip" && raw_value.as_bool() == Some(false) {
            continue;
        }
        if raw_value.is_null() {
            continue;
        }
        let path = format!("export.{field}");
        push_migration_operation(omitted_operations, &path);
        warnings.push(recipe_migration_warning(path, "omitted", message));
    }

    let known_fields = [
        "formats",
        "selected_columns",
        "privacy_mode",
        "report_format",
        "csv_separator",
        "csv_encoding",
        "date_format",
        "package_zip",
        "sql_table_name",
        "sql_dialect",
        "sql_if_exists",
    ];
    if export
        .keys()
        .any(|key| !known_fields.contains(&key.as_str()))
    {
        push_migration_operation(omitted_operations, "export.additional");
        warnings.push(recipe_migration_warning(
            "export.additional",
            "omitted",
            "Se omitieron opciones de entrega DataPrep no reconocidas por el contrato local.",
        ));
    }

    Ok(Some(RecipeExportOptions {
        formats,
        selected_columns,
        privacy_mode,
    }))
}

fn migration_dataprep_recipe(raw: &JsonValue) -> Result<StoredTransformRecipe, String> {
    let root = raw
        .as_object()
        .ok_or_else(|| "La receta DataPrep debe ser un objeto JSON.".to_owned())?;
    if let Some(version) = root.get("version").and_then(JsonValue::as_u64) {
        if version > 3 {
            return Err(format!(
                "La receta DataPrep usa la versión {version}; solo se admiten versiones 1 a 3."
            ));
        }
    }
    let transform = migration_recipe_transform(root)?;
    let mut canonical = JsonMap::new();
    let mut converted_operations = Vec::new();
    let mut omitted_operations = Vec::new();
    let mut warnings = Vec::new();
    let session = migration_session_metadata(root)?;

    if let Some(selected_cleaning_operations) =
        migration_session_field(root, "selected_cleaning_operations")
    {
        let operations = selected_cleaning_operations.as_array().ok_or_else(|| {
            "selected_cleaning_operations de DataPrep debe ser un arreglo.".to_owned()
        })?;
        for operation in operations {
            let Some(name) = migration_metadata_name(operation) else {
                push_migration_operation(&mut omitted_operations, "selected_cleaning_operations");
                warnings.push(recipe_migration_warning(
                    "selected_cleaning_operations",
                    "omitted",
                    "Una operación de limpieza no tiene un identificador migrable y requiere revisión manual.",
                ));
                continue;
            };
            if let Some(canonical) = migration_cleaning_operation(&name) {
                push_migration_operation(
                    &mut converted_operations,
                    format!("selected_cleaning_operations.{canonical}"),
                );
            } else {
                push_migration_operation(
                    &mut omitted_operations,
                    format!("selected_cleaning_operations.{name}"),
                );
                warnings.push(recipe_migration_warning(
                    format!("selected_cleaning_operations.{name}"),
                    "omitted",
                    "La operación de limpieza DataPrep no tiene un equivalente reversible automático y requiere revisión manual.",
                ));
            }
        }
    }
    for (key, canonical_key) in [
        ("analysis", "analysis"),
        ("quality_rules", "quality_rules"),
        ("qualityRules", "quality_rules"),
        ("quality", "quality"),
    ] {
        if root.get(key).is_some_and(|value| !value.is_null()) {
            if key != canonical_key
                && root
                    .get(canonical_key)
                    .is_some_and(|value| !value.is_null())
            {
                continue;
            }
            push_migration_operation(&mut omitted_operations, canonical_key);
            warnings.push(recipe_migration_warning(
                canonical_key,
                "omitted",
                "El artefacto contiene un bloque que requiere una migración separada.",
            ));
        }
    }
    for key in [
        "analysis_checks",
        "source_path",
        "snapshot_path",
        "sheet_name",
        "stage_label",
    ] {
        if migration_session_field(root, key).is_some_and(|value| !value.is_null()) {
            let path = format!("session.{key}");
            push_migration_operation(&mut omitted_operations, &path);
            warnings.push(recipe_migration_warning(
                &path,
                "omitted",
                "El metadato de sesión requiere revisión manual y no se aplica automáticamente a la receta Columnia.",
            ));
        }
    }
    for artifact in migration_session_artifact_names(root) {
        let path = format!("session.{artifact}");
        push_migration_operation(&mut omitted_operations, &path);
        warnings.push(recipe_migration_warning(
            &path,
            "omitted",
            "El artefacto de sesión no forma parte del contrato portable y debe regenerarse en Columnia.",
        ));
    }
    if let Some(applied_ops) = migration_session_field(root, "applied_ops") {
        let operations = applied_ops
            .as_array()
            .ok_or_else(|| "applied_ops de la sesión DataPrep debe ser un arreglo.".to_owned())?;
        let mut replayable_count = 0;
        for operation in operations {
            if let Some(canonical) = migration_metadata_name(operation)
                .and_then(|name| migration_cleaning_operation(&name))
            {
                push_migration_operation(
                    &mut converted_operations,
                    format!("session.applied_ops.{canonical}"),
                );
                replayable_count += 1;
            }
        }
        let non_replayable_count = operations.len().saturating_sub(replayable_count);
        if non_replayable_count > 0 {
            push_migration_operation(&mut omitted_operations, "session.applied_ops");
            warnings.push(recipe_migration_warning(
                "session.applied_ops",
                "omitted",
                format!(
                    "Se omitieron {non_replayable_count} operaciones aplicadas de la sesión; deben revisarse contra el dataset importado."
                ),
            ));
        }
    }
    let export_options = migration_export_options(
        root,
        &mut converted_operations,
        &mut omitted_operations,
        &mut warnings,
    )?;

    if let Some(rename_text) =
        migration_text_field(transform, &["rename_text", "renameText"], "rename_text")?
    {
        let mut renames = Vec::new();
        for line in rename_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let (from, to) = line.split_once("->").ok_or_else(|| {
                format!("No se pudo migrar el renombrado DataPrep '{line}': falta '->'.")
            })?;
            if from.trim().is_empty() || to.trim().is_empty() {
                return Err("Los renombrados DataPrep no pueden tener nombres vacíos.".to_owned());
            }
            renames.push(serde_json::json!({ "from": from.trim(), "to": to.trim() }));
        }
        canonical.insert("renames".to_owned(), JsonValue::Array(renames));
    }

    if let Some(column) = migration_text_field(transform, &["dtype_col", "dtypeCol"], "dtype_col")?
    {
        let raw_type = migration_text_field(transform, &["dtype_type", "dtypeType"], "dtype_type")?
            .unwrap_or_else(|| "auto".to_owned());
        if !raw_type.eq_ignore_ascii_case("auto") {
            let target = migration_cast_target(&raw_type).ok_or_else(|| {
                format!("El tipo DataPrep '{raw_type}' no se puede migrar de forma segura.")
            })?;
            canonical.insert(
                "casts".to_owned(),
                serde_json::json!([{ "column": column, "target": target }]),
            );
        }
    }

    if let Some(date_columns) =
        migration_string_array(transform, &["parse_date_cols", "parseDateCols"])?
    {
        let format =
            migration_text_field(transform, &["date_format", "dateFormat"], "date_format")?
                .unwrap_or_else(|| "iso8601".to_owned());
        let format = migration_date_format(&format).ok_or_else(|| {
            format!("El formato de fecha DataPrep '{format}' no se puede migrar.")
        })?;
        canonical.insert(
            "dateParses".to_owned(),
            JsonValue::Array(date_columns.into_iter().map(|column| {
                serde_json::json!({ "column": column, "format": format, "target": "date" })
            }).collect()),
        );
    }

    if let Some(filters) = transform.get("filters") {
        let filters = filters
            .as_array()
            .ok_or_else(|| "La lista de filtros DataPrep debe ser un arreglo.".to_owned())?
            .iter()
            .map(|value| {
                let filter = value
                    .as_object()
                    .ok_or_else(|| "Cada filtro DataPrep debe ser un objeto.".to_owned())?;
                let column = migration_required_text(filter, &["col", "column"], "filter.col")?;
                let raw_operator =
                    migration_required_text(filter, &["op", "operator"], "filter.op")?;
                let operator = migration_filter_operator(&raw_operator).ok_or_else(|| {
                    format!("El operador de filtro DataPrep '{raw_operator}' no se puede migrar.")
                })?;
                let value = if matches!(operator, "is_null" | "not_null") {
                    JsonValue::Null
                } else {
                    let raw_value = filter
                        .get("val")
                        .or_else(|| filter.get("value"))
                        .ok_or_else(|| {
                            format!("El filtro DataPrep sobre '{column}' necesita val.")
                        })?;
                    JsonValue::String(migration_scalar_text(raw_value, "filter.val")?)
                };
                Ok(serde_json::json!({ "column": column, "operator": operator, "value": value }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        canonical.insert("filters".to_owned(), JsonValue::Array(filters));
    }

    if let Some(find_replace) = transform
        .get("find_replace")
        .or_else(|| transform.get("findReplace"))
    {
        let config = find_replace
            .as_object()
            .ok_or_else(|| "find_replace de DataPrep debe ser un objeto.".to_owned())?;
        let find =
            migration_text_field(config, &["find"], "find_replace.find")?.unwrap_or_default();
        let replace =
            migration_text_field(config, &["replace"], "find_replace.replace")?.unwrap_or_default();
        let regex = config
            .get("regex")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        if regex {
            return Err("find_replace con expresiones regulares requiere revisión manual y no se importa automáticamente.".to_owned());
        }
        if !find.is_empty() {
            let column = migration_text_field(config, &["col", "column"], "find_replace.col")?;
            let scope = if column.is_some() {
                "column"
            } else {
                "all_text_columns"
            };
            canonical.insert(
                "findReplace".to_owned(),
                serde_json::json!({
                    "scope": scope, "column": column, "find": find, "replace": replace
                }),
            );
        }
    }

    if let Some(columns) = migration_string_array(transform, &["keep_columns", "keepColumns"])? {
        if !columns.is_empty() {
            canonical.insert(
                "keepColumns".to_owned(),
                JsonValue::Array(columns.into_iter().map(JsonValue::String).collect()),
            );
        }
    }

    if let Some(calc) = transform
        .get("calc")
        .or_else(|| transform.get("calculatedColumn"))
    {
        let config = calc
            .as_object()
            .ok_or_else(|| "calc de DataPrep debe ser un objeto.".to_owned())?;
        let name = migration_text_field(config, &["name"], "calc.name")?.unwrap_or_default();
        let source = migration_text_field(config, &["col_a", "source", "column"], "calc.col_a")?
            .unwrap_or_default();
        let operation = migration_text_field(config, &["operation", "op"], "calc.operation")?
            .unwrap_or_else(|| "add".to_owned());
        if !name.is_empty() || !source.is_empty() {
            if name.is_empty() || source.is_empty() {
                return Err(
                    "La columna calculada DataPrep necesita nombre y columna de origen.".to_owned(),
                );
            }
            let operation = migration_calculated_operation(&operation).ok_or_else(|| {
                format!("La operación calculada DataPrep '{operation}' no se puede migrar.")
            })?;
            let operand = migration_text_field(
                config,
                &["col_b_or_val", "operand", "value"],
                "calc.col_b_or_val",
            )?
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::json!({ "kind": "literal", "value": value }));
            canonical.insert(
                "calculatedColumn".to_owned(),
                serde_json::json!({
                    "name": name, "source": source, "operation": operation, "operand": operand
                }),
            );
        }
    }

    if let Some(split) = transform
        .get("split_column")
        .or_else(|| transform.get("splitColumn"))
    {
        let config = split
            .as_object()
            .ok_or_else(|| "split_column de DataPrep debe ser un objeto.".to_owned())?;
        let source = if config
            .get("column")
            .or_else(|| config.get("source"))
            .is_some_and(|value| !value.is_null())
        {
            migration_text_field(config, &["column", "source"], "split_column.column")?
        } else {
            None
        };
        let delimiter = if config
            .get("delimiter")
            .is_some_and(|value| !value.is_null())
        {
            migration_text_field(config, &["delimiter"], "split_column.delimiter")?
        } else {
            None
        };
        let names = config.get("new_names").or_else(|| config.get("names"));
        let drop_source = config
            .get("drop_source")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let is_configured = source.is_some()
            || delimiter.as_deref().is_some_and(|value| !value.is_empty())
            || names.is_some_and(|value| value.as_array().is_some_and(|values| !values.is_empty()))
            || drop_source;
        if is_configured {
            let source = source.ok_or_else(|| "split_column.column es obligatorio.".to_owned())?;
            let delimiter =
                delimiter.ok_or_else(|| "split_column.delimiter es obligatorio.".to_owned())?;
            let names =
                migration_string_array(config, &["new_names", "names"])?.unwrap_or_default();
            if names.len() < 2 {
                return Err(
                    "split_column de DataPrep necesita al menos dos nombres de salida.".to_owned(),
                );
            }
            canonical.insert("splitColumn".to_owned(), serde_json::json!({
                "source": source, "delimiter": delimiter, "names": names, "dropSource": drop_source
            }));
        }
    }

    if let Some(merge) = transform
        .get("merge_columns")
        .or_else(|| transform.get("mergeColumns"))
    {
        let config = merge
            .as_object()
            .ok_or_else(|| "merge_columns de DataPrep debe ser un objeto.".to_owned())?;
        let sources = migration_string_array(config, &["columns", "sources"])?.unwrap_or_default();
        let name =
            migration_text_field(config, &["name"], "merge_columns.name")?.unwrap_or_default();
        if !sources.is_empty() || !name.is_empty() {
            if sources.len() < 2 || name.is_empty() {
                return Err(
                    "merge_columns de DataPrep necesita dos columnas y un nombre.".to_owned(),
                );
            }
            let separator =
                migration_text_field(config, &["separator"], "merge_columns.separator")?
                    .unwrap_or_else(|| " ".to_owned());
            let drop_sources = config
                .get("drop_sources")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
            canonical.insert("mergeColumns".to_owned(), serde_json::json!({
                "sources": sources, "name": name, "separator": separator, "dropSources": drop_sources
            }));
        }
    }

    if let Some(outliers) = transform.get("outliers") {
        let config = outliers
            .as_object()
            .ok_or_else(|| "outliers de DataPrep debe ser un objeto.".to_owned())?;
        let mut treatments = Vec::new();
        for (key, action) in [("cap_cols", "cap"), ("drop_cols", "drop")] {
            if let Some(columns) = migration_string_array(config, &[key])? {
                treatments.extend(
                    columns
                        .into_iter()
                        .map(|column| serde_json::json!({ "column": column, "action": action })),
                );
            }
        }
        if !treatments.is_empty() {
            canonical.insert("outlierTreatments".to_owned(), JsonValue::Array(treatments));
        }
    }

    if let Some(group) = transform
        .get("group_summary")
        .or_else(|| transform.get("groupSummary"))
    {
        let config = group
            .as_object()
            .ok_or_else(|| "group_summary de DataPrep debe ser un objeto.".to_owned())?;
        let group_by =
            migration_string_array(config, &["group_by", "groupBy"])?.unwrap_or_default();
        let aggregations = config
            .get("aggregations")
            .and_then(JsonValue::as_object)
            .map(|values| {
                values
                    .iter()
                    .map(|(column, operation)| {
                        let operation = operation
                            .as_str()
                            .and_then(migration_summary_operation)
                            .ok_or_else(|| {
                                format!("La agregación DataPrep '{column}' no se puede migrar.")
                            })?;
                        Ok(serde_json::json!({ "column": column, "operation": operation }))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default();
        if !group_by.is_empty() || !aggregations.is_empty() {
            if group_by.is_empty() || aggregations.is_empty() {
                return Err("group_summary de DataPrep necesita claves y agregaciones.".to_owned());
            }
            canonical.insert(
                "groupSummary".to_owned(),
                serde_json::json!({ "groupBy": group_by, "aggregations": aggregations }),
            );
        }
    }

    if let Some(contacts) = transform
        .get("normalize_contacts")
        .or_else(|| transform.get("normalizeContacts"))
    {
        let config = contacts
            .as_object()
            .ok_or_else(|| "normalize_contacts de DataPrep debe ser un objeto.".to_owned())?;
        let mut normalized = Vec::new();
        for (key, kind) in [
            ("email_cols", "email"),
            ("phone_cols", "phone"),
            ("address_cols", "address"),
        ] {
            if let Some(columns) = migration_string_array(config, &[key])? {
                normalized.extend(
                    columns
                        .into_iter()
                        .map(|column| serde_json::json!({ "column": column, "kind": kind })),
                );
            }
        }
        if !normalized.is_empty() {
            canonical.insert(
                "contactNormalizations".to_owned(),
                JsonValue::Array(normalized),
            );
        }
    }

    if let Some(extraction) = transform
        .get("extract_text")
        .or_else(|| transform.get("extractText"))
    {
        let config = extraction
            .as_object()
            .ok_or_else(|| "extract_text de DataPrep debe ser un objeto.".to_owned())?;
        let source =
            migration_text_field(config, &["source_col", "source"], "extract_text.source_col")?
                .unwrap_or_default();
        let name = migration_text_field(config, &["new_name", "name"], "extract_text.new_name")?
            .unwrap_or_default();
        let kind =
            migration_text_field(config, &["extraction", "kind"], "extract_text.extraction")?
                .unwrap_or_default()
                .to_ascii_lowercase();
        if !source.is_empty() || !name.is_empty() || !kind.is_empty() {
            let kind = match kind.as_str() {
                "first_token" | "first" => "first_token",
                "last_token" | "last" => "last_token",
                "digits" => "digits",
                "letters" => "letters",
                "before" => "before",
                "after" => "after",
                _ => {
                    return Err(format!(
                        "La extracción DataPrep '{kind}' no se puede migrar."
                    ))
                }
            };
            if source.is_empty() || name.is_empty() {
                return Err(
                    "extract_text de DataPrep necesita origen y nombre de salida.".to_owned(),
                );
            }
            let delimiter = migration_text_field(
                config,
                &["pattern_or_delimiter", "delimiter"],
                "extract_text.pattern_or_delimiter",
            )?;
            canonical.insert(
                "textExtractions".to_owned(),
                serde_json::json!([{
                    "source": source, "kind": kind, "name": name, "delimiter": delimiter
                }]),
            );
        }
    }

    for key in ["true_values", "false_values", "trueValues", "falseValues"] {
        if let Some(values) = transform.get(key).and_then(JsonValue::as_array) {
            if !values.is_empty() {
                return Err(format!(
                    "La normalización de booleanos DataPrep ('{key}') requiere revisión manual."
                ));
            }
        }
    }

    let recipe: TransformRecipe = serde_json::from_value(JsonValue::Object(canonical))
        .map_err(|error| format!("La transformación DataPrep convertida no es válida: {error}"))?;
    let mut canonical_operations = Vec::new();
    for key in [
        "renames",
        "casts",
        "dateParses",
        "filters",
        "calculatedColumn",
        "findReplace",
        "keepColumns",
        "splitColumn",
        "mergeColumns",
        "outlierTreatments",
        "groupSummary",
        "contactNormalizations",
        "textExtractions",
    ] {
        let field_is_present = match key {
            "renames" => !recipe.renames.is_empty(),
            "casts" => !recipe.casts.is_empty(),
            "dateParses" => !recipe.date_parses.is_empty(),
            "filters" => !recipe.filters.is_empty(),
            "calculatedColumn" => recipe.calculated_column.is_some(),
            "findReplace" => recipe.find_replace.is_some(),
            "keepColumns" => recipe.keep_columns.is_some(),
            "splitColumn" => recipe.split_column.is_some(),
            "mergeColumns" => recipe.merge_columns.is_some(),
            "outlierTreatments" => !recipe.outlier_treatments.is_empty(),
            "groupSummary" => recipe.group_summary.is_some(),
            "contactNormalizations" => !recipe.contact_normalizations.is_empty(),
            "textExtractions" => !recipe.text_extractions.is_empty(),
            _ => false,
        };
        if field_is_present {
            canonical_operations.push(key.to_owned());
        }
    }
    converted_operations.extend(canonical_operations);
    let source_version = root.get("version").and_then(JsonValue::as_u64);
    let source_format = if source_version.is_some() {
        "dataprep"
    } else {
        "legacy"
    };
    let name = migration_string_field(root, &["name"])
        .unwrap_or_else(|| "Receta DataPrep importada".to_owned());
    let saved_at = migration_string_field(root, &["savedAt", "saved_at"])
        .unwrap_or_else(current_recipe_timestamp);
    let document = StoredTransformRecipe {
        version: RECIPE_FILE_VERSION,
        name,
        saved_at,
        recipe,
        export_options,
        migration_report: Some(build_recipe_migration_report(
            source_format,
            source_version,
            converted_operations,
            omitted_operations,
            warnings,
            session,
        )),
    };
    validate_stored_recipe(&document)?;
    Ok(document)
}

fn migration_string_field(map: &JsonMap<String, JsonValue>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(JsonValue::as_str).map(str::to_owned))
}

fn migration_policy_value(
    map: &JsonMap<String, JsonValue>,
    key: &str,
    default: &str,
) -> Result<String, String> {
    let value = match key {
        "on_missing" => map
            .get(key)
            .filter(|value| !value.is_null())
            .or_else(|| map.get("onMissing").filter(|value| !value.is_null())),
        "null_policy" => map
            .get(key)
            .filter(|value| !value.is_null())
            .or_else(|| map.get("nullPolicy").filter(|value| !value.is_null())),
        _ => map.get(key).filter(|value| !value.is_null()),
    };
    match value {
        None | Some(JsonValue::Null) => Ok(default.to_owned()),
        Some(JsonValue::String(value)) => {
            let value = value.trim().to_ascii_lowercase();
            let normalized = match key {
                "on_missing" if matches!(value.as_str(), "error" | "block" | "blocking") => "fail",
                "null_policy" if matches!(value.as_str(), "reject" | "error" | "fail") => "invalid",
                _ => value.as_str(),
            };
            Ok(normalized.to_owned())
        }
        Some(_) => Err(format!("El campo '{key}' debe ser texto.")),
    }
}

fn migration_blocking_value(map: &JsonMap<String, JsonValue>) -> Result<Option<bool>, String> {
    let Some(value) = map
        .get("blocking")
        .or_else(|| map.get("is_blocking"))
        .or_else(|| map.get("isBlocking"))
    else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| "El campo 'blocking' debe ser booleano.".to_owned())
}

fn migration_severity_value(map: &JsonMap<String, JsonValue>) -> Result<String, String> {
    let blocking = migration_blocking_value(map)?;
    let severity_is_absent = map.get("severity").is_none_or(JsonValue::is_null);
    let severity = if severity_is_absent && blocking == Some(false) {
        "warning".to_owned()
    } else {
        migration_policy_value(map, "severity", "blocking")?
    };
    let normalized = match severity.as_str() {
        // DataPrep historically called blocking rules "error" or "critical".
        // They are equivalent to Columnia's blocking gate and can be retained.
        "blocking" | "error" | "critical" | "fatal" => "blocking",
        "warning" | "warn" | "non_blocking" | "non-blocking" | "info" => "non_blocking",
        _ => return Err(format!("La severidad '{severity}' no está soportada.")),
    };
    if let Some(blocking) = blocking {
        let blocking_severity = if blocking { "blocking" } else { "non_blocking" };
        if normalized != blocking_severity {
            return Err(
                "severity y blocking describen políticas contradictorias; la regla se omitió."
                    .to_owned(),
            );
        }
    }
    Ok(normalized.to_owned())
}

fn migration_has_true_nullable(map: &JsonMap<String, JsonValue>) -> Result<bool, String> {
    let Some(value) = map
        .get("nullable")
        .or_else(|| map.get("allow_nulls"))
        .or_else(|| map.get("allowNulls"))
    else {
        return Ok(false);
    };
    value
        .as_bool()
        .ok_or_else(|| "El campo 'nullable' debe ser booleano.".to_owned())
}

fn migration_validate_quality_metadata(
    map: &JsonMap<String, JsonValue>,
    label: &str,
) -> Result<(), String> {
    if migration_has_true_nullable(map)? {
        return Err(format!(
            "{label} declara nullable/allowNulls=true, pero el contrato Columnia no puede conservar esa política sin degradar la regla."
        ));
    }
    if [
        "reference_revision",
        "referenceRevision",
        "reference_dataset",
        "referenceDataset",
    ]
    .iter()
    .any(|key| map.get(*key).is_some_and(|value| !value.is_null()))
    {
        return Err(format!(
            "{label} usa una referencia externa (referenceRevision/referenceDataset) que Columnia no puede resolver de forma segura."
        ));
    }
    Ok(())
}

fn migration_number_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<f64>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let number = value
        .as_f64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<f64>().ok())
        })
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("El campo '{key}' debe ser un número finito."))?;
    Ok(Some(number))
}

fn migration_usize_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<usize>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let number = value
        .as_u64()
        .or_else(|| {
            value.as_f64().and_then(|number| {
                (number.is_finite() && number >= 0.0 && number.fract() == 0.0)
                    .then_some(number as u64)
            })
        })
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| format!("El campo '{key}' debe ser un entero no negativo."))?;
    Ok(Some(number))
}

fn migration_string_array(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<Vec<String>>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("El campo '{key}' debe ser una lista."))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Todos los elementos de '{key}' deben ser texto."))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(values))
}

fn migration_reference_values(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
    column_count: usize,
) -> Result<Option<Vec<String>>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("El campo '{key}' debe ser una lista."))?;
    if values.len() > MAX_QUALITY_VALUES {
        return Err(format!(
            "El campo '{key}' supera el máximo de {MAX_QUALITY_VALUES} valores."
        ));
    }
    values
        .iter()
        .map(|value| {
            if column_count == 1 {
                match value {
                    JsonValue::String(value) => Ok(value.clone()),
                    JsonValue::Bool(value) => Ok(value.to_string()),
                    JsonValue::Number(value) => Ok(value.to_string()),
                    JsonValue::Null => Err(format!(
                        "Los elementos de '{key}' deben ser valores escalares no nulos."
                    )),
                    JsonValue::Array(_) | JsonValue::Object(_) => Err(format!(
                        "Los elementos de '{key}' deben ser escalares para una columna."
                    )),
                }
            } else {
                let components = value.as_array().ok_or_else(|| {
                    format!(
                        "Los elementos de '{key}' deben ser arreglos para una clave compuesta."
                    )
                })?;
                if components.len() != column_count
                    || components.iter().any(|component| {
                        matches!(component, JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_))
                    })
                {
                    return Err(format!(
                        "Cada referencia de '{key}' debe contener {column_count} escalares no nulos."
                    ));
                }
                serde_json::to_string(value).map_err(|error| {
                    format!("No se pudo normalizar un valor de '{key}': {error}")
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn migration_quality_kind(value: &str) -> Option<QualityRuleKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "not_null" => Some(QualityRuleKind::NotNull),
        "non_empty" => Some(QualityRuleKind::NonEmpty),
        "unique" => Some(QualityRuleKind::Unique),
        "numeric_range" | "range" => Some(QualityRuleKind::NumericRange),
        "allowed_values" => Some(QualityRuleKind::AllowedValues),
        "regex" => Some(QualityRuleKind::Regex),
        "dtype" => Some(QualityRuleKind::Dtype),
        "unique_together" => Some(QualityRuleKind::UniqueTogether),
        "column_compare" | "column_comparison" => Some(QualityRuleKind::ColumnCompare),
        "referential_integrity" | "referential" => Some(QualityRuleKind::ReferentialIntegrity),
        "monotonic" => Some(QualityRuleKind::Monotonic),
        "aggregate_check" | "aggregate" => Some(QualityRuleKind::AggregateCheck),
        "aggregate_reconciliation" | "aggregate_reconcile" | "reconciliation" => {
            Some(QualityRuleKind::AggregateReconciliation)
        }
        "distribution_drift" | "drift" => Some(QualityRuleKind::DistributionDrift),
        "date_range" => Some(QualityRuleKind::DateRange),
        "conditional" => Some(QualityRuleKind::Conditional),
        "schema_contract" | "schema" => Some(QualityRuleKind::SchemaContract),
        "row_count" => Some(QualityRuleKind::RowCount),
        _ => None,
    }
}

fn migration_quality_monotonic_direction(value: &str) -> Option<QualityMonotonicDirection> {
    match value.trim().to_ascii_lowercase().as_str() {
        "increasing" | "increase" | "asc" | "ascending" => {
            Some(QualityMonotonicDirection::Increasing)
        }
        "decreasing" | "decrease" | "desc" | "descending" => {
            Some(QualityMonotonicDirection::Decreasing)
        }
        _ => None,
    }
}

fn migration_quality_aggregate(value: &str) -> Option<QualityAggregate> {
    match value.trim().to_ascii_lowercase().as_str() {
        "count" | "counts" | "n" => Some(QualityAggregate::Count),
        "sum" | "total" => Some(QualityAggregate::Sum),
        "min" | "minimum" => Some(QualityAggregate::Min),
        "max" | "maximum" => Some(QualityAggregate::Max),
        _ => None,
    }
}

fn migration_quality_comparison(value: &str) -> Option<QualityComparison> {
    match value.trim().to_ascii_lowercase().as_str() {
        "eq" | "equal" | "equals" => Some(QualityComparison::Eq),
        "ne" | "neq" | "not_equal" => Some(QualityComparison::Ne),
        "lt" | "less_than" => Some(QualityComparison::Lt),
        "lte" | "le" | "less_or_equal" => Some(QualityComparison::Lte),
        "gt" | "greater_than" => Some(QualityComparison::Gt),
        "gte" | "ge" | "greater_or_equal" => Some(QualityComparison::Gte),
        _ => None,
    }
}

fn migration_condition_value(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn migration_quality_condition(
    value: Option<&JsonValue>,
) -> Result<Option<QualityCondition>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let map = value
        .as_object()
        .ok_or_else(|| "conditional necesita un objeto when.".to_owned())?;
    let column = migration_string_field(map, &["column"])
        .filter(|column| !column.trim().is_empty())
        .ok_or_else(|| "conditional.when necesita column.".to_owned())?;
    let operator = match migration_string_field(map, &["operator", "op"]) {
        Some(value) => Some(migration_quality_comparison(&value).ok_or_else(|| {
            "conditional.when necesita operator eq, ne, lt, lte, gt o gte.".to_owned()
        })?),
        None => Some(QualityComparison::Eq),
    };
    let condition_value = map
        .get("value")
        .or_else(|| map.get("val"))
        .and_then(migration_condition_value);
    if condition_value.is_none() {
        return Err("conditional.when necesita value.".to_owned());
    }
    Ok(Some(QualityCondition {
        column,
        operator,
        value: condition_value,
    }))
}

fn migrate_conditional_then(
    value: Option<&JsonValue>,
    fallback_column: &str,
) -> Result<QualityRule, String> {
    let map = value
        .and_then(JsonValue::as_object)
        .ok_or_else(|| "conditional necesita un objeto then.".to_owned())?;
    let source_kind =
        migration_string_field(map, &["kind", "type"]).unwrap_or_else(|| "not_null".to_owned());
    let kind = migration_quality_kind(&source_kind).ok_or_else(|| {
        "La subregla then de conditional todavía no tiene representación equivalente.".to_owned()
    })?;
    if !matches!(
        kind,
        QualityRuleKind::NotNull
            | QualityRuleKind::NonEmpty
            | QualityRuleKind::NumericRange
            | QualityRuleKind::AllowedValues
            | QualityRuleKind::Regex
            | QualityRuleKind::Dtype
    ) {
        return Err(
            "conditional solo admite subreglas then fila-a-fila: not_null, non_empty, numeric_range, allowed_values, regex o dtype.".to_owned(),
        );
    }
    let severity = migration_severity_value(map)?;
    if severity != "blocking" {
        return Err(format!(
            "La severidad de la subregla then ({severity}) no tiene equivalente seguro en Columnia."
        ));
    }
    for (key, expected, label) in [
        ("on_missing", "fail", "La política on_missing"),
        ("null_policy", "invalid", "La política de nulos"),
    ] {
        let value = migration_policy_value(map, key, expected)?;
        if value != expected {
            return Err(format!(
                "{label} de la subregla then ({value}) no tiene equivalente seguro en Columnia."
            ));
        }
    }
    migration_validate_quality_metadata(map, "La subregla then")?;
    let column = migration_string_field(map, &["column"])
        .filter(|column| !column.trim().is_empty())
        .unwrap_or_else(|| fallback_column.to_owned());
    let mut rule = QualityRule {
        column,
        kind,
        max_invalid: Some(0),
        max_invalid_pct: None,
        min: None,
        max: None,
        values: None,
        reference_values: None,
        baseline: None,
        direction: None,
        expected: None,
        aggregate: None,
        tolerance_abs: None,
        tolerance_rel: None,
        threshold: None,
        pattern: None,
        dtype: None,
        columns: None,
        operator: None,
        min_date: None,
        max_date: None,
        when: None,
        then: None,
        allow_additional: None,
        required_order: None,
    };
    match kind {
        QualityRuleKind::NumericRange => {
            rule.min = migration_number_field(map, &["min", "min_value", "minValue"])?;
            rule.max = migration_number_field(map, &["max", "max_value", "maxValue"])?;
            if rule.min.is_none() && rule.max.is_none() {
                return Err("La subregla numeric_range necesita min o max.".to_owned());
            }
        }
        QualityRuleKind::AllowedValues => {
            rule.values = migration_reference_values(map, &["values"], 1)?;
            if rule.values.as_ref().is_none_or(Vec::is_empty) {
                return Err("La subregla allowed_values necesita values[].".to_owned());
            }
        }
        QualityRuleKind::Regex => {
            rule.pattern = migration_string_field(map, &["pattern"]);
            if rule.pattern.as_deref().is_none_or(str::is_empty) {
                return Err("La subregla regex necesita pattern.".to_owned());
            }
        }
        QualityRuleKind::Dtype => {
            let source_dtype = migration_string_field(map, &["dtype"])
                .ok_or_else(|| "La subregla dtype necesita dtype.".to_owned())?;
            rule.dtype = migration_dtype(&source_dtype).map(str::to_owned);
            if rule.dtype.is_none() {
                return Err("La subregla dtype no contiene un tipo soportado.".to_owned());
            }
        }
        QualityRuleKind::NotNull | QualityRuleKind::NonEmpty => {}
        _ => unreachable!("conditional then fue validado arriba"),
    }
    Ok(rule)
}

fn parse_quality_datetime(value: &str) -> Option<NaiveDateTime> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    parse_recipe_datetime(value, RecipeDateFormat::Iso8601)
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%d/%m/%Y")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y/%m/%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })
}

fn migration_dtype(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "string" | "text" | "str" => Some("string"),
        "integer" | "int" | "int64" => Some("integer"),
        "numeric" | "number" | "float" | "float64" | "decimal" => Some("float"),
        "boolean" | "bool" => Some("boolean"),
        "date" => Some("date"),
        "datetime" | "timestamp" => Some("datetime"),
        _ => None,
    }
}

fn migration_warning(
    rule_index: usize,
    source_kind: &str,
    severity: &'static str,
    message: impl Into<String>,
) -> QualityMigrationWarning {
    QualityMigrationWarning {
        rule_index,
        source_kind: source_kind.to_owned(),
        severity,
        message: message.into(),
    }
}

fn quality_migration_report(
    artifact_sha256: Option<String>,
    total_items: usize,
    converted_items: usize,
    omitted_items: usize,
    warnings: &[QualityMigrationWarning],
) -> QualityMigrationReport {
    let mut manual_actions = vec!["Validar el contrato convertido antes de exportar.".to_owned()];
    if omitted_items > 0 {
        manual_actions.push(
            "Revisar las reglas omitidas y recrearlas manualmente si siguen siendo necesarias."
                .to_owned(),
        );
    }
    if warnings.iter().any(|warning| warning.severity == "warning") {
        manual_actions.push(
            "Revisar las tolerancias ajustadas o asumidas frente al documento original.".to_owned(),
        );
    }
    QualityMigrationReport {
        artifact_sha256,
        total_items,
        converted_items,
        omitted_items,
        warning_count: warnings.len(),
        manual_actions,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyQualityRulesDocument {
    version: u8,
    rules: Vec<QualityRule>,
}

fn build_quality_rules_document(rules: Vec<QualityRule>) -> Result<QualityRulesDocument, String> {
    let document = QualityRulesDocument {
        format: QUALITY_RULES_DOCUMENT_FORMAT.to_owned(),
        version: QUALITY_RULES_DOCUMENT_VERSION,
        rules,
    };
    validate_quality_rules_document(&document)?;
    Ok(document)
}

fn validate_quality_rules_document(document: &QualityRulesDocument) -> Result<(), String> {
    if document.format != QUALITY_RULES_DOCUMENT_FORMAT {
        return Err(format!(
            "El formato del contrato debe ser '{QUALITY_RULES_DOCUMENT_FORMAT}'."
        ));
    }
    if document.version != QUALITY_RULES_DOCUMENT_VERSION {
        return Err(format!(
            "La versión {} del contrato de calidad no es compatible; Columnia admite la versión {QUALITY_RULES_DOCUMENT_VERSION}.",
            document.version
        ));
    }
    validate_quality_rules_payload(&document.rules)?;
    let encoded = serde_json::to_vec(document)
        .map_err(|error| format!("No se pudo validar el contrato de calidad: {error}"))?;
    if encoded.len() as u64 > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    Ok(())
}

fn parse_quality_rules_document(
    document: JsonValue,
    allow_legacy_v1: bool,
) -> Result<QualityRulesDocument, String> {
    if document
        .as_object()
        .is_some_and(|map| map.contains_key("format"))
    {
        let document = serde_json::from_value::<QualityRulesDocument>(document)
            .map_err(|error| format!("El contrato Columnia no es válido: {error}"))?;
        validate_quality_rules_document(&document)?;
        return Ok(document);
    }
    if !allow_legacy_v1 {
        return Err(format!(
            "El contrato Columnia debe declarar format='{QUALITY_RULES_DOCUMENT_FORMAT}'."
        ));
    }
    let legacy = serde_json::from_value::<LegacyQualityRulesDocument>(document)
        .map_err(|error| format!("El contrato de calidad v1 no es válido: {error}"))?;
    if legacy.version != QUALITY_RULES_DOCUMENT_VERSION {
        return Err(format!(
            "La versión {} del contrato de calidad no es compatible; Columnia admite la versión {QUALITY_RULES_DOCUMENT_VERSION}.",
            legacy.version
        ));
    }
    build_quality_rules_document(legacy.rules)
}

fn migration_quality_document_version(
    document: &JsonMap<String, JsonValue>,
) -> Result<Option<u8>, String> {
    let Some((field, raw_version)) = ["version", "schema_version"]
        .iter()
        .find_map(|field| document.get(*field).map(|value| (*field, value)))
    else {
        return Ok(None);
    };
    let version = raw_version
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| raw_version.as_str()?.trim().parse::<u8>().ok())
        .ok_or_else(|| format!("El campo '{field}' debe ser un entero de versión."))?;
    if !(1..=DATAPREP_QUALITY_DOCUMENT_MAX_VERSION).contains(&version) {
        return Err(format!(
            "La versión DataPrep {version} no es compatible; se admiten las versiones 1 a {DATAPREP_QUALITY_DOCUMENT_MAX_VERSION}."
        ));
    }
    Ok(Some(version))
}

fn migrate_quality_rules_document(document: JsonValue) -> Result<QualityMigrationResult, String> {
    if document
        .as_object()
        .is_some_and(|map| map.contains_key("format"))
    {
        let document = parse_quality_rules_document(document, false)?;
        let total_items = document.rules.len();
        let warnings = Vec::new();
        return Ok(QualityMigrationResult {
            source_format: "columnia",
            source_version: Some(document.version.to_string()),
            converted_rules: document.rules,
            report: quality_migration_report(None, total_items, total_items, 0, &warnings),
            warnings,
            omitted_rules: 0,
        });
    }

    let (source_format, source_version, raw_rules) = match document {
        JsonValue::Array(rules) => ("legacy", None, rules),
        JsonValue::Object(document) => {
            let source_version = migration_quality_document_version(&document)?;
            let source_format = if document.contains_key("quality_rules")
                || document.contains_key("schema_version")
                || source_version.is_some_and(|version| version >= 2)
            {
                "dataprep"
            } else {
                "legacy"
            };
            let rules = document
                .get("rules")
                .or_else(|| document.get("quality_rules"))
                .and_then(JsonValue::as_array)
                .cloned()
                .ok_or_else(|| "El documento debe contener una lista 'rules'.".to_owned())?;
            (
                source_format,
                source_version.map(|version| version.to_string()),
                rules,
            )
        }
        _ => {
            return Err(
                "Las reglas migradas deben ser una lista o un objeto con 'rules'.".to_owned(),
            )
        }
    };

    if raw_rules.len() > MAX_QUALITY_RULES {
        return Err(format!(
            "El documento contiene {} reglas; Columnia admite como máximo {} en un contrato.",
            raw_rules.len(),
            MAX_QUALITY_RULES
        ));
    }

    let total_items = raw_rules.len();
    let mut converted_rules = Vec::new();
    let mut warnings = Vec::new();
    let mut omitted_rules = 0;
    for (zero_index, raw_rule) in raw_rules.into_iter().enumerate() {
        let rule_index = zero_index + 1;
        let JsonValue::Object(map) = raw_rule else {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                "unknown",
                "omitted",
                "La entrada no es un objeto JSON y se omitió.",
            ));
            continue;
        };
        let source_kind =
            migration_string_field(&map, &["kind", "type"]).unwrap_or_else(|| "unknown".to_owned());
        let Some(kind) = migration_quality_kind(&source_kind) else {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La regla todavía no tiene representación equivalente en Columnia.",
            ));
            continue;
        };
        let mut column = migration_string_field(&map, &["column"])
            .filter(|column| !column.trim().is_empty())
            .unwrap_or_else(|| QUALITY_DATASET_COLUMN.to_owned());
        if kind == QualityRuleKind::ReferentialIntegrity && column == QUALITY_DATASET_COLUMN {
            column = map
                .get("columns")
                .or_else(|| map.get("key_columns"))
                .and_then(JsonValue::as_array)
                .and_then(|columns| columns.first())
                .and_then(JsonValue::as_str)
                .filter(|column| !column.trim().is_empty())
                .unwrap_or(QUALITY_DATASET_COLUMN)
                .to_owned();
        }
        if !matches!(
            kind,
            QualityRuleKind::RowCount | QualityRuleKind::SchemaContract
        ) && column == QUALITY_DATASET_COLUMN
        {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La regla necesita una columna concreta.",
            ));
            continue;
        }

        let severity = match migration_severity_value(&map) {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if severity != "blocking" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La severidad no bloqueante no se convierte porque Columnia aún no tiene severidades por regla; la regla se omitió para no aprobarla silenciosamente.",
            ));
            continue;
        }
        if let Err(error) = migration_validate_quality_metadata(&map, "La regla") {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                error,
            ));
            continue;
        }
        let on_missing = match migration_policy_value(&map, "on_missing", "fail") {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if on_missing != "fail" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La política on_missing no bloqueante no tiene equivalente seguro en Columnia.",
            ));
            continue;
        }
        let null_policy = match migration_policy_value(&map, "null_policy", "invalid") {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if null_policy != "invalid" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La política de nulos distinta de invalid no tiene equivalente seguro en Columnia.",
            ));
            continue;
        }

        let max_invalid = match migration_usize_field(&map, &["maxInvalid", "max_invalid"]) {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        let raw_max_invalid_pct =
            match migration_number_field(&map, &["maxInvalidPct", "max_invalid_pct"]) {
                Ok(value) => value,
                Err(error) => {
                    omitted_rules += 1;
                    warnings.push(migration_warning(
                        rule_index,
                        &source_kind,
                        "omitted",
                        error,
                    ));
                    continue;
                }
            };
        let max_invalid_pct = raw_max_invalid_pct.map(|number| number.clamp(0.0, 100.0));
        let (max_invalid, max_invalid_pct) = if max_invalid.is_none() && max_invalid_pct.is_none() {
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "warning",
                "La regla no traía tolerancia; se importó con máximo de inválidos igual a 0.",
            ));
            (Some(0), None)
        } else {
            (max_invalid, max_invalid_pct)
        };
        let mut rule = QualityRule {
            column,
            kind,
            max_invalid,
            max_invalid_pct,
            min: None,
            max: None,
            values: None,
            reference_values: None,
            baseline: None,
            direction: None,
            expected: None,
            aggregate: None,
            tolerance_abs: None,
            tolerance_rel: None,
            threshold: None,
            pattern: None,
            dtype: None,
            columns: None,
            operator: None,
            min_date: None,
            max_date: None,
            when: None,
            then: None,
            allow_additional: None,
            required_order: None,
        };

        let conversion_result = (|| -> Result<Option<String>, String> {
            Ok(match kind {
                QualityRuleKind::NumericRange => {
                    rule.min = migration_number_field(&map, &["min", "min_value", "minValue"])?;
                    rule.max = migration_number_field(&map, &["max", "max_value", "maxValue"])?;
                    (rule.min.is_none() && rule.max.is_none())
                        .then(|| "numeric_range necesita min o max.".to_owned())
                }
                QualityRuleKind::AllowedValues => {
                    rule.values = migration_reference_values(&map, &["values"], 1)?;
                    rule.values
                        .as_ref()
                        .filter(|values| !values.is_empty())
                        .is_none()
                        .then(|| "allowed_values necesita values[].".to_owned())
                }
                QualityRuleKind::Regex => {
                    rule.pattern = migration_string_field(&map, &["pattern"]);
                    rule.pattern
                        .as_ref()
                        .filter(|pattern| !pattern.is_empty())
                        .is_none()
                        .then(|| "regex necesita pattern.".to_owned())
                }
                QualityRuleKind::Dtype => {
                    let source_dtype =
                        migration_string_field(&map, &["dtype", "expected_type", "expectedType"]);
                    rule.dtype = source_dtype
                        .as_deref()
                        .and_then(migration_dtype)
                        .map(str::to_owned);
                    source_dtype
                        .as_ref()
                        .filter(|_| rule.dtype.is_some())
                        .is_none()
                        .then(|| "dtype no contiene un tipo soportado.".to_owned())
                }
                QualityRuleKind::UniqueTogether => {
                    rule.columns = migration_string_array(
                        &map,
                        &["columns", "required_columns", "requiredColumns"],
                    )?;
                    rule.columns
                        .as_ref()
                        .filter(|columns| columns.len() >= 2)
                        .is_none()
                        .then(|| "unique_together necesita al menos dos columnas.".to_owned())
                }
                QualityRuleKind::ColumnCompare => {
                    rule.columns = migration_string_array(&map, &["columns"])?;
                    if rule.columns.is_none() {
                        if let Some(other_column) = migration_string_field(
                            &map,
                            &["other_column", "otherColumn", "right_column", "rightColumn"],
                        ) {
                            rule.columns = Some(vec![rule.column.clone(), other_column]);
                        }
                    }
                    rule.operator = migration_string_field(&map, &["operator", "comparison"])
                        .and_then(|value| migration_quality_comparison(&value));
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.len() != 2)
                    {
                        Some("column_compare necesita exactamente dos columnas.".to_owned())
                    } else if rule.operator.is_none() {
                        Some(
                            "column_compare necesita operator eq, ne, lt, lte, gt o gte."
                                .to_owned(),
                        )
                    } else {
                        None
                    }
                }
                QualityRuleKind::ReferentialIntegrity => {
                    rule.columns =
                        migration_string_array(&map, &["columns", "key_columns", "keyColumns"])?;
                    if rule.columns.is_none() {
                        rule.columns = Some(vec![rule.column.clone()]);
                    }
                    if let Some(columns) = rule.columns.as_ref() {
                        if let Some(first) = columns.first() {
                            rule.column = first.clone();
                        }
                    }
                    let column_count = rule.columns.as_ref().map_or(0, Vec::len);
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        column_count,
                    )?;
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.is_empty())
                    {
                        Some("referential_integrity necesita columns[].".to_owned())
                    } else if rule.reference_values.as_ref().is_none_or(Vec::is_empty) {
                        Some("referential_integrity necesita reference_values[].".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::Monotonic => {
                    let source_direction = migration_string_field(&map, &["direction", "order"]);
                    rule.direction = match source_direction.as_deref() {
                        Some(value) => {
                            Some(migration_quality_monotonic_direction(value).ok_or_else(|| {
                                "monotonic necesita direction increasing o decreasing.".to_owned()
                            })?)
                        }
                        None => Some(QualityMonotonicDirection::Increasing),
                    };
                    None
                }
                QualityRuleKind::AggregateCheck => {
                    rule.expected = migration_number_field(
                        &map,
                        &["expected", "expected_value", "expectedValue"],
                    )?;
                    rule.aggregate = migration_string_field(&map, &["aggregate", "aggregation"])
                        .map(|value| {
                            migration_quality_aggregate(&value).ok_or_else(|| {
                                "aggregate_check necesita aggregate count, sum, min o max."
                                    .to_owned()
                            })
                        })
                        .transpose()?;
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        1,
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    rule.tolerance_rel = migration_number_field(
                        &map,
                        &["tolerance_rel", "toleranceRel", "relative_tolerance"],
                    )?;
                    if rule.expected.is_none()
                        && rule.reference_values.as_ref().is_none_or(Vec::is_empty)
                    {
                        Some("aggregate_check necesita expected o reference_values.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::AggregateReconciliation => {
                    rule.columns = migration_string_array(&map, &["columns", "source_columns"])?;
                    rule.expected = migration_number_field(
                        &map,
                        &["expected", "expected_value", "expectedValue"],
                    )?;
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        1,
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    rule.tolerance_rel = migration_number_field(
                        &map,
                        &["tolerance_rel", "toleranceRel", "relative_tolerance"],
                    )?;
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.len() < 2)
                        && rule.expected.is_none()
                        && rule.reference_values.as_ref().is_none_or(Vec::is_empty)
                    {
                        Some(
                            "aggregate_reconciliation necesita dos columnas o expected/reference_values."
                                .to_owned(),
                        )
                    } else {
                        None
                    }
                }
                QualityRuleKind::DistributionDrift => {
                    rule.baseline =
                        migration_reference_values(&map, &["baseline", "baselineValues"], 1)?;
                    if rule.baseline.as_ref().is_none_or(Vec::is_empty) {
                        rule.baseline = migration_reference_values(
                            &map,
                            &[
                                "reference_values",
                                "referenceValues",
                                "reference",
                                "references",
                            ],
                            1,
                        )?;
                    }
                    rule.threshold = migration_number_field(
                        &map,
                        &["threshold", "drift_threshold", "driftThreshold"],
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    if rule.baseline.as_ref().is_none_or(Vec::is_empty) {
                        Some("distribution_drift necesita baseline[].".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::DateRange => {
                    rule.min_date = migration_string_field(
                        &map,
                        &["min_date", "minDate", "min_value", "minValue", "min"],
                    );
                    rule.max_date = migration_string_field(
                        &map,
                        &["max_date", "maxDate", "max_value", "maxValue", "max"],
                    );
                    let has_invalid_bound = rule
                        .min_date
                        .as_deref()
                        .is_some_and(|value| parse_quality_datetime(value).is_none())
                        || rule
                            .max_date
                            .as_deref()
                            .is_some_and(|value| parse_quality_datetime(value).is_none());
                    if rule.min_date.is_none() && rule.max_date.is_none() {
                        Some("date_range necesita min_value/min o max_value/max.".to_owned())
                    } else if has_invalid_bound {
                        Some("date_range necesita límites de fecha válidos.".to_owned())
                    } else if rule
                        .min_date
                        .as_deref()
                        .zip(rule.max_date.as_deref())
                        .is_some_and(|(min, max)| {
                            parse_quality_datetime(min)
                                .zip(parse_quality_datetime(max))
                                .is_some_and(|(min, max)| min > max)
                        })
                    {
                        Some("El mínimo de date_range no puede superar el máximo.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::Conditional => {
                    rule.when = migration_quality_condition(
                        map.get("when")
                            .or_else(|| map.get("condition"))
                            .or_else(|| map.get("if")),
                    )?;
                    let then = migrate_conditional_then(map.get("then"), &rule.column)?;
                    rule.then = Some(Box::new(then));
                    if rule.when.is_none() {
                        Some("conditional necesita when.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::SchemaContract => {
                    rule.column = QUALITY_DATASET_COLUMN.to_owned();
                    rule.columns = migration_string_array(
                        &map,
                        &["columns", "required_columns", "requiredColumns"],
                    )?;
                    let invalid_columns = rule.columns.as_ref().is_none_or(|columns| {
                        columns.is_empty()
                            || columns.iter().any(|column| column.trim().is_empty())
                            || columns.iter().collect::<HashSet<_>>().len() != columns.len()
                    });
                    if invalid_columns {
                        Some("schema_contract necesita columns[].".to_owned())
                    } else {
                        rule.allow_additional = Some(
                            map.get("allow_additional")
                                .or_else(|| map.get("allowAdditional"))
                                .map(|value| {
                                    value.as_bool().ok_or_else(|| {
                                        "allowAdditional de schema_contract debe ser booleano."
                                            .to_owned()
                                    })
                                })
                                .transpose()?
                                .unwrap_or(true),
                        );
                        rule.required_order =
                            migration_string_array(&map, &["required_order", "requiredOrder"])?;
                        let invalid_order = rule.required_order.as_ref().is_some_and(|order| {
                            order.is_empty()
                                || order.iter().any(|column| column.trim().is_empty())
                                || order.iter().collect::<HashSet<_>>().len() != order.len()
                        });
                        if invalid_order {
                            Some(
                                "requiredOrder de schema_contract no puede estar vacío.".to_owned(),
                            )
                        } else {
                            None
                        }
                    }
                }
                QualityRuleKind::RowCount => {
                    rule.min = migration_number_field(&map, &["min_value", "min"])?;
                    rule.max = migration_number_field(&map, &["max_value", "max"])?;
                    (rule.min.is_none() && rule.max.is_none())
                        .then(|| "row_count necesita min_value/min o max_value/max.".to_owned())
                }
                QualityRuleKind::NotNull | QualityRuleKind::NonEmpty | QualityRuleKind::Unique => {
                    None
                }
            })
        })();
        let conversion_error = match conversion_result {
            Ok(error) => error,
            Err(error) => Some(error),
        };
        let conversion_error = conversion_error.or_else(|| {
            [
                ("toleranceAbs", rule.tolerance_abs),
                ("toleranceRel", rule.tolerance_rel),
                ("threshold", rule.threshold),
            ]
            .into_iter()
            .find_map(|(field, value)| {
                value
                    .filter(|value| *value < 0.0)
                    .map(|_| format!("{field} no puede ser negativo."))
            })
        });
        if let Some(error) = conversion_error {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                error,
            ));
            continue;
        }
        if raw_max_invalid_pct.is_some_and(|value| value != value.clamp(0.0, 100.0)) {
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "warning",
                "La tolerancia porcentual fue ajustada al intervalo 0–100.",
            ));
        }
        converted_rules.push(rule);
    }

    let report = quality_migration_report(
        None,
        total_items,
        converted_rules.len(),
        omitted_rules,
        &warnings,
    );
    Ok(QualityMigrationResult {
        source_format,
        source_version,
        converted_rules,
        warnings,
        omitted_rules,
        report,
    })
}

fn read_quality_rules_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let path = canonicalize_existing_file(path, "el contrato de calidad seleccionado")?;
    if !path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err("El contrato de calidad debe ser JSON.".to_owned());
    }
    let file = File::open(path)
        .map_err(|error| format!("No se pudo abrir el contrato de calidad: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("No se pudo verificar el contrato de calidad: {error}"))?
        .len();
    if size > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(QUALITY_MIGRATION_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer el contrato de calidad: {error}"))?;
    if bytes.len() as u64 > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    Ok(bytes)
}

fn read_quality_rules_json(path: &Path) -> Result<JsonValue, String> {
    let bytes = read_quality_rules_bytes(path)?;
    serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))
}

fn save_quality_rules_atomic(
    document: &QualityRulesDocument,
    destination: &Path,
) -> Result<(), String> {
    validate_quality_rules_document(document)?;
    let destination = canonicalize_write_destination(destination, "el contrato de calidad")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta del contrato de calidad.".to_owned())?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo preparar el contrato de calidad: {error}"))?;
    serde_json::to_writer_pretty(temporary.as_file(), document)
        .map_err(|error| format!("No se pudo escribir el contrato de calidad: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el contrato de calidad: {error}"))?;
    temporary.persist(&destination).map_err(|error| {
        format!(
            "No se pudo publicar el contrato de calidad: {}",
            error.error
        )
    })?;
    Ok(())
}

fn load_quality_migration_file(path: &Path) -> Result<QualityMigrationResult, String> {
    let bytes = read_quality_rules_bytes(path)?;
    let document = serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))?;
    let mut result = migrate_quality_rules_document(document)?;
    result.report.artifact_sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    Ok(result)
}

fn validate_quality_rules_payload(quality_rules: &[QualityRule]) -> Result<(), String> {
    if quality_rules.len() > MAX_QUALITY_RULES {
        return Err(format!(
            "Se admiten como máximo {MAX_QUALITY_RULES} reglas de calidad."
        ));
    }
    let mut text_fields = Vec::new();
    for rule in quality_rules {
        text_fields.push(("column", rule.column.as_str()));
        if let Some(pattern) = rule.pattern.as_deref() {
            text_fields.push(("pattern", pattern));
        }
        if let Some(values) = rule.values.as_deref() {
            for value in values {
                text_fields.push(("value", value.as_str()));
            }
        }
        if let Some(reference_values) = rule.reference_values.as_deref() {
            for value in reference_values {
                text_fields.push(("referenceValue", value.as_str()));
            }
        }
        if let Some(columns) = rule.columns.as_deref() {
            for column in columns {
                text_fields.push(("columns", column.as_str()));
            }
        }
        if let Some(condition) = rule.when.as_ref() {
            text_fields.push(("when.column", condition.column.as_str()));
            if let Some(value) = condition.value.as_deref() {
                text_fields.push(("when.value", value));
            }
        }
        if let Some(then) = rule.then.as_deref() {
            text_fields.push(("then.column", then.column.as_str()));
            if let Some(pattern) = then.pattern.as_deref() {
                text_fields.push(("then.pattern", pattern));
            }
            if let Some(values) = then.values.as_deref() {
                for value in values {
                    text_fields.push(("then.value", value.as_str()));
                }
            }
        }
        if let Some(required_order) = rule.required_order.as_deref() {
            for column in required_order {
                text_fields.push(("requiredOrder", column.as_str()));
            }
        }
    }
    validate_semantic_text_budget(
        "payload de reglas de calidad",
        text_fields,
        MAX_QUALITY_COLUMN_CHARS,
        MAX_QUALITY_TOTAL_TEXT_CHARS,
    )
}

fn validate_quality_rule_definition(frame: &DataFrame, rule: &QualityRule) -> Result<(), String> {
    if rule.kind != QualityRuleKind::ColumnCompare && rule.operator.is_some() {
        return Err(format!(
            "El operator de '{}' solo aplica a column_compare.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::DateRange
        && (rule.min_date.is_some() || rule.max_date.is_some())
    {
        return Err(format!(
            "Los límites de fecha de '{}' solo aplican a date_range.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::Conditional && (rule.when.is_some() || rule.then.is_some()) {
        return Err(format!(
            "when y then solo aplican a la regla conditional de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::SchemaContract
        && (rule.allow_additional.is_some() || rule.required_order.is_some())
    {
        return Err(format!(
            "allowAdditional y requiredOrder solo aplican a schema_contract de '{}'.",
            rule.column
        ));
    }
    let is_aggregate_rule = matches!(
        rule.kind,
        QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation
    );
    let is_distribution_drift = rule.kind == QualityRuleKind::DistributionDrift;
    let is_aggregate_or_drift = is_aggregate_rule || is_distribution_drift;
    if !is_distribution_drift && (rule.baseline.is_some() || rule.threshold.is_some()) {
        return Err(format!(
            "baseline y threshold solo aplican a distribution_drift de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::ReferentialIntegrity
        && !is_aggregate_or_drift
        && rule.reference_values.is_some()
    {
        return Err(format!(
            "referenceValues solo aplica a referential_integrity, reglas agregadas o distribution_drift de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::Monotonic && rule.direction.is_some() {
        return Err(format!(
            "direction solo aplica a monotonic de '{}'.",
            rule.column
        ));
    }
    if !is_aggregate_or_drift
        && (rule.expected.is_some()
            || rule.aggregate.is_some()
            || rule.tolerance_abs.is_some()
            || rule.tolerance_rel.is_some())
    {
        return Err(format!(
            "expected, aggregate y tolerancias numéricas solo aplican a reglas agregadas de '{}'.",
            rule.column
        ));
    }
    if rule
        .reference_values
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La regla de '{}' supera el máximo de {MAX_QUALITY_VALUES} referencias.",
            rule.column
        ));
    }
    if rule
        .baseline
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La línea base de '{}' supera el máximo de {MAX_QUALITY_VALUES} valores.",
            rule.column
        ));
    }
    if rule
        .values
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La regla de '{}' supera el máximo de {MAX_QUALITY_VALUES} valores permitidos.",
            rule.column
        ));
    }
    if rule
        .columns
        .as_ref()
        .is_some_and(|columns| columns.is_empty() || columns.len() > MAX_QUALITY_COLUMNS_PER_RULE)
    {
        return Err(format!(
            "La regla de '{}' debe tener entre 1 y {MAX_QUALITY_COLUMNS_PER_RULE} columnas.",
            rule.column
        ));
    }
    let is_dataset_rule = rule.kind == QualityRuleKind::RowCount;
    let is_schema_rule = rule.kind == QualityRuleKind::SchemaContract;
    let is_dataset_level_rule = is_dataset_rule || is_schema_rule;
    if rule.column.trim().is_empty() && !is_dataset_level_rule {
        return Err("La columna de una regla de calidad no puede estar vacía.".to_owned());
    }
    if is_dataset_level_rule && rule.column != QUALITY_DATASET_COLUMN {
        return Err(format!(
            "La regla {} debe usar la columna lógica '{}'.",
            if is_schema_rule {
                "schema_contract"
            } else {
                "row_count"
            },
            QUALITY_DATASET_COLUMN,
        ));
    }
    let column = (!is_dataset_level_rule)
        .then(|| frame.column(&rule.column))
        .transpose()
        .map_err(|_| {
            format!(
                "La columna '{}' de la regla de calidad no existe.",
                rule.column
            )
        })?;
    if rule.max_invalid.is_none() && rule.max_invalid_pct.is_none() {
        return Err(format!(
            "La regla de '{}' debe indicar maxInvalid, maxInvalidPct o ambos.",
            rule.column
        ));
    }
    if let Some(value) = rule.max_invalid_pct {
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            return Err(format!(
                "maxInvalidPct de '{}' debe estar entre 0 y 100.",
                rule.column
            ));
        }
    }
    for (name, value) in [
        ("min", rule.min),
        ("max", rule.max),
        ("expected", rule.expected),
        ("toleranceAbs", rule.tolerance_abs),
        ("toleranceRel", rule.tolerance_rel),
        ("threshold", rule.threshold),
    ] {
        if value.is_some_and(|number| !number.is_finite()) {
            return Err(format!(
                "{name} de '{}' debe ser un número finito.",
                rule.column
            ));
        }
    }
    if rule.tolerance_abs.is_some_and(|value| value < 0.0)
        || rule.tolerance_rel.is_some_and(|value| value < 0.0)
        || rule.threshold.is_some_and(|value| value < 0.0)
    {
        return Err(format!(
            "Las tolerancias y umbral de '{}' deben ser mayores o iguales que cero.",
            rule.column
        ));
    }
    if rule
        .min
        .zip(rule.max)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(format!(
            "min no puede ser mayor que max en la regla de '{}'.",
            rule.column
        ));
    }
    match rule.kind {
        QualityRuleKind::NotNull | QualityRuleKind::Unique
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some() =>
        {
            Err(format!(
                "La regla '{}' no admite parámetros de otra comprobación.",
                rule.column
            ))
        }
        QualityRuleKind::NonEmpty
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some() =>
        {
            Err(format!(
                "La regla '{}' no admite parámetros de otra comprobación.",
                rule.column
            ))
        }
        QualityRuleKind::NonEmpty
            if column
                .as_ref()
                .is_none_or(|column| column.dtype() != &DataType::String) =>
        {
            Err(format!(
                "La regla non_empty solo admite columnas String; '{}' es {}.",
                rule.column,
                column.map_or_else(|| "dataset".to_owned(), |value| value.dtype().to_string())
            ))
        }
        QualityRuleKind::NumericRange
            if column.as_ref().is_none_or(|column| {
                !matches!(column.dtype(), DataType::Int64 | DataType::Float64)
            }) =>
        {
            Err(format!(
                "La regla numeric_range solo admite columnas Int64 o Float64; '{}' es {}.",
                rule.column,
                column.map_or_else(|| "dataset".to_owned(), |value| value.dtype().to_string())
            ))
        }
        QualityRuleKind::NumericRange if rule.min.is_none() && rule.max.is_none() => Err(format!(
            "La regla numeric_range de '{}' debe indicar min, max o ambos.",
            rule.column
        )),
        QualityRuleKind::NumericRange
            if column
                .as_ref()
                .is_some_and(|column| column.dtype() == &DataType::Int64) =>
        {
            for (name, value) in [("min", rule.min), ("max", rule.max)] {
                if value.is_some_and(|number| {
                    number.fract() != 0.0
                        || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&number)
                }) {
                    return Err(format!(
                        "{name} de '{}' debe ser un entero seguro para una columna Int64.",
                        rule.column
                    ));
                }
            }
            Ok(())
        }
        QualityRuleKind::NumericRange => {
            if rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla numeric_range de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::AllowedValues => {
            let column = column.ok_or_else(|| "allowed_values requiere una columna.".to_owned())?;
            let values = rule
                .values
                .as_ref()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla allowed_values de '{}' debe indicar al menos un valor.",
                        rule.column
                    )
                })?;
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La regla allowed_values solo admite columnas String; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla allowed_values de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            if values
                .iter()
                .any(|value| value.chars().count() > MAX_QUALITY_COLUMN_CHARS)
            {
                return Err(format!(
                    "La regla allowed_values de '{}' contiene un valor demasiado largo.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Regex => {
            let column = column.ok_or_else(|| "regex requiere una columna.".to_owned())?;
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La regla regex solo admite columnas String; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            let pattern = rule
                .pattern
                .as_deref()
                .filter(|pattern| !pattern.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla regex de '{}' debe indicar un patrón.",
                        rule.column
                    )
                })?;
            Regex::new(pattern).map_err(|error| {
                format!(
                    "El patrón regex de '{}' no es válido: {error}.",
                    rule.column
                )
            })?;
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla regex de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Dtype => {
            let column = column.ok_or_else(|| "dtype requiere una columna.".to_owned())?;
            let expected = rule
                .dtype
                .as_deref()
                .filter(|dtype| !dtype.trim().is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla dtype de '{}' debe indicar un tipo esperado.",
                        rule.column
                    )
                })?;
            if !quality_dtype_is_supported(expected) {
                return Err(format!(
                    "El tipo esperado '{}' de '{}' no está soportado.",
                    expected, rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla dtype de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            let _ = column;
            Ok(())
        }
        QualityRuleKind::UniqueTogether => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| columns.len() >= 2)
                .ok_or_else(|| {
                    format!(
                        "La regla unique_together de '{}' debe indicar al menos dos columnas.",
                        rule.column
                    )
                })?;
            for name in columns {
                frame.column(name).map_err(|_| {
                    format!(
                        "La columna '{}' de la regla unique_together no existe.",
                        name
                    )
                })?;
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
            {
                return Err(format!(
                    "La regla unique_together de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::ColumnCompare => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| columns.len() == 2)
                .ok_or_else(|| {
                    format!(
                        "La regla column_compare de '{}' debe indicar exactamente dos columnas.",
                        rule.column
                    )
                })?;
            if columns[0] != rule.column {
                return Err(format!(
                    "La primera columna de column_compare debe coincidir con '{}'.",
                    rule.column
                ));
            }
            if columns[0] == columns[1] {
                return Err(format!(
                    "La regla column_compare de '{}' necesita dos columnas distintas.",
                    rule.column
                ));
            }
            let left = frame.column(&columns[0]).map_err(|_| {
                format!(
                    "La columna '{}' de la regla column_compare no existe.",
                    columns[0]
                )
            })?;
            let right = frame.column(&columns[1]).map_err(|_| {
                format!(
                    "La columna '{}' de la regla column_compare no existe.",
                    columns[1]
                )
            })?;
            if left.dtype() != right.dtype() {
                return Err(format!(
                    "Las columnas de '{}' deben compartir tipo físico; {} y {} no coinciden.",
                    rule.column,
                    left.dtype(),
                    right.dtype()
                ));
            }
            let operator = rule.operator.ok_or_else(|| {
                format!(
                    "La regla column_compare de '{}' debe indicar operator.",
                    rule.column
                )
            })?;
            if matches!(
                operator,
                QualityComparison::Lt
                    | QualityComparison::Lte
                    | QualityComparison::Gt
                    | QualityComparison::Gte
            ) && !matches!(
                left.dtype(),
                DataType::String
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "El operator de orden de '{}' solo admite texto o columnas numéricas.",
                    rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
            {
                return Err(format!(
                    "La regla column_compare de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::ReferentialIntegrity => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| !columns.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla referential_integrity de '{}' debe indicar columns[].",
                        rule.column
                    )
                })?;
            if columns.len() > MAX_QUALITY_COLUMNS_PER_RULE {
                return Err(format!(
                    "La regla referential_integrity de '{}' supera el máximo de {MAX_QUALITY_COLUMNS_PER_RULE} columnas.",
                    rule.column
                ));
            }
            if columns[0] != rule.column {
                return Err(format!(
                    "La primera columna de referential_integrity debe coincidir con '{}'.",
                    rule.column
                ));
            }
            if columns.iter().any(|column| column.trim().is_empty()) {
                return Err("referential_integrity no admite nombres de columna vacíos.".to_owned());
            }
            if columns.iter().collect::<HashSet<_>>().len() != columns.len() {
                return Err(
                    "referential_integrity no admite columnas de clave duplicadas.".to_owned(),
                );
            }
            let key_columns = columns
                .iter()
                .map(|name| {
                    frame.column(name).map_err(|_| {
                        format!(
                            "La columna '{}' de la regla referential_integrity no existe.",
                            name
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if key_columns.iter().any(|column| {
                !matches!(
                    column.dtype(),
                    DataType::String
                        | DataType::Boolean
                        | DataType::Int8
                        | DataType::Int16
                        | DataType::Int32
                        | DataType::Int64
                        | DataType::UInt8
                        | DataType::UInt16
                        | DataType::UInt32
                        | DataType::UInt64
                        | DataType::Float32
                        | DataType::Float64
                )
            }) {
                return Err(
                    "referential_integrity solo admite columnas String, Boolean o numéricas."
                        .to_owned(),
                );
            }
            let references = rule
                .reference_values
                .as_ref()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla referential_integrity de '{}' debe indicar referenceValues[].",
                        rule.column
                    )
                })?;
            if references
                .iter()
                .any(|value| value.chars().count() > MAX_QUALITY_COLUMN_CHARS)
            {
                return Err(format!(
                    "La regla referential_integrity de '{}' contiene una referencia demasiado larga.",
                    rule.column
                ));
            }
            if references.iter().collect::<HashSet<_>>().len() != references.len() {
                return Err("referential_integrity no admite referencias duplicadas.".to_owned());
            }
            if columns.len() == 1 {
                if references.iter().any(|value| value.is_empty()) {
                    return Err(
                        "referential_integrity no admite referencias vacías para una columna."
                            .to_owned(),
                    );
                }
            } else {
                for reference in references {
                    let value = serde_json::from_str::<JsonValue>(reference).map_err(|_| {
                        format!(
                            "Cada referencia compuesta de '{}' debe ser un arreglo JSON.",
                            rule.column
                        )
                    })?;
                    let components = value.as_array().ok_or_else(|| {
                        format!(
                            "Cada referencia compuesta de '{}' debe ser un arreglo JSON.",
                            rule.column
                        )
                    })?;
                    if components.len() != columns.len()
                        || components.iter().any(|component| {
                            matches!(
                                component,
                                JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_)
                            )
                        })
                    {
                        return Err(format!(
                            "Cada referencia compuesta de '{}' debe contener {} escalares no nulos.",
                            rule.column,
                            columns.len()
                        ));
                    }
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla referential_integrity de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Monotonic => {
            let column = column.ok_or_else(|| "monotonic requiere una columna.".to_owned())?;
            if !matches!(
                column.dtype(),
                DataType::String
                    | DataType::Date
                    | DataType::Datetime(_, _)
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "La regla monotonic solo admite texto, fechas o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.reference_values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla monotonic de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::DistributionDrift => {
            let column = column.ok_or_else(|| {
                format!(
                    "La regla distribution_drift de '{}' requiere una columna.",
                    rule.column
                )
            })?;
            if !matches!(
                column.dtype(),
                DataType::String
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "distribution_drift solo admite texto numérico, booleanos o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            let baseline = rule
                .baseline
                .as_ref()
                .filter(|values| !values.is_empty())
                .or(rule
                    .reference_values
                    .as_ref()
                    .filter(|values| !values.is_empty()))
                .ok_or_else(|| {
                    format!(
                        "La regla distribution_drift de '{}' debe indicar baseline[].",
                        rule.column
                    )
                })?;
            if baseline
                .iter()
                .any(|value| quality_aggregate_text_value(value).is_none())
            {
                return Err(format!(
                    "La línea base de distribution_drift en '{}' debe contener números finitos.",
                    rule.column
                ));
            }
            if rule.expected.is_some()
                || rule.aggregate.is_some()
                || rule.tolerance_rel.is_some()
                || rule.values.is_some()
                || rule.min.is_some()
                || rule.max.is_some()
                || rule.direction.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla distribution_drift de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation => {
            let column = column.ok_or_else(|| {
                format!(
                    "La regla agregada de '{}' requiere una columna.",
                    rule.column
                )
            })?;
            let supports_aggregate = |value: &Column| {
                matches!(
                    value.dtype(),
                    DataType::String
                        | DataType::Boolean
                        | DataType::Int8
                        | DataType::Int16
                        | DataType::Int32
                        | DataType::Int64
                        | DataType::UInt8
                        | DataType::UInt16
                        | DataType::UInt32
                        | DataType::UInt64
                        | DataType::Float32
                        | DataType::Float64
                )
            };
            if !supports_aggregate(column) {
                return Err(format!(
                    "Las reglas agregadas solo admiten texto numérico, booleanos o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule
                .reference_values
                .as_ref()
                .is_some_and(|values| values.is_empty())
            {
                return Err(format!(
                    "La regla agregada de '{}' no admite referenceValues vacío.",
                    rule.column
                ));
            }
            if rule.reference_values.as_ref().is_some_and(|values| {
                values
                    .iter()
                    .any(|value| quality_aggregate_text_value(value).is_none())
            }) {
                return Err(format!(
                    "Las referencias agregadas de '{}' deben ser números finitos.",
                    rule.column
                ));
            }
            let has_expected = rule.expected.is_some();
            let has_references = rule
                .reference_values
                .as_ref()
                .is_some_and(|values| !values.is_empty());
            let has_column_pair = rule.kind == QualityRuleKind::AggregateReconciliation
                && rule
                    .columns
                    .as_ref()
                    .is_some_and(|columns| columns.len() >= 2);
            if !has_column_pair && !has_expected && !has_references {
                return Err(format!(
                    "La regla {} de '{}' necesita expected o referenceValues.",
                    if rule.kind == QualityRuleKind::AggregateCheck {
                        "aggregate_check"
                    } else {
                        "aggregate_reconciliation"
                    },
                    rule.column
                ));
            }
            if let Some(aggregate) = rule.aggregate {
                if rule.kind == QualityRuleKind::AggregateReconciliation
                    && rule
                        .columns
                        .as_ref()
                        .is_some_and(|columns| columns.len() >= 2)
                {
                    return Err(
                        "aggregate_reconciliation por columnas siempre compara sumas y no admite aggregate."
                            .to_owned(),
                    );
                }
                let _ = aggregate;
            }
            if rule.kind == QualityRuleKind::AggregateCheck && rule.columns.is_some() {
                return Err(
                    "aggregate_check no admite columns; selecciona una sola columna.".to_owned(),
                );
            }
            if rule.kind == QualityRuleKind::AggregateReconciliation {
                if let Some(columns) = rule.columns.as_ref() {
                    if columns.len() != 2 {
                        return Err(
                            "aggregate_reconciliation necesita exactamente dos columnas."
                                .to_owned(),
                        );
                    }
                    if columns[0] != rule.column {
                        return Err(
                            "La primera columna de aggregate_reconciliation debe coincidir con la columna principal."
                                .to_owned(),
                        );
                    }
                    if columns[0] == columns[1] {
                        return Err(
                            "aggregate_reconciliation necesita dos columnas distintas.".to_owned()
                        );
                    }
                    let right = frame.column(&columns[1]).map_err(|_| {
                        format!(
                            "La columna '{}' de aggregate_reconciliation no existe.",
                            columns[1]
                        )
                    })?;
                    if !supports_aggregate(right) {
                        return Err(format!(
                            "La columna '{}' de aggregate_reconciliation no es agregable.",
                            columns[1]
                        ));
                    }
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.direction.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla agregada de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::DateRange => {
            let column = column.ok_or_else(|| "date_range requiere una columna.".to_owned())?;
            if !matches!(
                column.dtype(),
                DataType::String | DataType::Date | DataType::Datetime(_, _)
            ) {
                return Err(format!(
                    "La regla date_range de '{}' solo admite texto, date o datetime.",
                    rule.column
                ));
            }
            let parse_bound = |value: Option<&str>, label: &str| {
                value
                    .map(|value| {
                        parse_quality_datetime(value).ok_or_else(|| {
                            format!(
                                "El límite {label} de date_range en '{}' no es una fecha válida.",
                                rule.column
                            )
                        })
                    })
                    .transpose()
            };
            let minimum = parse_bound(rule.min_date.as_deref(), "mínimo")?;
            let maximum = parse_bound(rule.max_date.as_deref(), "máximo")?;
            if minimum.is_none() && maximum.is_none() {
                return Err(format!(
                    "La regla date_range de '{}' debe indicar minDate, maxDate o ambos.",
                    rule.column
                ));
            }
            if minimum
                .zip(maximum)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            {
                return Err(format!(
                    "El mínimo de date_range en '{}' no puede superar el máximo.",
                    rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla date_range de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Conditional => {
            let condition = rule.when.as_ref().ok_or_else(|| {
                format!(
                    "La regla conditional de '{}' debe indicar when.",
                    rule.column
                )
            })?;
            let condition_column = frame.column(&condition.column).map_err(|_| {
                format!(
                    "La columna '{}' de when no existe en conditional.",
                    condition.column
                )
            })?;
            if !matches!(
                condition_column.dtype(),
                DataType::String
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "La condición de '{}' solo admite columnas String, numéricas o Boolean.",
                    rule.column
                ));
            }
            if condition.operator.is_none() {
                return Err(format!(
                    "La condición de '{}' debe indicar operator.",
                    rule.column
                ));
            }
            if matches!(
                condition.operator,
                Some(
                    QualityComparison::Lt
                        | QualityComparison::Lte
                        | QualityComparison::Gt
                        | QualityComparison::Gte
                )
            ) && !matches!(
                condition_column.dtype(),
                DataType::String
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "El operator de orden de la condición de '{}' solo admite texto o columnas numéricas.",
                    rule.column
                ));
            }
            if condition.value.is_none() {
                return Err(format!(
                    "La condición de '{}' debe indicar value.",
                    rule.column
                ));
            }
            let then = rule.then.as_deref().ok_or_else(|| {
                format!(
                    "La regla conditional de '{}' debe indicar then.",
                    rule.column
                )
            })?;
            if !matches!(
                then.kind,
                QualityRuleKind::NotNull
                    | QualityRuleKind::NonEmpty
                    | QualityRuleKind::NumericRange
                    | QualityRuleKind::AllowedValues
                    | QualityRuleKind::Regex
                    | QualityRuleKind::Dtype
            ) {
                return Err(
                    "conditional solo admite subreglas then fila-a-fila: not_null, non_empty, numeric_range, allowed_values, regex o dtype.".to_owned(),
                );
            }
            if then.max_invalid != Some(0) || then.max_invalid_pct.is_some() {
                return Err(
                    "La tolerancia de la subregla then debe ser exactamente 0 inválidos."
                        .to_owned(),
                );
            }
            validate_quality_rule_definition(frame, then)?;
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
            {
                return Err(format!(
                    "La regla conditional de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::SchemaContract => {
            let required = rule
                .columns
                .as_ref()
                .filter(|columns| !columns.is_empty())
                .ok_or_else(|| "La regla schema_contract necesita columns[].".to_owned())?;
            if required.iter().any(|column| column.trim().is_empty()) {
                return Err("schema_contract no admite nombres de columna vacíos.".to_owned());
            }
            let unique_required = required.iter().collect::<HashSet<_>>();
            if unique_required.len() != required.len() {
                return Err("schema_contract no admite columnas requeridas duplicadas.".to_owned());
            }
            if let Some(order) = rule.required_order.as_ref() {
                if order.len() > MAX_QUALITY_COLUMNS_PER_RULE {
                    return Err(format!(
                        "requiredOrder de schema_contract supera el máximo de {MAX_QUALITY_COLUMNS_PER_RULE} columnas."
                    ));
                }
                if order.is_empty() || order.iter().any(|column| column.trim().is_empty()) {
                    return Err(
                        "requiredOrder de schema_contract debe contener nombres no vacíos."
                            .to_owned(),
                    );
                }
                let unique_order = order.iter().collect::<HashSet<_>>();
                if unique_order.len() != order.len() {
                    return Err(
                        "requiredOrder de schema_contract no admite columnas duplicadas."
                            .to_owned(),
                    );
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
            {
                return Err(
                    "La regla schema_contract no admite parámetros de otra comprobación."
                        .to_owned(),
                );
            }
            Ok(())
        }
        QualityRuleKind::RowCount => {
            if rule.min.is_none() && rule.max.is_none() {
                return Err("La regla row_count debe indicar min, max o ambos.".to_owned());
            }
            if rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err("La regla row_count no admite parámetros adicionales.".to_owned());
            }
            Ok(())
        }
        QualityRuleKind::NotNull | QualityRuleKind::NonEmpty | QualityRuleKind::Unique => Ok(()),
    }
}

fn quality_dtype_is_supported(expected: &str) -> bool {
    matches!(
        expected.trim().to_ascii_lowercase().as_str(),
        "string"
            | "text"
            | "integer"
            | "int"
            | "int64"
            | "float"
            | "decimal"
            | "float64"
            | "boolean"
            | "bool"
            | "date"
            | "datetime"
            | "timestamp"
    )
}

fn quality_dtype_matches(actual: &DataType, expected: &str) -> bool {
    match expected.trim().to_ascii_lowercase().as_str() {
        "string" | "text" => actual == &DataType::String,
        "integer" | "int" | "int64" => matches!(
            actual,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        ),
        "float" | "decimal" | "float64" => matches!(actual, DataType::Float32 | DataType::Float64),
        "boolean" | "bool" => actual == &DataType::Boolean,
        "date" => actual == &DataType::Date,
        "datetime" | "timestamp" => matches!(actual, DataType::Datetime(_, _)),
        _ => false,
    }
}

fn duplicate_combination_count(frame: &DataFrame, columns: &[String]) -> Result<usize, String> {
    let mut seen = HashSet::new();
    let mut duplicate_count = 0;
    for row_index in 0..frame.height() {
        let key = columns
            .iter()
            .map(|name| {
                frame
                    .column(name)
                    .and_then(|column| column.get(row_index))
                    .map(|value| format!("{value:?}"))
                    .map_err(|error| format!("No se pudo evaluar unique_together: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !seen.insert(key) {
            duplicate_count += 1;
        }
    }
    Ok(duplicate_count)
}

fn quality_value_ordering(left: AnyValue<'_>, right: AnyValue<'_>) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (AnyValue::Int8(left), AnyValue::Int8(right)) => Some(left.cmp(&right)),
        (AnyValue::Int16(left), AnyValue::Int16(right)) => Some(left.cmp(&right)),
        (AnyValue::Int32(left), AnyValue::Int32(right)) => Some(left.cmp(&right)),
        (AnyValue::Int64(left), AnyValue::Int64(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt8(left), AnyValue::UInt8(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt16(left), AnyValue::UInt16(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt32(left), AnyValue::UInt32(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt64(left), AnyValue::UInt64(right)) => Some(left.cmp(&right)),
        (AnyValue::Float32(left), AnyValue::Float32(right)) => left.partial_cmp(&right),
        (AnyValue::Float64(left), AnyValue::Float64(right)) => left.partial_cmp(&right),
        (AnyValue::Boolean(left), AnyValue::Boolean(right)) => Some(left.cmp(&right)),
        (AnyValue::String(left), AnyValue::String(right)) => Some(left.cmp(right)),
        (AnyValue::String(left), AnyValue::StringOwned(right)) => Some(left.cmp(right.as_str())),
        (AnyValue::StringOwned(left), AnyValue::String(right)) => Some(left.as_str().cmp(right)),
        (AnyValue::StringOwned(left), AnyValue::StringOwned(right)) => {
            Some(left.as_str().cmp(right.as_str()))
        }
        _ => None,
    }
}

fn quality_datetime_value(value: AnyValue<'_>) -> Option<NaiveDateTime> {
    match value {
        AnyValue::String(value) => parse_quality_datetime(value),
        AnyValue::StringOwned(value) => parse_quality_datetime(value.as_str()),
        AnyValue::Date(days) => NaiveDate::from_ymd_opt(1970, 1, 1)
            .and_then(|epoch| epoch.checked_add_signed(chrono::Duration::days(days.into())))
            .and_then(|date| date.and_hms_opt(0, 0, 0)),
        AnyValue::Datetime(raw, unit, _) => {
            let divisor = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            DateTime::from_timestamp(
                raw.div_euclid(divisor),
                (raw.rem_euclid(divisor) as u64 * (1_000_000_000 / divisor as u64)) as u32,
            )
            .map(|date| date.naive_utc())
        }
        _ => None,
    }
}

fn quality_comparison_matches(
    left: AnyValue<'_>,
    right: AnyValue<'_>,
    operator: QualityComparison,
) -> bool {
    match operator {
        QualityComparison::Eq => format!("{left:?}") == format!("{right:?}"),
        QualityComparison::Ne => format!("{left:?}") != format!("{right:?}"),
        QualityComparison::Lt => quality_value_ordering(left, right)
            .is_some_and(|ordering| ordering == std::cmp::Ordering::Less),
        QualityComparison::Lte => quality_value_ordering(left, right).is_some_and(|ordering| {
            matches!(
                ordering,
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal
            )
        }),
        QualityComparison::Gt => quality_value_ordering(left, right)
            .is_some_and(|ordering| ordering == std::cmp::Ordering::Greater),
        QualityComparison::Gte => quality_value_ordering(left, right).is_some_and(|ordering| {
            matches!(
                ordering,
                std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
            )
        }),
    }
}

fn quality_comparison_ordering_matches(
    ordering: std::cmp::Ordering,
    operator: QualityComparison,
) -> bool {
    match operator {
        QualityComparison::Eq => ordering == std::cmp::Ordering::Equal,
        QualityComparison::Ne => ordering != std::cmp::Ordering::Equal,
        QualityComparison::Lt => ordering == std::cmp::Ordering::Less,
        QualityComparison::Lte => {
            matches!(
                ordering,
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal
            )
        }
        QualityComparison::Gt => ordering == std::cmp::Ordering::Greater,
        QualityComparison::Gte => matches!(
            ordering,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
        ),
    }
}

fn quality_condition_matches(value: AnyValue<'_>, condition: &QualityCondition) -> bool {
    let Some(operator) = condition.operator else {
        return false;
    };
    let Some(expected) = condition.value.as_deref() else {
        return false;
    };
    match value {
        AnyValue::String(actual) => {
            quality_comparison_ordering_matches(actual.cmp(expected), operator)
        }
        AnyValue::StringOwned(actual) => {
            quality_comparison_ordering_matches(actual.as_str().cmp(expected), operator)
        }
        AnyValue::Boolean(actual) => expected.parse::<bool>().is_ok_and(|expected| {
            quality_comparison_ordering_matches(actual.cmp(&expected), operator)
        }),
        AnyValue::Null => false,
        value if quality_numeric_value(value.clone()).is_some() => {
            let actual = quality_numeric_value(value).expect("se verificó el tipo numérico");
            expected.parse::<f64>().ok().is_some_and(|expected| {
                actual
                    .partial_cmp(&expected)
                    .is_some_and(|ordering| quality_comparison_ordering_matches(ordering, operator))
            })
        }
        _ => false,
    }
}

fn quality_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    match value {
        AnyValue::Int8(value) => Some(value as f64),
        AnyValue::Int16(value) => Some(value as f64),
        AnyValue::Int32(value) => Some(value as f64),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::UInt8(value) => Some(value as f64),
        AnyValue::UInt16(value) => Some(value as f64),
        AnyValue::UInt32(value) => Some(value as f64),
        AnyValue::UInt64(value) => Some(value as f64),
        AnyValue::Float32(value) => Some(value as f64),
        AnyValue::Float64(value) => Some(value),
        _ => None,
    }
}

fn quality_reference_scalar_matches(value: AnyValue<'_>, expected: &str) -> bool {
    match value {
        AnyValue::String(actual) => actual == expected,
        AnyValue::StringOwned(actual) => actual.as_str() == expected,
        AnyValue::Boolean(actual) => expected
            .parse::<bool>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int8(actual) => expected
            .parse::<i8>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int16(actual) => expected
            .parse::<i16>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int32(actual) => expected
            .parse::<i32>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int64(actual) => expected
            .parse::<i64>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt8(actual) => expected
            .parse::<u8>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt16(actual) => expected
            .parse::<u16>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt32(actual) => expected
            .parse::<u32>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt64(actual) => expected
            .parse::<u64>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Float32(actual) => expected
            .parse::<f32>()
            .is_ok_and(|expected| expected.is_finite() && actual.is_finite() && actual == expected),
        AnyValue::Float64(actual) => expected
            .parse::<f64>()
            .is_ok_and(|expected| expected.is_finite() && actual.is_finite() && actual == expected),
        AnyValue::Null => false,
        _ => false,
    }
}

fn quality_reference_json_component_matches(value: AnyValue<'_>, expected: &JsonValue) -> bool {
    match expected {
        JsonValue::String(expected) => quality_reference_scalar_matches(value, expected),
        JsonValue::Bool(expected) => quality_reference_scalar_matches(value, &expected.to_string()),
        JsonValue::Number(expected) => {
            quality_reference_scalar_matches(value, &expected.to_string())
        }
        JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => false,
    }
}

fn quality_monotonic_ordering(
    left: AnyValue<'_>,
    right: AnyValue<'_>,
) -> Option<std::cmp::Ordering> {
    if matches!(&left, AnyValue::Date(_) | AnyValue::Datetime(_, _, _))
        || matches!(&right, AnyValue::Date(_) | AnyValue::Datetime(_, _, _))
    {
        return quality_datetime_value(left)
            .zip(quality_datetime_value(right))
            .map(|(left, right)| left.cmp(&right));
    }
    quality_value_ordering(left, right)
}

fn quality_aggregate_text_value(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn quality_aggregate_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    match value {
        AnyValue::Int8(value) => Some(value as f64),
        AnyValue::Int16(value) => Some(value as f64),
        AnyValue::Int32(value) => Some(value as f64),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::UInt8(value) => Some(value as f64),
        AnyValue::UInt16(value) => Some(value as f64),
        AnyValue::UInt32(value) => Some(value as f64),
        AnyValue::UInt64(value) => (value as f64).is_finite().then_some(value as f64),
        AnyValue::Float32(value) => value.is_finite().then_some(value as f64),
        AnyValue::Float64(value) => value.is_finite().then_some(value),
        AnyValue::Boolean(value) => Some(if value { 1.0 } else { 0.0 }),
        AnyValue::String(value) => quality_aggregate_text_value(value),
        AnyValue::StringOwned(value) => quality_aggregate_text_value(value.as_str()),
        _ => None,
    }
}

fn quality_aggregate_observation<C>(
    column: &Column,
    row_count: usize,
    is_cancelled: &C,
) -> Result<(usize, f64, Option<f64>, Option<f64>), String>
where
    C: Fn() -> bool,
{
    let mut count = 0;
    let mut sum = 0.0;
    let mut minimum = None;
    let mut maximum = None;
    for row_index in 0..row_count {
        if row_index % 1024 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column.get(row_index).map_err(|error| error.to_string())?;
        let Some(number) = quality_aggregate_numeric_value(value) else {
            continue;
        };
        count += 1;
        sum += number;
        minimum = Some(minimum.map_or(number, |current: f64| current.min(number)));
        maximum = Some(maximum.map_or(number, |current: f64| current.max(number)));
    }
    Ok((count, sum, minimum, maximum))
}

fn quality_aggregate_expected(rule: &QualityRule, aggregate: QualityAggregate) -> Option<f64> {
    if let Some(expected) = rule.expected {
        return Some(expected);
    }
    let references = rule.reference_values.as_deref()?;
    if aggregate == QualityAggregate::Sum {
        let mut total = 0.0;
        for reference in references {
            total += quality_aggregate_text_value(reference)?;
        }
        Some(total)
    } else {
        references
            .first()
            .and_then(|reference| quality_aggregate_text_value(reference))
    }
}

fn quality_aggregate_tolerance(rule: &QualityRule, reference: f64) -> f64 {
    let relative = rule
        .tolerance_rel
        .map_or(0.0, |value| reference.abs() * value);
    rule.tolerance_abs.unwrap_or(0.0).max(relative)
}

fn quality_distribution_baseline_mean(rule: &QualityRule) -> Option<f64> {
    let values = rule
        .baseline
        .as_deref()
        .filter(|values| !values.is_empty())
        .or(rule
            .reference_values
            .as_deref()
            .filter(|values| !values.is_empty()))?;
    let mut count = 0usize;
    let mut sum = 0.0;
    for value in values {
        let number = quality_aggregate_text_value(value)?;
        count += 1;
        sum += number;
    }
    (count > 0).then_some(sum / count as f64)
}

fn quality_conditional_row_invalid(
    frame: &DataFrame,
    row_index: usize,
    rule: &QualityRule,
) -> Result<bool, String> {
    let column = frame
        .column(&rule.column)
        .map_err(|error| format!("No se pudo evaluar conditional.then: {error}"))?;
    let value = column
        .get(row_index)
        .map_err(|error| format!("No se pudo leer conditional.then: {error}"))?;
    match rule.kind {
        QualityRuleKind::NotNull => Ok(matches!(value, AnyValue::Null)),
        QualityRuleKind::NonEmpty => Ok(match value {
            AnyValue::String(text) => text.trim().is_empty(),
            AnyValue::StringOwned(text) => text.trim().is_empty(),
            AnyValue::Null => true,
            _ => true,
        }),
        QualityRuleKind::NumericRange => {
            let number = quality_numeric_value(value);
            Ok(number.is_none_or(|number| {
                !number.is_finite()
                    || rule.min.is_some_and(|minimum| number < minimum)
                    || rule.max.is_some_and(|maximum| number > maximum)
            }))
        }
        QualityRuleKind::AllowedValues => {
            let allowed = rule.values.as_ref().expect("values validados");
            Ok(match value {
                AnyValue::String(text) => !allowed.iter().any(|item| item == text),
                AnyValue::StringOwned(text) => !allowed.iter().any(|item| item == text.as_str()),
                AnyValue::Null => true,
                _ => true,
            })
        }
        QualityRuleKind::Regex => {
            let regex = Regex::new(rule.pattern.as_deref().expect("pattern validado"))
                .expect("pattern validado");
            Ok(match value {
                AnyValue::String(text) => !regex.is_match(text),
                AnyValue::StringOwned(text) => !regex.is_match(text.as_str()),
                AnyValue::Null => true,
                _ => true,
            })
        }
        QualityRuleKind::Dtype => Ok(!quality_dtype_matches(
            column.dtype(),
            rule.dtype.as_deref().expect("dtype validado"),
        )),
        _ => Err(
            "conditional contiene una subregla then que no es fila-a-fila o no fue validada."
                .to_owned(),
        ),
    }
}

fn evaluate_quality_rules(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
) -> Result<QualityValidationResult, String> {
    evaluate_quality_rules_with_cancel(frame, quality_rules, || false)
}

fn evaluate_quality_rules_with_cancel<C>(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    is_cancelled: C,
) -> Result<QualityValidationResult, String>
where
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    for rule in quality_rules {
        ensure_not_cancelled(is_cancelled())?;
        validate_quality_rule_definition(frame, rule)?;
    }

    let row_count = frame.height();
    let mut results = Vec::with_capacity(quality_rules.len());
    for rule in quality_rules {
        ensure_not_cancelled(is_cancelled())?;
        let (checked_count, invalid_count) = match rule.kind {
            QualityRuleKind::Conditional => {
                let condition = rule.when.as_ref().expect("when validado");
                let then = rule.then.as_deref().expect("then validado");
                let condition_column = frame
                    .column(&condition.column)
                    .map_err(|error| error.to_string())?;
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    let condition_value = condition_column
                        .get(row_index)
                        .map_err(|error| error.to_string())?;
                    if !matches!(condition_value, AnyValue::Null)
                        && quality_condition_matches(condition_value, condition)
                        && quality_conditional_row_invalid(frame, row_index, then)?
                    {
                        invalid_count += 1;
                    }
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::DateRange => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let minimum = rule.min_date.as_deref().and_then(parse_quality_datetime);
                let maximum = rule.max_date.as_deref().and_then(parse_quality_datetime);
                let invalid_count = (0..row_count)
                    .filter(|row_index| {
                        let value = column.get(*row_index).ok().and_then(quality_datetime_value);
                        value.is_none_or(|value| {
                            minimum.is_some_and(|minimum| value < minimum)
                                || maximum.is_some_and(|maximum| value > maximum)
                        })
                    })
                    .count();
                (row_count, invalid_count)
            }
            QualityRuleKind::SchemaContract => {
                let required = rule.columns.as_deref().expect("columns validadas");
                let actual = frame
                    .get_column_names()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let missing_count = required
                    .iter()
                    .filter(|required_name| !actual.iter().any(|name| name == *required_name))
                    .count();
                let additional_count = if rule.allow_additional.unwrap_or(true) {
                    0
                } else {
                    actual
                        .iter()
                        .filter(|name| {
                            !required
                                .iter()
                                .any(|required_name| *required_name == **name)
                        })
                        .count()
                };
                let order_count = rule.required_order.as_ref().map_or(0, |required_order| {
                    usize::from(actual.as_slice() != required_order.as_slice())
                });
                (1, missing_count + additional_count + order_count)
            }
            QualityRuleKind::RowCount => {
                let minimum_ok = rule.min.is_none_or(|minimum| row_count as f64 >= minimum);
                let maximum_ok = rule.max.is_none_or(|maximum| row_count as f64 <= maximum);
                (1, usize::from(!(minimum_ok && maximum_ok)))
            }
            QualityRuleKind::Dtype => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let expected = rule.dtype.as_deref().expect("dtype validado");
                (
                    1,
                    usize::from(!quality_dtype_matches(column.dtype(), expected)),
                )
            }
            QualityRuleKind::UniqueTogether => (
                row_count,
                duplicate_combination_count(
                    frame,
                    rule.columns.as_deref().expect("columns validadas"),
                )?,
            ),
            QualityRuleKind::ColumnCompare => {
                let columns = rule.columns.as_deref().expect("columns validadas");
                let left = frame
                    .column(&columns[0])
                    .map_err(|error| error.to_string())?;
                let right = frame
                    .column(&columns[1])
                    .map_err(|error| error.to_string())?;
                let operator = rule.operator.expect("operator validado");
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    let left_value = left.get(row_index).map_err(|error| error.to_string())?;
                    let right_value = right.get(row_index).map_err(|error| error.to_string())?;
                    if matches!(left_value, AnyValue::Null)
                        || matches!(right_value, AnyValue::Null)
                        || !quality_comparison_matches(left_value, right_value, operator)
                    {
                        invalid_count += 1;
                    }
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::ReferentialIntegrity => {
                let columns = rule.columns.as_deref().expect("columns validadas");
                let references = rule
                    .reference_values
                    .as_deref()
                    .expect("referenceValues validados");
                let key_columns = columns
                    .iter()
                    .map(|name| frame.column(name).map_err(|error| error.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let invalid_count = if key_columns.len() == 1 {
                    let column = key_columns[0];
                    (0..row_count)
                        .filter(|row_index| {
                            column.get(*row_index).ok().is_none_or(|value| {
                                !references.iter().any(|reference| {
                                    quality_reference_scalar_matches(value.clone(), reference)
                                })
                            })
                        })
                        .count()
                } else {
                    let parsed_references = references
                        .iter()
                        .map(|reference| {
                            serde_json::from_str::<JsonValue>(reference)
                                .expect("referencias compuestas validadas")
                        })
                        .collect::<Vec<_>>();
                    (0..row_count)
                        .filter(|row_index| {
                            let values = key_columns
                                .iter()
                                .map(|column| column.get(*row_index))
                                .collect::<Result<Vec<_>, _>>();
                            let Ok(values) = values else {
                                return true;
                            };
                            !parsed_references.iter().any(|reference| {
                                let Some(components) = reference.as_array() else {
                                    return false;
                                };
                                components.len() == values.len()
                                    && values.iter().zip(components).all(|(value, expected)| {
                                        quality_reference_json_component_matches(
                                            value.clone(),
                                            expected,
                                        )
                                    })
                            })
                        })
                        .count()
                };
                (row_count, invalid_count)
            }
            QualityRuleKind::Monotonic => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let direction = rule
                    .direction
                    .unwrap_or(QualityMonotonicDirection::Increasing);
                let mut previous: Option<AnyValue<'_>> = None;
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    let value = column.get(row_index).map_err(|error| error.to_string())?;
                    if matches!(&value, AnyValue::Null) {
                        previous = None;
                        continue;
                    }
                    if let Some(previous_value) = previous.as_ref() {
                        let invalid =
                            match quality_monotonic_ordering(previous_value.clone(), value.clone())
                            {
                                Some(ordering) => match direction {
                                    QualityMonotonicDirection::Increasing => {
                                        ordering == std::cmp::Ordering::Greater
                                    }
                                    QualityMonotonicDirection::Decreasing => {
                                        ordering == std::cmp::Ordering::Less
                                    }
                                },
                                None => true,
                            };
                        invalid_count += usize::from(invalid);
                    }
                    previous = Some(value);
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::DistributionDrift => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let (count, sum, _, _) =
                    quality_aggregate_observation(column, row_count, &is_cancelled)?;
                let observed_mean = if count == 0 { 0.0 } else { sum / count as f64 };
                let baseline_mean = quality_distribution_baseline_mean(rule)
                    .expect("baseline validado para distribution_drift");
                let threshold = rule.tolerance_abs.or(rule.threshold).unwrap_or(0.0);
                (
                    row_count,
                    usize::from((observed_mean - baseline_mean).abs() > threshold),
                )
            }
            QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation => {
                let is_reconciliation = rule.kind == QualityRuleKind::AggregateReconciliation;
                let has_column_pair = is_reconciliation
                    && rule
                        .columns
                        .as_ref()
                        .is_some_and(|columns| columns.len() >= 2);
                if has_column_pair {
                    let columns = rule.columns.as_deref().expect("columns validadas");
                    let left = frame
                        .column(&columns[0])
                        .map_err(|error| error.to_string())?;
                    let right = frame
                        .column(&columns[1])
                        .map_err(|error| error.to_string())?;
                    let (_, left_sum, _, _) =
                        quality_aggregate_observation(left, row_count, &is_cancelled)?;
                    let (_, right_sum, _, _) =
                        quality_aggregate_observation(right, row_count, &is_cancelled)?;
                    let reference = left_sum.abs().max(right_sum.abs());
                    let tolerance = quality_aggregate_tolerance(rule, reference);
                    (
                        row_count,
                        usize::from((left_sum - right_sum).abs() > tolerance),
                    )
                } else {
                    let column = frame
                        .column(&rule.column)
                        .map_err(|error| error.to_string())?;
                    let aggregate = rule.aggregate.unwrap_or(QualityAggregate::Sum);
                    let (count, sum, minimum, maximum) =
                        quality_aggregate_observation(column, row_count, &is_cancelled)?;
                    let observed = match aggregate {
                        QualityAggregate::Count => Some(count as f64),
                        QualityAggregate::Sum => Some(sum),
                        QualityAggregate::Min => minimum,
                        QualityAggregate::Max => maximum,
                    };
                    let expected = quality_aggregate_expected(rule, aggregate)
                        .expect("expected o referenceValues validados");
                    let tolerance = quality_aggregate_tolerance(rule, expected);
                    let invalid = usize::from(
                        observed.is_none_or(|value| (value - expected).abs() > tolerance),
                    );
                    (row_count, invalid)
                }
            }
            _ => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let invalid_count = match rule.kind {
                    QualityRuleKind::NotNull => column.null_count(),
                    QualityRuleKind::NonEmpty => column
                        .str()
                        .map_err(|error| error.to_string())?
                        .iter()
                        .filter(|value| value.is_none_or(|text| text.trim().is_empty()))
                        .count(),
                    QualityRuleKind::Unique => {
                        let distinct_including_null = column
                            .n_unique()
                            .map_err(|error| format!("No se pudo evaluar unique: {error}"))?;
                        let distinct_non_null = distinct_including_null
                            .saturating_sub(usize::from(column.null_count() > 0));
                        row_count.saturating_sub(distinct_non_null)
                    }
                    QualityRuleKind::NumericRange => match column.dtype() {
                        DataType::Int64 => column
                            .i64()
                            .map_err(|error| error.to_string())?
                            .iter()
                            .filter(|value| {
                                value.is_none_or(|number| {
                                    rule.min.is_some_and(|minimum| number < minimum as i64)
                                        || rule.max.is_some_and(|maximum| number > maximum as i64)
                                })
                            })
                            .count(),
                        DataType::Float64 => column
                            .f64()
                            .map_err(|error| error.to_string())?
                            .iter()
                            .filter(|value| {
                                value.is_none_or(|number| {
                                    !number.is_finite()
                                        || rule.min.is_some_and(|minimum| number < minimum)
                                        || rule.max.is_some_and(|maximum| number > maximum)
                                })
                            })
                            .count(),
                        _ => unreachable!("el tipo numérico ya fue validado"),
                    },
                    QualityRuleKind::AllowedValues => {
                        let allowed = rule.values.as_ref().expect("values validados");
                        column
                            .str()
                            .map_err(|error| error.to_string())?
                            .iter()
                            .filter(|value| {
                                value.is_none_or(|text| !allowed.iter().any(|item| item == text))
                            })
                            .count()
                    }
                    QualityRuleKind::Regex => {
                        let pattern = rule.pattern.as_deref().expect("pattern validado");
                        let regex = Regex::new(pattern).expect("pattern validado");
                        column
                            .str()
                            .map_err(|error| error.to_string())?
                            .iter()
                            .filter(|value| value.is_none_or(|text| !regex.is_match(text)))
                            .count()
                    }
                    QualityRuleKind::Dtype
                    | QualityRuleKind::UniqueTogether
                    | QualityRuleKind::ColumnCompare
                    | QualityRuleKind::ReferentialIntegrity
                    | QualityRuleKind::Monotonic
                    | QualityRuleKind::AggregateCheck
                    | QualityRuleKind::AggregateReconciliation
                    | QualityRuleKind::DistributionDrift
                    | QualityRuleKind::DateRange
                    | QualityRuleKind::Conditional
                    | QualityRuleKind::SchemaContract
                    | QualityRuleKind::RowCount => unreachable!("la regla se evaluó arriba"),
                };
                (row_count, invalid_count)
            }
        };
        let invalid_pct = if checked_count == 0 {
            0.0
        } else {
            invalid_count as f64 * 100.0 / checked_count as f64
        };
        let passed = rule
            .max_invalid
            .is_none_or(|maximum| invalid_count <= maximum)
            && rule
                .max_invalid_pct
                .is_none_or(|maximum| invalid_pct <= maximum);
        results.push(QualityRuleResult {
            column: rule.column.clone(),
            kind: rule.kind,
            max_invalid: rule.max_invalid,
            max_invalid_pct: rule.max_invalid_pct,
            min: rule.min,
            max: rule.max,
            values: rule.values.clone(),
            reference_values: rule.reference_values.clone(),
            baseline: rule.baseline.clone(),
            direction: rule.direction,
            expected: rule.expected,
            aggregate: rule.aggregate,
            tolerance_abs: rule.tolerance_abs,
            tolerance_rel: rule.tolerance_rel,
            threshold: rule.threshold,
            pattern: rule.pattern.clone(),
            dtype: rule.dtype.clone(),
            columns: rule.columns.clone(),
            operator: rule.operator,
            min_date: rule.min_date.clone(),
            max_date: rule.max_date.clone(),
            when: rule.when.clone(),
            then: rule.then.clone(),
            allow_additional: rule.allow_additional,
            required_order: rule.required_order.clone(),
            checked_count,
            invalid_count,
            invalid_pct,
            passed,
        });
    }
    let failed_rules = results.iter().filter(|result| !result.passed).count();
    Ok(QualityValidationResult {
        passed: failed_rules == 0,
        row_count,
        total_rules: results.len(),
        failed_rules,
        rules: results,
    })
}

fn enforce_export_quality_with_cancel<C>(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    allow_unvalidated: bool,
    is_cancelled: C,
) -> Result<Option<QualityValidationResult>, String>
where
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    if quality_rules.is_empty() {
        return if allow_unvalidated {
            Ok(None)
        } else {
            Err("La exportación sin reglas de calidad requiere confirmación explícita.".to_owned())
        };
    }
    let validation = evaluate_quality_rules_with_cancel(frame, quality_rules, is_cancelled)?;
    if !validation.passed {
        return Err(format!(
            "La exportación fue bloqueada: {} de {} reglas de calidad fallaron.",
            validation.failed_rules, validation.total_rules
        ));
    }
    Ok(Some(validation))
}

fn starts_with_spreadsheet_formula_prefix(value: &str) -> bool {
    matches!(
        value.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r' | '\n')
    )
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    if starts_with_spreadsheet_formula_prefix(value) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

fn csv_formula_safe_frame(frame: &DataFrame) -> Result<DataFrame, String> {
    let mut safe = frame.clone();
    let text_columns = frame
        .columns()
        .iter()
        .filter(|column| column.dtype() == &DataType::String)
        .map(|column| column.name().as_str().to_owned())
        .collect::<Vec<_>>();

    for name in text_columns {
        let values = frame
            .column(&name)
            .and_then(|column| column.str())
            .map_err(|_| "No se pudo preparar texto seguro para CSV.".to_owned())?
            .iter()
            .map(|value| value.map(neutralize_spreadsheet_formula))
            .collect::<Vec<_>>();
        safe.replace(&name, Column::new(name.clone().into(), values))
            .map_err(|_| "No se pudo proteger una columna de texto para CSV.".to_owned())?;
    }
    Ok(safe)
}

fn frame_for_export(frame: &DataFrame, format: ExportFormat) -> Result<DataFrame, String> {
    match format {
        // CSV suele abrirse en hojas de cálculo: una comilla inicial fuerza texto y evita
        // ejecutar celdas controladas por datos. Parquet conserva los valores originales.
        ExportFormat::Csv => csv_formula_safe_frame(frame),
        ExportFormat::Json => Ok(frame.clone()),
        ExportFormat::Parquet => Ok(frame.clone()),
        ExportFormat::Sql => Ok(frame.clone()),
        ExportFormat::Excel => Ok(frame.clone()),
        ExportFormat::Sqlite => Ok(frame.clone()),
        ExportFormat::Bundle => csv_formula_safe_frame(frame),
    }
}

fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_type(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "BOOLEAN",
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => "BIGINT",
        DataType::Float32 | DataType::Float64 => "DOUBLE",
        DataType::Date => "DATE",
        DataType::Datetime(_, _) => "TIMESTAMP",
        _ => "TEXT",
    }
}

fn sql_value(value: AnyValue<'_>) -> Result<String, String> {
    match value {
        AnyValue::Null => Ok("NULL".to_owned()),
        AnyValue::Boolean(value) => Ok(if value { "TRUE" } else { "FALSE" }.to_owned()),
        AnyValue::Int8(value) => Ok(value.to_string()),
        AnyValue::Int16(value) => Ok(value.to_string()),
        AnyValue::Int32(value) => Ok(value.to_string()),
        AnyValue::Int64(value) => Ok(value.to_string()),
        AnyValue::UInt8(value) => Ok(value.to_string()),
        AnyValue::UInt16(value) => Ok(value.to_string()),
        AnyValue::UInt32(value) => Ok(value.to_string()),
        AnyValue::UInt64(value) => Ok(value.to_string()),
        AnyValue::Float32(value) if value.is_finite() => Ok(value.to_string()),
        AnyValue::Float64(value) if value.is_finite() => Ok(value.to_string()),
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            Err("SQL no puede representar valores numéricos no finitos.".to_owned())
        }
        AnyValue::String(value) => Ok(sql_string_literal(value)),
        AnyValue::StringOwned(value) => Ok(sql_string_literal(value.as_str())),
        value => Ok(sql_string_literal(&value.to_string())),
    }
}

fn write_sql_script<F, C>(
    frame: &DataFrame,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    const TABLE: &str = "dataset";
    if frame.width() == 0 {
        return Err("SQL requiere al menos una columna para crear la tabla.".to_owned());
    }
    let table = sql_identifier(TABLE);
    writeln!(
        output,
        "-- Exportado por Columnia como script SQL portable."
    )
    .map_err(|error| format!("No se pudo escribir el encabezado SQL: {error}"))?;
    writeln!(output, "BEGIN TRANSACTION;")
        .map_err(|error| format!("No se pudo escribir el inicio SQL: {error}"))?;
    writeln!(output, "DROP TABLE IF EXISTS {table};")
        .map_err(|error| format!("No se pudo escribir la limpieza SQL: {error}"))?;
    write!(output, "CREATE TABLE {table} (")
        .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
    for (index, column) in frame.columns().iter().enumerate() {
        if index > 0 {
            write!(output, ", ")
                .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
        }
        write!(
            output,
            "{} {}",
            sql_identifier(column.name().as_str()),
            sql_type(column.dtype())
        )
        .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
    }
    writeln!(output, ");").map_err(|error| format!("No se pudo cerrar el esquema SQL: {error}"))?;

    let columns = frame.columns();
    for row_index in 0..frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        write!(output, "INSERT INTO {table} (")
            .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            write!(output, "{}", sql_identifier(column.name().as_str()))
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        }
        write!(output, ") VALUES (")
            .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            let value = column.get(row_index).map_err(|error| {
                format!("No se pudo leer la fila {row_index} para SQL: {error}")
            })?;
            write!(output, "{}", sql_value(value)?)
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        }
        writeln!(output, ");")
            .map_err(|error| format!("No se pudo cerrar una fila SQL: {error}"))?;
        let percent = if frame.height() == 0 {
            85
        } else {
            30 + (((row_index + 1) * 55) / frame.height()) as u8
        };
        report(percent);
    }
    ensure_not_cancelled(is_cancelled())?;
    writeln!(output, "COMMIT;")
        .map_err(|error| format!("No se pudo escribir el cierre SQL: {error}"))?;
    report(85);
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xlsx_column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = (index / 26) - 1;
    }
    name
}

fn xlsx_cell(column_index: usize, row_index: usize, value: AnyValue<'_>) -> Result<String, String> {
    let reference = format!("{}{}", xlsx_column_name(column_index), row_index + 1);
    let cell = match value {
        AnyValue::Null => format!("<c r=\"{reference}\"/>"),
        AnyValue::Boolean(value) => format!(
            "<c r=\"{reference}\" t=\"b\"><v>{}</v></c>",
            if value { 1 } else { 0 }
        ),
        AnyValue::Int8(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::Int16(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::Int32(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::Int64(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt8(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt16(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt32(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt64(value) => format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"),
        AnyValue::Float32(value) if value.is_finite() => {
            format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>")
        }
        AnyValue::Float64(value) if value.is_finite() => {
            format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>")
        }
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            return Err("Excel no puede representar valores numéricos no finitos.".to_owned())
        }
        AnyValue::String(value) => format!(
            "<c r=\"{reference}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            xml_escape(value)
        ),
        AnyValue::StringOwned(value) => format!(
            "<c r=\"{reference}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            xml_escape(value.as_str())
        ),
        value => format!(
            "<c r=\"{reference}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            xml_escape(&value.to_string())
        ),
    };
    Ok(cell)
}

fn write_xlsx<F, C>(
    frame: &DataFrame,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    if frame.width() == 0 {
        return Err("Excel requiere al menos una columna.".to_owned());
    }
    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
    const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="dataset" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#;

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut archive = ZipWriter::new(output);
    for (name, contents) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("xl/workbook.xml", WORKBOOK),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS),
        ("xl/styles.xml", STYLES),
    ] {
        archive
            .start_file(name, options)
            .map_err(|error| format!("No se pudo preparar el libro Excel: {error}"))?;
        archive
            .write_all(contents.as_bytes())
            .map_err(|error| format!("No se pudo escribir el libro Excel: {error}"))?;
    }

    archive
        .start_file("xl/worksheets/sheet1.xml", options)
        .map_err(|error| format!("No se pudo preparar la hoja Excel: {error}"))?;
    archive
        .write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#)
        .map_err(|error| format!("No se pudo escribir la hoja Excel: {error}"))?;

    archive
        .write_all(b"<row r=\"1\">")
        .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    for (column_index, column) in frame.columns().iter().enumerate() {
        let header = xlsx_cell(column_index, 0, AnyValue::String(column.name().as_str()))?;
        archive
            .write_all(header.as_bytes())
            .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    }
    archive
        .write_all(b"</row>")
        .map_err(|error| format!("No se pudo cerrar el encabezado Excel: {error}"))?;

    for row_index in 0..frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        write!(archive, "<row r=\"{}\">", row_index + 2)
            .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
        for (column_index, column) in frame.columns().iter().enumerate() {
            let value = column.get(row_index).map_err(|error| {
                format!("No se pudo leer la fila {row_index} para Excel: {error}")
            })?;
            archive
                .write_all(xlsx_cell(column_index, row_index + 1, value)?.as_bytes())
                .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
        }
        archive
            .write_all(b"</row>")
            .map_err(|error| format!("No se pudo cerrar una fila Excel: {error}"))?;
        let percent = if frame.height() == 0 {
            85
        } else {
            30 + (((row_index + 1) * 55) / frame.height()) as u8
        };
        report(percent);
    }
    archive
        .write_all(b"</sheetData></worksheet>")
        .map_err(|error| format!("No se pudo cerrar la hoja Excel: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("No se pudo finalizar el libro Excel: {error}"))?;
    report(85);
    Ok(())
}

fn sqlite_type(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => "INTEGER",
        DataType::Float32 | DataType::Float64 => "REAL",
        _ => "TEXT",
    }
}

fn sqlite_value(value: AnyValue<'_>) -> Result<SqlValue, String> {
    match value {
        AnyValue::Null => Ok(SqlValue::Null),
        AnyValue::Boolean(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int8(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int16(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int32(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int64(value) => Ok(SqlValue::Integer(value)),
        AnyValue::UInt8(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt16(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt32(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt64(value) => i64::try_from(value)
            .map(SqlValue::Integer)
            .or_else(|_| Ok(SqlValue::Text(value.to_string()))),
        AnyValue::Float32(value) if value.is_finite() => Ok(SqlValue::Real(value as f64)),
        AnyValue::Float64(value) if value.is_finite() => Ok(SqlValue::Real(value)),
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            Err("SQLite no puede representar valores numéricos no finitos.".to_owned())
        }
        AnyValue::String(value) => Ok(SqlValue::Text(value.to_owned())),
        AnyValue::StringOwned(value) => Ok(SqlValue::Text(value.to_string())),
        value => Ok(SqlValue::Text(value.to_string())),
    }
}

fn write_sqlite_database<F, C>(
    frame: &DataFrame,
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    if frame.width() == 0 {
        return Err("SQLite requiere al menos una columna.".to_owned());
    }
    let mut connection = Connection::open(path)
        .map_err(|error| format!("No se pudo crear la base SQLite: {error}"))?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("No se pudo iniciar la transacción SQLite: {error}"))?;
    transaction
        .execute_batch("DROP TABLE IF EXISTS \"dataset\";")
        .map_err(|error| format!("No se pudo preparar la tabla SQLite: {error}"))?;
    let definition = frame
        .columns()
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                sql_identifier(column.name().as_str()),
                sqlite_type(column.dtype())
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    transaction
        .execute_batch(&format!("CREATE TABLE \"dataset\" ({definition});"))
        .map_err(|error| format!("No se pudo crear la tabla SQLite: {error}"))?;
    let placeholders = (0..frame.width())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let columns = frame
        .columns()
        .iter()
        .map(|column| sql_identifier(column.name().as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = transaction
        .prepare(&format!(
            "INSERT INTO \"dataset\" ({columns}) VALUES ({placeholders});"
        ))
        .map_err(|error| format!("No se pudo preparar la inserción SQLite: {error}"))?;
    for row_index in 0..frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        let values = frame
            .columns()
            .iter()
            .map(|column| {
                column
                    .get(row_index)
                    .map_err(|error| {
                        format!("No se pudo leer la fila {row_index} para SQLite: {error}")
                    })
                    .and_then(sqlite_value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        statement
            .execute(params_from_iter(values))
            .map_err(|error| {
                format!("No se pudo insertar la fila {row_index} en SQLite: {error}")
            })?;
        let percent = if frame.height() == 0 {
            85
        } else {
            30 + (((row_index + 1) * 55) / frame.height()) as u8
        };
        report(percent);
    }
    drop(statement);
    transaction
        .commit()
        .map_err(|error| format!("No se pudo confirmar la base SQLite: {error}"))?;
    connection
        .execute_batch("PRAGMA user_version = 1;")
        .map_err(|error| format!("No se pudo versionar la base SQLite: {error}"))?;
    report(85);
    Ok(())
}

fn privacy_safe_frame(
    frame: &DataFrame,
    mode: PrivacyMode,
) -> Result<(DataFrame, Vec<String>), String> {
    if mode == PrivacyMode::None {
        return Ok((frame.clone(), Vec::new()));
    }
    let mut safe = frame.clone();
    let protected_columns = frame
        .columns()
        .iter()
        .filter(|column| privacy_signal(column.name()).is_some())
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    for column in frame
        .columns()
        .iter()
        .filter(|column| privacy_signal(column.name()).is_some())
    {
        let values = (0..column.len())
            .map(|row_index| {
                column
                    .get(row_index)
                    .map_err(|_| {
                        "No se pudo preparar una columna para protección de privacidad.".to_owned()
                    })
                    .map(|value| match value {
                        AnyValue::Null => None,
                        value => {
                            let value = value.to_string();
                            Some(match mode {
                                PrivacyMode::None => value,
                                PrivacyMode::Mask => REDACTED_VALUE.to_owned(),
                                PrivacyMode::Hash => {
                                    format!("{:x}", Sha256::digest(value.as_bytes()))
                                }
                            })
                        }
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        safe.replace(
            column.name().as_str(),
            Column::new(column.name().clone(), values),
        )
        .map_err(|_| "No se pudo proteger una columna de datos personales.".to_owned())?;
    }
    Ok((safe, protected_columns))
}

fn export_frame_atomic<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy(
        frame,
        destination,
        format,
        PrivacyMode::None,
        report,
        is_cancelled,
    )
}

fn export_frame_atomic_with_privacy<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy_and_quality(
        frame,
        destination,
        format,
        privacy_mode,
        None,
        report,
        is_cancelled,
    )
}

fn export_frame_atomic_with_privacy_and_quality<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    quality_validation: Option<&QualityValidationResult>,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy_and_quality_and_recipe(
        frame,
        destination,
        format,
        privacy_mode,
        quality_validation,
        None,
        report,
        is_cancelled,
    )
}

#[allow(clippy::too_many_arguments)]
fn export_frame_atomic_with_privacy_and_quality_and_recipe<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    mut report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    let destination = canonicalize_write_destination(destination, "la exportación")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?;
    report("Preparando archivo temporal", 10);
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo crear el archivo temporal: {error}"))?;
    let (protected_frame, protected_columns) = privacy_safe_frame(frame, privacy_mode)?;
    let mut output_frame = frame_for_export(&protected_frame, format)?;

    report("Escribiendo dataset", 25);
    match format {
        ExportFormat::Csv => CsvWriter::new(temporary.as_file())
            .finish(&mut output_frame)
            .map_err(|error| format!("No se pudo escribir el CSV: {error}"))?,
        ExportFormat::Json => JsonWriter::new(temporary.as_file())
            .with_json_format(JsonFormat::Json)
            .finish(&mut output_frame)
            .map_err(|error| format!("No se pudo escribir el JSON: {error}"))?,
        ExportFormat::Parquet => ParquetWriter::new(temporary.as_file())
            .finish(&mut output_frame)
            .map(|_| ())
            .map_err(|error| format!("No se pudo escribir Parquet: {error}"))?,
        ExportFormat::Sql => write_sql_script(
            &protected_frame,
            temporary.as_file_mut(),
            |percent| report("Escribiendo SQL", percent),
            &is_cancelled,
        )?,
        ExportFormat::Excel => write_xlsx(
            &protected_frame,
            temporary.as_file_mut(),
            |percent| report("Escribiendo Excel", percent),
            &is_cancelled,
        )?,
        ExportFormat::Sqlite => write_sqlite_database(
            &protected_frame,
            temporary.path(),
            |percent| report("Escribiendo SQLite", percent),
            &is_cancelled,
        )?,
        ExportFormat::Bundle => write_bundle(
            &protected_frame,
            quality_validation,
            recipe,
            temporary.as_file_mut(),
            &mut report,
            &is_cancelled,
        )?,
    }

    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la exportación: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    report("Publicando archivo completo", 90);
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar la exportación: {}", error.error))?;

    let file_size_bytes = fs::metadata(&destination)
        .map_err(|error| format!("No se pudo verificar la exportación: {error}"))?
        .len();
    report("Exportación lista", 100);
    Ok(ExportResult {
        file_name: destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dataset")
            .to_owned(),
        file_size_bytes,
        format: format.label(),
        protected_column_count: protected_columns.len(),
        protected_columns,
    })
}

fn bundle_json_bytes<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| format!("No se pudo preparar {label} del paquete: {error}"))
}

fn hash_and_rewind(file: &mut File) -> Result<(u64, String), String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo leer el dataset temporal del paquete: {error}"))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo calcular el hash del paquete: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo rebobinar el dataset temporal: {error}"))?;
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

fn write_bundle<F, C>(
    frame: &DataFrame,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    if let Some(recipe) = recipe {
        validate_stored_recipe(recipe)?;
    }
    report("Preparando paquete", 10);

    // El dataset se materializa en disco antes de abrir el ZIP para poder
    // calcular su hash sin duplicar datasets grandes en memoria.
    let mut dataset_file = tempfile::tempfile()
        .map_err(|error| format!("No se pudo preparar el dataset del paquete: {error}"))?;
    let mut csv_frame = frame_for_export(frame, ExportFormat::Csv)?;
    CsvWriter::new(&mut dataset_file)
        .finish(&mut csv_frame)
        .map_err(|error| format!("No se pudo escribir el dataset del paquete: {error}"))?;
    dataset_file
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el dataset del paquete: {error}"))?;
    let (dataset_bytes, dataset_sha256) = hash_and_rewind(&mut dataset_file)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Preparando diccionario", 35);

    let dictionary = BundleDictionary {
        format: "columnia-dictionary".to_owned(),
        version: 1,
        columns: frame
            .columns()
            .iter()
            .map(|column| BundleDictionaryColumn {
                name: column.name().to_string(),
                data_type: column.dtype().to_string(),
                null_count: column.null_count(),
            })
            .collect(),
    };
    let dictionary_bytes = bundle_json_bytes(&dictionary, "el diccionario")?;
    let dictionary_sha256 = format!("{:x}", Sha256::digest(&dictionary_bytes));
    let quality_bytes = quality_validation
        .map(|validation| {
            let report = BundleQualityReport {
                format: "columnia-quality-report".to_owned(),
                version: 1,
                passed: validation.passed,
                row_count: validation.row_count,
                total_rules: validation.total_rules,
                failed_rules: validation.failed_rules,
                rules: validation
                    .rules
                    .iter()
                    .map(|rule| BundleQualityRuleReport {
                        column: rule.column.clone(),
                        kind: rule.kind,
                        checked_count: rule.checked_count,
                        invalid_count: rule.invalid_count,
                        invalid_pct: rule.invalid_pct,
                        passed: rule.passed,
                    })
                    .collect(),
            };
            bundle_json_bytes(&report, "el reporte de calidad")
        })
        .transpose()?;
    let quality_manifest = quality_bytes.as_ref().map(|bytes| BundleFileManifest {
        path: "quality-report.json".to_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    });
    let recipe_bytes = recipe
        .map(|recipe| bundle_json_bytes(recipe, "la receta"))
        .transpose()?;
    let recipe_manifest = recipe_bytes.as_ref().map(|bytes| BundleFileManifest {
        path: "recipe.json".to_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    });
    let manifest = BundleManifest {
        format: "columnia-bundle".to_owned(),
        version: 1,
        dataset_file: "dataset.csv".to_owned(),
        dataset_format: "csv".to_owned(),
        row_count: frame.height(),
        column_count: frame.width(),
        dictionary_file: "dictionary.json".to_owned(),
        quality_report_file: quality_bytes
            .as_ref()
            .map(|_| "quality-report.json".to_owned()),
        recipe_file: recipe_bytes.as_ref().map(|_| "recipe.json".to_owned()),
        files: [
            BundleFileManifest {
                path: "dataset.csv".to_owned(),
                bytes: dataset_bytes,
                sha256: dataset_sha256,
            },
            BundleFileManifest {
                path: "dictionary.json".to_owned(),
                bytes: dictionary_bytes.len() as u64,
                sha256: dictionary_sha256,
            },
        ]
        .into_iter()
        .chain(quality_manifest)
        .chain(recipe_manifest)
        .collect(),
    };
    let manifest_bytes = bundle_json_bytes(&manifest, "el manifest")?;
    ensure_not_cancelled(is_cancelled())?;
    report("Empaquetando archivos", 50);

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut archive = ZipWriter::new(output);
    archive
        .start_file("dataset.csv", options)
        .map_err(|error| format!("No se pudo preparar el dataset del paquete: {error}"))?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let read = dataset_file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo leer el dataset del paquete: {error}"))?;
        if read == 0 {
            break;
        }
        archive
            .write_all(&buffer[..read])
            .map_err(|error| format!("No se pudo empaquetar el dataset: {error}"))?;
        copied += read as u64;
        let percent = if dataset_bytes == 0 {
            65
        } else {
            50 + (copied
                .saturating_mul(15)
                .checked_div(dataset_bytes)
                .unwrap_or(0) as u8)
                .min(15)
        };
        report("Empaquetando dataset", percent);
    }
    ensure_not_cancelled(is_cancelled())?;
    archive
        .start_file("dictionary.json", options)
        .map_err(|error| format!("No se pudo preparar el diccionario del paquete: {error}"))?;
    archive
        .write_all(&dictionary_bytes)
        .map_err(|error| format!("No se pudo empaquetar el diccionario: {error}"))?;
    if let Some(quality_bytes) = quality_bytes {
        ensure_not_cancelled(is_cancelled())?;
        archive
            .start_file("quality-report.json", options)
            .map_err(|error| format!("No se pudo preparar el reporte de calidad: {error}"))?;
        archive
            .write_all(&quality_bytes)
            .map_err(|error| format!("No se pudo empaquetar el reporte de calidad: {error}"))?;
    }
    if let Some(recipe_bytes) = recipe_bytes {
        ensure_not_cancelled(is_cancelled())?;
        archive
            .start_file("recipe.json", options)
            .map_err(|error| format!("No se pudo preparar la receta del paquete: {error}"))?;
        archive
            .write_all(&recipe_bytes)
            .map_err(|error| format!("No se pudo empaquetar la receta: {error}"))?;
    }
    ensure_not_cancelled(is_cancelled())?;
    archive
        .start_file("manifest.json", options)
        .map_err(|error| format!("No se pudo preparar el manifest del paquete: {error}"))?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|error| format!("No se pudo empaquetar el manifest: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("No se pudo cerrar el paquete: {error}"))?;
    report("Paquete listo", 88);
    Ok(())
}

fn load_compare_frame(path: &Path, extension: &str) -> Result<DataFrame, String> {
    if spreadsheet_extensions(extension) {
        let sheets = inspect_workbook(path)?;
        let sheet = sheets
            .first()
            .ok_or_else(|| "El libro no contiene hojas que se puedan comparar.".to_owned())?;
        load_spreadsheet_sheet(path, sheet, SpreadsheetHeaderMode::FirstRow)
    } else {
        load_dataset_with_progress(path, |_, _| {}, || false).map(|(frame, _)| frame)
    }
}

fn row_signature(
    frame: &DataFrame,
    columns: &[String],
    row_index: usize,
) -> Result<String, String> {
    let mut signature = String::new();
    for name in columns {
        let value = frame
            .column(name)
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?
            .get(row_index)
            .map_err(|error| format!("No se pudo comparar la fila {row_index}: {error}"))?;
        match preview_value(value) {
            Some(value) => {
                use std::fmt::Write;
                write!(&mut signature, "v{}:{value};", value.len())
                    .map_err(|_| "No se pudo preparar la comparación.".to_owned())?;
            }
            None => signature.push_str("n;"),
        }
    }
    Ok(signature)
}

fn row_signatures(frame: &DataFrame, columns: &[String]) -> Result<HashMap<String, usize>, String> {
    let mut counts = HashMap::new();
    for start in (0..frame.height()).step_by(LOCAL_QUERY_BLOCK_ROWS) {
        let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
        let partial = (start..end)
            .into_par_iter()
            .try_fold(
                HashMap::<String, usize>::new,
                |mut partial, row_index| -> Result<HashMap<String, usize>, String> {
                    let signature = row_signature(frame, columns, row_index)?;
                    *partial.entry(signature).or_insert(0) += 1;
                    Ok(partial)
                },
            )
            .try_reduce(HashMap::new, |mut left, right| {
                for (signature, count) in right {
                    *left.entry(signature).or_insert(0) += count;
                }
                Ok(left)
            })?;
        for (signature, count) in partial {
            *counts.entry(signature).or_insert(0) += count;
        }
    }
    Ok(counts)
}

fn key_rows(
    frame: &DataFrame,
    key_columns: &[String],
) -> Result<HashMap<String, Vec<usize>>, String> {
    let mut rows_by_key = HashMap::new();
    for start in (0..frame.height()).step_by(LOCAL_QUERY_BLOCK_ROWS) {
        let end = (start + LOCAL_QUERY_BLOCK_ROWS).min(frame.height());
        let partial = (start..end)
            .into_par_iter()
            .try_fold(
                HashMap::new,
                |mut partial, row_index| -> Result<_, String> {
                    let signature = row_signature(frame, key_columns, row_index)?;
                    partial
                        .entry(signature)
                        .or_insert_with(Vec::new)
                        .push(row_index);
                    Ok(partial)
                },
            )
            .try_reduce(HashMap::new, |mut left, right| -> Result<_, String> {
                for (signature, mut rows) in right {
                    left.entry(signature)
                        .or_insert_with(Vec::new)
                        .append(&mut rows);
                }
                Ok(left)
            })?;
        for (signature, mut rows) in partial {
            rows_by_key
                .entry(signature)
                .or_insert_with(Vec::new)
                .append(&mut rows);
        }
    }
    for rows in rows_by_key.values_mut() {
        rows.sort_unstable();
    }
    Ok(rows_by_key)
}

#[derive(Default)]
struct KeyComparisonSummary {
    matched_key_count: usize,
    current_only_key_count: usize,
    compared_only_key_count: usize,
    conflicting_key_count: usize,
    duplicate_key_count: usize,
}

fn validate_key_columns(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
) -> Result<(), String> {
    if key_columns.is_empty() {
        return Err("Selecciona al menos una columna clave para unir datasets.".to_owned());
    }
    for key in key_columns {
        let current_column = current
            .column(key)
            .map_err(|_| format!("La columna clave '{key}' no existe en el dataset activo."))?;
        let compared_column = compared
            .column(key)
            .map_err(|_| format!("La columna clave '{key}' no existe en el dataset comparado."))?;
        if current_column.dtype() != compared_column.dtype() {
            return Err(format!(
                "La columna clave '{key}' tiene tipos incompatibles entre los datasets."
            ));
        }
    }
    Ok(())
}

fn compare_keyed_frames(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<KeyComparisonSummary, String> {
    validate_key_columns(current, compared, key_columns)?;

    let current_rows = key_rows(current, key_columns)?;
    let compared_rows = key_rows(compared, key_columns)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let duplicate_key_count = current_rows
        .iter()
        .filter(|(_, rows)| rows.len() > 1)
        .count()
        + compared_rows
            .iter()
            .filter(|(key, rows)| {
                rows.len() > 1
                    && current_rows
                        .get(*key)
                        .is_none_or(|current_rows| current_rows.len() <= 1)
            })
            .count();
    let mut summary = KeyComparisonSummary {
        matched_key_count: current_rows
            .keys()
            .filter(|key| compared_rows.contains_key(*key))
            .count(),
        current_only_key_count: current_rows
            .keys()
            .filter(|key| !compared_rows.contains_key(*key))
            .count(),
        compared_only_key_count: compared_rows
            .keys()
            .filter(|key| !current_rows.contains_key(*key))
            .count(),
        duplicate_key_count,
        ..Default::default()
    };

    for (key, current_key_rows) in &current_rows {
        let Some(compared_key_rows) = compared_rows.get(key) else {
            continue;
        };
        if current_key_rows.len() != 1 || compared_key_rows.len() != 1 {
            continue;
        }
        let current_payload = row_signature(current, &shared_payload_columns, current_key_rows[0])?;
        let compared_payload =
            row_signature(compared, &shared_payload_columns, compared_key_rows[0])?;
        if current_payload != compared_payload {
            summary.conflicting_key_count += 1;
        }
    }

    Ok(summary)
}

struct KeyConflictRows {
    current_row_index: usize,
    compared_row_index: usize,
    conflict: DatasetConflict,
}

fn collect_key_conflicts_page(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
    offset: usize,
    limit: usize,
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    validate_key_columns(current, compared, key_columns)?;
    let current_rows = key_rows(current, key_columns)?;
    let compared_rows = key_rows(compared, key_columns)?;
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let mut conflicts = Vec::new();
    let mut total = 0usize;

    for current_row_index in 0..current.height() {
        let signature = row_signature(current, key_columns, current_row_index)?;
        if !seen.insert(signature.clone()) {
            continue;
        }
        let Some(current_key_rows) = current_rows.get(&signature) else {
            continue;
        };
        let Some(compared_key_rows) = compared_rows.get(&signature) else {
            continue;
        };
        if current_key_rows.len() != 1 || compared_key_rows.len() != 1 {
            continue;
        }
        let current_row_index = current_key_rows[0];
        let compared_row_index = compared_key_rows[0];
        let current_payload = row_signature(current, &shared_payload_columns, current_row_index)?;
        let compared_payload =
            row_signature(compared, &shared_payload_columns, compared_row_index)?;
        if current_payload == compared_payload {
            continue;
        }

        let key = key_columns
            .iter()
            .map(|column| {
                current
                    .column(column)
                    .map_err(|error| format!("No se pudo leer la clave '{column}': {error}"))?
                    .get(current_row_index)
                    .map_err(|error| format!("No se pudo leer la fila en conflicto: {error}"))
                    .map(preview_value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cells = shared_payload_columns
            .iter()
            .map(|column| {
                let current_value = current
                    .column(column)
                    .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                    .get(current_row_index)
                    .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                    .map(preview_value)?;
                let compared_value = compared
                    .column(column)
                    .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
                    .get(compared_row_index)
                    .map_err(|error| format!("No se pudo leer el valor en conflicto: {error}"))
                    .map(preview_value)?;
                Ok((column.clone(), current_value, compared_value))
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .filter(|(_, current_value, compared_value)| current_value != compared_value)
            .map(|(column, current, compared)| DatasetConflictCell {
                column,
                current,
                compared,
            })
            .collect::<Vec<_>>();
        if cells.is_empty() {
            continue;
        }
        let conflict_index = total;
        total = total.saturating_add(1);
        if conflict_index >= offset && conflicts.len() < limit {
            conflicts.push(KeyConflictRows {
                current_row_index,
                compared_row_index,
                conflict: DatasetConflict { key, cells },
            });
        }
    }
    let page_end = offset.saturating_add(conflicts.len());
    Ok((conflicts, total > page_end))
}

fn collect_key_conflicts(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<(Vec<KeyConflictRows>, bool), String> {
    collect_key_conflicts_page(
        current,
        compared,
        key_columns,
        shared_columns,
        0,
        MAX_CONFLICT_PREVIEW,
    )
}

fn collect_all_key_conflicts(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    shared_columns: &[String],
) -> Result<Vec<KeyConflictRows>, String> {
    let (conflicts, _) = collect_key_conflicts_page(
        current,
        compared,
        key_columns,
        shared_columns,
        0,
        usize::MAX,
    )?;
    Ok(conflicts)
}

fn normalize_key_columns(key_columns: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for key in key_columns.unwrap_or_default() {
        if key.is_empty() {
            return Err("Las columnas clave no pueden estar vacías.".to_owned());
        }
        if !normalized.contains(&key) {
            normalized.push(key);
        }
    }
    if normalized.len() > 16 {
        return Err("La comparación admite como máximo 16 columnas clave.".to_owned());
    }
    Ok(normalized)
}

fn rows_with_new_keys(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
) -> Result<DataFrame, String> {
    let current_keys = key_rows(current, key_columns)?;
    let keep = (0..compared.height())
        .map(|row_index| {
            row_signature(compared, key_columns, row_index)
                .map(|signature| !current_keys.contains_key(&signature))
        })
        .collect::<Result<Vec<_>, String>>()?;
    compared
        .filter(&BooleanChunked::from_slice("new_keys".into(), &keep))
        .map_err(|error| format!("No se pudieron seleccionar las claves nuevas: {error}"))
}

fn join_frames(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    join_type: DatasetJoinType,
) -> Result<DataFrame, String> {
    join_frames_on_keys(current, compared, key_columns, key_columns, join_type)
}

fn join_frames_on_keys(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
) -> Result<DataFrame, String> {
    join_frames_on_keys_with_cancel(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        &|| false,
    )
}

fn local_join_key(
    frame: &DataFrame,
    row_index: usize,
    key_columns: &[String],
) -> Result<String, String> {
    use std::fmt::Write as _;

    let mut key = String::new();
    for column_name in key_columns {
        let value = frame
            .column(column_name)
            .map_err(|_| format!("La columna clave '{column_name}' no existe en el dataset."))?
            .get(row_index)
            .map_err(|error| format!("No se pudo leer la clave '{column_name}': {error}"))?;
        match preview_value(value) {
            Some(value) => {
                write!(&mut key, "1{}:", value.len())
                    .expect("escribir en un String no debe fallar");
                key.push_str(&value);
            }
            None => key.push_str("0;"),
        }
    }
    Ok(key)
}

fn join_cardinality_upper_bound<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Sync,
{
    let mut compared_counts = HashMap::<String, usize>::new();
    for row_index in 0..compared.height() {
        if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let key = local_join_key(compared, row_index, compared_keys)?;
        let count = compared_counts.entry(key).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?;
    }

    let mut inner_rows = 0usize;
    for row_index in 0..current.height() {
        if row_index % LOCAL_QUERY_CANCEL_CHECK_ROWS == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let key = local_join_key(current, row_index, current_keys)?;
        let compared_count = compared_counts.get(&key).copied().unwrap_or(0);
        inner_rows = inner_rows
            .checked_add(compared_count)
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?;
        if inner_rows > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
            return Ok(inner_rows);
        }
    }

    let bound = match join_type {
        DatasetJoinType::Inner => inner_rows,
        DatasetJoinType::Left => inner_rows
            .checked_add(current.height())
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?,
        DatasetJoinType::Full => inner_rows
            .checked_add(current.height())
            .and_then(|value| value.checked_add(compared.height()))
            .ok_or_else(|| "El cardinal del JOIN supera la capacidad local.".to_owned())?,
    };
    ensure_not_cancelled(is_cancelled())?;
    Ok(bound)
}

fn join_frames_on_keys_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    current_keys: &[String],
    compared_keys: &[String],
    join_type: DatasetJoinType,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    if current_keys.is_empty() || current_keys.len() != compared_keys.len() {
        return Err(
            "El JOIN necesita el mismo número de columnas clave en ambos datasets.".to_owned(),
        );
    }
    if current.height().saturating_add(compared.height()) > LOCAL_QUERY_JOIN_MAX_INPUT_ROWS {
        return Err(format!(
            "El JOIN local limita las entradas a {LOCAL_QUERY_JOIN_MAX_INPUT_ROWS} filas para proteger la memoria."
        ));
    }
    for (current_key, compared_key) in current_keys.iter().zip(compared_keys) {
        let current_column = current.column(current_key).map_err(|_| {
            format!("La columna clave '{current_key}' no existe en el dataset activo.")
        })?;
        let compared_column = compared.column(compared_key).map_err(|_| {
            format!("La columna clave '{compared_key}' no existe en el dataset comparado.")
        })?;
        if current_column.dtype() != compared_column.dtype() {
            return Err(format!(
                "Las columnas clave '{current_key}' y '{compared_key}' tienen tipos incompatibles."
            ));
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    let cardinality = join_cardinality_upper_bound(
        current,
        compared,
        current_keys,
        compared_keys,
        join_type,
        is_cancelled,
    )?;
    if cardinality > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
        return Err(format!(
            "El resultado estimado del JOIN supera el límite local de {LOCAL_QUERY_JOIN_MAX_RESULT_ROWS} filas; reduce los duplicados de las claves."
        ));
    }
    ensure_not_cancelled(is_cancelled())?;
    let left_on = current_keys.iter().map(col).collect::<Vec<_>>();
    let right_on = compared_keys.iter().map(col).collect::<Vec<_>>();
    let mut join_args =
        JoinArgs::new(join_type.polars_type()).with_coalesce(JoinCoalesce::CoalesceColumns);
    join_args.maintain_order = MaintainOrderJoin::Left;
    let plan = current
        .clone()
        .lazy()
        .join(compared.clone().lazy(), left_on, right_on, join_args);
    let joined = collect_lazy_frame_streaming(plan, "No se pudieron unir los datasets por clave")?;
    ensure_not_cancelled(is_cancelled())?;
    if joined.height() > LOCAL_QUERY_JOIN_MAX_RESULT_ROWS {
        return Err(format!(
            "El resultado del JOIN supera el límite local de {LOCAL_QUERY_JOIN_MAX_RESULT_ROWS} filas."
        ));
    }
    Ok(joined)
}

fn compare_frames(
    current: &DataFrame,
    current_file_name: &str,
    compared: &DataFrame,
    compared_file_name: &str,
    key_columns: &[String],
) -> Result<DatasetComparison, String> {
    let current_columns = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let compared_columns = compared
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let shared_columns = current_columns
        .iter()
        .filter(|name| compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let current_only_columns = current_columns
        .iter()
        .filter(|name| !compared_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let compared_only_columns = compared_columns
        .iter()
        .filter(|name| !current_columns.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    let schema_compatible = current_columns == compared_columns
        && current
            .columns()
            .iter()
            .zip(compared.columns())
            .all(|(left, right)| left.dtype() == right.dtype());
    let (common_row_count, current_only_row_count, compared_only_row_count) =
        if shared_columns.is_empty() {
            (0, current.height(), compared.height())
        } else {
            let current_rows = row_signatures(current, &shared_columns)?;
            let compared_rows = row_signatures(compared, &shared_columns)?;
            let common = current_rows
                .iter()
                .map(|(signature, count)| {
                    (*count).min(compared_rows.get(signature).copied().unwrap_or(0))
                })
                .sum::<usize>();
            (
                common,
                current.height().saturating_sub(common),
                compared.height().saturating_sub(common),
            )
        };
    let (key_summary, conflicts, conflicts_truncated) = if key_columns.is_empty() {
        (KeyComparisonSummary::default(), Vec::new(), false)
    } else {
        let summary = compare_keyed_frames(current, compared, key_columns, &shared_columns)?;
        let (conflicts, truncated) =
            collect_key_conflicts(current, compared, key_columns, &shared_columns)?;
        (summary, conflicts, truncated)
    };
    let can_consolidate = schema_compatible
        && key_summary.conflicting_key_count == 0
        && key_summary.duplicate_key_count == 0;

    Ok(DatasetComparison {
        current_file_name: current_file_name.to_owned(),
        compared_file_name: compared_file_name.to_owned(),
        current_row_count: current.height(),
        compared_row_count: compared.height(),
        common_row_count,
        current_only_row_count,
        compared_only_row_count,
        shared_columns,
        current_only_columns,
        compared_only_columns,
        schema_compatible,
        key_columns: key_columns.to_vec(),
        matched_key_count: key_summary.matched_key_count,
        current_only_key_count: key_summary.current_only_key_count,
        compared_only_key_count: key_summary.compared_only_key_count,
        conflicting_key_count: key_summary.conflicting_key_count,
        duplicate_key_count: key_summary.duplicate_key_count,
        conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
        conflict_offset: 0,
        conflicts_truncated,
        can_consolidate,
    })
}

#[tauri::command]
pub async fn compare_dataset(
    app: AppHandle,
    key_columns: Option<Vec<String>>,
) -> Result<Option<DatasetComparison>, String> {
    let key_columns = normalize_key_columns(key_columns)?;
    let (current_frame, current_file_name) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        (dataset.frame.clone(), dataset.file_name.clone())
    };
    let selection = app
        .dialog()
        .file()
        .add_filter(
            "Datasets compatibles",
            &[
                "csv", "tsv", "txt", "json", "jsonl", "ndjson", "parquet", "xlsx", "xls", "xlsb",
                "ods",
            ],
        )
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let compared_file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let compared_frame = load_compare_frame(&path, &extension)?;
        let comparison = compare_frames(
            &current_frame,
            &current_file_name,
            &compared_frame,
            &compared_file_name,
            &key_columns,
        )?;
        let state = app.state::<DatasetState>();
        *state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? =
            Some(PendingComparison {
                file_name: compared_file_name,
                file_size_bytes,
                frame: compared_frame,
                key_columns,
            });
        Ok(Some(comparison))
    })
    .await
    .map_err(|error| format!("La comparación se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn get_dataset_conflict_page(
    app: AppHandle,
    offset: usize,
    limit: usize,
) -> Result<Option<DatasetConflictPage>, String> {
    if limit == 0 || limit > MAX_CONFLICT_PREVIEW {
        return Err(format!(
            "El tamaño de página de conflictos debe estar entre 1 y {MAX_CONFLICT_PREVIEW}."
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let current_frame = {
            let current = state
                .current
                .lock()
                .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
            current
                .as_ref()
                .ok_or_else(|| {
                    "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
                })?
                .frame
                .clone()
        };
        let (compared_frame, key_columns) = {
            let comparison = state
                .comparison
                .lock()
                .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?;
            let pending = comparison
                .as_ref()
                .ok_or_else(|| "No hay una comparación activa para paginar.".to_owned())?;
            (pending.frame.clone(), pending.key_columns.clone())
        };
        let current_columns = current_frame
            .get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let compared_columns = compared_frame
            .get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let shared_columns = current_columns
            .iter()
            .filter(|name| compared_columns.contains(name))
            .cloned()
            .collect::<Vec<_>>();
        let (conflicts, has_next) = collect_key_conflicts_page(
            &current_frame,
            &compared_frame,
            &key_columns,
            &shared_columns,
            offset,
            limit,
        )?;
        Ok(Some(DatasetConflictPage {
            offset,
            conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
            has_next,
        }))
    })
    .await
    .map_err(|error| format!("La página de conflictos se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn join_dataset(
    app: AppHandle,
    key_columns: Vec<String>,
    join_type: DatasetJoinType,
) -> Result<Option<DatasetPreview>, String> {
    let key_columns = normalize_key_columns(Some(key_columns))?;
    if key_columns.is_empty() {
        return Err("Selecciona al menos una columna clave para unir datasets.".to_owned());
    }
    let (current_frame, current_file_name, current_file_size) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        (
            dataset.frame.clone(),
            dataset.file_name.clone(),
            dataset.file_size_bytes,
        )
    };
    let selection = app
        .dialog()
        .file()
        .add_filter(
            "Datasets compatibles",
            &[
                "csv", "tsv", "txt", "json", "jsonl", "ndjson", "parquet", "xlsx", "xls", "xlsb",
                "ods",
            ],
        )
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let compared_file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let compared_frame = load_compare_frame(&path, &extension)?;
        let joined = join_frames(&current_frame, &compared_frame, &key_columns, join_type)?;
        let file_name = format!(
            "Join {} · {} + {compared_file_name}",
            join_type.label(),
            current_file_name
        );
        let file_size_bytes = current_file_size.saturating_add(file_size_bytes);
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let preview = dataset_preview_with_size(&file_name, file_size_bytes, &joined)?;
        dataset
            .history
            .record(&joined, &format!("Unir datasets ({})", join_type.label()))?;
        dataset.source_path = None;
        dataset.file_name = file_name;
        dataset.file_size_bytes = file_size_bytes;
        dataset.frame = joined;
        dataset.profile = None;
        drop(current);
        *state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? = None;
        Ok(Some(preview))
    })
    .await
    .map_err(|error| format!("La unión se interrumpió: {error}"))?
}

fn resolved_conflict_frame(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    decisions: &[ConflictResolution],
) -> Result<DataFrame, String> {
    if key_columns.is_empty() {
        return Err("La resolución de conflictos requiere al menos una columna clave.".to_owned());
    }
    if current.get_column_names() != compared.get_column_names()
        || current
            .columns()
            .iter()
            .zip(compared.columns())
            .any(|(left, right)| left.dtype() != right.dtype())
    {
        return Err("Los esquemas no son compatibles para resolver conflictos.".to_owned());
    }
    let shared_columns = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let conflicts = collect_all_key_conflicts(current, compared, key_columns, &shared_columns)?;
    let mut choices = HashMap::new();
    for decision in decisions {
        let Some(conflict) = conflicts.get(decision.conflict_index) else {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        };
        if let Some(column) = decision.column.as_ref() {
            if !conflict
                .conflict
                .cells
                .iter()
                .any(|cell| &cell.column == column)
            {
                return Err(format!(
                    "La columna '{column}' no pertenece al conflicto seleccionado."
                ));
            }
        }
        if choices
            .insert(
                (decision.conflict_index, decision.column.clone()),
                decision.source,
            )
            .is_some()
        {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        }
    }

    for (conflict_index, conflict) in conflicts.iter().enumerate() {
        let conflict_choices = choices
            .iter()
            .filter(|((index, _), _)| *index == conflict_index)
            .collect::<Vec<_>>();
        let uses_columns = conflict_choices
            .iter()
            .any(|((_, column), _)| column.is_some());
        if conflict_choices
            .iter()
            .any(|((_, column), _)| column.is_some() != uses_columns)
        {
            return Err(
                "Cada conflicto debe resolverse por fila completa o por todas sus columnas."
                    .to_owned(),
            );
        }
        if uses_columns {
            let selected_columns = conflict_choices
                .iter()
                .filter_map(|((_, column), _)| column.as_deref())
                .collect::<HashSet<_>>();
            let expected_columns = conflict
                .conflict
                .cells
                .iter()
                .map(|cell| cell.column.as_str())
                .collect::<HashSet<_>>();
            if selected_columns != expected_columns {
                return Err(
                    "Faltan decisiones de columna para alguno de los conflictos visibles."
                        .to_owned(),
                );
            }
        } else if conflict_choices.len() != 1 {
            return Err("Debes elegir un origen para cada conflicto visible.".to_owned());
        }
    }

    let conflict_rows = conflicts
        .iter()
        .enumerate()
        .map(|(index, conflict)| (conflict.current_row_index, index))
        .collect::<HashMap<_, _>>();
    let mut resolved_columns = Vec::with_capacity(current.width());
    for current_column in current.columns() {
        let name = current_column.name().to_string();
        let compared_column = compared
            .column(&name)
            .map_err(|error| format!("No se pudo leer la columna comparada '{name}': {error}"))?;
        let values = (0..current.height())
            .map(|row_index| {
                let source = conflict_rows.get(&row_index).and_then(|conflict_index| {
                    choices
                        .get(&(*conflict_index, Some(name.clone())))
                        .or_else(|| choices.get(&(*conflict_index, None)))
                        .copied()
                });
                let source_row = match source {
                    Some(ConflictSource::Compared) => conflicts
                        .get(*conflict_rows.get(&row_index).expect("conflict row exists"))
                        .map(|conflict| conflict.compared_row_index)
                        .ok_or_else(|| {
                            "No se encontró la fila comparada en conflicto.".to_owned()
                        })?,
                    _ => row_index,
                };
                let column = if matches!(source, Some(ConflictSource::Compared)) {
                    compared_column
                } else {
                    current_column
                };
                let value = column
                    .get(source_row)
                    .map_err(|error| format!("No se pudo leer la fila resuelta: {error}"))?;
                Ok(value.clone())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let resolved = Series::from_any_values_and_dtype(
            name.clone().into(),
            &values,
            current_column.dtype(),
            true,
        )
        .map_err(|error| format!("No se pudo conservar el tipo de '{name}': {error}"))?
        .into_column();
        resolved_columns.push(resolved);
    }
    DataFrame::new(current.height(), resolved_columns)
        .map_err(|error| format!("No se pudo construir el dataset resuelto: {error}"))
}

#[tauri::command]
pub async fn resolve_dataset_conflicts(
    app: AppHandle,
    decisions: Vec<ConflictResolution>,
) -> Result<DatasetPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let (compared_file_name, compared_file_size, compared_frame, key_columns) = {
            let comparison = state
                .comparison
                .lock()
                .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?;
            let pending = comparison
                .as_ref()
                .ok_or_else(|| "No hay una comparación activa para resolver.".to_owned())?;
            (
                pending.file_name.clone(),
                pending.file_size_bytes,
                pending.frame.clone(),
                pending.key_columns.clone(),
            )
        };
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let resolved =
            resolved_conflict_frame(&dataset.frame, &compared_frame, &key_columns, &decisions)?;
        let file_name = format!("Resuelto · {} + {compared_file_name}", dataset.file_name);
        let file_size_bytes = dataset.file_size_bytes.saturating_add(compared_file_size);
        let preview = dataset_preview_with_size(&file_name, file_size_bytes, &resolved)?;
        dataset
            .history
            .record(&resolved, "Resolver conflictos por clave")?;
        dataset.source_path = None;
        dataset.file_name = file_name;
        dataset.file_size_bytes = file_size_bytes;
        dataset.frame = resolved;
        dataset.profile = None;
        drop(current);
        *state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? = None;
        Ok(preview)
    })
    .await
    .map_err(|error| format!("La resolución de conflictos se interrumpió: {error}"))?
}

#[tauri::command]
pub fn clear_dataset_comparison(state: State<'_, DatasetState>) -> Result<(), String> {
    *state
        .comparison
        .lock()
        .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? = None;
    Ok(())
}

#[tauri::command]
pub fn use_consolidated_dataset(state: State<'_, DatasetState>) -> Result<DatasetPreview, String> {
    let (compared_file_name, compared_file_size, compared_frame, key_columns) = {
        let comparison = state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?;
        let pending = comparison
            .as_ref()
            .ok_or_else(|| "No hay un dataset comparado listo para consolidar.".to_owned())?;
        (
            pending.file_name.clone(),
            pending.file_size_bytes,
            pending.frame.clone(),
            pending.key_columns.clone(),
        )
    };
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if dataset.frame.get_column_names() != compared_frame.get_column_names()
        || dataset
            .frame
            .columns()
            .iter()
            .zip(compared_frame.columns())
            .any(|(left, right)| left.dtype() != right.dtype())
    {
        return Err(
            "Los esquemas no son compatibles. La consolidación requiere las mismas columnas y tipos."
            .to_owned(),
        );
    }
    if !key_columns.is_empty() {
        let shared_columns = dataset
            .frame
            .get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        let key_summary = compare_keyed_frames(
            &dataset.frame,
            &compared_frame,
            &key_columns,
            &shared_columns,
        )?;
        if key_summary.conflicting_key_count > 0 || key_summary.duplicate_key_count > 0 {
            return Err(
                "No se pueden consolidar claves con conflictos o duplicados. Revisa la comparación antes de continuar."
                    .to_owned(),
            );
        }
    }
    let additions = if key_columns.is_empty() {
        compared_frame.clone()
    } else {
        rows_with_new_keys(&dataset.frame, &compared_frame, &key_columns)?
    };
    let mut consolidated = dataset.frame.clone();
    consolidated
        .vstack_mut(&additions)
        .map_err(|error| format!("No se pudieron unir los datasets: {error}"))?;
    let file_name = format!("Consolidado · {} + {compared_file_name}", dataset.file_name);
    let file_size_bytes = dataset.file_size_bytes.saturating_add(compared_file_size);
    let preview = dataset_preview_with_size(&file_name, file_size_bytes, &consolidated)?;
    dataset
        .history
        .record(&consolidated, "Consolidar datasets")?;
    dataset.source_path = None;
    dataset.file_name = file_name;
    dataset.file_size_bytes = file_size_bytes;
    dataset.frame = consolidated;
    dataset.profile = None;
    drop(current);
    *state
        .comparison
        .lock()
        .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? = None;
    Ok(preview)
}

#[tauri::command]
pub async fn pick_dataset_source(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter(
            "Datasets compatibles",
            &[
                "csv", "tsv", "txt", "json", "jsonl", "ndjson", "parquet", "xlsx", "xls", "xlsb",
                "ods",
            ],
        )
        .blocking_pick_file();

    let Some(selection) = selection else {
        return Ok(None);
    };

    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    inspect_dataset_path(&app, path).await.map(Some)
}

async fn inspect_dataset_path(
    app: &AppHandle,
    path: PathBuf,
) -> Result<DatasetSourceInspection, String> {
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let sheets = if spreadsheet_extensions(&extension) {
        let path = path.clone();
        tauri::async_runtime::spawn_blocking(move || inspect_workbook(&path))
            .await
            .map_err(|error| format!("La inspección del libro se interrumpió: {error}"))??
    } else {
        Vec::new()
    };
    let state = app.state::<DatasetState>();
    let selection_id = format!("selection-{}", state.begin_load());
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    let format = if spreadsheet_extensions(&extension) {
        "excel"
    } else if extension == "parquet" {
        "parquet"
    } else if matches!(extension.as_str(), "json" | "jsonl" | "ndjson") {
        "json"
    } else if extension == "tsv" {
        "tsv"
    } else {
        "csv"
    };
    let workbook_sheets = sheets
        .iter()
        .enumerate()
        .map(|(index, name)| WorkbookSheet {
            id: index.to_string(),
            name: name.clone(),
        })
        .collect();
    *state
        .pending_selection
        .lock()
        .map_err(|_| "La selección local quedó bloqueada inesperadamente.".to_owned())? =
        Some(PendingSelection {
            id: selection_id.clone(),
            path,
            file_size_bytes,
            sheets,
        });
    Ok(DatasetSourceInspection {
        selection_id,
        file_name,
        file_size_bytes,
        format,
        sheets: workbook_sheets,
        default_sheet_id: if spreadsheet_extensions(&extension) {
            Some("0".to_owned())
        } else {
            None
        },
        is_compressed_container: matches!(extension.as_str(), "xlsx" | "xlsb" | "ods"),
    })
}

/// Consumes the path captured by Tauri's native drag/drop event. The path never
/// crosses the IPC boundary; React receives only the resulting inspection.
#[tauri::command]
pub async fn inspect_dropped_dataset(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    let Some(path) = app.state::<DatasetState>().take_dropped_path()? else {
        return Ok(None);
    };
    inspect_dataset_path(&app, path).await.map(Some)
}

#[tauri::command]
pub async fn load_dataset_selection(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
    on_progress: Channel<OperationProgress>,
) -> Result<DatasetPreview, String> {
    let generation = app.state::<DatasetState>().begin_load();
    let pending = {
        let state = app.state::<DatasetState>();
        let selection = state
            .pending_selection
            .lock()
            .map_err(|_| "La selección local quedó bloqueada inesperadamente.".to_owned())?;
        let pending = selection
            .as_ref()
            .cloned()
            .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
        if pending.id != selection_id {
            return Err("La selección no coincide con el archivo pendiente.".to_owned());
        }
        pending
    };
    tauri::async_runtime::spawn_blocking(move || {
        send_progress(&on_progress, "load", "Validando archivo", 10);
        let current_size = validate_dataset_file(&pending.path)?.1;
        if current_size != pending.file_size_bytes {
            return Err(
                "El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned(),
            );
        }
        let extension = dataset_extension(&pending.path)?;
        let (frame, preview) = if spreadsheet_extensions(&extension) {
            let header_mode = header_mode
                .ok_or_else(|| "Elige cómo interpretar los encabezados del libro.".to_owned())?;
            let index = sheet_id
                .as_deref()
                .ok_or_else(|| "Selecciona una hoja del libro.".to_owned())?
                .parse::<usize>()
                .map_err(|_| "La hoja seleccionada no es válida.".to_owned())?;
            let sheet_name = pending
                .sheets
                .get(index)
                .ok_or_else(|| "La hoja seleccionada no existe en el libro.".to_owned())?;
            send_progress(&on_progress, "load", "Leyendo hoja", 25);
            let frame = load_spreadsheet_sheet(&pending.path, sheet_name, header_mode)?;
            ensure_not_cancelled(app.state::<DatasetState>().load_was_cancelled(generation))?;
            send_progress(&on_progress, "load", "Preparando vista previa", 85);
            let preview = dataset_preview(&pending.path, &frame)?;
            (frame, preview)
        } else {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
            }
            if header_mode.is_some() {
                return Err("Este formato no utiliza opciones de encabezado de Excel.".to_owned());
            }
            load_dataset_with_progress(
                &pending.path,
                |stage, percent| send_progress(&on_progress, "load", stage, percent),
                || app.state::<DatasetState>().load_was_cancelled(generation),
            )?
        };
        let state = app.state::<DatasetState>();
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let history = HistoryManager::new(&frame)?;
        *state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())? =
            Some(LoadedDataset {
                source_path: Some(pending.path.clone()),
                file_name: pending
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("dataset.csv")
                    .to_owned(),
                file_size_bytes: pending.file_size_bytes,
                frame,
                profile: None,
                history,
            });
        *state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())? = None;
        let mut selection = state
            .pending_selection
            .lock()
            .map_err(|_| "La selección local quedó bloqueada inesperadamente.".to_owned())?;
        if selection
            .as_ref()
            .is_some_and(|value| value.id == selection_id)
        {
            *selection = None;
        }
        send_progress(&on_progress, "load", "Dataset listo", 100);
        Ok(preview)
    })
    .await
    .map_err(|error| format!("La carga del dataset se interrumpió: {error}"))?
}

#[tauri::command]
pub fn discard_dataset_selection(
    state: State<'_, DatasetState>,
    selection_id: String,
) -> Result<(), String> {
    let mut selection = state
        .pending_selection
        .lock()
        .map_err(|_| "La selección local quedó bloqueada inesperadamente.".to_owned())?;
    if selection
        .as_ref()
        .is_some_and(|pending| pending.id == selection_id)
    {
        *selection = None;
    }
    let _ = state.take_dropped_path();
    Ok(())
}

#[tauri::command]
pub fn get_dataset_page(
    state: State<'_, DatasetState>,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    let current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_ref().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;

    dataset_page(&dataset.frame, offset, limit)
}

#[tauri::command]
pub async fn query_dataset(app: AppHandle, query: String) -> Result<DatasetQueryResult, String> {
    let generation = app.state::<DatasetState>().begin_query();
    let (frame, compared) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let frame = current
            .as_ref()
            .ok_or_else(|| "No hay un dataset activo para consultar.".to_owned())?
            .frame
            .clone();
        let compared = state
            .comparison
            .lock()
            .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?
            .as_ref()
            .map(|pending| pending.frame.clone());
        (frame, compared)
    };
    tauri::async_runtime::spawn_blocking(move || {
        execute_local_query_with_comparison_and_cancel(&frame, compared.as_ref(), &query, &|| {
            app.state::<DatasetState>().query_was_cancelled(generation)
        })
    })
    .await
    .map_err(|error| format!("La consulta local se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn get_dataset_profile(
    app: AppHandle,
    on_progress: Channel<OperationProgress>,
) -> Result<DatasetProfile, String> {
    let generation = app.state::<DatasetState>().begin_profile();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;

        if let Some(profile) = &dataset.profile {
            let has_enough_numeric_columns = profile
                .columns
                .iter()
                .filter(|column| column.outlier_count.is_some())
                .nth(1)
                .is_some();
            let has_group_candidate = profile.columns.iter().any(|column| {
                column.empty_count.is_some()
                    && column.suggested_type.is_none()
                    && column.privacy_signal.is_none()
                    && column.unique_count >= 2
            });
            let has_temporal_candidate = profile.columns.iter().any(|column| {
                (column.data_type == "Date" || column.data_type.starts_with("Datetime"))
                    || column.suggested_type.as_deref() == Some("date")
            });
            if (profile.numeric_correlations.is_some() || !has_enough_numeric_columns)
                && (profile.categorical_group_summaries.is_some() || !has_group_candidate)
                && (profile.temporal_series.is_some() || !has_temporal_candidate)
            {
                send_progress(&on_progress, "profile", "Perfil disponible", 100);
                return Ok(profile.clone());
            }
        }

        let profile = profile_dataset_with_progress(
            &dataset.frame,
            |stage, percent| send_progress(&on_progress, "profile", stage, percent),
            || {
                app.state::<DatasetState>()
                    .profile_was_cancelled(generation)
            },
        )?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    })
    .await
    .map_err(|error| format!("El análisis de calidad se interrumpió: {error}"))?
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, DatasetState>, operation: String) -> Result<(), String> {
    state.cancel(&operation)
}

#[tauri::command]
pub async fn validate_quality_rules(
    app: AppHandle,
    quality_rules: Vec<QualityRule>,
) -> Result<QualityValidationResult, String> {
    validate_quality_rules_payload(&quality_rules)?;
    let frame = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        current
            .as_ref()
            .ok_or_else(|| {
                "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
            })?
            .frame
            .clone()
    };
    tauri::async_runtime::spawn_blocking(move || evaluate_quality_rules(&frame, &quality_rules))
        .await
        .map_err(|error| format!("La validación de calidad se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn export_dataset(
    app: AppHandle,
    format: ExportFormat,
    quality_rules: Vec<QualityRule>,
    allow_unvalidated: bool,
    privacy_mode: PrivacyMode,
    recipe: Option<StoredTransformRecipe>,
    on_progress: Channel<OperationProgress>,
) -> Result<Option<ExportResult>, String> {
    validate_quality_rules_payload(&quality_rules)?;
    if let Some(recipe) = recipe.as_ref() {
        validate_stored_recipe(recipe)?;
    }
    let (frame, suggested_name) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let stem = Path::new(&dataset.file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("dataset");
        (
            dataset.frame.clone(),
            format!("{stem}-columnia.{}", format.extension()),
        )
    };

    // La compuerta se evalúa sobre el mismo snapshot que después será escrito y
    // antes de abrir el selector, para que una exportación bloqueada no solicite destino.
    let generation = app.state::<DatasetState>().begin_export();
    let validation_app = app.clone();
    let (frame, suggested_name, quality_validation) =
        tauri::async_runtime::spawn_blocking(move || {
            let quality_validation = enforce_export_quality_with_cancel(
                &frame,
                &quality_rules,
                allow_unvalidated,
                || {
                    validation_app
                        .state::<DatasetState>()
                        .export_was_cancelled(generation)
                },
            )?;
            ensure_not_cancelled(
                validation_app
                    .state::<DatasetState>()
                    .export_was_cancelled(generation),
            )?;
            Ok::<_, String>((frame, suggested_name, quality_validation))
        })
        .await
        .map_err(|error| {
            format!("La validación previa a la exportación se interrumpió: {error}")
        })??;
    ensure_not_cancelled(app.state::<DatasetState>().export_was_cancelled(generation))?;

    send_progress(&on_progress, "export", "Esperando destino", 0);
    let selection = app
        .dialog()
        .file()
        .add_filter(format.label(), &[format.extension()])
        .set_file_name(suggested_name)
        .blocking_save_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let destination = path_with_extension(
        selection
            .into_path()
            .map_err(|error| format!("No se pudo resolver el destino: {error}"))?,
        format,
    );

    let export_state = app.clone();
    let remembered_destination = destination.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        export_frame_atomic_with_privacy_and_quality_and_recipe(
            &frame,
            &destination,
            format,
            privacy_mode,
            quality_validation.as_ref(),
            recipe.as_ref(),
            |stage, percent| send_progress(&on_progress, "export", stage, percent),
            || app.state::<DatasetState>().export_was_cancelled(generation),
        )
        .map(Some)
    })
    .await
    .map_err(|error| format!("La exportación se interrumpió: {error}"))??;
    if result.is_some() {
        export_state
            .state::<DatasetState>()
            .remember_last_export(remembered_destination);
    }
    Ok(result)
}

#[tauri::command]
pub fn open_last_export(state: State<'_, DatasetState>) -> Result<(), String> {
    let path = state.last_export()?;
    let path = canonicalize_existing_file(&path, "la última exportación")
        .map_err(|_| "La última exportación ya no está disponible.".to_owned())?;
    #[cfg(target_os = "linux")]
    let parent = path
        .parent()
        .ok_or_else(|| "La carpeta de la última exportación no está disponible.".to_owned())?;

    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|_| "No se pudo abrir la carpeta de la última exportación.".to_owned())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|_| "No se pudo abrir la carpeta de la última exportación.".to_owned())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|_| "No se pudo abrir la carpeta de la última exportación.".to_owned())?;
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        return Err("Este sistema no permite abrir la carpeta de la exportación.".to_owned());
    }

    Ok(())
}

#[tauri::command]
pub async fn save_transform_recipe(
    app: AppHandle,
    recipe: TransformRecipe,
    name: String,
    migration_report: Option<RecipeMigrationReport>,
    export_options: Option<RecipeExportOptions>,
) -> Result<Option<StoredTransformRecipe>, String> {
    let mut document = build_stored_recipe(recipe, name)?;
    document.migration_report = migration_report;
    document.export_options = export_options;
    validate_stored_recipe(&document)?;
    let suggested_name = recipe_suggested_file_name(&document.name);
    let selection = app
        .dialog()
        .file()
        .add_filter("Receta Columnia o DataPrep", &["json"])
        .set_file_name(suggested_name)
        .blocking_save_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let destination = recipe_path_with_extension(
        selection
            .into_path()
            .map_err(|error| format!("No se pudo resolver el destino de la receta: {error}"))?,
    );
    let saved = document.clone();
    tauri::async_runtime::spawn_blocking(move || save_recipe_atomic(&document, &destination))
        .await
        .map_err(|error| format!("El guardado de la receta se interrumpió: {error}"))??;
    Ok(Some(saved))
}

#[tauri::command]
pub async fn pick_transform_recipe(
    app: AppHandle,
) -> Result<Option<StoredTransformRecipe>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Receta Columnia o DataPrep", &["json"])
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la receta seleccionada: {error}"))?;
    let loaded = tauri::async_runtime::spawn_blocking(move || load_recipe_file(&path))
        .await
        .map_err(|error| format!("La carga de la receta se interrumpió: {error}"))??;
    Ok(Some(loaded))
}

#[tauri::command]
pub async fn save_quality_rules_document(
    app: AppHandle,
    quality_rules: Vec<QualityRule>,
) -> Result<Option<QualityRulesDocument>, String> {
    let document = build_quality_rules_document(quality_rules)?;
    let selection = app
        .dialog()
        .file()
        .add_filter("Contrato de calidad Columnia", &["json"])
        .set_file_name("columnia-quality-rules-v1.json")
        .blocking_save_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let destination = recipe_path_with_extension(
        selection
            .into_path()
            .map_err(|error| format!("No se pudo resolver el destino del contrato: {error}"))?,
    );
    let saved = document.clone();
    tauri::async_runtime::spawn_blocking(move || {
        save_quality_rules_atomic(&document, &destination)
    })
    .await
    .map_err(|error| format!("El guardado del contrato se interrumpió: {error}"))??;
    Ok(Some(saved))
}

#[tauri::command]
pub async fn pick_quality_rules_migration(
    app: AppHandle,
) -> Result<Option<QualityMigrationResult>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Contrato de calidad", &["json"])
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver el contrato seleccionado: {error}"))?;
    let loaded = tauri::async_runtime::spawn_blocking(move || load_quality_migration_file(&path))
        .await
        .map_err(|error| format!("La migración del contrato se interrumpió: {error}"))??;
    Ok(Some(loaded))
}

#[tauri::command]
pub async fn pick_dataprep_session_migration(
    app: AppHandle,
) -> Result<Option<DataprepSessionMigrationPlan>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Sesión DataPrep", &["json"])
        .blocking_pick_file();
    let Some(selection) = selection else {
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la sesión seleccionada: {error}"))?;
    let loaded =
        tauri::async_runtime::spawn_blocking(move || load_dataprep_session_migration_plan(&path))
            .await
            .map_err(|error| format!("La inspección de la sesión se interrumpió: {error}"))??;
    Ok(Some(loaded))
}

#[tauri::command]
pub async fn remove_duplicates(app: AppHandle) -> Result<DatasetMutation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count) = remove_duplicate_rows(&dataset.frame)?;

        let preview = if affected_row_count > 0 {
            publish_candidate(dataset, cleaned, "Eliminar filas duplicadas")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };

        Ok(DatasetMutation {
            dataset: preview,
            affected_row_count,
        })
    })
    .await
    .map_err(|error| format!("La eliminación de duplicados se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn remove_near_duplicates(app: AppHandle) -> Result<DatasetMutation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count) = remove_near_duplicate_rows(&dataset.frame)?;

        let preview = if affected_row_count > 0 {
            publish_candidate(dataset, cleaned, "Eliminar filas duplicadas parecidas")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };

        Ok(DatasetMutation {
            dataset: preview,
            affected_row_count,
        })
    })
    .await
    .map_err(|error| format!("La eliminación de duplicados parecidos se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn remove_empty_rows(app: AppHandle) -> Result<DatasetMutation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count) = remove_empty_rows_from_frame(&dataset.frame)?;
        let preview = if affected_row_count > 0 {
            publish_candidate(dataset, cleaned, "Eliminar filas completamente vacías")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(DatasetMutation {
            dataset: preview,
            affected_row_count,
        })
    })
    .await
    .map_err(|error| format!("La eliminación de filas vacías se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn enable_row_audit(app: AppHandle) -> Result<DatasetMutation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (candidate, added) = add_audit_column_to_frame(&dataset.frame)?;
        let preview = if added {
            publish_candidate(dataset, candidate, "Activar trazabilidad por fila")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(DatasetMutation {
            dataset: preview,
            affected_row_count: 0,
        })
    })
    .await
    .map_err(|error| format!("La activación de trazabilidad se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn remove_constant_columns(app: AppHandle) -> Result<ColumnRemovalResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, removed_columns) = remove_constant_columns_from_frame(&dataset.frame)?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate(dataset, cleaned, "Eliminar columnas constantes")?
        };
        Ok(ColumnRemovalResult {
            dataset: preview,
            removed_column_count: removed_columns.len(),
            removed_columns,
        })
    })
    .await
    .map_err(|error| format!("La eliminación de columnas constantes se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn remove_empty_columns(app: AppHandle) -> Result<ColumnRemovalResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, removed_columns) = remove_empty_columns_from_frame(&dataset.frame)?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate(dataset, cleaned, "Eliminar columnas completamente vacías")?
        };
        Ok(ColumnRemovalResult {
            dataset: preview,
            removed_column_count: removed_columns.len(),
            removed_columns,
        })
    })
    .await
    .map_err(|error| format!("La eliminación de columnas vacías se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn remove_high_null_columns(app: AppHandle) -> Result<ColumnRemovalResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, removed_columns) = remove_high_null_columns_from_frame(&dataset.frame)?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate(dataset, cleaned, "Eliminar columnas con alta nulidad")?
        };
        Ok(ColumnRemovalResult {
            dataset: preview,
            removed_column_count: removed_columns.len(),
            removed_columns,
        })
    })
    .await
    .map_err(|error| {
        format!("La eliminación de columnas con alta nulidad se interrumpió: {error}")
    })?
}

#[tauri::command]
pub async fn remove_identifier_columns(app: AppHandle) -> Result<ColumnRemovalResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&dataset.frame)?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate(dataset, cleaned, "Retirar columnas identificadoras")?
        };
        Ok(ColumnRemovalResult {
            dataset: preview,
            removed_column_count: removed_columns.len(),
            removed_columns,
        })
    })
    .await
    .map_err(|error| {
        format!("La eliminación de columnas identificadoras se interrumpió: {error}")
    })?
}

#[tauri::command]
pub async fn remove_personal_columns(app: AppHandle) -> Result<ColumnRemovalResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, removed_columns) = remove_personal_columns_from_frame(&dataset.frame)?;
        let removed_column_count = removed_columns.len();
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate(dataset, cleaned, "Retirar datos personales detectados")?
        };
        Ok(ColumnRemovalResult {
            dataset: preview,
            removed_column_count,
            // Personal-column names stay in the native operation only; the IPC result is aggregate.
            removed_columns: Vec::new(),
        })
    })
    .await
    .map_err(|error| {
        format!("La eliminación de columnas con datos personales se interrumpió: {error}")
    })?
}

#[tauri::command]
pub async fn mask_personal_values(app: AppHandle) -> Result<PersonalDataMaskResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (masked, changed_cell_count, changed_column_count) =
            mask_personal_values_from_frame(&dataset.frame)?;
        let preview = if changed_cell_count > 0 {
            publish_candidate(dataset, masked, "Proteger valores personales detectados")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(PersonalDataMaskResult {
            dataset: preview,
            changed_cell_count,
            changed_column_count,
        })
    })
    .await
    .map_err(|error| format!("La protección de datos personales se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_column_names(app: AppHandle) -> Result<ColumnNormalizationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (names, renames) = normalized_column_names(&dataset.frame);

        let preview = if !renames.is_empty() {
            let mut candidate = dataset.frame.clone();
            candidate
                .set_column_names(&names)
                .map_err(|error| format!("No se pudieron normalizar las columnas: {error}"))?;
            publish_candidate(dataset, candidate, "Normalizar nombres de columnas")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };

        Ok(ColumnNormalizationResult {
            dataset: preview,
            renamed_column_count: renames.len(),
            renames,
        })
    })
    .await
    .map_err(|error| format!("La normalización de columnas se interrumpió: {error}"))?
}

fn apply_text_cleaning(
    app: AppHandle,
    selected_columns: Option<Vec<String>>,
    mode: TextCleaningMode,
) -> Result<TextCleaningResult, String> {
    let state = app.state::<DatasetState>();
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
        clean_text_columns(&dataset.frame, selected_columns.as_deref(), mode)?;

    let label = match mode {
        TextCleaningMode::Trim => "Recortar espacios",
        TextCleaningMode::Normalize { .. } => "Normalizar texto",
        TextCleaningMode::Sentinels => "Normalizar valores centinela",
        TextCleaningMode::Booleans => "Normalizar booleanos",
        TextCleaningMode::FixEncoding => "Corregir codificación UTF-8",
        TextCleaningMode::NullifyInvalidTypes => "Apartar tipos incompatibles",
    };
    let preview = if changed_cell_count > 0 {
        publish_candidate(dataset, cleaned, label)?
    } else {
        loaded_dataset_preview(dataset, &dataset.frame)?
    };

    Ok(TextCleaningResult {
        dataset: preview,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    })
}

fn apply_dataprep_date_parsing(app: AppHandle) -> Result<TextCleaningResult, String> {
    let state = app.state::<DatasetState>();
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let (parsed, affected_row_count, changed_cell_count, changed_columns) =
        parse_dataprep_date_columns(&dataset.frame)?;
    let preview = if changed_cell_count > 0 {
        publish_candidate(dataset, parsed, "Interpretar fechas detectadas")?
    } else {
        loaded_dataset_preview(dataset, &dataset.frame)?
    };

    Ok(TextCleaningResult {
        dataset: preview,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    })
}

fn apply_dataprep_numeric_cast(app: AppHandle) -> Result<TextCleaningResult, String> {
    let state = app.state::<DatasetState>();
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let (cast, affected_row_count, changed_cell_count, changed_columns) =
        cast_dataprep_numeric_columns(&dataset.frame)?;
    let preview = if changed_cell_count > 0 {
        publish_candidate(dataset, cast, "Convertir números detectados")?
    } else {
        loaded_dataset_preview(dataset, &dataset.frame)?
    };

    Ok(TextCleaningResult {
        dataset: preview,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    })
}

#[tauri::command]
pub async fn trim_text_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Trim)
    })
    .await
    .map_err(|error| format!("La limpieza de espacios se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_text_values(
    app: AppHandle,
    columns: Vec<String>,
    remove_accents: bool,
) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(
            app,
            Some(columns),
            TextCleaningMode::Normalize { remove_accents },
        )
    })
    .await
    .map_err(|error| format!("La normalización de texto se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn parse_date_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || apply_dataprep_date_parsing(app))
        .await
        .map_err(|error| format!("La interpretación de fechas se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn cast_numeric_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || apply_dataprep_numeric_cast(app))
        .await
        .map_err(|error| format!("La conversión numérica se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_sentinel_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Sentinels)
    })
    .await
    .map_err(|error| format!("La normalización de valores centinela se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_boolean_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Booleans)
    })
    .await
    .map_err(|error| format!("La normalización de booleanos se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn fix_encoding_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::FixEncoding)
    })
    .await
    .map_err(|error| format!("La corrección de codificación se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn nullify_invalid_type_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::NullifyInvalidTypes)
    })
    .await
    .map_err(|error| format!("La corrección de tipos incompatibles se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn impute_missing_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_missing_values_in_frame(&dataset.frame)?;
        let preview = if changed_cell_count > 0 {
            publish_candidate(dataset, cleaned, "Imputación conservadora")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(TextCleaningResult {
            dataset: preview,
            affected_row_count,
            changed_cell_count,
            changed_columns,
        })
    })
    .await
    .map_err(|error| format!("La imputación de valores nulos se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn impute_categorical_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_categorical_values_in_frame(&dataset.frame)?;
        let preview = if changed_cell_count > 0 {
            publish_candidate(dataset, cleaned, "Imputación categórica")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(TextCleaningResult {
            dataset: preview,
            affected_row_count,
            changed_cell_count,
            changed_columns,
        })
    })
    .await
    .map_err(|error| format!("La imputación categórica se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn impute_outlier_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_outlier_values_in_frame(&dataset.frame)?;
        let preview = if changed_cell_count > 0 {
            publish_candidate(dataset, cleaned, "Imputación de outliers")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };
        Ok(TextCleaningResult {
            dataset: preview,
            affected_row_count,
            changed_cell_count,
            changed_columns,
        })
    })
    .await
    .map_err(|error| format!("La imputación de outliers se interrumpió: {error}"))?
}

fn apply_direct_outlier_mode(
    app: AppHandle,
    mode: DataprepOutlierMode,
    label: &'static str,
) -> Result<TextCleaningResult, String> {
    let state = app.state::<DatasetState>();
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
        apply_dataprep_outlier_mode(&dataset.frame, mode)?;
    let preview = if changed_cell_count > 0 || affected_row_count > 0 {
        publish_candidate(dataset, cleaned, label)?
    } else {
        loaded_dataset_preview(dataset, &dataset.frame)?
    };
    Ok(TextCleaningResult {
        dataset: preview,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    })
}

#[tauri::command]
pub async fn cap_outlier_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_direct_outlier_mode(app, DataprepOutlierMode::Cap, "Limitar outliers con IQR")
    })
    .await
    .map_err(|error| format!("La limitación de outliers se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn drop_outlier_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        apply_direct_outlier_mode(app, DataprepOutlierMode::Drop, "Eliminar filas atípicas")
    })
    .await
    .map_err(|error| format!("La eliminación de outliers se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn apply_safe_corrections(app: AppHandle) -> Result<SafeCorrectionsResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;

        let (candidate, affected_row_count, changed_cell_count, renames) =
            safe_corrected_frame(&dataset.frame)?;
        let renamed_column_count = renames.len();

        let preview = if changed_cell_count > 0 || renamed_column_count > 0 {
            publish_candidate(dataset, candidate, "Aplicar correcciones recomendadas")?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };

        Ok(SafeCorrectionsResult {
            dataset: preview,
            changed_cell_count,
            affected_row_count,
            renamed_column_count,
            renames,
        })
    })
    .await
    .map_err(|error| format!("Las correcciones recomendadas se interrumpieron: {error}"))?
}

#[tauri::command]
pub async fn undo_last_change(app: AppHandle) -> Result<HistoryResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        undo_dataset(dataset)
    })
    .await
    .map_err(|error| format!("No se pudo deshacer el cambio: {error}"))?
}

fn undo_dataset(dataset: &mut LoadedDataset) -> Result<HistoryResult, String> {
    if !dataset.history.state().can_undo {
        return Err("No hay un cambio disponible para deshacer.".to_owned());
    }
    let target = dataset.history.cursor - 1;
    let previous = dataset.history.restore(target)?;
    let preview = loaded_dataset_preview(dataset, &previous)?;
    dataset.frame = previous;
    dataset.history.cursor = target;
    dataset.profile = None;
    Ok(HistoryResult {
        dataset: preview,
        history: dataset.history.state(),
        message: "Se deshizo el último cambio.".to_owned(),
    })
}

#[tauri::command]
pub async fn redo_last_change(app: AppHandle) -> Result<HistoryResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        redo_dataset(dataset)
    })
    .await
    .map_err(|error| format!("No se pudo rehacer el cambio: {error}"))?
}

fn redo_dataset(dataset: &mut LoadedDataset) -> Result<HistoryResult, String> {
    if !dataset.history.state().can_redo {
        return Err("No hay un cambio disponible para rehacer.".to_owned());
    }
    let target = dataset.history.cursor + 1;
    let next = dataset.history.restore(target)?;
    let preview = loaded_dataset_preview(dataset, &next)?;
    dataset.frame = next;
    dataset.history.cursor = target;
    dataset.profile = None;
    Ok(HistoryResult {
        dataset: preview,
        history: dataset.history.state(),
        message: "Se rehízo el último cambio.".to_owned(),
    })
}

#[tauri::command]
pub fn get_history_state(state: State<'_, DatasetState>) -> Result<HistoryState, String> {
    let current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_ref().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    Ok(dataset.history.state())
}

fn strict_column_text(column: &Column) -> Result<Vec<Option<String>>, String> {
    (0..column.len())
        .map(|row| {
            column
                .get(row)
                .map(preview_value)
                .map_err(|error| format!("No se pudo leer la fila {}: {error}", row + 1))
        })
        .collect()
}

fn recipe_column<'a>(frame: &'a DataFrame, name: &str) -> Result<&'a Column, String> {
    frame
        .column(name)
        .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))
}

fn strict_cast_column(column: &Column, target: RecipeCastTarget) -> Result<Column, String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let invalid = |row: usize, value: &str, target: &str| {
        format!(
            "La columna '{}' no se puede convertir a {target}: fila {}, valor '{}'.",
            name,
            row + 1,
            value
        )
    };

    match target {
        RecipeCastTarget::String => Ok(Series::new(name, values).into_column()),
        RecipeCastTarget::Integer => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| {
                            value
                                .trim()
                                .parse::<i64>()
                                .map_err(|_| invalid(row, value, "entero"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
        RecipeCastTarget::Decimal => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| {
                            value
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .filter(|number| number.is_finite())
                                .ok_or_else(|| invalid(row, value, "decimal"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
        RecipeCastTarget::Boolean => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                            "true" => Ok(true),
                            "false" => Ok(false),
                            _ => Err(invalid(row, value, "booleano (true/false)")),
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
    }
}

fn parse_recipe_datetime(value: &str, format: RecipeDateFormat) -> Result<NaiveDateTime, ()> {
    let value = value.trim();
    let date_format = match format {
        RecipeDateFormat::Ymd => Some("%Y-%m-%d"),
        RecipeDateFormat::Dmy => Some("%d/%m/%Y"),
        RecipeDateFormat::Mdy => Some("%m/%d/%Y"),
        RecipeDateFormat::Iso8601 => None,
    };
    if let Some(format) = date_format {
        return NaiveDate::parse_from_str(value, format)
            .ok()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
            .ok_or(());
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0).ok_or(());
    }
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.naive_utc())
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map_err(|_| ())
}

fn strict_date_column(
    column: &Column,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
) -> Result<Column, String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let parsed = values
        .iter()
        .enumerate()
        .map(|(row, value)| {
            value
                .as_deref()
                .map(|value| {
                    parse_recipe_datetime(value, format).map_err(|_| {
                        format!(
                            "La columna '{}' contiene una fecha inválida en la fila {}: '{}'.",
                            name,
                            row + 1,
                            value
                        )
                    })
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    match target {
        RecipeDateTarget::Date => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
            let days = parsed
                .into_iter()
                .map(|value| value.map(|value| (value.date() - epoch).num_days() as i32))
                .collect::<Vec<_>>();
            Ok(Series::new(name, days)
                .cast(&polars::prelude::DataType::Date)
                .map_err(|error| format!("No se pudo crear la columna de fecha: {error}"))?
                .into_column())
        }
        RecipeDateTarget::Datetime => {
            let milliseconds = parsed
                .into_iter()
                .map(|value| value.map(|value| value.and_utc().timestamp_millis()))
                .collect::<Vec<_>>();
            Ok(Series::new(name, milliseconds)
                .cast(&polars::prelude::DataType::Datetime(
                    TimeUnit::Milliseconds,
                    None,
                ))
                .map_err(|error| format!("No se pudo crear la columna de fecha y hora: {error}"))?
                .into_column())
        }
    }
}

fn remapped_name<'a>(name: &'a str, renames: &HashMap<&'a str, &'a str>) -> &'a str {
    renames.get(name).copied().unwrap_or(name)
}

fn strict_f64(value: &str, context: &str) -> Result<f64, String> {
    let trimmed = value.trim();
    if !trimmed.contains(['.', 'e', 'E'])
        && trimmed
            .parse::<i128>()
            .is_ok_and(|integer| integer.unsigned_abs() > (1_u128 << 53))
    {
        return Err(format!(
            "{context} excede la precisión numérica segura: '{value}'."
        ));
    }
    trimmed
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("{context} debe ser un número finito: '{value}'."))
}

fn apply_recipe_filters(
    frame: DataFrame,
    filters: &[RecipeFilter],
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize), String> {
    if filters.len() > 3 {
        return Err("La receta admite como máximo tres filtros combinados con AND.".into());
    }
    let original_height = frame.height();
    let mut combined_mask = vec![true; original_height];
    for filter in filters {
        let name = remapped_name(&filter.column, renames);
        let values = strict_column_text(recipe_column(&frame, name)?)?;
        let unary = matches!(
            filter.operator,
            RecipeFilterOperator::IsNull | RecipeFilterOperator::NotNull
        );
        if unary != filter.value.is_none() {
            return Err(format!(
                "El filtro '{}' {} un valor.",
                filter.column,
                if unary { "no acepta" } else { "requiere" }
            ));
        }
        let literal = filter.value.as_deref().unwrap_or_default();
        if matches!(
            filter.operator,
            RecipeFilterOperator::Gt
                | RecipeFilterOperator::Lt
                | RecipeFilterOperator::Gte
                | RecipeFilterOperator::Lte
                | RecipeFilterOperator::Contains
                | RecipeFilterOperator::NotContains
        ) && literal.is_empty()
        {
            return Err(format!(
                "El filtro '{}' requiere un valor no vacío.",
                filter.column
            ));
        }

        let numeric_literal = if matches!(
            filter.operator,
            RecipeFilterOperator::Gt
                | RecipeFilterOperator::Lt
                | RecipeFilterOperator::Gte
                | RecipeFilterOperator::Lte
        ) {
            if matches!(
                recipe_column(&frame, name)?.dtype(),
                polars::prelude::DataType::Date | polars::prelude::DataType::Datetime(_, _)
            ) {
                return Err(format!(
                    "La comparación numérica de '{name}' no admite fechas en este hito."
                ));
            }
            Some(strict_f64(literal, "El valor del filtro")?)
        } else {
            None
        };
        let numeric_values = if numeric_literal.is_some() {
            Some(
                values
                    .iter()
                    .enumerate()
                    .map(|(row, value)| {
                        value
                            .as_deref()
                            .map(|value| {
                                strict_f64(
                                    value,
                                    &format!("La fila {} de la columna '{name}'", row + 1),
                                )
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            None
        };
        let needle = literal.to_lowercase();
        let mask = values
            .iter()
            .enumerate()
            .map(|(row, value)| match filter.operator {
                RecipeFilterOperator::IsNull => value.is_none(),
                RecipeFilterOperator::NotNull => value.is_some(),
                RecipeFilterOperator::Eq => value.as_deref() == Some(literal),
                RecipeFilterOperator::Neq => value.as_deref().is_some_and(|value| value != literal),
                RecipeFilterOperator::Contains => value
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&needle)),
                RecipeFilterOperator::NotContains => value
                    .as_deref()
                    .is_some_and(|value| !value.to_lowercase().contains(&needle)),
                RecipeFilterOperator::Gt => numeric_values.as_ref().unwrap()[row]
                    .is_some_and(|value| value > numeric_literal.unwrap()),
                RecipeFilterOperator::Lt => numeric_values.as_ref().unwrap()[row]
                    .is_some_and(|value| value < numeric_literal.unwrap()),
                RecipeFilterOperator::Gte => numeric_values.as_ref().unwrap()[row]
                    .is_some_and(|value| value >= numeric_literal.unwrap()),
                RecipeFilterOperator::Lte => numeric_values.as_ref().unwrap()[row]
                    .is_some_and(|value| value <= numeric_literal.unwrap()),
            })
            .collect::<Vec<_>>();
        combined_mask
            .iter_mut()
            .zip(mask)
            .for_each(|(combined, current)| *combined &= current);
    }
    let filtered = frame
        .filter(&BooleanChunked::from_slice("filter".into(), &combined_mask))
        .map_err(|error| format!("No se pudieron aplicar los filtros: {error}"))?;
    let removed = original_height.saturating_sub(filtered.height());
    Ok((filtered, removed))
}

fn date_parts(column: &Column, operation: CalculatedOperation) -> Result<Vec<Option<i32>>, String> {
    let name = column.name();
    match column.dtype() {
        polars::prelude::DataType::Date => {
            let physical = column
                .cast(&polars::prelude::DataType::Int32)
                .map_err(|error| error.to_string())?;
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
            Ok((0..physical.len())
                .map(|row| {
                    let days = match physical.get(row).map_err(|error| error.to_string())? {
                        AnyValue::Null => None,
                        AnyValue::Int32(value) => Some(value),
                        _ => {
                            return Err("La fecha no tiene una representación física válida.".into())
                        }
                    };
                    let date = days
                        .map(|days| {
                            epoch
                                .checked_add_signed(chrono::Duration::days(days.into()))
                                .ok_or_else(|| {
                                    format!(
                                        "La fecha de la fila {} está fuera del rango admitido.",
                                        row + 1
                                    )
                                })
                        })
                        .transpose()?;
                    Ok(date.map(|date| match operation {
                        CalculatedOperation::Year => date.year(),
                        CalculatedOperation::Month => date.month() as i32,
                        CalculatedOperation::Day => date.day() as i32,
                        _ => unreachable!(),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?)
        }
        polars::prelude::DataType::Datetime(unit, _) => {
            let physical = column
                .cast(&polars::prelude::DataType::Int64)
                .map_err(|error| error.to_string())?;
            let divisor = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            Ok((0..physical.len())
                .map(|row| {
                    let raw = match physical.get(row).map_err(|error| error.to_string())? {
                        AnyValue::Null => None,
                        AnyValue::Int64(value) => Some(value),
                        _ => {
                            return Err(
                                "La fecha y hora no tiene una representación física válida.".into(),
                            )
                        }
                    };
                    let date = raw
                        .map(|raw| {
                            DateTime::from_timestamp(
                                raw.div_euclid(divisor),
                                (raw.rem_euclid(divisor) as u64 * (1_000_000_000 / divisor as u64))
                                    as u32,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "La fecha y hora de la fila {} está fuera del rango admitido.",
                                    row + 1
                                )
                            })
                        })
                        .transpose()?;
                    Ok(date.map(|date| match operation {
                        CalculatedOperation::Year => date.year(),
                        CalculatedOperation::Month => date.month() as i32,
                        CalculatedOperation::Day => date.day() as i32,
                        _ => unreachable!(),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?)
        }
        _ => Err(format!(
            "La columna '{name}' debe ser date o datetime para extraer componentes."
        )),
    }
}

fn add_calculated_column(
    frame: &mut DataFrame,
    calculation: &CalculatedColumnRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(), String> {
    if calculation.name.trim().is_empty() || calculation.name != calculation.name.trim() {
        return Err(
            "El nombre de la columna calculada no puede estar vacío ni tener espacios exteriores."
                .into(),
        );
    }
    if frame.column(&calculation.name).is_ok() {
        return Err(format!(
            "La columna calculada '{}' ya existe.",
            calculation.name
        ));
    }
    let source_name = remapped_name(&calculation.source, renames);
    let source = recipe_column(frame, source_name)?;
    let unary = matches!(
        calculation.operation,
        CalculatedOperation::Year | CalculatedOperation::Month | CalculatedOperation::Day
    );
    if unary != calculation.operand.is_none() {
        return Err(if unary {
            "year, month y day no aceptan operando.".into()
        } else {
            "La operación calculada requiere un operando.".into()
        });
    }
    let column = if unary {
        Series::new(
            calculation.name.clone().into(),
            date_parts(source, calculation.operation)?,
        )
        .into_column()
    } else {
        let source_values = strict_column_text(source)?;
        let operand = calculation.operand.as_ref().unwrap();
        let operand_values = match operand.kind {
            CalculatedOperandKind::Literal => vec![Some(operand.value.clone()); frame.height()],
            CalculatedOperandKind::Column => strict_column_text(recipe_column(
                frame,
                remapped_name(&operand.value, renames),
            )?)?,
        };
        match calculation.operation {
            CalculatedOperation::Concat => Series::new(
                calculation.name.clone().into(),
                source_values
                    .iter()
                    .zip(&operand_values)
                    .map(|(left, right)| {
                        left.as_ref()
                            .zip(right.as_ref())
                            .map(|(left, right)| format!("{left}{right}"))
                    })
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            CalculatedOperation::Add
            | CalculatedOperation::Subtract
            | CalculatedOperation::Multiply
            | CalculatedOperation::Divide => {
                if operand.kind == CalculatedOperandKind::Literal && operand.value.is_empty() {
                    return Err("El operando numérico no puede estar vacío.".into());
                }
                let values = source_values
                    .iter()
                    .zip(&operand_values)
                    .enumerate()
                    .map(|(row, (left, right))| {
                        left.as_deref()
                            .zip(right.as_deref())
                            .map(|(left, right)| {
                                let left = strict_f64(
                                    left,
                                    &format!("La fila {} de '{source_name}'", row + 1),
                                )?;
                                let right = strict_f64(
                                    right,
                                    &format!("El operando de la fila {}", row + 1),
                                )?;
                                if calculation.operation == CalculatedOperation::Divide
                                    && right == 0.0
                                {
                                    return Err(format!(
                                        "División por cero en la fila {}.",
                                        row + 1
                                    ));
                                }
                                let result = match calculation.operation {
                                    CalculatedOperation::Add => left + right,
                                    CalculatedOperation::Subtract => left - right,
                                    CalculatedOperation::Multiply => left * right,
                                    CalculatedOperation::Divide => left / right,
                                    _ => unreachable!(),
                                };
                                if !result.is_finite() {
                                    return Err(format!(
                                        "El cálculo produjo un valor no finito en la fila {}.",
                                        row + 1
                                    ));
                                }
                                Ok(result)
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(calculation.name.clone().into(), values).into_column()
            }
            _ => unreachable!(),
        }
    };
    frame
        .with_column(column)
        .map_err(|error| format!("No se pudo agregar la columna calculada: {error}"))?;
    Ok(())
}

fn apply_find_replace(
    frame: &mut DataFrame,
    recipe: &FindReplaceRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<usize, String> {
    if recipe.find.is_empty() {
        return Err("El texto buscado no puede estar vacío.".into());
    }
    let targets = match recipe.scope {
        FindReplaceScope::Column => {
            let column = recipe
                .column
                .as_deref()
                .ok_or_else(|| "La búsqueda por columna requiere una columna.".to_owned())?;
            vec![remapped_name(column, renames).to_owned()]
        }
        FindReplaceScope::AllTextColumns => {
            if recipe.column.is_some() {
                return Err(
                    "La búsqueda en todas las columnas no acepta una columna concreta.".into(),
                );
            }
            frame
                .columns()
                .iter()
                .filter(|column| column.dtype() == &polars::prelude::DataType::String)
                .map(|column| column.name().to_string())
                .collect()
        }
    };
    let mut count = 0;
    for name in targets {
        let column = recipe_column(frame, &name)?;
        if column.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{name}' debe ser de texto para buscar y reemplazar."
            ));
        }
        let values = strict_column_text(column)?;
        let replaced = values
            .into_iter()
            .map(|value| {
                value.map(|value| {
                    let updated = value.replace(&recipe.find, &recipe.replace);
                    if updated != value {
                        count += 1;
                        updated
                    } else {
                        value
                    }
                })
            })
            .collect::<Vec<_>>();
        frame
            .replace(
                &name,
                Series::new(name.clone().into(), replaced).into_column(),
            )
            .map_err(|error| format!("No se pudo reemplazar texto en '{name}': {error}"))?;
    }
    Ok(count)
}

fn resolve_keep_column_names(
    frame: &DataFrame,
    keep_columns: Option<&[String]>,
    renames: &HashMap<&str, &str>,
) -> Result<(Vec<String>, usize, bool), String> {
    let Some(keep_columns) = keep_columns else {
        return Ok((Vec::new(), 0, false));
    };
    if keep_columns.is_empty() {
        return Err("Debes conservar al menos una columna.".into());
    }
    let mut seen = HashSet::new();
    let names = keep_columns
        .iter()
        .map(|name| {
            if !seen.insert(name) {
                return Err(format!(
                    "La columna '{name}' aparece más de una vez en la selección."
                ));
            }
            let effective = remapped_name(name, renames).to_owned();
            recipe_column(frame, &effective)?;
            Ok(effective)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let order_changed = frame
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .ne(names.iter().map(String::as_str));
    Ok((
        names,
        frame.width().saturating_sub(keep_columns.len()),
        order_changed,
    ))
}

fn apply_keep_columns(
    frame: DataFrame,
    keep_columns: Option<&[String]>,
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, bool), String> {
    let (names, dropped_count, order_changed) =
        resolve_keep_column_names(&frame, keep_columns, renames)?;
    if keep_columns.is_none() {
        return Ok((frame, 0, false));
    }
    let selected = frame
        .select(&names)
        .map_err(|error| format!("No se pudieron conservar las columnas seleccionadas: {error}"))?;
    Ok((selected, dropped_count, order_changed))
}

fn apply_split_column(
    frame: &mut DataFrame,
    split: &SplitColumnRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if split.delimiter.is_empty() {
        return Err("El delimitador de división no puede estar vacío.".into());
    }
    if !(2..=16).contains(&split.names.len()) {
        return Err("La división requiere entre 2 y 16 columnas de destino.".into());
    }
    let source_name = remapped_name(&split.source, renames);
    let source = recipe_column(frame, source_name)?;
    if source.dtype() != &polars::prelude::DataType::String {
        return Err(format!(
            "La columna '{source_name}' debe ser de texto para dividirse."
        ));
    }
    let mut unique = HashSet::new();
    let names = split
        .names
        .iter()
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed != name {
                return Err(
                    "Los nombres divididos no pueden estar vacíos ni tener espacios exteriores."
                        .into(),
                );
            }
            if !unique.insert(trimmed) {
                return Err(format!("El nombre dividido '{trimmed}' está duplicado."));
            }
            if frame.column(trimmed).is_ok() {
                return Err(format!("La columna dividida '{trimmed}' ya existe."));
            }
            Ok(trimmed.to_owned())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let source_values = strict_column_text(source)?;
    let mut outputs = vec![Vec::with_capacity(frame.height()); names.len()];
    for value in source_values {
        if let Some(value) = value {
            let parts = value
                .splitn(names.len(), &split.delimiter)
                .collect::<Vec<_>>();
            for (index, output) in outputs.iter_mut().enumerate() {
                output.push(parts.get(index).map(|part| (*part).to_owned()));
            }
        } else {
            outputs.iter_mut().for_each(|output| output.push(None));
        }
    }
    for (name, values) in names.into_iter().zip(outputs) {
        frame
            .with_column(Series::new(name.into(), values).into_column())
            .map_err(|error| format!("No se pudo crear una columna dividida: {error}"))?;
    }
    let dropped = if split.drop_source {
        frame
            .drop_in_place(source_name)
            .map_err(|error| format!("No se pudo descartar '{source_name}': {error}"))?;
        1
    } else {
        0
    };
    Ok((split.names.len(), dropped))
}

fn apply_merge_columns(
    frame: &mut DataFrame,
    merge: &MergeColumnsRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if !(2..=16).contains(&merge.sources.len()) {
        return Err("La unión requiere entre 2 y 16 columnas fuente.".into());
    }
    if merge.name.trim().is_empty() || merge.name != merge.name.trim() {
        return Err(
            "El nombre de la columna unida no puede estar vacío ni tener espacios exteriores."
                .into(),
        );
    }
    if frame.column(&merge.name).is_ok() {
        return Err(format!("La columna unida '{}' ya existe.", merge.name));
    }
    let mut unique = HashSet::new();
    let sources = merge
        .sources
        .iter()
        .map(|source| {
            if !unique.insert(source) {
                return Err(format!("La columna fuente '{source}' está duplicada."));
            }
            let effective = remapped_name(source, renames).to_owned();
            let column = recipe_column(frame, &effective)?;
            if column.dtype() != &polars::prelude::DataType::String {
                return Err(format!(
                    "La columna '{effective}' debe ser de texto para unirse."
                ));
            }
            Ok(effective)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let columns = sources
        .iter()
        .map(|name| strict_column_text(recipe_column(frame, name)?))
        .collect::<Result<Vec<_>, _>>()?;
    let merged = (0..frame.height())
        .map(|row| {
            let values = columns
                .iter()
                .filter_map(|column| column[row].as_deref())
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(&merge.separator))
        })
        .collect::<Vec<_>>();
    frame
        .with_column(Series::new(merge.name.clone().into(), merged).into_column())
        .map_err(|error| format!("No se pudo crear la columna unida: {error}"))?;
    let dropped = if merge.drop_sources {
        for source in &sources {
            frame
                .drop_in_place(source)
                .map_err(|error| format!("No se pudo descartar '{source}': {error}"))?;
        }
        sources.len()
    } else {
        0
    };
    Ok((1, dropped))
}

fn outlier_linear_quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

fn physical_numeric_values(column: &Column) -> Result<Vec<Option<f64>>, String> {
    let name = column.name();
    (0..column.len())
        .map(|row| {
            let value = column.get(row).map_err(|error| error.to_string())?;
            match value {
                AnyValue::Null => Ok(None),
                AnyValue::Int64(value) if value.unsigned_abs() <= (1_u64 << 53) => Ok(Some(value as f64)),
                AnyValue::Int64(value) => Err(format!("La columna '{name}' contiene un entero fuera de la precisión segura en la fila {}: {value}.", row + 1)),
                AnyValue::Float64(value) if value.is_finite() => Ok(Some(value)),
                AnyValue::Float64(_) => Err(format!(
                    "La columna '{name}' contiene NaN o infinito en la fila {}.",
                    row + 1
                )),
                _ => Err(format!("La columna '{name}' debe ser Int64 o Float64 para tratar valores numéricos.")),
            }
        })
        .collect()
}

fn apply_outlier_treatments(
    mut frame: DataFrame,
    treatments: &[OutlierTreatment],
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, usize, usize), String> {
    if treatments.len() > 16 {
        return Err("La receta admite como máximo 16 tratamientos de atípicos.".into());
    }
    let mut unique = HashSet::new();
    let mut prepared = Vec::with_capacity(treatments.len());
    for treatment in treatments {
        if !unique.insert(treatment.column.as_str()) {
            return Err(format!(
                "La columna '{}' tiene más de un tratamiento de atípicos.",
                treatment.column
            ));
        }
        let name = remapped_name(&treatment.column, renames).to_owned();
        let column = recipe_column(&frame, &name)?;
        if !matches!(
            column.dtype(),
            polars::prelude::DataType::Int64 | polars::prelude::DataType::Float64
        ) {
            return Err(format!(
                "La columna '{name}' debe ser Int64 o Float64 para tratar valores numéricos."
            ));
        }
        let values = physical_numeric_values(column)?;
        let mut valid = values.iter().flatten().copied().collect::<Vec<_>>();
        if valid.len() < 4 {
            return Err(format!(
                "La columna '{name}' requiere al menos cuatro valores numéricos válidos."
            ));
        }
        valid.sort_by(f64::total_cmp);
        let q1 = outlier_linear_quantile(&valid, 0.25);
        let q3 = outlier_linear_quantile(&valid, 0.75);
        let iqr = q3 - q1;
        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;
        if ![q1, q3, iqr, lower, upper].into_iter().all(f64::is_finite) {
            return Err(format!(
                "Los umbrales IQR de '{name}' exceden el rango numérico finito."
            ));
        }
        let median = if column.dtype() == &DataType::Int64 {
            valid[(valid.len() - 1) / 2]
        } else {
            outlier_linear_quantile(&valid, 0.5)
        };
        prepared.push((
            name,
            treatment.action,
            values,
            lower,
            upper,
            median,
            column.dtype().clone(),
        ));
    }

    let baseline_height = frame.height();
    let mut drop_mask = vec![false; baseline_height];
    let mut adjusted = 0;
    for (name, action, values, lower, upper, median, dtype) in prepared {
        match action {
            OutlierAction::Cap => {
                let capped = values
                    .into_iter()
                    .map(|value| {
                        value.map(|value| {
                            let capped = value.clamp(lower, upper);
                            if capped != value {
                                adjusted += 1;
                            }
                            capped
                        })
                    })
                    .collect::<Vec<_>>();
                if capped
                    .iter()
                    .zip((0..baseline_height).map(|row| {
                        frame
                            .column(&name)
                            .unwrap()
                            .get(row)
                            .ok()
                            .and_then(numeric_value)
                    }))
                    .any(|(left, right)| *left != right)
                {
                    frame
                        .replace(
                            &name,
                            Series::new(name.clone().into(), capped).into_column(),
                        )
                        .map_err(|error| format!("No se pudo limitar '{name}': {error}"))?;
                }
            }
            OutlierAction::Drop => {
                for (row, value) in values.into_iter().enumerate() {
                    if value.is_some_and(|value| value < lower || value > upper) {
                        drop_mask[row] = true;
                    }
                }
            }
            OutlierAction::Impute => {
                let imputed = values
                    .into_iter()
                    .map(|value| {
                        value.map(|value| {
                            if value < lower || value > upper {
                                adjusted += 1;
                                median
                            } else {
                                value
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                let replacement_column = Column::new(name.clone().into(), imputed)
                    .cast(&dtype)
                    .map_err(|error| format!("No se pudo imputar '{name}': {error}"))?;
                frame
                    .replace(&name, replacement_column)
                    .map_err(|error| format!("No se pudo actualizar '{name}': {error}"))?;
            }
        }
    }
    let keep = drop_mask.iter().map(|drop| !drop).collect::<Vec<_>>();
    let removed = drop_mask.iter().filter(|drop| **drop).count();
    if removed > 0 {
        frame = frame
            .filter(&BooleanChunked::from_slice("outliers".into(), &keep))
            .map_err(|error| format!("No se pudieron retirar filas atípicas: {error}"))?;
    }
    Ok((frame, adjusted, removed, treatments.len()))
}

fn apply_group_summary(
    frame: DataFrame,
    summary: &GroupSummaryRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, usize, usize), String> {
    if summary.group_by.is_empty() || summary.group_by.len() > 8 {
        return Err("Agrupar requiere entre 1 y 8 columnas clave.".into());
    }
    if summary.aggregations.is_empty() || summary.aggregations.len() > 32 {
        return Err("Resumir requiere entre 1 y 32 agregaciones.".into());
    }
    let groups = summary
        .group_by
        .iter()
        .map(|name| remapped_name(name, renames).to_owned())
        .collect::<Vec<_>>();
    let mut group_unique = HashSet::new();
    for name in &groups {
        if !group_unique.insert(name) {
            return Err(format!("La clave de grupo '{name}' está duplicada."));
        }
        recipe_column(&frame, name)?;
    }
    let mut aggregation_unique = HashSet::new();
    let mut output_names = HashSet::new();
    let aggregations = summary
        .aggregations
        .iter()
        .map(|aggregation| {
            let name = remapped_name(&aggregation.column, renames).to_owned();
            if !aggregation_unique.insert((name.clone(), aggregation.operation)) {
                return Err(format!(
                    "La agregación '{}_{}' está duplicada.",
                    name,
                    aggregation.operation.suffix()
                ));
            }
            let output = format!("{}_{}", name, aggregation.operation.suffix());
            if group_unique.contains(&output) || !output_names.insert(output.clone()) {
                return Err(format!(
                    "El nombre de salida '{output}' colisiona con otra columna."
                ));
            }
            let column = recipe_column(&frame, &name)?;
            match aggregation.operation {
                SummaryOperation::Sum | SummaryOperation::Mean
                    if !matches!(
                        column.dtype(),
                        polars::prelude::DataType::Int64 | polars::prelude::DataType::Float64
                    ) =>
                {
                    return Err(format!(
                        "La agregación {} requiere que '{name}' sea Int64 o Float64.",
                        aggregation.operation.suffix()
                    ))
                }
                SummaryOperation::Min | SummaryOperation::Max
                    if !matches!(
                        column.dtype(),
                        polars::prelude::DataType::Int64
                            | polars::prelude::DataType::Float64
                            | polars::prelude::DataType::String
                            | polars::prelude::DataType::Date
                            | polars::prelude::DataType::Datetime(_, _)
                    ) =>
                {
                    return Err(format!(
                        "La agregación {} no admite el tipo de '{name}'.",
                        aggregation.operation.suffix()
                    ))
                }
                _ => {}
            }
            Ok((name, output, aggregation.operation))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut positions: HashMap<Vec<Option<String>>, usize> = HashMap::new();
    let mut buckets: Vec<Vec<IdxSize>> = Vec::new();
    for row in 0..frame.height() {
        let key = groups
            .iter()
            .map(|name| {
                match frame
                    .column(name)
                    .unwrap()
                    .get(row)
                    .map_err(|error| error.to_string())?
                {
                    AnyValue::Null => Ok(None),
                    AnyValue::Float64(value) if !value.is_finite() => Err(format!(
                        "La clave de grupo '{name}' contiene NaN o infinito."
                    )),
                    AnyValue::Float64(0.0) => Ok(Some("0".into())),
                    value => Ok(Some(value.to_string())),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        let index = if let Some(index) = positions.get(&key) {
            *index
        } else {
            let index = buckets.len();
            positions.insert(key, index);
            buckets.push(Vec::new());
            index
        };
        buckets[index].push(row as IdxSize);
    }
    let first_rows = buckets.iter().map(|rows| rows[0]).collect::<Vec<_>>();
    let mut columns = groups
        .iter()
        .map(|name| {
            frame
                .column(name)
                .unwrap()
                .take_slice(&first_rows)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (name, output, operation) in aggregations {
        let source = frame.column(&name).map_err(|error| error.to_string())?;
        let result = match operation {
            SummaryOperation::Count => Series::new(
                output.clone().into(),
                buckets
                    .iter()
                    .map(|rows| rows.len() as i64)
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            SummaryOperation::CountUnique => {
                let counts = buckets
                    .iter()
                    .map(|rows| {
                        let mut unique = HashSet::new();
                        for row in rows {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => {}
                                AnyValue::Float64(value) if !value.is_finite() => {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ));
                                }
                                AnyValue::Float64(0.0) => {
                                    unique.insert("0".to_owned());
                                }
                                value => {
                                    unique.insert(value.to_string());
                                }
                            }
                        }
                        Ok(unique.len() as i64)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), counts).into_column()
            }
            SummaryOperation::Sum if source.dtype() == &polars::prelude::DataType::Int64 => {
                let values = buckets
                    .iter()
                    .map(|rows| {
                        rows.iter().try_fold(0_i64, |total, row| {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => Ok(total),
                                AnyValue::Int64(value) => total
                                    .checked_add(value)
                                    .ok_or_else(|| format!("La suma de '{name}' desbordó Int64.")),
                                _ => unreachable!(),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), values).into_column()
            }
            SummaryOperation::Sum | SummaryOperation::Mean => {
                let is_mean = operation == SummaryOperation::Mean;
                let values = buckets
                    .iter()
                    .map(|rows| {
                        let mut sum = 0.0;
                        let mut count = 0;
                        for row in rows {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => {}
                                AnyValue::Int64(value) => {
                                    if value.unsigned_abs() > (1_u64 << 53) {
                                        return Err(format!(
                                            "La media de '{name}' excede la precisión segura."
                                        ));
                                    }
                                    sum += value as f64;
                                    count += 1;
                                }
                                AnyValue::Float64(value) if value.is_finite() => {
                                    sum += value;
                                    count += 1;
                                }
                                AnyValue::Float64(_) => {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ))
                                }
                                _ => unreachable!(),
                            }
                        }
                        if !sum.is_finite() {
                            return Err(format!(
                                "La agregación de '{name}' produjo un valor no finito."
                            ));
                        }
                        Ok(if is_mean {
                            (count > 0).then(|| sum / count as f64)
                        } else {
                            Some(sum)
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), values).into_column()
            }
            SummaryOperation::Min | SummaryOperation::Max => {
                let take_max = operation == SummaryOperation::Max;
                let selected = buckets
                    .iter()
                    .map(|rows| {
                        let mut best: Option<(IdxSize, AnyValue<'_>)> = None;
                        for row in rows {
                            let value = source.get(*row as usize).unwrap();
                            if !matches!(value, AnyValue::Null) {
                                if matches!(value, AnyValue::Float64(v) if !v.is_finite()) {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ));
                                }
                                let better = best.as_ref().is_none_or(|(_, current)| {
                                    match (&value, current) {
                                        (AnyValue::Int64(a), AnyValue::Int64(b)) => {
                                            if take_max {
                                                a > b
                                            } else {
                                                a < b
                                            }
                                        }
                                        (AnyValue::Float64(a), AnyValue::Float64(b)) => {
                                            if take_max {
                                                a > b
                                            } else {
                                                a < b
                                            }
                                        }
                                        _ => {
                                            if take_max {
                                                value.to_string() > current.to_string()
                                            } else {
                                                value.to_string() < current.to_string()
                                            }
                                        }
                                    }
                                });
                                if better {
                                    best = Some((*row, value));
                                }
                            }
                        }
                        Ok(best.map(|(row, _)| row))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                match source.dtype() {
                    polars::prelude::DataType::Int64 => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::Int64(value) => value,
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::Float64 => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::Float64(value) => value,
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::String => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::String(value) => value.to_owned(),
                                    AnyValue::StringOwned(value) => value.to_string(),
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::Date => {
                        let physical = source
                            .cast(&polars::prelude::DataType::Int32)
                            .map_err(|error| error.to_string())?;
                        Series::new(
                            output.clone().into(),
                            selected
                                .iter()
                                .map(|row| {
                                    row.map(|row| match physical.get(row as usize).unwrap() {
                                        AnyValue::Int32(value) => value,
                                        _ => unreachable!(),
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                        .cast(&polars::prelude::DataType::Date)
                        .map_err(|error| error.to_string())?
                        .into_column()
                    }
                    polars::prelude::DataType::Datetime(unit, zone) => {
                        let physical = source
                            .cast(&polars::prelude::DataType::Int64)
                            .map_err(|error| error.to_string())?;
                        Series::new(
                            output.clone().into(),
                            selected
                                .iter()
                                .map(|row| {
                                    row.map(|row| match physical.get(row as usize).unwrap() {
                                        AnyValue::Int64(value) => value,
                                        _ => unreachable!(),
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                        .cast(&polars::prelude::DataType::Datetime(*unit, zone.clone()))
                        .map_err(|error| error.to_string())?
                        .into_column()
                    }
                    _ => unreachable!(),
                }
            }
        };
        columns.push(result);
    }
    let group_count = buckets.len();
    let collapsed = frame.height().saturating_sub(group_count);
    Ok((
        DataFrame::new(group_count, columns).map_err(|error| error.to_string())?,
        group_count,
        summary.aggregations.len(),
        collapsed,
    ))
}

fn apply_contact_normalizations(
    frame: &mut DataFrame,
    treatments: &[ContactNormalization],
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if treatments.len() > 16 {
        return Err("La receta admite como máximo 16 normalizaciones de contacto.".into());
    }
    let mut unique = HashSet::new();
    let mut changed_cells = 0;
    for treatment in treatments {
        if !unique.insert(treatment.column.as_str()) {
            return Err(format!(
                "La columna '{}' tiene más de una normalización de contacto.",
                treatment.column
            ));
        }
        let name = remapped_name(&treatment.column, renames).to_owned();
        let column = recipe_column(frame, &name)?;
        if column.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{name}' debe ser de texto para normalizar contactos."
            ));
        }
        let values = strict_column_text(column)?
            .into_iter()
            .map(|value| {
                value.map(|value| {
                    let normalized = match treatment.kind {
                        ContactKind::Email => value.trim().to_lowercase(),
                        ContactKind::Phone => {
                            let trimmed = value.trim();
                            let plus = trimmed.starts_with('+');
                            let digits = trimmed
                                .chars()
                                .filter(char::is_ascii_digit)
                                .collect::<String>();
                            if plus {
                                format!("+{digits}")
                            } else {
                                digits
                            }
                        }
                        ContactKind::Address => {
                            value.split_whitespace().collect::<Vec<_>>().join(" ")
                        }
                    };
                    if normalized != value {
                        changed_cells += 1;
                    }
                    normalized
                })
            })
            .collect::<Vec<_>>();
        frame
            .replace(
                &name,
                Series::new(name.clone().into(), values).into_column(),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok((changed_cells, treatments.len()))
}

fn first_run(value: &str, matches: impl Fn(char) -> bool) -> Option<String> {
    let mut output = String::new();
    let mut started = false;
    for character in value.chars() {
        if matches(character) {
            started = true;
            output.push(character);
        } else if started {
            break;
        }
    }
    started.then_some(output)
}

fn apply_text_extractions(
    frame: &mut DataFrame,
    extractions: &[TextExtraction],
    renames: &HashMap<&str, &str>,
) -> Result<usize, String> {
    if extractions.len() > 16 {
        return Err("La receta admite como máximo 16 extracciones de texto.".into());
    }
    let mut names = HashSet::new();
    for extraction in extractions {
        if extraction.name.trim().is_empty() || extraction.name != extraction.name.trim() {
            return Err(
                "El nombre extraído no puede estar vacío ni tener espacios exteriores.".into(),
            );
        }
        if !names.insert(extraction.name.as_str()) {
            return Err(format!(
                "La columna extraída '{}' está duplicada.",
                extraction.name
            ));
        }
        if frame.column(&extraction.name).is_ok() {
            return Err(format!(
                "La columna extraída '{}' ya existe.",
                extraction.name
            ));
        }
        let delimiter_based = matches!(
            extraction.kind,
            ExtractionKind::Before | ExtractionKind::After
        );
        if delimiter_based != extraction.delimiter.is_some() {
            return Err(format!(
                "La extracción '{}' {} delimitador.",
                extraction.name,
                if delimiter_based {
                    "requiere"
                } else {
                    "no acepta"
                }
            ));
        }
        if extraction.delimiter.as_deref().is_some_and(str::is_empty) {
            return Err("El delimitador de extracción no puede estar vacío.".into());
        }
        let source_name = remapped_name(&extraction.source, renames);
        let source = recipe_column(frame, source_name)?;
        if source.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{source_name}' debe ser de texto para extraerse."
            ));
        }
        let values = strict_column_text(source)?
            .into_iter()
            .map(|value| {
                value.and_then(|value| match extraction.kind {
                    ExtractionKind::FirstToken => {
                        value.split_whitespace().next().map(str::to_owned)
                    }
                    ExtractionKind::LastToken => value.split_whitespace().last().map(str::to_owned),
                    ExtractionKind::Digits => {
                        first_run(&value, |character| character.is_ascii_digit())
                    }
                    ExtractionKind::Letters => first_run(&value, char::is_alphabetic),
                    ExtractionKind::Before => value
                        .find(extraction.delimiter.as_deref().unwrap())
                        .map(|index| value[..index].to_owned()),
                    ExtractionKind::After => value
                        .find(extraction.delimiter.as_deref().unwrap())
                        .map(|index| {
                            value[index + extraction.delimiter.as_deref().unwrap().len()..]
                                .to_owned()
                        }),
                })
            })
            .collect::<Vec<_>>();
        frame
            .with_column(Series::new(extraction.name.clone().into(), values).into_column())
            .map_err(|error| error.to_string())?;
    }
    Ok(extractions.len())
}

type RecipeFrameOutcome = (
    DataFrame,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    bool,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
);

type LazySummaryAggregation = (String, String, SummaryOperation);
type LazySummaryPlan = (Vec<String>, Vec<LazySummaryAggregation>);

fn lazy_recipe_supported(recipe: &TransformRecipe) -> bool {
    recipe.date_parses.is_empty()
        && !(recipe.split_column.is_some() && recipe.merge_columns.is_some())
        && recipe.outlier_treatments.is_empty()
        && recipe
            .group_summary
            .as_ref()
            .is_none_or(|_| recipe.filters.is_empty() && recipe.find_replace.is_none())
        && recipe.contact_normalizations.is_empty()
        && recipe.text_extractions.is_empty()
        && recipe.calculated_column.as_ref().is_none_or(|calculation| {
            matches!(
                calculation.operation,
                CalculatedOperation::Add
                    | CalculatedOperation::Subtract
                    | CalculatedOperation::Multiply
            )
        })
}

fn validate_lazy_recipe_inputs(source: &DataFrame, recipe: &TransformRecipe) -> Result<(), String> {
    for cast in &recipe.casts {
        let column = recipe_column(source, &cast.column)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (DataType::String, RecipeCastTarget::String)
                | (DataType::Int64, RecipeCastTarget::Integer)
                | (DataType::Float64, RecipeCastTarget::Decimal)
                | (DataType::Boolean, RecipeCastTarget::Boolean)
        );
        if !already_target {
            strict_cast_column(column, cast.target)?;
        }
    }

    for filter in &recipe.filters {
        if !matches!(
            filter.operator,
            RecipeFilterOperator::Gt
                | RecipeFilterOperator::Lt
                | RecipeFilterOperator::Gte
                | RecipeFilterOperator::Lte
        ) {
            continue;
        }
        let name = filter.column.as_str();
        let values = strict_column_text(recipe_column(source, name)?)?;
        for (row, value) in values.iter().enumerate() {
            if let Some(value) = value {
                strict_f64(
                    value,
                    &format!("La fila {} de la columna '{name}'", row + 1),
                )?;
            }
        }
    }

    if let Some(calculation) = &recipe.calculated_column {
        let source_name = calculation.source.as_str();
        let source_values = strict_column_text(recipe_column(source, source_name)?)?;
        for (row, value) in source_values.iter().enumerate() {
            if let Some(value) = value {
                strict_f64(
                    value,
                    &format!("La fila {} de la columna '{source_name}'", row + 1),
                )?;
            }
        }
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Literal,
            value,
        }) = &calculation.operand
        {
            strict_f64(value, "El operando numérico")?;
        }
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            let values = strict_column_text(recipe_column(source, value)?)?;
            for (row, value) in values.iter().enumerate() {
                if let Some(value) = value {
                    strict_f64(
                        value,
                        &format!("La fila {} de la columna '{value}'", row + 1),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn lazy_filter_expression(
    frame: &DataFrame,
    filter: &RecipeFilter,
    effective_name: &str,
) -> Result<Expr, String> {
    let unary = matches!(
        filter.operator,
        RecipeFilterOperator::IsNull | RecipeFilterOperator::NotNull
    );
    if unary != filter.value.is_none() {
        return Err(format!(
            "El filtro '{}' {} un valor.",
            filter.column,
            if unary { "no acepta" } else { "requiere" }
        ));
    }
    let literal = filter.value.as_deref().unwrap_or_default();
    if matches!(
        filter.operator,
        RecipeFilterOperator::Gt
            | RecipeFilterOperator::Lt
            | RecipeFilterOperator::Gte
            | RecipeFilterOperator::Lte
            | RecipeFilterOperator::Contains
            | RecipeFilterOperator::NotContains
    ) && literal.is_empty()
    {
        return Err(format!(
            "El filtro '{}' requiere un valor no vacío.",
            filter.column
        ));
    }

    let value = col(effective_name);
    Ok(match filter.operator {
        RecipeFilterOperator::IsNull => value.is_null(),
        RecipeFilterOperator::NotNull => value.is_not_null(),
        RecipeFilterOperator::Eq => value.cast(DataType::String).eq(lit(literal.to_owned())),
        RecipeFilterOperator::Neq => value.cast(DataType::String).neq(lit(literal.to_owned())),
        RecipeFilterOperator::Contains => value
            .cast(DataType::String)
            .str()
            .to_lowercase()
            .str()
            .contains_literal(lit(literal.to_lowercase())),
        RecipeFilterOperator::NotContains => value
            .cast(DataType::String)
            .str()
            .to_lowercase()
            .str()
            .contains_literal(lit(literal.to_lowercase()))
            .not(),
        RecipeFilterOperator::Gt
        | RecipeFilterOperator::Lt
        | RecipeFilterOperator::Gte
        | RecipeFilterOperator::Lte => {
            if matches!(
                recipe_column(frame, effective_name)?.dtype(),
                DataType::Date | DataType::Datetime(_, _)
            ) {
                return Err(format!(
                    "La comparación numérica de '{effective_name}' no admite fechas en este hito."
                ));
            }
            let numeric_literal = strict_f64(literal, "El valor del filtro")?;
            let value = value.strict_cast(DataType::Float64);
            match filter.operator {
                RecipeFilterOperator::Gt => value.gt(lit(numeric_literal)),
                RecipeFilterOperator::Lt => value.lt(lit(numeric_literal)),
                RecipeFilterOperator::Gte => value.gt_eq(lit(numeric_literal)),
                RecipeFilterOperator::Lte => value.lt_eq(lit(numeric_literal)),
                _ => unreachable!("el match exterior limita esta rama a comparaciones numéricas"),
            }
        }
    })
}

fn lazy_find_replace_targets(
    source: &DataFrame,
    recipe: &FindReplaceRecipe,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Vec<String>, String> {
    if recipe.find.is_empty() {
        return Err("El texto buscado no puede estar vacío.".into());
    }
    let cast_target = |effective_name: &str| {
        casts
            .iter()
            .find(|cast| remapped_name(&cast.column, renames) == effective_name)
            .map(|cast| cast.target)
    };
    let is_text_after_cast = |source_name: &str, source_dtype: &DataType| {
        cast_target(remapped_name(source_name, renames))
            .map(|target| target == RecipeCastTarget::String)
            .unwrap_or(source_dtype == &DataType::String)
    };

    match recipe.scope {
        FindReplaceScope::Column => {
            let column = recipe
                .column
                .as_deref()
                .ok_or_else(|| "La búsqueda por columna requiere una columna.".to_owned())?;
            let source_column = recipe_column(source, column)?;
            let effective_name = remapped_name(column, renames);
            if !is_text_after_cast(column, source_column.dtype()) {
                return Err(format!(
                    "La columna '{effective_name}' debe ser de texto para buscar y reemplazar."
                ));
            }
            Ok(vec![effective_name.to_owned()])
        }
        FindReplaceScope::AllTextColumns => {
            if recipe.column.is_some() {
                return Err(
                    "La búsqueda en todas las columnas no acepta una columna concreta.".into(),
                );
            }
            Ok(source
                .get_column_names()
                .iter()
                .zip(source.columns())
                .filter(|(name, column)| is_text_after_cast(name, column.dtype()))
                .map(|(name, _)| remapped_name(name, renames).to_owned())
                .collect())
        }
    }
}

fn lazy_summary_column(
    source: &DataFrame,
    name: &str,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Column, String> {
    let column = recipe_column(source, name)?;
    let effective_name = remapped_name(name, renames);
    let Some(cast) = casts
        .iter()
        .find(|cast| remapped_name(&cast.column, renames) == effective_name)
    else {
        return Ok(column.clone());
    };
    let already_target = matches!(
        (column.dtype(), cast.target),
        (DataType::String, RecipeCastTarget::String)
            | (DataType::Int64, RecipeCastTarget::Integer)
            | (DataType::Float64, RecipeCastTarget::Decimal)
            | (DataType::Boolean, RecipeCastTarget::Boolean)
    );
    if already_target {
        Ok(column.clone())
    } else {
        strict_cast_column(column, cast.target)
    }
}

fn lazy_summary_group_key(
    column: &Column,
    row: usize,
    name: &str,
) -> Result<Option<String>, String> {
    match column
        .get(row)
        .map_err(|error| format!("No se pudo leer la clave de grupo '{name}': {error}"))?
    {
        AnyValue::Null => Ok(None),
        AnyValue::Float64(value) if !value.is_finite() => Err(format!(
            "La clave de grupo '{name}' contiene NaN o infinito."
        )),
        AnyValue::Float64(0.0) => Ok(Some("0".into())),
        value => Ok(Some(value.to_string())),
    }
}

fn validate_lazy_group_summary(
    source: &DataFrame,
    summary: &GroupSummaryRecipe,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<LazySummaryPlan, String> {
    if summary.group_by.is_empty() || summary.group_by.len() > 8 {
        return Err("Agrupar requiere entre 1 y 8 columnas clave.".into());
    }
    if summary.aggregations.is_empty() || summary.aggregations.len() > 32 {
        return Err("Resumir requiere entre 1 y 32 agregaciones.".into());
    }

    let groups = summary
        .group_by
        .iter()
        .map(|name| remapped_name(name, renames).to_owned())
        .collect::<Vec<_>>();
    let mut group_unique = HashSet::new();
    let group_columns = groups
        .iter()
        .zip(&summary.group_by)
        .map(|(effective_name, source_name)| {
            if !group_unique.insert(effective_name.clone()) {
                return Err(format!(
                    "La clave de grupo '{effective_name}' está duplicada."
                ));
            }
            let column = lazy_summary_column(source, source_name, renames, casts)?;
            for row in 0..column.len() {
                lazy_summary_group_key(&column, row, effective_name)?;
            }
            Ok(column)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut aggregation_unique = HashSet::new();
    let mut output_names = HashSet::new();
    let mut aggregations = Vec::with_capacity(summary.aggregations.len());
    let mut integer_sum_columns = Vec::new();
    for aggregation in &summary.aggregations {
        let name = remapped_name(&aggregation.column, renames).to_owned();
        if !aggregation_unique.insert((name.clone(), aggregation.operation)) {
            return Err(format!(
                "La agregación '{}_{}' está duplicada.",
                name,
                aggregation.operation.suffix()
            ));
        }
        let output = format!("{}_{}", name, aggregation.operation.suffix());
        if group_unique.contains(&output) || !output_names.insert(output.clone()) {
            return Err(format!(
                "El nombre de salida '{output}' colisiona con otra columna."
            ));
        }
        let column = lazy_summary_column(source, &aggregation.column, renames, casts)?;
        match aggregation.operation {
            SummaryOperation::Sum | SummaryOperation::Mean
                if !matches!(column.dtype(), DataType::Int64 | DataType::Float64) =>
            {
                return Err(format!(
                    "La agregación {} requiere que '{name}' sea Int64 o Float64.",
                    aggregation.operation.suffix()
                ));
            }
            SummaryOperation::Min | SummaryOperation::Max
                if !matches!(
                    column.dtype(),
                    DataType::Int64
                        | DataType::Float64
                        | DataType::String
                        | DataType::Date
                        | DataType::Datetime(_, _)
                ) =>
            {
                return Err(format!(
                    "La agregación {} no admite el tipo de '{name}'.",
                    aggregation.operation.suffix()
                ));
            }
            _ => {}
        }

        if matches!(
            aggregation.operation,
            SummaryOperation::Sum
                | SummaryOperation::Mean
                | SummaryOperation::Min
                | SummaryOperation::Max
                | SummaryOperation::CountUnique
        ) {
            for row in 0..column.len() {
                match column.get(row).map_err(|error| error.to_string())? {
                    AnyValue::Float64(value) if !value.is_finite() => {
                        return Err(format!("La columna '{name}' contiene NaN o infinito."));
                    }
                    AnyValue::Int64(value)
                        if aggregation.operation == SummaryOperation::Mean
                            && value.unsigned_abs() > (1_u64 << 53) =>
                    {
                        return Err(format!("La media de '{name}' excede la precisión segura."));
                    }
                    _ => {}
                }
            }
        }
        if aggregation.operation == SummaryOperation::Sum && column.dtype() == &DataType::Int64 {
            integer_sum_columns.push((name.clone(), column.clone()));
        }
        aggregations.push((name, output, aggregation.operation));
    }

    if !integer_sum_columns.is_empty() {
        let mut sums = vec![HashMap::<Vec<Option<String>>, i64>::new(); integer_sum_columns.len()];
        for row in 0..source.height() {
            let key = group_columns
                .iter()
                .zip(&groups)
                .map(|(column, name)| lazy_summary_group_key(column, row, name))
                .collect::<Result<Vec<_>, String>>()?;
            for (index, (name, column)) in integer_sum_columns.iter().enumerate() {
                let Some(value) = (match column
                    .get(row)
                    .map_err(|error| format!("No se pudo leer '{name}': {error}"))?
                {
                    AnyValue::Null => None,
                    AnyValue::Int64(value) => Some(value),
                    _ => unreachable!("la validación de tipo garantiza Int64"),
                }) else {
                    continue;
                };
                let total = sums[index].entry(key.clone()).or_insert(0);
                *total = total
                    .checked_add(value)
                    .ok_or_else(|| format!("La suma de '{name}' desbordó Int64."))?;
            }
        }
    }

    Ok((groups, aggregations))
}

fn apply_lazy_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    if recipe.filters.len() > 3 {
        return Err("La receta admite como máximo tres filtros combinados con AND.".into());
    }
    validate_lazy_recipe_inputs(source, recipe)?;

    let mut rename_sources = HashSet::new();
    let rename_map = recipe
        .renames
        .iter()
        .map(|rename| {
            if rename.from.trim().is_empty() || rename.to.trim().is_empty() {
                return Err("Los nombres de columna no pueden estar vacíos.".to_owned());
            }
            if rename.to != rename.to.trim() {
                return Err(
                    "El nuevo nombre de columna no puede tener espacios exteriores.".to_owned(),
                );
            }
            if !rename_sources.insert(rename.from.as_str()) {
                return Err(format!(
                    "La columna '{}' aparece en más de un renombrado.",
                    rename.from
                ));
            }
            recipe_column(source, &rename.from)?;
            Ok((rename.from.as_str(), rename.to.as_str()))
        })
        .collect::<Result<HashMap<_, _>, String>>()?;

    for cast in &recipe.casts {
        recipe_column(source, &cast.column)?;
    }
    for filter in &recipe.filters {
        recipe_column(source, &filter.column)?;
    }
    if let Some(calculation) = &recipe.calculated_column {
        recipe_column(source, &calculation.source)?;
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            recipe_column(source, value)?;
        }
        if calculation.name.trim().is_empty() || calculation.name != calculation.name.trim() {
            return Err(
                "El nombre calculado no puede estar vacío ni tener espacios exteriores.".into(),
            );
        }
    }

    let final_names = source
        .get_column_names()
        .iter()
        .map(|name| {
            rename_map
                .get(name.as_str())
                .copied()
                .unwrap_or(name.as_str())
                .to_owned()
        })
        .collect::<Vec<_>>();
    let mut unique_names = HashSet::new();
    if final_names.iter().any(|name| !unique_names.insert(name)) {
        return Err("Los renombrados producirían nombres de columna duplicados.".to_owned());
    }
    let renamed_count = source
        .get_column_names()
        .iter()
        .zip(&final_names)
        .filter(|(before, after)| before.as_str() != after.as_str())
        .count();

    let mut plan = source.clone().lazy();
    if renamed_count > 0 {
        let old_names = source
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>();
        let new_names = final_names.iter().map(String::as_str).collect::<Vec<_>>();
        plan = plan.rename(old_names, new_names, true);
    }

    let mut cast_columns = HashSet::new();
    let mut cast_expressions = Vec::new();
    let mut cast_count = 0;
    for cast in &recipe.casts {
        let effective_name = rename_map
            .get(cast.column.as_str())
            .copied()
            .unwrap_or(cast.column.as_str());
        if !cast_columns.insert(effective_name) {
            return Err(format!(
                "La columna '{}' aparece en más de una conversión.",
                cast.column
            ));
        }
        let column = recipe_column(source, &cast.column)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (DataType::String, RecipeCastTarget::String)
                | (DataType::Int64, RecipeCastTarget::Integer)
                | (DataType::Float64, RecipeCastTarget::Decimal)
                | (DataType::Boolean, RecipeCastTarget::Boolean)
        );
        if !already_target {
            let target = match cast.target {
                RecipeCastTarget::String => DataType::String,
                RecipeCastTarget::Integer => DataType::Int64,
                RecipeCastTarget::Decimal => DataType::Float64,
                RecipeCastTarget::Boolean => DataType::Boolean,
            };
            cast_expressions.push(
                col(effective_name)
                    .strict_cast(target)
                    .alias(effective_name),
            );
            cast_count += 1;
        }
    }
    if !cast_expressions.is_empty() {
        plan = plan.with_columns(cast_expressions);
    }

    for filter in &recipe.filters {
        let effective_name = remapped_name(&filter.column, &rename_map);
        plan = plan.filter(lazy_filter_expression(source, filter, effective_name)?);
    }

    let replaced_cell_count = if let Some(find_replace) = &recipe.find_replace {
        let targets = lazy_find_replace_targets(source, find_replace, &rename_map, &recipe.casts)?;
        if targets.is_empty() {
            0
        } else {
            let count_aliases = targets
                .iter()
                .enumerate()
                .map(|(index, _)| format!("__columnia_replaced_{index}"))
                .collect::<Vec<_>>();
            let count_expressions = targets
                .iter()
                .zip(&count_aliases)
                .map(|(name, alias)| {
                    let original = col(name);
                    let replaced = original.clone().str().replace_all(
                        lit(find_replace.find.clone()),
                        lit(find_replace.replace.clone()),
                        true,
                    );
                    replaced
                        .neq_missing(original)
                        .cast(DataType::UInt64)
                        .sum()
                        .alias(alias)
                })
                .collect::<Vec<_>>();
            let count_frame = collect_lazy_frame_streaming(
                plan.clone().select(count_expressions),
                "No se pudo contar el reemplazo de texto",
            )?;
            let count = count_aliases
                .iter()
                .map(|alias| {
                    let value = count_frame
                        .column(alias)
                        .map_err(|error| {
                            format!("No se pudo leer el conteo de reemplazos: {error}")
                        })?
                        .get(0)
                        .map_err(|error| {
                            format!("No se pudo leer el conteo de reemplazos: {error}")
                        })?;
                    match value {
                        AnyValue::UInt64(value) => usize::try_from(value).map_err(|_| {
                            "El conteo de reemplazos excede el límite de memoria.".to_owned()
                        }),
                        AnyValue::Int64(value) if value >= 0 => usize::try_from(value as u64)
                            .map_err(|_| {
                                "El conteo de reemplazos excede el límite de memoria.".to_owned()
                            }),
                        AnyValue::Null => Ok(0),
                        _ => Err("El conteo de reemplazos devolvió un tipo inválido.".to_owned()),
                    }
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .sum();
            let replacement_expressions = targets
                .iter()
                .map(|name| {
                    col(name)
                        .str()
                        .replace_all(
                            lit(find_replace.find.clone()),
                            lit(find_replace.replace.clone()),
                            true,
                        )
                        .alias(name)
                })
                .collect::<Vec<_>>();
            plan = plan.with_columns(replacement_expressions);
            count
        }
    } else {
        0
    };

    let renamed_source = if recipe.keep_columns.is_some() {
        let mut frame = source.clone();
        frame
            .set_column_names(&final_names)
            .map_err(|error| format!("No se pudieron validar las columnas conservadas: {error}"))?;
        Some(frame)
    } else {
        None
    };
    let keep_frame = renamed_source.as_ref().unwrap_or(source);
    let (keep_names, dropped_column_count, kept_order_changed) =
        resolve_keep_column_names(keep_frame, recipe.keep_columns.as_deref(), &rename_map)?;
    if recipe.keep_columns.is_some() {
        plan = plan.select(keep_names.iter().map(col).collect::<Vec<_>>());
    }

    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        let source_name = remapped_name(&calculation.source, &rename_map);
        if recipe.keep_columns.is_some() && !keep_names.iter().any(|name| name == source_name) {
            return Err(format!(
                "La columna fuente calculada '{source_name}' fue descartada por keepColumns."
            ));
        }
        let source_column = recipe_column(source, &calculation.source)?;
        let left = col(source_name).strict_cast(DataType::Float64);
        let operand = calculation.operand.as_ref().ok_or_else(|| {
            "La columna calculada requiere un operando para esta operación.".to_owned()
        })?;
        let right = match operand.kind {
            CalculatedOperandKind::Literal => {
                lit(strict_f64(&operand.value, "El operando numérico")?)
            }
            CalculatedOperandKind::Column => {
                let operand_name = remapped_name(&operand.value, &rename_map);
                recipe_column(source, &operand.value)?;
                col(operand_name).strict_cast(DataType::Float64)
            }
        };
        let expression = match calculation.operation {
            CalculatedOperation::Add => left + right,
            CalculatedOperation::Subtract => left - right,
            CalculatedOperation::Multiply => left * right,
            CalculatedOperation::Divide => left / right,
            _ => return Err("La operación calculada no está soportada por el plan lazy.".into()),
        };
        if recipe_column(source, &calculation.name).is_ok() {
            return Err(format!(
                "La columna calculada '{}' ya existe.",
                calculation.name
            ));
        }
        if source_column.dtype() == &DataType::Boolean {
            return Err(format!(
                "La columna fuente calculada '{}' debe ser numérica.",
                calculation.source
            ));
        }
        plan = plan.with_columns(vec![expression.alias(calculation.name.clone())]);
        1
    } else {
        0
    };

    let (split_column_count, split_dropped_source_count) = if let Some(split) = &recipe.split_column
    {
        if split.delimiter.is_empty() {
            return Err("El delimitador de división no puede estar vacío.".into());
        }
        if !(2..=16).contains(&split.names.len()) {
            return Err("La división requiere entre 2 y 16 columnas de destino.".into());
        }
        let source_name = remapped_name(&split.source, &rename_map).to_owned();
        let mut output_names = if recipe.keep_columns.is_some() {
            keep_names.clone()
        } else {
            final_names.clone()
        };
        if let Some(calculation) = &recipe.calculated_column {
            output_names.push(calculation.name.clone());
        }
        if !output_names.iter().any(|name| name == &source_name) {
            return Err(format!(
                "La columna '{source_name}' requerida por split fue descartada por keepColumns."
            ));
        }
        let source_column = recipe_column(source, &split.source)?;
        let cast_target = recipe
            .casts
            .iter()
            .find(|cast| remapped_name(&cast.column, &rename_map) == source_name)
            .map(|cast| cast.target);
        let is_text = cast_target
            .map(|target| target == RecipeCastTarget::String)
            .unwrap_or(source_column.dtype() == &DataType::String);
        if !is_text {
            return Err(format!(
                "La columna '{source_name}' debe ser de texto para dividirse."
            ));
        }
        let mut unique = HashSet::new();
        for name in &split.names {
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed != name {
                return Err(
                    "Los nombres divididos no pueden estar vacíos ni tener espacios exteriores."
                        .into(),
                );
            }
            if !unique.insert(trimmed) {
                return Err(format!("El nombre dividido '{trimmed}' está duplicado."));
            }
            if output_names.iter().any(|existing| existing == trimmed) {
                return Err(format!("La columna dividida '{trimmed}' ya existe."));
            }
        }
        let split_expression = col(source_name.as_str())
            .str()
            .splitn(lit(split.delimiter.clone()), split.names.len());
        let split_expressions = split
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                split_expression
                    .clone()
                    .struct_()
                    .field_by_name(&format!("field_{index}"))
                    .alias(name.clone())
            })
            .collect::<Vec<_>>();
        plan = plan.with_columns(split_expressions);
        output_names.extend(split.names.iter().cloned());
        let split_dropped_source_count = if split.drop_source {
            plan = plan.select(
                output_names
                    .iter()
                    .filter(|name| name.as_str() != source_name)
                    .map(col)
                    .collect::<Vec<_>>(),
            );
            1
        } else {
            0
        };
        (split.names.len(), split_dropped_source_count)
    } else {
        (0, 0)
    };

    let (merged_column_count, dropped_source_column_count) = if let Some(merge) =
        &recipe.merge_columns
    {
        if !(2..=16).contains(&merge.sources.len()) {
            return Err("La unión requiere entre 2 y 16 columnas fuente.".into());
        }
        if merge.name.trim().is_empty() || merge.name != merge.name.trim() {
            return Err(
                "El nombre de la columna unida no puede estar vacío ni tener espacios exteriores."
                    .into(),
            );
        }
        let mut output_names = if recipe.keep_columns.is_some() {
            keep_names.clone()
        } else {
            final_names.clone()
        };
        if let Some(calculation) = &recipe.calculated_column {
            output_names.push(calculation.name.clone());
        }
        if output_names.iter().any(|name| name == &merge.name) {
            return Err(format!("La columna unida '{}' ya existe.", merge.name));
        }
        let mut unique = HashSet::new();
        let sources = merge
                .sources
                .iter()
                .map(|source_name| {
                    if !unique.insert(source_name) {
                        return Err(format!(
                            "La columna fuente '{source_name}' está duplicada."
                        ));
                    }
                    let effective_name = remapped_name(source_name, &rename_map).to_owned();
                    if !output_names.iter().any(|name| name == &effective_name) {
                        return Err(format!(
                            "La columna '{effective_name}' requerida por merge fue descartada por keepColumns."
                        ));
                    }
                    let source_column = recipe_column(source, source_name)?;
                    let cast_target = recipe
                        .casts
                        .iter()
                        .find(|cast| {
                            remapped_name(&cast.column, &rename_map) == effective_name
                        })
                        .map(|cast| cast.target);
                    let is_text = cast_target
                        .map(|target| target == RecipeCastTarget::String)
                        .unwrap_or(source_column.dtype() == &DataType::String);
                    if !is_text {
                        return Err(format!(
                            "La columna '{effective_name}' debe ser de texto para unirse."
                        ));
                    }
                    Ok(effective_name)
                })
                .collect::<Result<Vec<_>, String>>()?;
        let source_expressions = sources.iter().map(col).collect::<Vec<_>>();
        let expression = when(coalesce(&source_expressions).is_not_null())
            .then(concat_str(source_expressions, &merge.separator, true))
            .otherwise(lit(NULL))
            .alias(merge.name.clone());
        plan = plan.with_columns(vec![expression]);
        output_names.push(merge.name.clone());
        let dropped_source_column_count = if merge.drop_sources {
            let dropped = sources.iter().collect::<HashSet<_>>();
            plan = plan.select(
                output_names
                    .iter()
                    .filter(|name| !dropped.contains(name))
                    .map(col)
                    .collect::<Vec<_>>(),
            );
            sources.len()
        } else {
            0
        };
        (1, dropped_source_column_count)
    } else {
        (0, 0)
    };

    let (group_summary_input_rows, summary_aggregations) = if let Some(summary) =
        &recipe.group_summary
    {
        let schema = plan
            .collect_schema()
            .map_err(|error| format!("No se pudo validar el esquema del resumen: {error}"))?;
        let (groups, aggregations) =
            validate_lazy_group_summary(source, summary, &rename_map, &recipe.casts)?;
        for name in groups
            .iter()
            .chain(aggregations.iter().map(|(name, _, _)| name))
        {
            if schema.get(name).is_none() {
                return Err(format!(
                        "La columna '{name}' requerida por agrupar/resumir no sobrevivió las etapas anteriores."
                    ));
            }
        }
        let group_expressions = groups.iter().map(col).collect::<Vec<_>>();
        let aggregate_expressions = aggregations
            .iter()
            .map(|(name, output, operation)| {
                let expression = match operation {
                    SummaryOperation::Sum => col(name).sum(),
                    SummaryOperation::Mean => col(name).mean(),
                    SummaryOperation::Min => col(name).min(),
                    SummaryOperation::Max => col(name).max(),
                    SummaryOperation::Count => len().cast(DataType::Int64),
                    SummaryOperation::CountUnique => {
                        col(name).drop_nulls().n_unique().cast(DataType::Int64)
                    }
                };
                expression.alias(output)
            })
            .collect::<Vec<_>>();
        plan = plan
            .group_by_stable(group_expressions)
            .agg(aggregate_expressions);
        (Some(source.height()), Some(aggregations))
    } else {
        (None, None)
    };

    let candidate = collect_lazy_frame_streaming(plan, "No se pudo ejecutar la receta lazy")?;
    if let Some(aggregations) = &summary_aggregations {
        for (name, output, operation) in aggregations {
            if !matches!(
                operation,
                SummaryOperation::Sum
                    | SummaryOperation::Mean
                    | SummaryOperation::Min
                    | SummaryOperation::Max
            ) {
                continue;
            }
            let column = candidate
                .column(output)
                .map_err(|error| format!("No se pudo leer '{output}': {error}"))?;
            if column.dtype() == &DataType::Float64
                && (0..column.len()).any(|row| {
                    matches!(
                        column.get(row),
                        Ok(AnyValue::Float64(value)) if !value.is_finite()
                    )
                })
            {
                return Err(format!(
                    "La agregación de '{name}' produjo un valor no finito."
                ));
            }
        }
    }
    let removed_row_count = group_summary_input_rows
        .map(|input_rows| source.height().saturating_sub(input_rows))
        .unwrap_or_else(|| source.height().saturating_sub(candidate.height()));
    let group_count = summary_aggregations
        .as_ref()
        .map_or(0, |_| candidate.height());
    let collapsed_row_count =
        group_summary_input_rows.map_or(0, |input_rows| input_rows.saturating_sub(group_count));
    Ok((
        candidate,
        renamed_count,
        cast_count,
        0,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        kept_order_changed,
        split_column_count,
        merged_column_count,
        split_dropped_source_count + dropped_source_column_count,
        0,
        0,
        0,
        group_count,
        summary_aggregations.as_ref().map_or(0, Vec::len),
        collapsed_row_count,
        0,
        0,
        0,
    ))
}

fn apply_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    if lazy_recipe_supported(recipe) {
        return apply_lazy_recipe_to_frame(source, recipe);
    }
    apply_eager_recipe_to_frame(source, recipe)
}

fn apply_eager_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    let mut candidate = source.clone();
    let mut rename_sources = HashSet::new();
    let rename_map = recipe
        .renames
        .iter()
        .map(|rename| {
            if rename.from.trim().is_empty() || rename.to.trim().is_empty() {
                return Err("Los nombres de columna no pueden estar vacíos.".to_owned());
            }
            if rename.to != rename.to.trim() {
                return Err(
                    "El nuevo nombre de columna no puede tener espacios exteriores.".to_owned(),
                );
            }
            if !rename_sources.insert(rename.from.as_str()) {
                return Err(format!(
                    "La columna '{}' aparece en más de un renombrado.",
                    rename.from
                ));
            }
            recipe_column(source, &rename.from)?;
            Ok((rename.from.as_str(), rename.to.as_str()))
        })
        .collect::<Result<HashMap<_, _>, String>>()?;
    for cast in &recipe.casts {
        recipe_column(source, &cast.column)?;
    }
    for parse in &recipe.date_parses {
        recipe_column(source, &parse.column)?;
    }
    for filter in &recipe.filters {
        recipe_column(source, &filter.column)?;
    }
    if let Some(find_replace) = &recipe.find_replace {
        if let Some(column) = &find_replace.column {
            recipe_column(source, column)?;
        }
    }
    if let Some(keep_columns) = &recipe.keep_columns {
        for column in keep_columns {
            recipe_column(source, column)?;
        }
    }
    if let Some(split) = &recipe.split_column {
        recipe_column(source, &split.source)?;
    }
    if let Some(merge) = &recipe.merge_columns {
        for source_name in &merge.sources {
            recipe_column(source, source_name)?;
        }
    }
    for treatment in &recipe.outlier_treatments {
        recipe_column(source, &treatment.column)?;
    }
    if let Some(summary) = &recipe.group_summary {
        for name in &summary.group_by {
            recipe_column(source, name)?;
        }
        for aggregation in &summary.aggregations {
            recipe_column(source, &aggregation.column)?;
        }
    }
    if recipe.group_summary.is_some() && !recipe.text_extractions.is_empty() {
        return Err(
            "No se puede combinar extracción de texto con agrupar/resumir en la misma receta."
                .into(),
        );
    }
    for treatment in &recipe.contact_normalizations {
        recipe_column(source, &treatment.column)?;
    }
    for extraction in &recipe.text_extractions {
        recipe_column(source, &extraction.source)?;
    }
    if let Some(calculation) = &recipe.calculated_column {
        recipe_column(source, &calculation.source)?;
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            recipe_column(source, value)?;
        }
    }
    let final_names = source
        .get_column_names()
        .iter()
        .map(|name| {
            rename_map
                .get(name.as_str())
                .copied()
                .unwrap_or(name.as_str())
                .to_owned()
        })
        .collect::<Vec<_>>();
    let mut unique_names = HashSet::new();
    if final_names.iter().any(|name| !unique_names.insert(name)) {
        return Err("Los renombrados producirían nombres de columna duplicados.".to_owned());
    }
    let renamed_count = source
        .get_column_names()
        .iter()
        .zip(&final_names)
        .filter(|(before, after)| before.as_str() != after.as_str())
        .count();
    if renamed_count > 0 {
        candidate
            .set_column_names(&final_names)
            .map_err(|error| format!("No se pudieron aplicar los renombrados: {error}"))?;
    }

    let mut cast_columns = HashSet::new();
    let mut cast_count = 0;
    for cast in &recipe.casts {
        let effective_name = rename_map
            .get(cast.column.as_str())
            .copied()
            .unwrap_or(cast.column.as_str());
        if !cast_columns.insert(effective_name) {
            return Err(format!(
                "La columna '{}' aparece en más de una conversión.",
                cast.column
            ));
        }
        let column = recipe_column(&candidate, effective_name)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (polars::prelude::DataType::String, RecipeCastTarget::String)
                | (polars::prelude::DataType::Int64, RecipeCastTarget::Integer)
                | (
                    polars::prelude::DataType::Float64,
                    RecipeCastTarget::Decimal
                )
                | (
                    polars::prelude::DataType::Boolean,
                    RecipeCastTarget::Boolean
                )
        );
        if !already_target {
            let converted = strict_cast_column(column, cast.target)?;
            candidate
                .replace(effective_name, converted)
                .map_err(|error| format!("No se pudo convertir '{}': {error}", effective_name))?;
            cast_count += 1;
        }
    }

    let mut date_columns = HashSet::new();
    let mut date_count = 0;
    for parse in &recipe.date_parses {
        let effective_name = rename_map
            .get(parse.column.as_str())
            .copied()
            .unwrap_or(parse.column.as_str());
        if cast_columns.contains(effective_name) {
            return Err(format!(
                "La columna '{}' no puede convertirse y parsearse como fecha en la misma receta.",
                parse.column
            ));
        }
        if !date_columns.insert(effective_name) {
            return Err(format!(
                "La columna '{}' aparece en más de un parseo de fecha.",
                parse.column
            ));
        }
        let column = recipe_column(&candidate, effective_name)?;
        let already_target = matches!(
            (column.dtype(), parse.target),
            (polars::prelude::DataType::Date, RecipeDateTarget::Date)
                | (
                    polars::prelude::DataType::Datetime(_, _),
                    RecipeDateTarget::Datetime
                )
        );
        if !already_target {
            let converted = strict_date_column(column, parse.format, parse.target)?;
            candidate
                .replace(effective_name, converted)
                .map_err(|error| {
                    format!(
                        "No se pudo convertir la fecha '{}': {error}",
                        effective_name
                    )
                })?;
            date_count += 1;
        }
    }

    let (mut candidate, removed_row_count) =
        apply_recipe_filters(candidate, &recipe.filters, &rename_map)?;
    let replaced_cell_count = if let Some(find_replace) = &recipe.find_replace {
        apply_find_replace(&mut candidate, find_replace, &rename_map)?
    } else {
        0
    };
    let (mut candidate, dropped_column_count, kept_order_changed) =
        apply_keep_columns(candidate, recipe.keep_columns.as_deref(), &rename_map)?;
    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        let source_name = remapped_name(&calculation.source, &rename_map);
        recipe_column(&candidate, source_name).map_err(|_| {
            format!("La columna fuente calculada '{source_name}' fue descartada por keepColumns.")
        })?;
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            let operand_name = remapped_name(value, &rename_map);
            recipe_column(&candidate, operand_name).map_err(|_| {
                format!(
                    "La columna operando calculada '{operand_name}' fue descartada por keepColumns."
                )
            })?;
        }
        add_calculated_column(&mut candidate, calculation, &rename_map)?;
        1
    } else {
        0
    };
    if let (Some(split), Some(merge)) = (&recipe.split_column, &recipe.merge_columns) {
        if split.drop_source && merge.sources.contains(&split.source) {
            return Err(format!(
                "La unión necesita '{}', pero la división la descartaría.",
                split.source
            ));
        }
    }
    let (split_column_count, split_dropped) = if let Some(split) = &recipe.split_column {
        let effective = remapped_name(&split.source, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!("La columna '{effective}' requerida por split fue descartada por keepColumns.")
        })?;
        apply_split_column(&mut candidate, split, &rename_map)?
    } else {
        (0, 0)
    };
    let (merged_column_count, merge_dropped) = if let Some(merge) = &recipe.merge_columns {
        for source in &merge.sources {
            let effective = remapped_name(source, &rename_map);
            recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' requerida por merge fue descartada por keepColumns o split."))?;
        }
        apply_merge_columns(&mut candidate, merge, &rename_map)?
    } else {
        (0, 0)
    };
    let dropped_source_column_count = split_dropped + merge_dropped;
    for treatment in &recipe.outlier_treatments {
        let effective = remapped_name(&treatment.column, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' para tratar atípicos no sobrevivió las etapas estructurales."))?;
    }
    let (candidate, adjusted_outlier_cell_count, outlier_removed_row_count, outlier_column_count) =
        apply_outlier_treatments(candidate, &recipe.outlier_treatments, &rename_map)?;
    let mut candidate = candidate;
    for treatment in &recipe.contact_normalizations {
        let effective = remapped_name(&treatment.column, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!(
                "La columna '{effective}' para contactos no sobrevivió las etapas estructurales."
            )
        })?;
    }
    for extraction in &recipe.text_extractions {
        let effective = remapped_name(&extraction.source, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!(
                "La columna '{effective}' para extracción no sobrevivió las etapas estructurales."
            )
        })?;
    }
    let (normalized_contact_cell_count, normalized_contact_column_count) =
        apply_contact_normalizations(&mut candidate, &recipe.contact_normalizations, &rename_map)?;
    let extracted_column_count =
        apply_text_extractions(&mut candidate, &recipe.text_extractions, &rename_map)?;
    let (candidate, group_count, aggregated_column_count, collapsed_row_count) = if let Some(
        summary,
    ) =
        &recipe.group_summary
    {
        for name in summary.group_by.iter().chain(
            summary
                .aggregations
                .iter()
                .map(|aggregation| &aggregation.column),
        ) {
            let effective = remapped_name(name, &rename_map);
            recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' requerida por agrupar/resumir no sobrevivió las etapas anteriores."))?;
        }
        apply_group_summary(candidate, summary, &rename_map)?
    } else {
        (candidate, 0, 0, 0)
    };

    Ok((
        candidate,
        renamed_count,
        cast_count,
        date_count,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        kept_order_changed,
        split_column_count,
        merged_column_count,
        dropped_source_column_count,
        adjusted_outlier_cell_count,
        outlier_removed_row_count,
        outlier_column_count,
        group_count,
        aggregated_column_count,
        collapsed_row_count,
        normalized_contact_cell_count,
        normalized_contact_column_count,
        extracted_column_count,
    ))
}

pub(crate) fn load_dataset_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<(DataFrame, DatasetPreview), String> {
    let (canonical, _, extension) = validate_dataset_file(input)?;
    if spreadsheet_extensions(&extension) {
        let sheet_name = sheet_name.ok_or_else(|| "Selecciona una hoja del libro.".to_owned())?;
        let header_mode = header_mode
            .ok_or_else(|| "Elige cómo interpretar los encabezados del libro.".to_owned())?;
        if sheet_name.is_empty() {
            return Err("La hoja seleccionada no es válida.".to_owned());
        }
        let available_sheets = inspect_workbook(&canonical)?;
        if available_sheets
            .iter()
            .filter(|name| *name == sheet_name)
            .count()
            != 1
        {
            return Err("La hoja seleccionada no existe de forma única en el libro.".to_owned());
        }
        let frame = load_spreadsheet_sheet(&canonical, sheet_name, header_mode)?;
        let preview = dataset_preview(&canonical, &frame)?;
        Ok((frame, preview))
    } else {
        if sheet_name.is_some() || header_mode.is_some() {
            return Err("Este formato no utiliza selección de hoja ni encabezado.".to_owned());
        }
        load_dataset_with_progress(&canonical, |_, _| {}, || false)
    }
}

pub(crate) fn load_quality_rules_for_automation(input: &Path) -> Result<Vec<QualityRule>, String> {
    let document = parse_quality_rules_document(read_quality_rules_json(input)?, true)?;
    Ok(document.rules)
}

pub(crate) fn evaluate_quality_rules_for_automation(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
) -> Result<QualityValidationResult, String> {
    evaluate_quality_rules(frame, quality_rules)
}

pub(crate) fn validate_project_workspace(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    recipe_draft: Option<&StoredTransformRecipe>,
) -> Result<(), String> {
    validate_quality_rules_payload(quality_rules)?;
    for rule in quality_rules {
        validate_quality_rule_definition(frame, rule)?;
    }
    if let Some(recipe) = recipe_draft {
        validate_stored_recipe(recipe)?;
    }
    Ok(())
}

pub(crate) fn validate_project_profile(
    frame: &DataFrame,
    profile: &DatasetProfile,
) -> Result<(), String> {
    let finite = |value: Option<f64>| value.is_none_or(f64::is_finite);
    if !profile.duplicate_percentage.is_finite()
        || !(0.0..=100.0).contains(&profile.duplicate_percentage)
        || profile.columns.iter().any(|column| {
            !column.completeness_percentage.is_finite()
                || !(0.0..=100.0).contains(&column.completeness_percentage)
                || !finite(column.mean)
                || !finite(column.average_length)
                || !finite(column.type_match_percentage)
                || !finite(column.standard_deviation)
                || !finite(column.first_quartile)
                || !finite(column.median)
                || !finite(column.third_quartile)
        })
    {
        return Err("El perfil guardado contiene métricas no válidas.".to_owned());
    }
    if let Some(correlations) = &profile.numeric_correlations {
        if correlations.columns.len() < 2
            || correlations.columns.len() > MAX_NUMERIC_CORRELATION_COLUMNS
            || correlations.sampled_row_count > MAX_NUMERIC_CORRELATION_SAMPLE_ROWS
            || correlations.pairs.iter().any(|pair| {
                pair.first_column == pair.second_column
                    || !correlations.columns.contains(&pair.first_column)
                    || !correlations.columns.contains(&pair.second_column)
                    || pair.coefficient.is_some_and(|coefficient| {
                        !coefficient.is_finite() || !(-1.0..=1.0).contains(&coefficient)
                    })
            })
        {
            return Err("El perfil guardado contiene correlaciones no válidas.".to_owned());
        }
    }
    if let Some(summaries) = &profile.categorical_group_summaries {
        if summaries.len() > MAX_CATEGORICAL_GROUP_COLUMNS
            || summaries.iter().any(|summary| {
                summary.groups.is_empty()
                    || summary.groups.len() > MAX_CATEGORICAL_GROUPS + 1
                    || summary.distinct_count < 2
                    || summary.groups.iter().filter(|group| group.is_other).count() > 1
                    || summary.groups.iter().any(|group| {
                        group.label.is_empty()
                            || group.row_count == 0
                            || !group.percentage.is_finite()
                            || !(0.0..=100.0).contains(&group.percentage)
                            || group.label.chars().count() > MAX_GROUP_LABEL_CHARS
                    })
                    || summary
                        .groups
                        .iter()
                        .map(|group| group.row_count)
                        .sum::<usize>()
                        != profile.row_count
            })
        {
            return Err(
                "El perfil guardado contiene resúmenes de categorías no válidos.".to_owned(),
            );
        }
    }
    if let Some(summaries) = &profile.temporal_series {
        if summaries.len() > MAX_TEMPORAL_COLUMNS
            || summaries.iter().any(|summary| {
                !matches!(summary.granularity.as_str(), "day" | "month" | "year")
                    || summary.periods.is_empty()
                    || summary.periods.len() > MAX_TEMPORAL_PERIODS
                    || summary.parsed_row_count == 0
                    || summary.parsed_row_count > profile.row_count
                    || summary.unparsed_row_count
                        != profile.row_count.saturating_sub(summary.parsed_row_count)
                    || summary.periods.iter().any(|period| {
                        period.period.is_empty()
                            || period.period.chars().count() > 64
                            || period.row_count > summary.parsed_row_count
                            || !period.percentage.is_finite()
                            || !(0.0..=100.0).contains(&period.percentage)
                    })
                    || summary
                        .periods
                        .iter()
                        .map(|period| period.row_count)
                        .sum::<usize>()
                        != summary.parsed_row_count
            })
        {
            return Err("El perfil guardado contiene tendencias temporales no válidas.".to_owned());
        }
    }
    if profile.row_count != frame.height()
        || profile.columns.len() != frame.width()
        || profile
            .columns
            .iter()
            .zip(frame.columns())
            .any(|(stored, column)| {
                stored.name != column.name().as_str()
                    || stored.data_type != column.dtype().to_string()
                    || stored.null_count > profile.row_count
                    || stored.unique_count > profile.row_count
            })
    {
        return Err("El perfil guardado no coincide con el dataset.".to_owned());
    }
    Ok(())
}

pub(crate) fn load_recipe_for_automation(input: &Path) -> Result<TransformRecipe, String> {
    load_recipe_file(input).map(|document| document.recipe)
}

pub(crate) fn load_stored_recipe_for_automation(
    input: &Path,
) -> Result<StoredTransformRecipe, String> {
    load_recipe_file(input)
}

pub(crate) fn apply_recipe_for_automation(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<(DataFrame, bool), String> {
    validate_recipe_structure(recipe)?;
    let outcome = apply_recipe_to_frame(source, recipe)?;
    let candidate = outcome.0;
    let changed = !candidate.equals_missing(source);
    Ok((candidate, changed))
}

pub(crate) fn export_frame_for_automation(
    frame: &DataFrame,
    output: &Path,
    format: ExportFormat,
) -> Result<ExportResult, String> {
    export_frame_atomic(frame, output, format, |_, _| {}, || false)
}

pub(crate) fn export_frame_for_automation_with_recipe(
    frame: &DataFrame,
    output: &Path,
    format: ExportFormat,
    recipe: Option<&StoredTransformRecipe>,
) -> Result<ExportResult, String> {
    if recipe.is_none() {
        return export_frame_for_automation(frame, output, format);
    }
    export_frame_atomic_with_privacy_and_quality_and_recipe(
        frame,
        output,
        format,
        PrivacyMode::None,
        None,
        recipe,
        |_, _| {},
        || false,
    )
}

pub(crate) struct ActiveDatasetSnapshot {
    pub(crate) frame: DataFrame,
    pub(crate) file_name: String,
    pub(crate) row_count: usize,
    pub(crate) column_count: usize,
    pub(crate) profile: Option<DatasetProfile>,
    pub(crate) history: ProjectHistoryCapture,
}

pub(crate) struct ProjectHistoryCaptureEntry {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

pub(crate) struct ProjectHistoryCapture {
    _directory: tempfile::TempDir,
    pub(crate) entries: Vec<ProjectHistoryCaptureEntry>,
    pub(crate) cursor: usize,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) current_label: String,
    pub(crate) max_entries: usize,
    pub(crate) disk_budget_bytes: u64,
}

pub(crate) struct ProjectHistoryRestoreEntry {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

pub(crate) struct ProjectHistoryRestore {
    pub(crate) entries: Vec<ProjectHistoryRestoreEntry>,
    pub(crate) cursor: usize,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) current_label: String,
    pub(crate) max_entries: usize,
    pub(crate) disk_budget_bytes: u64,
}

pub(crate) struct ProjectDatasetCandidate {
    loaded: LoadedDataset,
    preview: DatasetPreview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectHistorySummary {
    pub(crate) entry_count: usize,
    pub(crate) current_index: usize,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) snapshots_enabled: bool,
    pub(crate) degraded: bool,
}

impl ProjectDatasetCandidate {
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.loaded.frame.height(), self.loaded.frame.width())
    }

    pub(crate) fn frame(&self) -> &DataFrame {
        &self.loaded.frame
    }

    pub(crate) fn history_summary(&self) -> ProjectHistorySummary {
        let state = self.loaded.history.state();
        ProjectHistorySummary {
            entry_count: state.entry_count,
            current_index: state.current_index,
            can_undo: state.can_undo,
            can_redo: state.can_redo,
            snapshots_enabled: state.snapshots_enabled,
            degraded: state.degraded_reason.is_some(),
        }
    }

    pub(crate) fn into_frame(self) -> DataFrame {
        self.loaded.frame
    }
}

fn validate_history_label(label: &str) -> Result<(), String> {
    let length = label.chars().count();
    if !(1..=256).contains(&length) {
        return Err("El historial del proyecto contiene una etiqueta no válida.".to_owned());
    }
    Ok(())
}

fn capture_project_history(
    history: &HistoryManager,
    current_frame: &DataFrame,
) -> Result<ProjectHistoryCapture, String> {
    if history.max_entries == 0
        || history.max_entries > HISTORY_MAX_ENTRIES
        || history.disk_budget_bytes > HISTORY_DISK_BUDGET_BYTES
        || history.entries.len() > history.max_entries
    {
        return Err("El historial activo supera los límites del proyecto.".to_owned());
    }
    validate_history_label(&history.current_label)?;
    if history.snapshots_enabled {
        if history.entries.is_empty()
            || history.cursor >= history.entries.len()
            || history.degraded_reason.is_some()
        {
            return Err("El historial activo no tiene un estado consistente.".to_owned());
        }
    } else if !history.entries.is_empty()
        || history.cursor != 0
        || history
            .degraded_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err("El historial degradado no tiene un estado consistente.".to_owned());
    }

    let directory = tempfile::tempdir()
        .map_err(|_| "No se pudo preparar el historial del proyecto.".to_owned())?;
    let mut entries = Vec::with_capacity(history.entries.len());
    let mut total_bytes = 0_u64;
    let mut cursor_matches = !history.snapshots_enabled;
    for (index, entry) in history.entries.iter().enumerate() {
        validate_history_label(&entry.label)?;
        let metadata = fs::symlink_metadata(&entry.path)
            .map_err(|_| "No se pudo leer el historial activo.".to_owned())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.bytes
        {
            return Err("El historial activo contiene un snapshot no válido.".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(entry.bytes)
            .ok_or_else(|| "El historial activo supera los límites del proyecto.".to_owned())?;
        if total_bytes > history.disk_budget_bytes {
            return Err("El historial activo supera el presupuesto de disco.".to_owned());
        }
        let destination = directory.path().join(format!("entry-{index:03}.parquet"));
        let copied = fs::copy(&entry.path, &destination)
            .map_err(|_| "No se pudo preparar el historial del proyecto.".to_owned())?;
        if copied != entry.bytes {
            return Err("El historial activo cambió mientras se guardaba.".to_owned());
        }
        let staged_frame = read_parquet_frame(&destination)
            .map_err(|_| "El historial activo contiene un snapshot corrupto.".to_owned())?;
        if index == history.cursor {
            cursor_matches = staged_frame.equals_missing(current_frame);
        }
        entries.push(ProjectHistoryCaptureEntry {
            label: entry.label.clone(),
            path: destination,
            bytes: copied,
        });
    }
    if !cursor_matches {
        return Err("El cursor del historial activo no coincide con el dataset.".to_owned());
    }
    Ok(ProjectHistoryCapture {
        _directory: directory,
        entries,
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason.clone(),
        current_label: history.current_label.clone(),
        max_entries: history.max_entries,
        disk_budget_bytes: history.disk_budget_bytes,
    })
}

fn restore_project_history(
    current_frame: &DataFrame,
    history: ProjectHistoryRestore,
) -> Result<HistoryManager, String> {
    if history.max_entries == 0
        || history.max_entries > HISTORY_MAX_ENTRIES
        || history.disk_budget_bytes > HISTORY_DISK_BUDGET_BYTES
        || history.entries.len() > history.max_entries
    {
        return Err("El historial guardado supera los límites admitidos.".to_owned());
    }
    validate_history_label(&history.current_label)?;
    if history.snapshots_enabled {
        if history.entries.is_empty()
            || history.cursor >= history.entries.len()
            || history.degraded_reason.is_some()
        {
            return Err("El historial guardado no tiene un cursor válido.".to_owned());
        }
    } else if !history.entries.is_empty()
        || history.cursor != 0
        || history
            .degraded_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty() || reason.chars().count() > 1024)
    {
        return Err("El historial degradado guardado no es válido.".to_owned());
    }

    let directory = tempfile::tempdir()
        .map_err(|_| "No se pudo preparar el historial restaurado.".to_owned())?;
    let mut entries = Vec::with_capacity(history.entries.len());
    let mut cursor_matches = !history.snapshots_enabled;
    let mut total_bytes = 0_u64;
    for (index, entry) in history.entries.into_iter().enumerate() {
        validate_history_label(&entry.label)?;
        let metadata = fs::symlink_metadata(&entry.path)
            .map_err(|_| "Un snapshot del historial no está disponible.".to_owned())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.bytes
        {
            return Err("Un snapshot del historial no es válido.".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(entry.bytes)
            .ok_or_else(|| "El historial guardado supera su presupuesto.".to_owned())?;
        if total_bytes > history.disk_budget_bytes {
            return Err("El historial guardado supera su presupuesto.".to_owned());
        }
        let frame = read_parquet_frame(&entry.path)
            .map_err(|_| "Un snapshot del historial no contiene un Parquet válido.".to_owned())?;
        let destination = directory
            .path()
            .join(format!("snapshot-{index:020}.parquet"));
        let copied = fs::copy(&entry.path, &destination)
            .map_err(|_| "No se pudo copiar el historial restaurado.".to_owned())?;
        if copied != entry.bytes {
            return Err("Un snapshot del historial cambió durante la apertura.".to_owned());
        }
        if index == history.cursor {
            cursor_matches = frame.equals_missing(current_frame);
        }
        entries.push(HistoryEntry {
            label: entry.label,
            path: destination,
            bytes: copied,
        });
    }
    if !cursor_matches {
        return Err("El cursor del historial no coincide con el dataset actual.".to_owned());
    }
    let next_id = entries.len() as u64;
    Ok(HistoryManager {
        directory,
        entries,
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason,
        current_label: history.current_label,
        next_id,
        max_entries: history.max_entries,
        disk_budget_bytes: history.disk_budget_bytes,
    })
}

impl DatasetState {
    pub(crate) fn for_project_import(frame: DataFrame, file_name: String) -> Result<Self, String> {
        let visible = Path::new(&file_name);
        if file_name.trim().is_empty()
            || file_name.chars().count() > 255
            || visible.file_name().and_then(OsStr::to_str) != Some(file_name.as_str())
        {
            return Err("La identidad visible del dataset no es válida.".to_owned());
        }
        let history = HistoryManager::new(&frame)
            .map_err(|_| "No se pudo iniciar el historial del proyecto.".to_owned())?;
        Ok(Self {
            current: Mutex::new(Some(LoadedDataset {
                source_path: None,
                file_name,
                file_size_bytes: 0,
                frame,
                profile: None,
                history,
            })),
            ..Self::default()
        })
    }

    pub(crate) fn apply_project_import_recipe(
        &self,
        recipe: &TransformRecipe,
    ) -> Result<bool, String> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| "La sesión de importación no está disponible.".to_owned())?;
        let dataset = current
            .as_mut()
            .ok_or_else(|| "La importación no contiene un dataset.".to_owned())?;
        apply_recipe_to_dataset(dataset, recipe).map(|result| result.changed)
    }

    pub(crate) fn apply_project_import_deterministic_cleaning(
        &self,
        applied_operations: &[String],
    ) -> Result<bool, String> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| "La sesión de importación no está disponible.".to_owned())?;
        let dataset = current
            .as_mut()
            .ok_or_else(|| "La importación no contiene un dataset.".to_owned())?;
        let selected_outlier_modes = ["cap_outliers", "impute_outliers", "drop_outliers"]
            .iter()
            .filter(|mode| {
                applied_operations
                    .iter()
                    .any(|operation| operation == **mode)
            })
            .count();
        if selected_outlier_modes > 1 {
            return Err(
                "Las estrategias de outliers son excluyentes: elige capear, imputar o eliminar."
                    .to_owned(),
            );
        }
        let mut cleaned = dataset.frame.clone();
        let mut changed = false;

        // DataPrep ejecuta el registro de limpieza en un orden fijo. Mantener
        // ese orden evita que el orden accidental del manifiesto cambie el
        // resultado cuando una sesión enumera varias operaciones.
        for operation in [
            "drop_duplicates",
            "drop_high_null_cols",
            "drop_id_cols",
            "drop_empty_cols",
            "drop_constant_cols",
            "drop_empty_rows",
            "normalize_sentinels",
            "impute_numeric",
            "impute_categorical",
            "parse_dates",
            "trim_text",
            "normalize_text",
            "fix_encoding",
            "cast_numeric",
            "cap_outliers",
            "impute_outliers",
            "drop_outliers",
            "normalize_booleans",
            "mask_pii",
            "drop_fuzzy_duplicates",
            "normalize_columns",
            "add_cambios_col",
        ] {
            if !applied_operations
                .iter()
                .any(|candidate| candidate == operation)
            {
                continue;
            }
            let (candidate, _, changed_cell_count, _) = match operation {
                "drop_duplicates" => {
                    let (candidate, affected_row_count) = remove_duplicate_rows(&cleaned)?;
                    (
                        candidate,
                        affected_row_count,
                        affected_row_count,
                        Vec::new(),
                    )
                }
                "drop_high_null_cols" => {
                    let (candidate, removed_columns) =
                        remove_dataprep_high_null_columns_from_frame(&cleaned)?;
                    (candidate, 0, removed_columns.len(), Vec::new())
                }
                "drop_id_cols" => {
                    let (candidate, removed_columns) =
                        remove_dataprep_identifier_columns_from_frame(&cleaned)?;
                    (candidate, 0, removed_columns.len(), Vec::new())
                }
                "drop_empty_cols" => {
                    let (candidate, removed_columns) = remove_empty_columns_from_frame(&cleaned)?;
                    (candidate, 0, removed_columns.len(), Vec::new())
                }
                "drop_constant_cols" => {
                    let (candidate, removed_columns) =
                        remove_constant_columns_from_frame(&cleaned)?;
                    (candidate, 0, removed_columns.len(), Vec::new())
                }
                "drop_empty_rows" => {
                    let (candidate, affected_row_count) =
                        remove_null_only_rows_from_frame(&cleaned)?;
                    (
                        candidate,
                        affected_row_count,
                        affected_row_count,
                        Vec::new(),
                    )
                }
                "impute_numeric" => impute_dataprep_numeric_values_in_frame(&cleaned)?,
                "parse_dates" => parse_dataprep_date_columns(&cleaned)?,
                "normalize_sentinels" => {
                    clean_text_columns(&cleaned, None, TextCleaningMode::Sentinels)?
                }
                "fix_encoding" => {
                    clean_text_columns(&cleaned, None, TextCleaningMode::FixEncoding)?
                }
                "trim_text" => clean_text_columns(&cleaned, None, TextCleaningMode::Trim)?,
                "normalize_text" => clean_text_columns(
                    &cleaned,
                    None,
                    TextCleaningMode::Normalize {
                        remove_accents: true,
                    },
                )?,
                "cast_numeric" => cast_dataprep_numeric_columns(&cleaned)?,
                "cap_outliers" => apply_dataprep_outlier_mode(&cleaned, DataprepOutlierMode::Cap)?,
                "impute_outliers" => impute_outlier_values_in_frame(&cleaned)?,
                "drop_outliers" => {
                    apply_dataprep_outlier_mode(&cleaned, DataprepOutlierMode::Drop)?
                }
                "normalize_booleans" => normalize_dataprep_boolean_columns(&cleaned)?,
                "mask_pii" => {
                    // DataPrep's default mode is ``mask``. The session
                    // manifest does not carry a portable HMAC key, so only
                    // the conservative local mask is replayed here; a
                    // materialized snapshot remains the exact source of
                    // truth whenever one is available.
                    let (candidate, _, changed_cell_count) =
                        mask_personal_values_from_frame(&cleaned)?;
                    (candidate, 0, changed_cell_count, Vec::new())
                }
                "drop_fuzzy_duplicates" => {
                    // DataPrep deliberately skips fuzzy deduplication above
                    // 5,000 rows. Keep that guard in the source fallback and
                    // use Columnia's normalized full-row fingerprint, which
                    // preserves exact repeats and the earliest row.
                    if cleaned.height() > 5_000 {
                        (cleaned.clone(), 0, 0, Vec::new())
                    } else {
                        let (candidate, affected_row_count) = remove_near_duplicate_rows(&cleaned)?;
                        (
                            candidate,
                            affected_row_count,
                            affected_row_count,
                            Vec::new(),
                        )
                    }
                }
                "normalize_columns" => {
                    let (names, renames) = normalized_column_names(&cleaned);
                    if renames.is_empty() {
                        (cleaned.clone(), 0, 0, Vec::new())
                    } else {
                        let mut candidate = cleaned.clone();
                        candidate.set_column_names(&names).map_err(|error| {
                            format!("No se pudieron normalizar las columnas: {error}")
                        })?;
                        (candidate, 0, renames.len(), Vec::new())
                    }
                }
                "add_cambios_col" => {
                    let (candidate, added) = add_audit_column_to_frame(&cleaned)?;
                    (candidate, 0, usize::from(added), Vec::new())
                }
                "impute_categorical" => impute_categorical_values_in_frame(&cleaned)?,
                _ => unreachable!("operación determinista no registrada"),
            };
            if changed_cell_count > 0 {
                cleaned = candidate;
                changed = true;
            }
        }

        if !changed {
            return Ok(false);
        }
        publish_candidate(dataset, cleaned, "Limpieza DataPrep").map(|_| true)
    }

    pub(crate) fn cache_project_import_profile(&self) -> Result<DatasetProfile, String> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| "La sesión de importación no está disponible.".to_owned())?;
        let dataset = current
            .as_mut()
            .ok_or_else(|| "La importación no contiene un dataset.".to_owned())?;
        let profile = profile_dataset_with_progress(&dataset.frame, |_, _| {}, || false)?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    }

    pub(crate) fn active_project_snapshot(&self) -> Result<ActiveDatasetSnapshot, String> {
        let current = self
            .current
            .lock()
            .map_err(|_| "La sesión de datos no está disponible.".to_owned())?;
        let dataset = current
            .as_ref()
            .ok_or_else(|| "Carga un dataset antes de guardar un proyecto.".to_owned())?;
        Ok(ActiveDatasetSnapshot {
            frame: dataset.frame.clone(),
            file_name: dataset.file_name.clone(),
            row_count: dataset.frame.height(),
            column_count: dataset.frame.width(),
            profile: dataset.profile.clone(),
            history: capture_project_history(&dataset.history, &dataset.frame)?,
        })
    }

    pub(crate) fn prepare_project_candidate(
        snapshot_path: PathBuf,
        file_name: String,
    ) -> Result<ProjectDatasetCandidate, String> {
        Self::prepare_durable_project_candidate(snapshot_path, file_name, None, None)
    }

    pub(crate) fn prepare_durable_project_candidate(
        snapshot_path: PathBuf,
        file_name: String,
        profile: Option<DatasetProfile>,
        history: Option<ProjectHistoryRestore>,
    ) -> Result<ProjectDatasetCandidate, String> {
        let frame = read_parquet_frame(&snapshot_path)
            .map_err(|_| "No se pudo restaurar el dataset del proyecto.".to_owned())?;
        let file_size_bytes = fs::metadata(&snapshot_path)
            .map_err(|_| "No se pudo verificar el snapshot del proyecto.".to_owned())?
            .len();
        let preview = dataset_preview_with_size(&file_name, file_size_bytes, &frame)
            .map_err(|_| "No se pudo preparar el dataset del proyecto.".to_owned())?;
        if let Some(profile) = profile.as_ref() {
            validate_project_profile(&frame, profile)?;
        }
        let history = match history {
            Some(history) => restore_project_history(&frame, history)?,
            None => HistoryManager::new(&frame)
                .map_err(|_| "No se pudo iniciar el historial temporal del proyecto.".to_owned())?,
        };
        Ok(ProjectDatasetCandidate {
            loaded: LoadedDataset {
                source_path: None,
                file_name,
                file_size_bytes,
                frame,
                profile,
                history,
            },
            preview,
        })
    }

    pub(crate) fn activate_project_candidate(
        &self,
        candidate: ProjectDatasetCandidate,
    ) -> Result<DatasetPreview, String> {
        let ProjectDatasetCandidate { loaded, preview } = candidate;
        *self
            .current
            .lock()
            .map_err(|_| "La sesión de datos no está disponible.".to_owned())? = Some(loaded);
        self.pending_selection
            .lock()
            .map_err(|_| "La selección local no está disponible.".to_owned())?
            .take();
        self.profile_generation.fetch_add(1, Ordering::SeqCst);
        self.export_generation.fetch_add(1, Ordering::SeqCst);
        Ok(preview)
    }
}

pub(crate) fn canonicalize_file_for_automation(input: &Path) -> Result<PathBuf, String> {
    canonicalize_existing_file(input, "el archivo de automatización")
}

pub(crate) fn canonicalize_output_for_automation(output: &Path) -> Result<PathBuf, String> {
    canonicalize_write_destination(output, "la salida de automatización")
}

fn sample_dataset_definition(sample_id: &str) -> Result<&'static SampleDatasetDefinition, String> {
    SAMPLE_DATASETS
        .iter()
        .find(|sample| sample.id == sample_id)
        .ok_or_else(|| "El dataset de ejemplo solicitado no está disponible.".to_owned())
}

fn ensure_sample_dataset(app_data_dir: &Path, sample_id: &str) -> Result<PathBuf, String> {
    let sample = sample_dataset_definition(sample_id)?;
    match fs::symlink_metadata(app_data_dir) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) || !metadata.is_dir() => {
            return Err("El almacenamiento local de ejemplos no es seguro.".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(app_data_dir).map_err(|_| {
                "No se pudo preparar el almacenamiento local de ejemplos.".to_owned()
            })?;
        }
        Err(_) => {
            return Err("No se pudo verificar el almacenamiento local de ejemplos.".to_owned());
        }
    }
    let examples_dir = app_data_dir.join("examples");
    match fs::symlink_metadata(&examples_dir) {
        Ok(metadata) if is_symbolic_link_or_reparse_point(&metadata) || !metadata.is_dir() => {
            return Err("El almacenamiento local de ejemplos no es seguro.".to_owned());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&examples_dir).map_err(|_| {
                "No se pudo preparar el almacenamiento local de ejemplos.".to_owned()
            })?;
        }
        Err(_) => {
            return Err("No se pudo verificar el almacenamiento local de ejemplos.".to_owned());
        }
    }

    let path = examples_dir.join(sample.file_name);
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = file
                .write_all(sample.content.as_bytes())
                .and_then(|_| file.sync_all())
            {
                let _ = fs::remove_file(&path);
                return Err(format!(
                    "No se pudo preparar el dataset de ejemplo: {error}"
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => {
            return Err("No se pudo preparar el dataset de ejemplo.".to_owned());
        }
    }

    canonicalize_existing_file(&path, "el dataset de ejemplo")
}

#[tauri::command]
pub fn list_sample_datasets() -> Vec<SampleDatasetDescriptor> {
    SAMPLE_DATASETS
        .iter()
        .map(|sample| SampleDatasetDescriptor {
            id: sample.id.to_owned(),
            name: sample.name.to_owned(),
            format: sample.format.to_owned(),
            description: sample.description.to_owned(),
        })
        .collect()
}

#[tauri::command]
pub async fn inspect_sample_dataset(
    app: AppHandle,
    sample_id: String,
) -> Result<DatasetSourceInspection, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "No se pudo resolver el almacenamiento local de ejemplos.".to_owned())?;
    let path = ensure_sample_dataset(&app_data_dir, &sample_id)?;
    inspect_dataset_path(&app, path).await
}

fn apply_recipe_to_dataset(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
) -> Result<TransformRecipeResult, String> {
    validate_recipe_structure(recipe)?;
    let (
        candidate,
        renamed_column_count,
        converted_column_count,
        parsed_date_column_count,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        kept_order_changed,
        split_column_count,
        merged_column_count,
        dropped_source_column_count,
        adjusted_outlier_cell_count,
        outlier_removed_row_count,
        outlier_column_count,
        group_count,
        aggregated_column_count,
        collapsed_row_count,
        normalized_contact_cell_count,
        normalized_contact_column_count,
        extracted_column_count,
    ) = apply_recipe_to_frame(&dataset.frame, recipe)?;
    let changed = renamed_column_count
        + converted_column_count
        + parsed_date_column_count
        + removed_row_count
        + calculated_column_count
        + replaced_cell_count
        + dropped_column_count
        + usize::from(kept_order_changed)
        + split_column_count
        + merged_column_count
        + dropped_source_column_count
        + adjusted_outlier_cell_count
        + outlier_removed_row_count
        + usize::from(recipe.group_summary.is_some())
        + normalized_contact_cell_count
        + extracted_column_count
        > 0;
    // Build every fallible response value before publishing the candidate. This keeps the
    // transaction atomic even if, for example, the source file disappeared after loading.
    let preview = if changed {
        publish_candidate(dataset, candidate, "Aplicar receta de transformación")?
    } else {
        loaded_dataset_preview(dataset, &dataset.frame)?
    };
    Ok(TransformRecipeResult {
        dataset: preview,
        renamed_column_count,
        converted_column_count,
        parsed_date_column_count,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        split_column_count,
        merged_column_count,
        dropped_source_column_count,
        adjusted_outlier_cell_count,
        outlier_removed_row_count,
        outlier_column_count,
        group_count,
        aggregated_column_count,
        collapsed_row_count,
        normalized_contact_cell_count,
        normalized_contact_column_count,
        extracted_column_count,
        changed,
    })
}

#[tauri::command]
pub async fn apply_transform_recipe(
    app: AppHandle,
    recipe: TransformRecipe,
) -> Result<TransformRecipeResult, String> {
    validate_recipe_structure(&recipe)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        apply_recipe_to_dataset(dataset, &recipe)
    })
    .await
    .map_err(|error| format!("La receta estructural se interrumpió: {error}"))?
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use ::zip::ZipArchive;

    fn temporary_csv(contents: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj del sistema debe ser válido")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "columnia-dataset-test-{}-{nonce}.csv",
            std::process::id()
        ));
        let mut file = File::create(&path).expect("se debe poder crear el CSV temporal");
        file.write_all(contents.as_bytes())
            .expect("se debe poder escribir el CSV temporal");
        path
    }

    #[test]
    fn sample_dataset_catalog_is_static_and_paths_stay_native() {
        let samples = list_sample_datasets();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].id, "quality");
        assert_eq!(samples[0].format, "csv");
        assert_eq!(samples[1].id, "temporal");
        assert_eq!(samples[1].format, "tsv");
        let serialized = serde_json::to_string(&samples).unwrap();
        assert!(!serialized.contains("examples"));
        assert!(!serialized.contains("app_data"));
    }

    #[test]
    fn sample_dataset_creation_is_allowlisted_and_reuses_a_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().join("app-data");
        let path = ensure_sample_dataset(&app_data_dir, "quality").unwrap();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("clientes_calidad.csv")
        );
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("cliente_id,nombre"));
        let second_path = ensure_sample_dataset(&app_data_dir, "quality").unwrap();
        assert_eq!(second_path, path);
        assert!(ensure_sample_dataset(&app_data_dir, "unknown").is_err());
    }

    fn complete_stored_recipe() -> StoredTransformRecipe {
        build_stored_recipe(
            TransformRecipe {
                renames: vec![RecipeRename {
                    from: "old".to_owned(),
                    to: "new".to_owned(),
                }],
                casts: vec![RecipeCast {
                    column: "amount".to_owned(),
                    target: RecipeCastTarget::Decimal,
                }],
                date_parses: vec![RecipeDateParse {
                    column: "created".to_owned(),
                    format: RecipeDateFormat::Iso8601,
                    target: RecipeDateTarget::Datetime,
                }],
                filters: vec![RecipeFilter {
                    column: "status".to_owned(),
                    operator: RecipeFilterOperator::Eq,
                    value: Some("active".to_owned()),
                }],
                calculated_column: Some(CalculatedColumnRecipe {
                    name: "total".to_owned(),
                    source: "amount".to_owned(),
                    operation: CalculatedOperation::Multiply,
                    operand: Some(CalculatedOperand {
                        kind: CalculatedOperandKind::Literal,
                        value: "2".to_owned(),
                    }),
                }),
                find_replace: Some(FindReplaceRecipe {
                    scope: FindReplaceScope::Column,
                    column: Some("city".to_owned()),
                    find: "SD".to_owned(),
                    replace: "Santo Domingo".to_owned(),
                }),
                keep_columns: Some(vec!["new".to_owned(), "total".to_owned()]),
                split_column: Some(SplitColumnRecipe {
                    source: "full_name".to_owned(),
                    delimiter: " ".to_owned(),
                    names: vec!["first_name".to_owned(), "last_name".to_owned()],
                    drop_source: true,
                }),
                merge_columns: Some(MergeColumnsRecipe {
                    sources: vec!["city".to_owned(), "country".to_owned()],
                    name: "location".to_owned(),
                    separator: ", ".to_owned(),
                    drop_sources: false,
                }),
                outlier_treatments: vec![OutlierTreatment {
                    column: "amount".to_owned(),
                    action: OutlierAction::Cap,
                }],
                group_summary: Some(GroupSummaryRecipe {
                    group_by: vec!["country".to_owned()],
                    aggregations: vec![SummaryAggregation {
                        column: "amount".to_owned(),
                        operation: SummaryOperation::Mean,
                    }],
                }),
                contact_normalizations: vec![ContactNormalization {
                    column: "email".to_owned(),
                    kind: ContactKind::Email,
                }],
                text_extractions: vec![TextExtraction {
                    source: "code".to_owned(),
                    kind: ExtractionKind::Digits,
                    name: "code_number".to_owned(),
                    delimiter: None,
                }],
            },
            "Limpieza completa".to_owned(),
        )
        .expect("la receta de prueba debe ser válida")
    }

    #[test]
    fn recipe_file_roundtrips_every_supported_operation_without_exposing_a_path() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("limpieza.json");
        let expected = complete_stored_recipe();

        save_recipe_atomic(&expected, &destination).expect("la receta debe guardarse");
        let loaded = load_recipe_file(&destination).expect("la receta debe volver a cargar");

        assert_eq!(loaded, expected);
        let public_json = serde_json::to_value(&loaded).expect("la respuesta debe serializarse");
        assert_eq!(public_json["version"], RECIPE_FILE_VERSION);
        assert!(public_json.get("path").is_none());
        assert!(!public_json
            .to_string()
            .contains(&directory.path().display().to_string()));
    }

    #[test]
    fn recipe_save_atomically_replaces_the_destination_and_leaves_no_temporary_file() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("recipe.json");
        fs::write(&destination, "contenido anterior").expect("se debe preparar el destino");
        let document = complete_stored_recipe();

        save_recipe_atomic(&document, &destination).expect("la receta debe reemplazarse");

        assert_eq!(load_recipe_file(&destination).unwrap(), document);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn recipe_load_rejects_future_versions_corruption_unknown_fields_and_oversize() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("recipe.json");

        fs::write(
            &path,
            r#"{"version":2,"name":"Futura","savedAt":"2026-01-01T00:00:00Z","recipe":{}}"#,
        )
        .unwrap();
        assert!(load_recipe_file(&path).unwrap_err().contains("versión 2"));

        fs::write(&path, b"{not-json").unwrap();
        assert!(load_recipe_file(&path)
            .unwrap_err()
            .contains("JSON no es válida"));

        fs::write(
            &path,
            r#"{"version":1,"name":"Extra","savedAt":"2026-01-01T00:00:00Z","recipe":{},"sourcePath":"secret.csv"}"#,
        )
        .unwrap();
        assert!(load_recipe_file(&path)
            .unwrap_err()
            .contains("unknown field"));

        fs::write(
            &path,
            r#"{"version":1,"name":"Fecha mala","savedAt":"ayer","recipe":{}}"#,
        )
        .unwrap();
        assert!(load_recipe_file(&path).unwrap_err().contains("RFC 3339"));

        let mut excessive = complete_stored_recipe();
        excessive.recipe.filters = vec![
            RecipeFilter {
                column: "status".to_owned(),
                operator: RecipeFilterOperator::IsNull,
                value: None,
            };
            4
        ];
        fs::write(&path, serde_json::to_vec(&excessive).unwrap()).unwrap();
        assert!(load_recipe_file(&path).unwrap_err().contains("máximo 3"));

        fs::write(&path, vec![b' '; RECIPE_FILE_LIMIT_BYTES as usize + 1]).unwrap();
        assert!(load_recipe_file(&path)
            .unwrap_err()
            .contains("supera el límite"));
    }

    #[test]
    fn imports_representable_dataprep_pipeline_recipe_without_paths() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("pipeline.json");
        let source = serde_json::json!({
            "version": 3,
            "name": "Pipeline DataPrep",
            "saved_at": "2026-08-24T00:00:00Z",
            "selected_cleaning_operations": ["normalize_text", "mask_pii"],
            "transform": {
                "rename_text": "old_name -> new_name",
                "dtype_col": "age",
                "dtype_type": "integer",
                "parse_date_cols": ["created_at"],
                "filters": [{"col": "age", "op": ">=", "val": 18}],
                "find_replace": {"col": "new_name", "find": "old", "replace": "new"},
                "keep_columns": ["new_name", "age", "created_at"]
            },
            "export": {
                "formats": ["csv", "xlsx", "database"],
                "selected_columns": ["new_name", "age"],
                "privacy_mode": "mask",
                "report_format": "html",
                "csv_separator": ";",
                "package_zip": true,
                "sql_dialect": "postgresql"
            }
        });
        fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();

        let loaded =
            load_recipe_file(&path).expect("la receta DataPrep representable debe importarse");
        let json = serde_json::to_value(&loaded).expect("la receta importada debe serializarse");

        assert_eq!(json["version"], RECIPE_FILE_VERSION);
        assert_eq!(json["name"], "Pipeline DataPrep");
        assert_eq!(json["recipe"]["renames"][0]["from"], "old_name");
        assert_eq!(json["recipe"]["casts"][0]["target"], "integer");
        assert_eq!(json["recipe"]["dateParses"][0]["format"], "iso8601");
        assert_eq!(json["recipe"]["filters"][0]["operator"], "gte");
        assert_eq!(json["recipe"]["findReplace"]["scope"], "column");
        assert_eq!(json["recipe"]["keepColumns"][2], "created_at");
        assert_eq!(json["exportOptions"]["formats"][0], "csv");
        assert_eq!(json["exportOptions"]["formats"][1], "excel");
        assert_eq!(json["exportOptions"]["selectedColumns"][1], "age");
        assert_eq!(json["exportOptions"]["privacyMode"], "mask");
        assert_eq!(
            json["migrationReport"]["session"]["appliedOperations"],
            serde_json::json!(["normalize_text", "mask_pii"])
        );
        assert!(json["migrationReport"]["convertedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "selected_cleaning_operations.normalize_text"));
        assert!(!json["migrationReport"]["omittedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "selected_cleaning_operations.mask_pii"));
        assert_eq!(json["migrationReport"]["sourceFormat"], "dataprep");
        assert_eq!(json["migrationReport"]["sourceVersion"], 3);
        assert!(json["migrationReport"]["convertedItems"].as_u64().unwrap() > 0);
        assert!(json["migrationReport"]["omittedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "export.report_format"));
        assert_eq!(
            json["migrationReport"]["artifactSha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert!(!json
            .to_string()
            .contains(&directory.path().display().to_string()));
    }

    #[test]
    fn imports_synthetic_session_fixture_without_restoring_session_paths() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("Cargo debe vivir dentro del repositorio")
            .join("fixtures/migration/dataprep-session-v1.json");
        let loaded = load_recipe_file(&fixture)
            .expect("la fixture de sesión DataPrep debe reducirse a receta revisable");
        let json = serde_json::to_value(&loaded).expect("la sesión convertida debe serializarse");
        let omitted = json["migrationReport"]["omittedOperations"]
            .as_array()
            .expect("el informe debe listar omisiones");

        assert!(omitted.iter().any(|value| value == "session.source_path"));
        assert!(omitted.iter().any(|value| value == "session.snapshot_path"));
        assert!(!omitted.iter().any(|value| value == "session.applied_ops"));
        assert!(json["migrationReport"]["convertedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "session.applied_ops.normalize_text"));
        assert!(omitted.iter().any(|value| value == "quality_rules"));
        assert!(omitted
            .iter()
            .any(|value| value == "session.analysis_checks"));
        assert_eq!(json["migrationReport"]["session"]["sheetName"], "Datos");
        assert_eq!(
            json["migrationReport"]["session"]["stageLabel"],
            "Transformación"
        );
        assert_eq!(
            json["migrationReport"]["session"]["appliedOperationCount"],
            1
        );
        assert_eq!(json["migrationReport"]["session"]["qualityRuleCount"], 1);
        assert_eq!(json["migrationReport"]["session"]["analysisCheckCount"], 1);
        assert_eq!(
            json["migrationReport"]["session"]["appliedOperations"],
            serde_json::json!(["normalize_text"])
        );
        assert_eq!(
            json["migrationReport"]["session"]["analysisChecks"],
            serde_json::json!(["completeness"])
        );
        assert_eq!(
            json["migrationReport"]["session"]["hasSourceReference"],
            true
        );
        assert_eq!(
            json["migrationReport"]["session"]["hasSnapshotReference"],
            true
        );
        assert!(!json.to_string().contains("fixture://"));
        assert!(!json.to_string().contains("ventas-sinteticas.csv"));
        assert_eq!(json["recipe"]["renames"][0]["to"], "new_name");
    }

    #[test]
    fn reports_non_portable_dataprep_artifacts_without_copying_their_contents() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("session-artifacts.json");
        let source = serde_json::json!({
            "version": 1,
            "name": "Sesión con artefactos externos",
            "analysis_results": {"private_column": ["no debe copiarse"]},
            "executionHistory": [{"query": "no debe copiarse"}],
            "cachePath": "profile-cache.json",
            "transform": {"rename_text": ""}
        });
        fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();

        let loaded = load_recipe_file(&path).expect("la sesión debe poder inspeccionarse");
        let json = serde_json::to_value(&loaded).expect("la sesión debe serializarse");
        assert_eq!(
            json["migrationReport"]["session"]["nonPortableArtifacts"],
            serde_json::json!(["analysis_results", "caches", "history"])
        );
        assert!(json["migrationReport"]["omittedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "session.analysis_results"));
        assert!(json["migrationReport"]["omittedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "session.history"));
        assert!(json["migrationReport"]["omittedOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "session.caches"));
        let serialized = json.to_string();
        assert!(!serialized.contains("private_column"));
        assert!(!serialized.contains("no debe copiarse"));
        assert!(!serialized.contains("profile-cache.json"));
    }

    #[test]
    fn imports_only_aggregate_analysis_sample_metadata_without_rows_or_values() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("session-sample-metadata.json");
        let source = serde_json::json!({
            "version": 3,
            "name": "Sesión con muestra agregada",
            "analysis_results": {
                "is_sampled": true,
                "sample_rows": 200,
                "n_total_rows": 1200,
                "rows": [{"email": "no debe copiarse"}],
                "private_result": "no debe copiarse"
            },
            "transform": {"rename_text": ""}
        });
        fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();

        let loaded = load_recipe_file(&path).expect("la sesión debe poder inspeccionarse");
        let json = serde_json::to_value(&loaded).expect("la sesión debe serializarse");
        let session = &json["migrationReport"]["session"];
        assert_eq!(session["analysisSampled"], true);
        assert_eq!(session["analysisSampleRowCount"], 200);
        assert_eq!(session["analysisTotalRowCount"], 1200);
        let serialized = json.to_string();
        assert!(!serialized.contains("no debe copiarse"));
        assert!(!serialized.contains("private_result"));
        assert!(serialized.contains("analysis_results"));
    }

    #[test]
    fn imports_camel_case_session_fixture_without_exposing_paths() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("Cargo debe vivir dentro del repositorio")
            .join("fixtures/migration/dataprep-session-v1-camel.json");
        let loaded = load_recipe_file(&fixture)
            .expect("la sesión camelCase DataPrep debe reducirse a receta revisable");
        let json = serde_json::to_value(&loaded).expect("la sesión convertida debe serializarse");
        let report = &json["migrationReport"];

        assert_eq!(report["session"]["sheetName"], "Datos");
        assert_eq!(report["session"]["stageLabel"], "Revisión");
        assert_eq!(report["session"]["appliedOperationCount"], 2);
        assert_eq!(report["session"]["qualityRuleCount"], 2);
        assert_eq!(report["session"]["analysisCheckCount"], 3);
        assert_eq!(
            report["session"]["appliedOperations"],
            serde_json::json!(["trim_text", "normalize_text"])
        );
        assert_eq!(
            report["session"]["analysisChecks"],
            serde_json::json!(["completeness", "duplicates", "outliers"])
        );
        assert_eq!(report["session"]["hasSourceReference"], true);
        assert_eq!(report["session"]["hasSnapshotReference"], true);
        assert!(!report["omittedOperations"]
            .as_array()
            .expect("el informe debe listar omisiones")
            .iter()
            .any(|value| value == "session.applied_ops"));
        assert!(report["convertedOperations"]
            .as_array()
            .expect("el informe debe listar conversiones")
            .iter()
            .any(|value| value == "session.applied_ops.normalize_text"));
        assert!(report["omittedOperations"]
            .as_array()
            .expect("el informe debe listar omisiones")
            .iter()
            .any(|value| value == "quality_rules"));
        assert!(!json.to_string().contains("fixture://"));
        assert!(!json.to_string().contains("ventas-camel.csv"));
    }

    #[test]
    fn imports_synthetic_legacy_fixture_as_columnia_recipe() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("Cargo debe vivir dentro del repositorio")
            .join("fixtures/migration/legacy-recipe-v1.json");
        let loaded = load_recipe_file(&fixture).expect("la fixture legacy debe ser compatible");
        assert_eq!(loaded.name, "Receta legacy sintética");
        assert_eq!(loaded.recipe.renames[0].from, "old_name");
        assert_eq!(loaded.recipe.casts[0].target, RecipeCastTarget::Integer);
        assert_eq!(
            loaded.migration_report.as_ref().unwrap().source_format,
            "legacy"
        );
    }

    #[test]
    fn rejects_ambiguous_dataprep_recipe_semantics_instead_of_dropping_them() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("pipeline.json");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "version": 3,
                "name": "Regex",
                "transform": {"find_replace": {"find": "^A", "replace": "B", "regex": true}}
            }))
            .unwrap(),
        )
        .unwrap();

        let error = load_recipe_file(&path).expect_err("la semántica regex no debe perderse");
        assert!(error.contains("expresiones regulares"));
    }

    #[test]
    fn ignores_empty_dataprep_operation_defaults_during_recipe_import() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("pipeline.json");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "version": 3,
                "name": "Pipeline sin operaciones",
                "transform": {
                    "rename_text": "",
                    "dtype_col": null,
                    "dtype_type": "auto",
                    "parse_date_cols": [],
                    "filters": [],
                    "find_replace": {
                        "col": null,
                        "find": "",
                        "replace": "",
                        "regex": false
                    },
                    "keep_columns": [],
                    "calc": {
                        "name": "",
                        "col_a": null,
                        "operation": "add",
                        "col_b_or_val": ""
                    },
                    "outliers": {"cap_cols": [], "drop_cols": []},
                    "split_column": {
                        "column": null,
                        "delimiter": "",
                        "new_names": [],
                        "drop_source": false
                    },
                    "merge_columns": {
                        "columns": [],
                        "name": "",
                        "separator": " ",
                        "drop_sources": false
                    },
                    "group_summary": {"group_by": [], "aggregations": {}},
                    "normalize_contacts": {
                        "enabled": false,
                        "auto_detect": true,
                        "phone_cols": [],
                        "email_cols": [],
                        "address_cols": []
                    },
                    "extract_text": {
                        "source_col": null,
                        "extraction": "",
                        "new_name": "",
                        "pattern_or_delimiter": ""
                    },
                    "true_values": [],
                    "false_values": []
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let loaded = load_recipe_file(&path)
            .expect("las configuraciones vacías de DataPrep deben ser no-op");
        let json = serde_json::to_value(&loaded).expect("la receta debe serializarse");
        assert!(json["recipe"]["splitColumn"].is_null());
    }

    #[test]
    fn recipe_semantic_budget_accepts_boundaries_and_rejects_large_apply_and_save_payloads() {
        let boundary_value = "x".repeat(MAX_RECIPE_TEXT_FIELD_CHARS);
        let boundary = TransformRecipe {
            keep_columns: Some(vec![boundary_value.clone(); 16]),
            ..Default::default()
        };
        assert_eq!(
            boundary
                .keep_columns
                .as_ref()
                .unwrap()
                .iter()
                .map(|value| value.chars().count())
                .sum::<usize>(),
            MAX_RECIPE_TOTAL_TEXT_CHARS
        );
        validate_recipe_structure(&boundary).expect("el límite exacto debe admitirse");

        let oversized_field = TransformRecipe {
            keep_columns: Some(vec!["x".repeat(MAX_RECIPE_TEXT_FIELD_CHARS + 1)]),
            ..Default::default()
        };
        let save_error = build_stored_recipe(oversized_field, "Receta grande".to_owned())
            .expect_err("guardar debe rechazar un campo desproporcionado");
        assert!(save_error.contains(&MAX_RECIPE_TEXT_FIELD_CHARS.to_string()));
        assert!(!save_error.contains(&"x".repeat(32)));

        let oversized_total = TransformRecipe {
            keep_columns: Some(vec![boundary_value; 17]),
            ..Default::default()
        };
        let path = temporary_csv("value\n1\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let mut dataset = loaded_dataset(path.clone(), frame);
        let apply_error = apply_recipe_to_dataset(&mut dataset, &oversized_total)
            .expect_err("aplicar debe rechazar el presupuesto total excedido");
        assert!(apply_error.contains(&MAX_RECIPE_TOTAL_TEXT_CHARS.to_string()));
        assert!(!apply_error.contains(&"x".repeat(32)));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    fn temporary_delimited(extension: &str, contents: &str) -> PathBuf {
        temporary_delimited_bytes(extension, contents.as_bytes())
    }

    fn loaded_dataset(path: PathBuf, frame: DataFrame) -> LoadedDataset {
        let history = HistoryManager::new(&frame).expect("el historial debe inicializarse");
        LoadedDataset {
            source_path: Some(path.clone()),
            file_name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("dataset.csv")
                .to_owned(),
            file_size_bytes: fs::metadata(&path).map(|value| value.len()).unwrap_or(0),
            frame,
            profile: None,
            history,
        }
    }

    fn temporary_delimited_bytes(extension: &str, contents: &[u8]) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj del sistema debe ser válido")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "columnia-delimited-test-{}-{nonce}.{extension}",
            std::process::id()
        ));
        let mut file = File::create(&path).expect("se debe crear el archivo temporal");
        file.write_all(contents)
            .expect("se debe escribir el archivo temporal");
        path
    }

    #[test]
    fn loads_schema_and_rows_from_a_csv() {
        let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");

        let (frame, preview) = load_csv(&path).expect("el CSV debe cargar");

        assert_eq!(frame.height(), 2);
        assert_eq!(
            preview.file_name,
            path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(preview.row_count, 2);
        assert_eq!(preview.column_count, 2);
        assert_eq!(preview.columns[0].name, "city");
        assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
        assert_eq!(preview.rows[0][0].as_deref(), Some("Santo Domingo"));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn reports_ordered_csv_loading_phases() {
        let path = temporary_csv("city\nSanto Domingo\n");
        let mut updates = Vec::new();

        load_csv_with_progress(
            &path,
            |stage, percent| updates.push((stage, percent)),
            || false,
        )
        .expect("el CSV debe cargar");

        assert_eq!(
            updates,
            vec![
                ("Validando archivo", 10),
                ("Leyendo y detectando columnas", 25),
                ("Preparando vista previa", 85),
                ("Preparando sesión", 95),
            ]
        );
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn rejects_unsupported_dataset_files() {
        let path = std::env::temp_dir().join("columnia-invalid-dataset.bin");
        File::create(&path).expect("se debe poder crear el archivo temporal");

        let error = validate_dataset_file(&path)
            .expect_err("un archivo que no es un dataset compatible debe rechazarse");

        assert!(error.contains("admite CSV, TSV, TXT delimitado, JSON, Parquet y libros Excel/ODS"));
        fs::remove_file(path).expect("se debe limpiar el archivo temporal");
    }

    #[test]
    fn canonicalizes_dataset_reads_and_write_parents() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("se debe crear la subcarpeta");
        let dataset = directory.path().join("datos.csv");
        fs::write(&dataset, "value\n1\n").expect("se debe crear el dataset");
        let non_canonical_dataset = nested.join("..").join("datos.csv");

        let (canonical_dataset, _, extension) = validate_dataset_file(&non_canonical_dataset)
            .expect("la ruta equivalente debe validarse");
        let destination =
            canonicalize_write_destination(&nested.join("..").join("salida.csv"), "la exportación")
                .expect("la carpeta de salida debe canonicalizarse");

        assert_eq!(canonical_dataset, fs::canonicalize(dataset).unwrap());
        assert_eq!(extension, "csv");
        assert_eq!(
            destination,
            fs::canonicalize(directory.path())
                .unwrap()
                .join("salida.csv")
        );
    }

    #[test]
    fn rejects_directories_as_read_sources_or_write_destinations() {
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");

        let read_error = canonicalize_existing_file(directory.path(), "el dataset seleccionado")
            .expect_err("un directorio no es un dataset");
        let write_error = canonicalize_write_destination(directory.path(), "la exportación")
            .expect_err("un directorio no es un archivo de destino");

        assert!(read_error.contains("archivo regular"));
        assert!(write_error.contains("archivo regular"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_for_reads_and_existing_destinations() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let target = directory.path().join("target.csv");
        let link = directory.path().join("link.csv");
        let dangling_link = directory.path().join("dangling.csv");
        fs::write(&target, "value\n1\n").expect("se debe crear el archivo real");
        symlink(&target, &link).expect("se debe crear el enlace simbólico");
        symlink(directory.path().join("missing.csv"), &dangling_link)
            .expect("se debe crear el enlace simbólico colgante");

        let read_error = canonicalize_existing_file(&link, "el dataset seleccionado")
            .expect_err("una lectura no debe seguir enlaces simbólicos");
        let write_error = canonicalize_write_destination(&link, "la exportación")
            .expect_err("una escritura no debe seguir enlaces simbólicos");
        let dangling_error = canonicalize_write_destination(&dangling_link, "la exportación")
            .expect_err("una escritura no debe aceptar enlaces simbólicos colgantes");

        assert!(read_error.contains("enlace simbólico"));
        assert!(write_error.contains("enlace simbólico"));
        assert!(dangling_error.contains("enlace simbólico"));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_reparse_points_including_dangling_links() {
        use std::io::ErrorKind;
        use std::os::windows::fs::symlink_file;

        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let target = directory.path().join("target.csv");
        let link = directory.path().join("link.csv");
        let dangling_link = directory.path().join("dangling.csv");
        fs::write(&target, "value\n1\n").expect("se debe crear el archivo real");

        match symlink_file(&target, &link) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
            Err(error) => panic!("no se pudo crear el enlace simbólico de prueba: {error}"),
        }
        match symlink_file(directory.path().join("missing.csv"), &dangling_link) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
            Err(error) => {
                panic!("no se pudo crear el enlace simbólico colgante de prueba: {error}")
            }
        }

        let read_error = canonicalize_existing_file(&link, "el dataset seleccionado")
            .expect_err("una lectura no debe seguir reparse points");
        let write_error = canonicalize_write_destination(&link, "la exportación")
            .expect_err("una escritura no debe seguir reparse points");
        let dangling_read_error =
            canonicalize_existing_file(&dangling_link, "el dataset seleccionado")
                .expect_err("una lectura no debe aceptar enlaces simbólicos colgantes");
        let dangling_write_error = canonicalize_write_destination(&dangling_link, "la exportación")
            .expect_err("una escritura no debe aceptar enlaces simbólicos colgantes");

        assert!(read_error.contains("punto de reanálisis"));
        assert!(write_error.contains("punto de reanálisis"));
        assert!(dangling_read_error.contains("punto de reanálisis"));
        assert!(dangling_write_error.contains("punto de reanálisis"));
    }

    #[test]
    fn accepts_a_dataset_above_the_previous_500_mebibyte_threshold() {
        let path = temporary_csv("value\n");
        let previous_limit = 500_u64 * 1024 * 1024;
        File::options()
            .write(true)
            .open(&path)
            .expect("se debe poder abrir el CSV temporal")
            .set_len(previous_limit + 1)
            .expect("se debe poder crear un archivo disperso grande");

        let (_, size, extension) = validate_dataset_file(&path)
            .expect("el tamaño no debe impedir seleccionar un dataset compatible");

        assert_eq!(size, previous_limit + 1);
        assert_eq!(extension, "csv");
        fs::remove_file(path).expect("se debe eliminar el CSV temporal");
    }

    #[test]
    fn returns_a_bounded_page_from_an_offset() {
        let path = temporary_csv("value\nfirst\nsecond\nthird\nfourth\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let page = dataset_page(&frame, 2, 2).expect("la página debe existir");

        assert_eq!(page.offset, 2);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0][0].as_deref(), Some("third"));
        assert_eq!(page.rows[1][0].as_deref(), Some("fourth"));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn rejects_oversized_pages() {
        let frame = DataFrame::empty();

        let error = dataset_page(&frame, 0, MAX_PAGE_SIZE + 1)
            .expect_err("una página excesiva debe rechazarse");

        assert!(error.contains("entre 1 y 200"));
    }

    #[test]
    fn local_query_is_read_only_projected_and_bounded() {
        let frame = df![
            "city" => &["Santo Domingo", "Santiago", "La Vega"],
            "value" => &[10_i64, 20_i64, 30_i64]
        ]
        .unwrap();
        let result =
            execute_local_query(&frame, "SELECT city, value FROM dataset LIMIT 1 OFFSET 1")
                .expect("la consulta segura debe ejecutarse");
        assert_eq!(
            result
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["city", "value"]
        );
        assert_eq!(result.row_count, 3);
        assert_eq!(result.offset, 1);
        assert_eq!(
            result.rows,
            vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
        );
        assert!(result.truncated);
        assert!(execute_local_query(&frame, "DELETE FROM dataset").is_err());
        assert!(execute_local_query(&frame, "SELECT * FROM dataset LIMIT 201").is_err());
        assert!(execute_local_query(&frame, "SELECT missing FROM dataset").is_err());
    }

    #[test]
    fn local_query_joins_only_the_loaded_comparison_and_pages_results() {
        let current = df![
            "id" => &[1_i64, 2, 3],
            "city" => &["Santo Domingo", "Santiago", "La Vega"]
        ]
        .unwrap();
        let compared = df![
            "id" => &[2_i64, 3],
            "segment" => &["B", "C"]
        ]
        .unwrap();

        let result = execute_local_query_with_comparison(
            &current,
            Some(&compared),
            "SELECT id, segment FROM dataset LEFT JOIN compared ON dataset.id = compared.id LIMIT 2",
        )
        .expect("el JOIN local debe usar la comparación cargada");

        assert_eq!(
            result
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["id", "segment"]
        );
        assert_eq!(result.row_count, 3);
        assert_eq!(result.offset, 0);
        assert_eq!(
            result.rows,
            vec![
                vec![Some("1".to_owned()), None],
                vec![Some("2".to_owned()), Some("B".to_owned())]
            ]
        );
        assert!(result.truncated);
    }

    #[test]
    fn local_query_join_requires_comparison_and_matching_key_types() {
        let current = df!["id" => &[1_i64], "value" => &[10_i64]].unwrap();
        let compared = df!["id" => &["1"], "segment" => &["A"]].unwrap();

        let missing = execute_local_query_with_comparison(
            &current,
            None,
            "SELECT * FROM dataset JOIN compared ON id = id LIMIT 1",
        )
        .expect_err("un JOIN sin comparación no debe leer una fuente arbitraria");
        assert!(missing.contains("No hay un dataset comparado cargado"));

        let incompatible = execute_local_query_with_comparison(
            &current,
            Some(&compared),
            "SELECT * FROM dataset JOIN compared ON id = id LIMIT 1",
        )
        .expect_err("las claves con tipos distintos deben rechazarse");
        assert!(incompatible.contains("tipos incompatibles"));

        assert!(execute_local_query_with_comparison(
            &current,
            Some(&current),
            "SELECT * FROM dataset RIGHT JOIN compared ON id = id LIMIT 1",
        )
        .is_err());
    }

    #[test]
    fn local_query_join_rejects_many_to_many_cardinality_before_materializing() {
        let current = DataFrame::new(
            1_501,
            vec![Series::new("id".into(), vec![1_i64; 1_501]).into_column()],
        )
        .expect("el dataset activo debe construirse");
        let compared = DataFrame::new(
            1_501,
            vec![Series::new("id".into(), vec![1_i64; 1_501]).into_column()],
        )
        .expect("el dataset comparado debe construirse");

        let error = execute_local_query_with_comparison(
            &current,
            Some(&compared),
            "SELECT id FROM dataset JOIN compared ON id = id LIMIT 1",
        )
        .expect_err("el fan-out del JOIN debe rechazarse antes de materializarse");

        assert!(error.contains("resultado estimado del JOIN"));
        assert!(error.contains(&LOCAL_QUERY_JOIN_MAX_RESULT_ROWS.to_string()));
    }

    #[test]
    fn local_query_aggregate_rejects_matching_rows_over_materialization_budget() {
        let row_count = LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS + 1;
        let frame = DataFrame::new(
            row_count,
            vec![Series::new("id".into(), vec![1_i64; row_count]).into_column()],
        )
        .expect("el dataset grande debe construirse");

        let error = execute_local_query(&frame, "SELECT COUNT(*) AS total FROM dataset")
            .expect_err("una agregación que excede el presupuesto debe rechazarse");

        assert!(error.contains("limita las filas coincidentes"));
        assert!(error.contains(&LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS.to_string()));
    }

    #[test]
    fn local_query_stops_at_cooperative_cancellation_point() {
        let frame = df!["id" => &[1_i64, 2, 3]].unwrap();
        let error =
            execute_local_query_with_cancel(&frame, "SELECT id FROM dataset LIMIT 1", &|| true)
                .expect_err("la consulta debe detenerse si se cancela antes de escanear");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    }

    #[test]
    fn local_query_filters_nulls_and_calculates_bounded_aggregates() {
        let frame = df![
            "city" => &[Some("Santo Domingo"), None, Some("Santiago"), Some("Santiago")],
            "value" => &[Some(10_i64), Some(20_i64), Some(30_i64), Some(40_i64)]
        ]
        .unwrap();

        let filtered = execute_local_query(
            &frame,
            "SELECT city, value FROM dataset WHERE city IS NOT NULL AND value >= 30 LIMIT 1",
        )
        .expect("el filtro local debe ejecutarse");
        assert_eq!(filtered.row_count, 2);
        assert_eq!(
            filtered.rows,
            vec![vec![Some("Santiago".to_owned()), Some("30".to_owned())]]
        );
        assert!(filtered.truncated);

        let aggregate = execute_local_query(
            &frame,
            "SELECT COUNT(*) AS total, AVG(value) AS average, MAX(value) AS highest FROM dataset WHERE value >= 20",
        )
        .expect("las agregaciones locales deben ejecutarse");
        assert_eq!(aggregate.row_count, 1);
        assert_eq!(
            aggregate
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["total", "average", "highest"]
        );
        assert_eq!(
            aggregate.rows,
            vec![vec![
                Some("3".to_owned()),
                Some("30".to_owned()),
                Some("40".to_owned())
            ]]
        );

        let grouped = execute_local_query(
            &frame,
            "SELECT city, COUNT(*) AS total, SUM(value) AS sum_value FROM dataset GROUP BY city LIMIT 10",
        )
        .expect("GROUP BY local debe ejecutarse");
        assert_eq!(grouped.row_count, 3);
        assert_eq!(
            grouped.rows[0],
            vec![
                Some("Santo Domingo".to_owned()),
                Some("1".to_owned()),
                Some("10".to_owned())
            ]
        );
        assert_eq!(
            grouped.rows[1],
            vec![None, Some("1".to_owned()), Some("20".to_owned())]
        );
        assert_eq!(
            grouped.rows[2],
            vec![
                Some("Santiago".to_owned()),
                Some("2".to_owned()),
                Some("70".to_owned())
            ]
        );

        assert!(
            execute_local_query(&frame, "SELECT city FROM dataset WHERE city = untrusted").is_err()
        );
        assert!(execute_local_query(&frame, "SELECT SUM(city) FROM dataset").is_err());
        assert!(execute_local_query(&frame, "SELECT city, COUNT(*) FROM dataset").is_err());
        assert!(execute_local_query(&frame, "SELECT city FROM dataset GROUP BY city").is_err());
    }

    #[test]
    fn local_query_parallel_blocks_preserve_page_order_and_aggregate_totals() {
        let row_count = LOCAL_QUERY_BLOCK_ROWS * 2 + 37;
        let ids = (0..row_count as i64).collect::<Vec<_>>();
        let values = ids.iter().map(|value| value * 2).collect::<Vec<_>>();
        let frame = DataFrame::new(
            row_count,
            vec![
                Series::new("id".into(), ids).into_column(),
                Series::new("value".into(), values).into_column(),
            ],
        )
        .expect("el dataset de prueba debe construirse");

        let offset = LOCAL_QUERY_BLOCK_ROWS - 2;
        let query =
            format!("SELECT id, value FROM dataset WHERE value >= 0 LIMIT 5 OFFSET {offset}");
        let first_page =
            execute_local_query(&frame, &query).expect("la página paralela debe ejecutarse");
        let second_page =
            execute_local_query(&frame, &query).expect("la página paralela debe ser determinista");

        assert_eq!(first_page, second_page);
        assert_eq!(first_page.row_count, row_count);
        assert_eq!(
            first_page.rows,
            (offset as i64..offset as i64 + 5)
                .map(|id| vec![Some(id.to_string()), Some((id * 2).to_string())])
                .collect::<Vec<_>>()
        );

        let aggregate = execute_local_query(
            &frame,
            "SELECT COUNT(*) AS total, SUM(value) AS total_value, AVG(value) AS average_value, MIN(value) AS minimum_value, MAX(value) AS maximum_value FROM dataset WHERE value >= 0",
        )
        .expect("la agregación paralela debe ejecutarse");
        let sum = (row_count as i64 * (row_count as i64 - 1)).to_string();
        let average = (row_count as f64 - 1.0).to_string();
        assert_eq!(
            aggregate.rows,
            vec![vec![
                Some(row_count.to_string()),
                Some(sum),
                Some(average),
                Some("0".to_owned()),
                Some(((row_count as i64 - 1) * 2).to_string()),
            ]]
        );
    }

    #[test]
    fn profiles_nulls_uniques_and_numeric_statistics() {
        let path = temporary_csv(
            "city,temperature\nSanto Domingo,30\nSantiago,\nSantiago,28\nSantiago,28\n,25\n",
        );
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let city = &profile.columns[0];
        let temperature = &profile.columns[1];

        assert_eq!(profile.row_count, 5);
        assert_eq!(profile.duplicate_row_count, 1);
        assert_eq!(profile.duplicate_percentage, 20.0);
        assert_eq!(city.unique_count, 2);
        assert_eq!(city.null_count, 1);
        assert_eq!(city.mean, None);
        assert_eq!(city.empty_count, Some(0));
        assert_eq!(city.minimum_length, Some(8));
        assert_eq!(city.maximum_length, Some(13));
        assert!((city.average_length.unwrap() - 9.25).abs() < 0.001);
        assert_eq!(city.suggested_type, None);
        assert_eq!(temperature.null_count, 1);
        assert_eq!(temperature.completeness_percentage, 80.0);
        assert_eq!(temperature.minimum.as_deref(), Some("25"));
        assert_eq!(temperature.maximum.as_deref(), Some("30"));
        assert_eq!(temperature.mean, Some(27.75));
        assert_eq!(temperature.empty_count, Some(0));
        assert_eq!(temperature.suggested_type, Some("integer".to_owned()));
        assert_eq!(temperature.first_quartile, Some(27.25));
        assert_eq!(temperature.median, Some(28.0));
        assert_eq!(temperature.third_quartile, Some(28.5));
        assert!((temperature.standard_deviation.unwrap() - 2.061_552).abs() < 0.001);
        assert_eq!(temperature.outlier_count, Some(1));
        let histogram = temperature
            .histogram
            .as_ref()
            .expect("la columna numérica debe incluir histograma");
        assert_eq!(histogram.len(), NUMERIC_HISTOGRAM_BUCKETS);
        assert_eq!(
            histogram.iter().map(|bucket| bucket.count).sum::<usize>(),
            4
        );
        assert_eq!(histogram.first().map(|bucket| bucket.lower), Some(25.0));
        assert_eq!(histogram.last().map(|bucket| bucket.upper), Some(30.0));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn csv_preserves_lexical_values_and_does_not_profile_identifiers_as_numbers() {
        let path = temporary_csv(
            "identifier,amount,huge\n00123,1.00,184467440737095516160\n00456,2.50,184467440737095516161\n00789,3.00,184467440737095516162\n",
        );
        let (frame, preview) =
            load_csv(&path).expect("el CSV debe cargar sin inferencia destructiva");
        let profile = profile_dataset(&frame).expect("el perfil semántico debe calcularse");

        assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
        assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
        assert_eq!(preview.rows[0][1].as_deref(), Some("1.00"));
        assert_eq!(preview.rows[0][2].as_deref(), Some("184467440737095516160"));
        assert_eq!(profile.columns[0].suggested_type, None);
        assert_eq!(profile.columns[0].mean, None);
        assert_eq!(profile.columns[0].outlier_count, None);
        assert_eq!(
            profile.columns[1].suggested_type,
            Some("decimal".to_owned())
        );
        assert_eq!(profile.columns[1].minimum.as_deref(), Some("1"));
        assert!((profile.columns[1].mean.unwrap() - 2.166_666).abs() < 0.001);
        assert_eq!(profile.columns[2].suggested_type, None);
        assert_eq!(profile.columns[2].mean, None);

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn profiles_bounded_numeric_correlations_without_exposing_cells() {
        let path = temporary_csv(
            "first,second,constant,identifier\n1,2,9,001\n2,4,9,002\n,8,9,003\n4,8,9,004\n",
        );
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let correlations = profile
            .numeric_correlations
            .as_ref()
            .expect("debe calcular correlaciones para dos columnas numéricas");
        assert_eq!(correlations.columns, vec!["first", "second", "constant"]);
        assert_eq!(correlations.sampled_row_count, 4);
        assert!(!correlations.truncated);
        let first_second = correlations
            .pairs
            .iter()
            .find(|pair| pair.first_column == "first" && pair.second_column == "second")
            .expect("debe incluir el par first-second");
        assert_eq!(first_second.sample_count, 3);
        assert!((first_second.coefficient.expect("debe ser definido") - 1.0).abs() < 1e-9);
        let first_constant = correlations
            .pairs
            .iter()
            .find(|pair| pair.first_column == "first" && pair.second_column == "constant")
            .expect("debe incluir el par first-constant");
        assert_eq!(first_constant.coefficient, None);
        assert!(profile
            .columns
            .iter()
            .all(|column| { column.name != "identifier" || column.mean.is_none() }));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn profiles_bounded_categorical_groups_and_keeps_private_columns_out() {
        let path = temporary_csv(
            "segment,email,status\nA,ana@example.com,ok\nA,beatriz@example.com,ok\nA,carlos@example.com,ok\nB,diana@example.com,ok\nB,elena@example.com,ok\nC,francisco@example.com,ok\n",
        );
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let summaries = profile
            .categorical_group_summaries
            .as_ref()
            .expect("debe resumir una columna categórica no sensible");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].column, "segment");
        assert_eq!(summaries[0].distinct_count, 3);
        assert_eq!(summaries[0].groups.len(), 2);
        assert_eq!(summaries[0].groups[0].label, "A");
        assert_eq!(summaries[0].groups[0].row_count, 3);
        assert!(!summaries[0].groups[0].is_other);
        assert_eq!(summaries[0].groups[1].label, "Resto");
        assert_eq!(summaries[0].groups[1].row_count, 3);
        assert!(summaries[0].groups[1].is_other);
        let serialized = serde_json::to_string(&profile).expect("el perfil debe serializar");
        assert!(!serialized.contains("ana@example.com"));
        assert!(!serialized.contains("francisco@example.com"));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn profiles_temporal_month_trend_with_empty_periods_and_bounded_payload() {
        let frame = DataFrame::new(
            5,
            vec![Series::new(
                "created_at".into(),
                [
                    "2024-01-15",
                    "2024-02-15",
                    "2024-02-20",
                    "2024-04-01",
                    "2025-01-01",
                ],
            )
            .into_column()],
        )
        .expect("el frame temporal debe ser válido");

        let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
        let summary = profile
            .temporal_series
            .as_ref()
            .and_then(|summaries| summaries.first())
            .expect("debe calcular una tendencia para una fecha sugerida");

        assert_eq!(summary.column, "created_at");
        assert_eq!(summary.granularity, "month");
        assert_eq!(summary.parsed_row_count, 5);
        assert_eq!(summary.unparsed_row_count, 0);
        assert!(!summary.truncated);
        assert_eq!(summary.periods.len(), 13);
        assert_eq!(summary.periods[0].period, "2024-01");
        assert_eq!(summary.periods[0].row_count, 1);
        assert_eq!(summary.periods[1].period, "2024-02");
        assert_eq!(summary.periods[1].row_count, 2);
        assert_eq!(summary.periods[2].period, "2024-03");
        assert_eq!(summary.periods[2].row_count, 0);
        assert_eq!(
            summary.periods.last().map(|period| period.period.as_str()),
            Some("2025-01")
        );
        assert_eq!(
            summary
                .periods
                .iter()
                .map(|period| period.row_count)
                .sum::<usize>(),
            5
        );
        assert!(serde_json::to_string(&profile)
            .expect("el perfil debe serializar")
            .contains("2024-02"));
    }

    #[test]
    fn profiles_temporal_day_trend_keeps_empty_days_for_short_spans() {
        let frame = DataFrame::new(
            4,
            vec![Series::new(
                "created_at".into(),
                ["2024-04-01", "2024-04-03", "2024-04-03", "2024-04-05"],
            )
            .into_column()],
        )
        .expect("el frame temporal debe ser válido");

        let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
        let summary = profile
            .temporal_series
            .as_ref()
            .and_then(|summaries| summaries.first())
            .expect("debe calcular una tendencia diaria para un rango corto");

        assert_eq!(summary.granularity, "day");
        assert_eq!(summary.periods.len(), 5);
        assert_eq!(summary.periods[0].period, "2024-04-01");
        assert_eq!(summary.periods[0].row_count, 1);
        assert_eq!(summary.periods[1].period, "2024-04-02");
        assert_eq!(summary.periods[1].row_count, 0);
        assert_eq!(summary.periods[2].period, "2024-04-03");
        assert_eq!(summary.periods[2].row_count, 2);
        assert_eq!(summary.periods[4].period, "2024-04-05");
        assert_eq!(
            summary
                .periods
                .iter()
                .map(|period| period.row_count)
                .sum::<usize>(),
            4
        );
        assert!(summary
            .periods
            .iter()
            .all(|period| period.period.chars().count() == 10));
    }

    #[test]
    fn profiles_temporal_year_trend_keeps_recent_periods_with_fixed_limit() {
        let dates = (1970..=2029)
            .map(|year| format!("{year}-01-01"))
            .collect::<Vec<_>>();
        let frame = DataFrame::new(
            dates.len(),
            vec![Series::new("created_at".into(), dates).into_column()],
        )
        .expect("el frame temporal debe ser válido");

        let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
        let summary = profile
            .temporal_series
            .as_ref()
            .and_then(|summaries| summaries.first())
            .expect("debe calcular una tendencia anual");

        assert_eq!(summary.granularity, "year");
        assert_eq!(summary.parsed_row_count, 60);
        assert!(summary.truncated);
        assert_eq!(summary.periods.len(), MAX_TEMPORAL_PERIODS);
        assert_eq!(summary.periods[0].period, "Periodos anteriores");
        assert_eq!(summary.periods[0].row_count, 13);
        assert_eq!(
            summary.periods.last().map(|period| period.period.as_str()),
            Some("2029")
        );
        assert_eq!(
            summary
                .periods
                .iter()
                .map(|period| period.row_count)
                .sum::<usize>(),
            60
        );
    }

    #[test]
    fn empty_numeric_dataset_does_not_attempt_to_sample_correlations() {
        let frame = DataFrame::new(
            0,
            vec![
                Series::new("first".into(), Vec::<i64>::new()).into_column(),
                Series::new("second".into(), Vec::<i64>::new()).into_column(),
            ],
        )
        .expect("el frame vacío debe ser válido");

        let profile = profile_dataset(&frame).expect("el perfil vacío debe calcularse");

        assert_eq!(profile.row_count, 0);
        assert_eq!(profile.numeric_correlations, None);
    }

    #[test]
    fn reports_profile_progress_per_column() {
        let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let mut updates = Vec::new();

        profile_dataset_with_progress(
            &frame,
            |stage, percent| {
                updates.push((stage, percent));
            },
            || false,
        )
        .expect("el perfil debe calcularse");

        assert_eq!(updates.first(), Some(&("Detectando filas duplicadas", 10)));
        assert_eq!(updates.last(), Some(&("Analizando columnas", 100)));
        assert!(updates
            .iter()
            .any(|(stage, _)| *stage == "Contando valores únicos"));
        assert!(updates
            .iter()
            .any(|(stage, _)| *stage == "Calculando estadísticas numéricas"));
        assert!(updates.windows(2).all(|pair| pair[0].1 <= pair[1].1));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn stops_profile_at_a_cooperative_cancellation_point() {
        use std::sync::atomic::AtomicUsize;

        let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let checks = AtomicUsize::new(0);

        let error = profile_dataset_with_progress(
            &frame,
            |_, _| {},
            || {
                let count = checks.fetch_add(1, Ordering::SeqCst) + 1;
                count >= 2
            },
        )
        .expect_err("el perfil debe detenerse al cancelar");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn invalidates_only_the_requested_operation_generation() {
        let state = DatasetState::default();
        let load_generation = state.begin_load();
        let profile_generation = state.begin_profile();
        let export_generation = state.begin_export();
        let query_generation = state.begin_query();

        state
            .cancel("profile")
            .expect("el perfil debe poder cancelarse");
        state
            .cancel("query")
            .expect("la consulta debe poder cancelarse");

        assert!(!state.load_was_cancelled(load_generation));
        assert!(state.profile_was_cancelled(profile_generation));
        assert!(!state.export_was_cancelled(export_generation));
        assert!(state.query_was_cancelled(query_generation));
        assert!(state.cancel("unknown").is_err());
    }

    #[test]
    fn queues_and_consumes_a_native_drop_path_once() {
        let state = DatasetState::default();
        let path = PathBuf::from("C:/datos/ventas.csv");

        state.queue_dropped_path(path.clone());
        assert_eq!(
            state
                .take_dropped_path()
                .expect("la cola debe estar disponible"),
            Some(path)
        );
        assert_eq!(
            state
                .take_dropped_path()
                .expect("la cola debe quedar vacía"),
            None
        );
    }

    #[test]
    fn exports_csv_by_atomically_replacing_the_destination() {
        let source = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
        let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("resultado.csv");
        fs::write(&destination, "contenido anterior").expect("se debe preparar el destino");
        let mut updates = Vec::new();

        let result = export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Csv,
            |stage, percent| updates.push((stage, percent)),
            || false,
        )
        .expect("el CSV debe exportarse");

        let exported = fs::read_to_string(&destination).expect("se debe leer la exportación");
        assert!(exported.starts_with("city,temperature"));
        assert!(exported.contains("Santo Domingo,30"));
        assert_eq!(result.file_name, "resultado.csv");
        assert_eq!(result.format, "CSV");
        assert_eq!(updates.last(), Some(&("Exportación lista", 100)));
        fs::remove_file(source).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn csv_export_neutralizes_spreadsheet_formulas_and_parquet_preserves_values() {
        let dangerous = [
            "=SUM(A1:A2)",
            "+cmd",
            "-2+3",
            "@SUM(A1:A2)",
            "\tformula",
            "\rformula",
            "\nformula",
        ];
        let benign = ["text", "123", "  =not-a-prefix", "'already-text"];
        let values = dangerous
            .iter()
            .chain(benign.iter())
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
            .collect::<Vec<_>>();
        let mut numbers = (0_i64..)
            .take(dangerous.len() + benign.len() + 1)
            .collect::<Vec<_>>();
        numbers[0] = -12;
        let frame = DataFrame::new(
            values.len(),
            vec![
                Series::new("text".into(), values).into_column(),
                Series::new("number".into(), numbers).into_column(),
            ],
        )
        .unwrap();

        let csv = frame_for_export(&frame, ExportFormat::Csv).expect("CSV debe protegerse");
        let exported_text = csv
            .column("text")
            .unwrap()
            .str()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        for (index, value) in dangerous.iter().enumerate() {
            assert_eq!(exported_text[index], Some(format!("'{value}").as_str()));
        }
        for (offset, value) in benign.iter().enumerate() {
            assert_eq!(exported_text[dangerous.len() + offset], Some(*value));
        }
        assert_eq!(exported_text.last(), Some(&None));
        assert_eq!(
            csv.column("number").unwrap(),
            frame.column("number").unwrap()
        );
        assert_eq!(
            csv.column("number").unwrap().i64().unwrap().get(0),
            Some(-12)
        );

        let parquet = frame_for_export(&frame, ExportFormat::Parquet)
            .expect("Parquet debe conservar los valores originales");
        assert!(parquet.equals_missing(&frame));
    }

    #[test]
    fn exports_a_valid_parquet_file() {
        let source = temporary_csv("value\n1\n2\n");
        let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("resultado.parquet");

        export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Parquet,
            |_, _| {},
            || false,
        )
        .expect("Parquet debe exportarse");

        let bytes = fs::read(destination).expect("se debe leer Parquet");
        assert!(bytes.starts_with(b"PAR1"));
        assert!(bytes.ends_with(b"PAR1"));
        fs::remove_file(source).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn exports_a_valid_json_array() {
        let source = temporary_csv("city,value\nSanto Domingo,30\nSantiago,28\n");
        let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("resultado.json");

        export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Json,
            |_, _| {},
            || false,
        )
        .expect("JSON debe exportarse");

        let value: JsonValue = serde_json::from_slice(&fs::read(&destination).unwrap())
            .expect("la salida debe ser JSON válido");
        let rows = value.as_array().expect("la salida debe ser un arreglo");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["city"], "Santo Domingo");
        assert_eq!(rows[1]["city"], "Santiago");
        fs::remove_file(source).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn exports_a_portable_sql_script_with_escaped_values_and_nulls() {
        let frame = DataFrame::new(
            2,
            vec![
                Series::new("name".into(), &["O'Brien", "Ana"]).into_column(),
                Series::new("total".into(), &[Some(10_i64), None]).into_column(),
                Series::new("active".into(), &[true, false]).into_column(),
            ],
        )
        .expect("el frame tipado debe ser válido");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("resultado.sql");

        export_frame_atomic(&frame, &destination, ExportFormat::Sql, |_, _| {}, || false)
            .expect("SQL debe exportarse");

        let script = fs::read_to_string(&destination).expect("se debe leer SQL");
        assert!(script.contains("CREATE TABLE \"dataset\""));
        assert!(script.contains("\"name\" TEXT"));
        assert!(script.contains("\"total\" BIGINT"));
        assert!(script.contains("'O''Brien'"));
        assert!(script.contains("NULL"));
        assert!(script.contains("TRUE"));
        assert!(script.contains("BEGIN TRANSACTION;"));
        assert!(script.contains("COMMIT;"));
    }

    #[test]
    fn exports_a_real_xlsx_with_safe_inline_strings() {
        let frame = df![
            "name" => &["A&B", "=SUM(A1:A2)"],
            "count" => &[1_i64, 2_i64]
        ]
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("resultado.xlsx");
        export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Excel,
            |_, _| {},
            || false,
        )
        .expect("Excel debe publicarse");

        let bytes = fs::read(&destination).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains("A&amp;B"));
        assert!(sheet.contains("=SUM(A1:A2)"));
        assert!(sheet.contains("t=\"inlineStr\""));
        let mut workbook = open_workbook_auto(&destination).expect("Excel debe poder reabrirse");
        let range = workbook
            .worksheet_range("dataset")
            .expect("la hoja dataset debe existir");
        assert_eq!(
            range.get((0, 0)).map(ToString::to_string).as_deref(),
            Some("name")
        );
        assert_eq!(
            range.get((1, 0)).map(ToString::to_string).as_deref(),
            Some("A&B")
        );
    }

    #[test]
    fn exports_a_typed_sqlite_database_atomically() {
        let frame = df![
            "name" => &[Some("Santo Domingo"), None],
            "count" => &[Some(2_i64), Some(3_i64)]
        ]
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("resultado.sqlite");
        export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Sqlite,
            |_, _| {},
            || false,
        )
        .expect("SQLite debe publicarse");

        let connection = Connection::open(&destination).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM dataset", [], |row| row.get(0))
            .unwrap();
        let nullable: Option<String> = connection
            .query_row("SELECT name FROM dataset WHERE count = 3", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(nullable, None);
    }

    #[test]
    fn exports_bundle_with_dataset_dictionary_quality_and_manifest_hashes() {
        let frame = df![
            "email" => &[Some("ana@example.com"), None],
            "count" => &[Some(2_i64), Some(3_i64)]
        ]
        .unwrap();
        let validation = QualityValidationResult {
            passed: true,
            row_count: 2,
            total_rules: 0,
            failed_rules: 0,
            rules: Vec::new(),
        };
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("resultado.zip");

        export_frame_atomic_with_privacy_and_quality(
            &frame,
            &destination,
            ExportFormat::Bundle,
            PrivacyMode::Mask,
            Some(&validation),
            |_, _| {},
            || false,
        )
        .expect("el bundle debe publicarse");

        let bytes = fs::read(&destination).unwrap();
        let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        for name in [
            "dataset.csv",
            "dictionary.json",
            "quality-report.json",
            "manifest.json",
        ] {
            assert!(archive.by_name(name).is_ok(), "falta {name} en el bundle");
        }
        let mut manifest = String::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        let manifest: JsonValue = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["format"], "columnia-bundle");
        assert_eq!(manifest["datasetFile"], "dataset.csv");
        assert_eq!(manifest["rowCount"], 2);
        assert_eq!(manifest["columnCount"], 2);
        assert!(manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "quality-report.json"));
        let mut dataset = String::new();
        archive
            .by_name("dataset.csv")
            .unwrap()
            .read_to_string(&mut dataset)
            .unwrap();
        assert!(dataset.contains("[REDACTED]"));
        assert!(!dataset.contains("ana@example.com"));
    }

    #[test]
    fn exports_validated_recipe_with_manifest_reference_and_hash() {
        let frame = df!["amount" => &[Some(2_i64), Some(3_i64)]].unwrap();
        let recipe = complete_stored_recipe();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("resultado-con-receta.zip");

        export_frame_atomic_with_privacy_and_quality_and_recipe(
            &frame,
            &destination,
            ExportFormat::Bundle,
            PrivacyMode::None,
            None,
            Some(&recipe),
            |_, _| {},
            || false,
        )
        .expect("el bundle debe incluir la receta válida");

        let bytes = fs::read(&destination).unwrap();
        let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut recipe_json = String::new();
        archive
            .by_name("recipe.json")
            .expect("falta recipe.json en el bundle")
            .read_to_string(&mut recipe_json)
            .unwrap();
        let recipe_value: JsonValue = serde_json::from_str(&recipe_json).unwrap();
        assert_eq!(recipe_value["name"], "Limpieza completa");
        assert_eq!(recipe_value["version"], RECIPE_FILE_VERSION);

        let mut manifest_json = String::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut manifest_json)
            .unwrap();
        let manifest: JsonValue = serde_json::from_str(&manifest_json).unwrap();
        assert_eq!(manifest["recipeFile"], "recipe.json");
        let expected_hash = format!("{:x}", Sha256::digest(recipe_json.as_bytes()));
        assert!(manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "recipe.json" && file["sha256"] == expected_hash));
    }

    #[test]
    fn privacy_modes_mask_or_hash_detect_all_detected_columns_without_values() {
        let frame = df![
            "email" => &["ana@example.com"],
            "city" => &["Santo Domingo"],
            "identifier" => &[42_i64]
        ]
        .unwrap();

        let (masked, protected_columns) = privacy_safe_frame(&frame, PrivacyMode::Mask).unwrap();
        assert_eq!(protected_columns, vec!["email", "identifier"]);
        assert_eq!(
            masked.column("email").unwrap().str().unwrap().get(0),
            Some("[REDACTED]")
        );
        assert_eq!(
            masked.column("city").unwrap().str().unwrap().get(0),
            Some("Santo Domingo")
        );
        assert_eq!(
            masked.column("identifier").unwrap().str().unwrap().get(0),
            Some("[REDACTED]")
        );

        let (hashed, _) = privacy_safe_frame(&frame, PrivacyMode::Hash).unwrap();
        let hashed_value = hashed
            .column("email")
            .unwrap()
            .str()
            .unwrap()
            .get(0)
            .unwrap();
        assert_eq!(hashed_value.len(), 64);
        assert_ne!(hashed_value, "ana@example.com");
        let (unprotected, protected_columns) =
            privacy_safe_frame(&frame, PrivacyMode::None).unwrap();
        assert_eq!(unprotected, frame);
        assert!(protected_columns.is_empty());
    }

    #[test]
    fn compares_multiset_rows_and_reports_schema_differences_before_consolidation() {
        let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
        let compared_path = temporary_csv("id,city\n2,Santiago\n3,La Vega\n");
        let (current, _) = load_csv(&current_path).expect("el dataset activo debe cargar");
        let (compared, _) = load_csv(&compared_path).expect("el dataset comparado debe cargar");

        let result = compare_frames(&current, "activo.csv", &compared, "comparado.csv", &[])
            .expect("la comparación debe calcularse");
        assert_eq!(result.common_row_count, 1);
        assert_eq!(result.current_only_row_count, 1);
        assert_eq!(result.compared_only_row_count, 1);
        assert_eq!(result.shared_columns, vec!["id", "city"]);
        assert!(result.schema_compatible);
        assert!(result.key_columns.is_empty());
        assert!(result.can_consolidate);

        let mut consolidated = current.clone();
        consolidated
            .vstack_mut(&compared)
            .expect("los esquemas compatibles deben consolidarse");
        assert_eq!(consolidated.height(), 4);

        fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
        fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
    }

    #[test]
    fn compares_explicit_keys_and_reports_conflicts_and_duplicate_keys() {
        let current_path = temporary_csv("id,city,total\n1,Santo Domingo,10\n2,Santiago,20\n");
        let compared_path = temporary_csv("id,city,total\n2,Santiago,25\n3,La Vega,30\n");
        let (current, _) = load_csv(&current_path).expect("el dataset activo debe cargar");
        let (compared, _) = load_csv(&compared_path).expect("el dataset comparado debe cargar");

        let result = compare_frames(
            &current,
            "activo.csv",
            &compared,
            "comparado.csv",
            &["id".to_owned()],
        )
        .expect("la comparación por clave debe calcularse");
        assert_eq!(result.key_columns, vec!["id"]);
        assert_eq!(result.matched_key_count, 1);
        assert_eq!(result.current_only_key_count, 1);
        assert_eq!(result.compared_only_key_count, 1);
        assert_eq!(result.conflicting_key_count, 1);
        assert_eq!(result.duplicate_key_count, 0);
        assert!(!result.can_consolidate);

        let duplicate_path =
            temporary_csv("id,city,total\n2,Santiago,20\n2,Santiago,20\n3,La Vega,30\n");
        let (duplicate, _) = load_csv(&duplicate_path).expect("el dataset duplicado debe cargar");
        let duplicate_result = compare_frames(
            &current,
            "activo.csv",
            &duplicate,
            "duplicado.csv",
            &["id".to_owned()],
        )
        .expect("la comparación duplicada debe calcularse");
        assert_eq!(duplicate_result.duplicate_key_count, 1);
        assert!(!duplicate_result.can_consolidate);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].key, vec![Some("2".to_owned())]);
        assert_eq!(result.conflicts[0].cells[0].column, "total");
        assert_eq!(result.conflicts[0].cells[0].current, Some("20".to_owned()));
        assert_eq!(result.conflicts[0].cells[0].compared, Some("25".to_owned()));

        fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
        fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
        fs::remove_file(duplicate_path).expect("se debe limpiar el CSV duplicado");
    }

    #[test]
    fn comparison_signatures_merge_fixed_blocks_without_changing_counts() {
        let row_count = LOCAL_QUERY_BLOCK_ROWS * 2 + 3;
        let ids = (0..row_count)
            .map(|index| (index % 7) as i64)
            .collect::<Vec<_>>();
        let frame = DataFrame::new(row_count, vec![Series::new("id".into(), ids).into_column()])
            .expect("el frame grande de comparación debe ser válido");
        let columns = vec!["id".to_owned()];

        let signatures = row_signatures(&frame, &columns).expect("las firmas deben fusionarse");
        let keys = key_rows(&frame, &columns).expect("las claves deben fusionarse");

        assert_eq!(signatures.len(), 7);
        assert_eq!(keys.len(), 7);
        assert_eq!(signatures.values().sum::<usize>(), row_count);
        assert_eq!(keys.values().map(Vec::len).sum::<usize>(), row_count);
    }

    #[test]
    fn keyed_consolidation_only_appends_rows_with_new_keys() {
        let current = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[1_i64, 2]).into_column(),
                Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
            ],
        )
        .expect("el frame activo debe ser válido");
        let compared = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[2_i64, 3]).into_column(),
                Series::new("city".into(), &["Santiago", "La Vega"]).into_column(),
            ],
        )
        .expect("el frame comparado debe ser válido");

        let additions = rows_with_new_keys(&current, &compared, &["id".to_owned()])
            .expect("se deben seleccionar las claves nuevas");
        assert_eq!(additions.height(), 1);
        assert_eq!(
            additions.column("id").unwrap().i64().unwrap().get(0),
            Some(3)
        );
    }

    #[test]
    fn resolves_key_conflicts_by_column_and_keeps_legacy_row_decisions() {
        let current = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[1_i64, 2]).into_column(),
                Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
                Series::new("total".into(), &[10_i64, 20]).into_column(),
            ],
        )
        .expect("el frame activo debe ser válido");
        let compared = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[1_i64, 2]).into_column(),
                Series::new("city".into(), &["La Vega", "Santiago"]).into_column(),
                Series::new("total".into(), &[15_i64, 20]).into_column(),
            ],
        )
        .expect("el frame comparado debe ser válido");
        let key_columns = vec!["id".to_owned()];
        let shared_columns = vec!["id".to_owned(), "city".to_owned(), "total".to_owned()];
        let (conflicts, truncated) =
            collect_key_conflicts(&current, &compared, &key_columns, &shared_columns)
                .expect("los conflictos deben poder inspeccionarse");
        assert!(!truncated);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict.key, vec![Some("1".to_owned())]);
        assert_eq!(conflicts[0].conflict.cells.len(), 2);

        let resolved = resolved_conflict_frame(
            &current,
            &compared,
            &key_columns,
            &[
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("city".to_owned()),
                    source: ConflictSource::Compared,
                },
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("total".to_owned()),
                    source: ConflictSource::Current,
                },
            ],
        )
        .expect("la resolución debe construir un frame válido");
        assert_eq!(
            resolved.column("id").unwrap().i64().unwrap().get(0),
            Some(1)
        );
        assert_eq!(
            resolved.column("city").unwrap().str().unwrap().get(0),
            Some("La Vega")
        );
        assert_eq!(
            resolved.column("total").unwrap().i64().unwrap().get(0),
            Some(10)
        );
        assert_eq!(
            resolved.column("city").unwrap().str().unwrap().get(1),
            Some("Santiago")
        );
        let legacy_resolved = resolved_conflict_frame(
            &current,
            &compared,
            &key_columns,
            &[ConflictResolution {
                conflict_index: 0,
                column: None,
                source: ConflictSource::Compared,
            }],
        )
        .expect("la resolución legacy por fila debe seguir funcionando");
        assert_eq!(
            legacy_resolved
                .column("total")
                .unwrap()
                .i64()
                .unwrap()
                .get(0),
            Some(15)
        );
        assert!(resolved_conflict_frame(&current, &compared, &key_columns, &[]).is_err());
        assert!(resolved_conflict_frame(
            &current,
            &compared,
            &key_columns,
            &[
                ConflictResolution {
                    conflict_index: 0,
                    column: None,
                    source: ConflictSource::Current,
                },
                ConflictResolution {
                    conflict_index: 0,
                    column: None,
                    source: ConflictSource::Compared,
                },
            ],
        )
        .is_err());
    }

    #[test]
    fn paginates_and_resolves_conflicts_beyond_visible_preview_limit() {
        let ids = (0_i64..51).collect::<Vec<_>>();
        let current_values = ids
            .iter()
            .map(|id| format!("activo-{id}"))
            .collect::<Vec<_>>();
        let compared_values = ids
            .iter()
            .map(|id| format!("comparado-{id}"))
            .collect::<Vec<_>>();
        let current = DataFrame::new(
            ids.len(),
            vec![
                Series::new("id".into(), ids.clone()).into_column(),
                Series::new("value".into(), current_values).into_column(),
            ],
        )
        .expect("el frame activo debe ser válido");
        let compared = DataFrame::new(
            ids.len(),
            vec![
                Series::new("id".into(), ids).into_column(),
                Series::new("value".into(), compared_values).into_column(),
            ],
        )
        .expect("el frame comparado debe ser válido");
        let key_columns = vec!["id".to_owned()];
        let result = compare_frames(
            &current,
            "activo.csv",
            &compared,
            "comparado.csv",
            &key_columns,
        )
        .expect("la comparación debe calcularse");

        assert_eq!(result.conflicting_key_count, 51);
        assert_eq!(result.conflicts.len(), 50);
        assert_eq!(result.conflict_offset, 0);
        assert!(result.conflicts_truncated);
        let (last_page, has_next) = collect_key_conflicts_page(
            &current,
            &compared,
            &key_columns,
            &["id".to_owned(), "value".to_owned()],
            50,
            MAX_CONFLICT_PREVIEW,
        )
        .expect("la segunda página debe poder calcularse");
        assert_eq!(last_page.len(), 1);
        assert!(!has_next);
        assert_eq!(last_page[0].conflict.key, vec![Some("50".to_owned())]);

        let decisions = (0..51)
            .map(|conflict_index| ConflictResolution {
                conflict_index,
                column: Some("value".to_owned()),
                source: ConflictSource::Compared,
            })
            .collect::<Vec<_>>();
        let resolved = resolved_conflict_frame(&current, &compared, &key_columns, &decisions)
            .expect("la resolución completa debe aceptar todas las páginas");
        assert_eq!(
            resolved.column("value").unwrap().str().unwrap().get(0),
            Some("comparado-0")
        );
        assert_eq!(
            resolved.column("value").unwrap().str().unwrap().get(50),
            Some("comparado-50")
        );
    }

    #[test]
    fn joins_frames_by_key_for_inner_left_and_full_relations() {
        let current = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[1_i64, 2]).into_column(),
                Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
            ],
        )
        .expect("el frame activo debe ser válido");
        let compared = DataFrame::new(
            2,
            vec![
                Series::new("id".into(), &[2_i64, 3]).into_column(),
                Series::new("city".into(), &["Santiago", "La Vega"]).into_column(),
                Series::new("segment".into(), &["B", "C"]).into_column(),
            ],
        )
        .expect("el frame comparado debe ser válido");
        let key = ["id".to_owned()];

        let inner = join_frames(&current, &compared, &key, DatasetJoinType::Inner)
            .expect("la unión inner debe calcularse");
        let left = join_frames(&current, &compared, &key, DatasetJoinType::Left)
            .expect("la unión left debe calcularse");
        let full = join_frames(&current, &compared, &key, DatasetJoinType::Full)
            .expect("la unión full debe calcularse");

        assert_eq!(inner.height(), 1);
        assert_eq!(left.height(), 2);
        assert_eq!(full.height(), 3);
        assert!(inner
            .get_column_names()
            .iter()
            .any(|name| name.as_str() == "city_right"));
        assert!(left
            .get_column_names()
            .iter()
            .any(|name| name.as_str() == "segment"));
        assert!(full
            .get_column_names()
            .iter()
            .any(|name| name.as_str() == "segment"));
    }

    #[test]
    fn cancellation_keeps_the_previous_export_untouched() {
        use std::cell::Cell;

        let source = temporary_csv("value\n1\n2\n");
        let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let destination = directory.path().join("resultado.csv");
        fs::write(&destination, "exportación anterior")
            .expect("se debe preparar la exportación anterior");
        let checks = Cell::new(0);

        let error = export_frame_atomic(
            &frame,
            &destination,
            ExportFormat::Csv,
            |_, _| {},
            || {
                checks.set(checks.get() + 1);
                checks.get() >= 2
            },
        )
        .expect_err("la exportación debe cancelarse antes de publicarse");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
        assert_eq!(
            fs::read_to_string(destination).expect("el destino debe conservarse"),
            "exportación anterior"
        );
        fs::remove_file(source).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn counts_blank_text_and_unicode_characters() {
        let path = temporary_csv("text\n\"  \"\nCafé\nRepública\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let text = &profile.columns[0];

        assert_eq!(text.empty_count, Some(1));
        assert_eq!(text.minimum_length, Some(2));
        assert_eq!(text.maximum_length, Some(9));
        assert_eq!(text.average_length, Some(5.0));
        assert_eq!(text.suggested_type, None);

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn suggests_dates_and_reports_values_that_do_not_match() {
        let path = temporary_csv(
            "date_added\n\"September 9, 2019\"\n\"September 10, 2019\"\n\"September 11, 2019\"\n\"September 12, 2019\"\n\"September 13, 2019\"\n\"September 14, 2019\"\n\"September 15, 2019\"\n\"September 16, 2019\"\n\"September 17, 2019\"\nunknown\n",
        );
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let date = &profile.columns[0];

        assert_eq!(date.suggested_type, Some("date".to_owned()));
        assert_eq!(date.type_match_percentage, Some(90.0));
        assert_eq!(date.invalid_type_count, Some(1));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn detects_numeric_outliers_with_the_iqr_rule() {
        let path = temporary_csv("value\n10\n11\n12\n13\n100\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        let value = &profile.columns[0];

        assert_eq!(value.first_quartile, Some(11.0));
        assert_eq!(value.median, Some(12.0));
        assert_eq!(value.third_quartile, Some(13.0));
        assert_eq!(value.outlier_count, Some(1));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn removes_only_additional_duplicate_rows_and_preserves_order() {
        let path = temporary_csv("city,value\nSanto Domingo,1\nSantiago,2\nSanto Domingo,1\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let (cleaned, affected_row_count) =
            remove_duplicate_rows(&frame).expect("los duplicados deben eliminarse");
        let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

        assert_eq!(affected_row_count, 1);
        assert_eq!(cleaned.height(), 2);
        assert_eq!(page.rows[0][0].as_deref(), Some("Santo Domingo"));
        assert_eq!(page.rows[1][0].as_deref(), Some("Santiago"));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn counts_normalized_near_duplicates_without_double_counting_exact_rows() {
        let frame = df![
            "city" => &["Santo Domingo", " santo   domingo ", "Santo Domingo", "Santiago"],
            "value" => &[1_i64, 1, 1, 2]
        ]
        .unwrap();

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");

        assert_eq!(profile.duplicate_row_count, 1);
        assert_eq!(profile.near_duplicate_row_count, 1);
    }

    #[test]
    fn removes_near_duplicates_stably_without_removing_exact_repeats() {
        let frame = df![
            "city" => &[" Ana ", "Ana", " Ana ", "Luis", " luis "],
            "value" => &[1_i64, 1, 1, 2, 2]
        ]
        .unwrap();

        let (cleaned, affected_row_count) =
            remove_near_duplicate_rows(&frame).expect("los duplicados parecidos deben eliminarse");
        let cities = cleaned.column("city").unwrap().str().unwrap();

        assert_eq!(affected_row_count, 2);
        assert_eq!(cleaned.height(), 3);
        assert_eq!(cities.get(0), Some(" Ana "));
        assert_eq!(cities.get(1), Some(" Ana "));
        assert_eq!(cities.get(2), Some("Luis"));
    }

    #[test]
    fn near_duplicate_removal_publishes_a_reversible_history_entry() {
        let path = temporary_csv("city,value\n Ana ,1\nAna,1\n Ana ,1\nLuis,2\n luis ,2\n");
        let (original, _) = load_csv(&path).expect("el CSV debe cargar");
        let (cleaned, affected_row_count) = remove_near_duplicate_rows(&original)
            .expect("los duplicados parecidos deben poder eliminarse");
        assert_eq!(affected_row_count, 2);

        let mut dataset = loaded_dataset(path.clone(), original);
        let preview =
            publish_candidate(&mut dataset, cleaned, "Eliminar filas duplicadas parecidas")
                .expect("la mutación debe publicarse");

        assert_eq!(preview.row_count, 3);
        assert_eq!(
            dataset.history.entries[1].label,
            "Eliminar filas duplicadas parecidas"
        );
        let undone = undo_dataset(&mut dataset).expect("la mutación debe poder deshacerse");
        assert_eq!(undone.dataset.row_count, 5);

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn removes_only_rows_that_are_completely_empty() {
        let frame = df![
            "name" => &[Some("Ana"), Some(""), None, Some("  ")],
            "amount" => &[Some(10_i64), None, None, None]
        ]
        .unwrap();

        let (cleaned, affected_row_count) =
            remove_empty_rows_from_frame(&frame).expect("las filas vacías deben eliminarse");
        assert_eq!(affected_row_count, 3);
        assert_eq!(cleaned.height(), 1);
        assert_eq!(
            dataset_page(&cleaned, 0, 10).unwrap().rows[0][0].as_deref(),
            Some("Ana")
        );
    }

    #[test]
    fn removes_constant_columns_but_preserves_a_usable_dataset() {
        let frame = df![
            "id" => &[1_i64, 2, 3],
            "constant" => &[Some("activo"), None, Some("activo")],
            "all_null" => &[None::<String>, None, None]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_constant_columns_from_frame(&frame)
            .expect("las columnas constantes deben poder eliminarse");
        assert_eq!(removed_columns, vec!["constant"]);
        assert_eq!(cleaned.get_column_names(), vec!["id", "all_null"]);
        assert_eq!(cleaned.height(), 3);

        let all_constant = df![
            "first" => &["same", "same"],
            "second" => &[1_i64, 1]
        ]
        .unwrap();
        let (kept, removed) = remove_constant_columns_from_frame(&all_constant)
            .expect("el dataset debe conservar una columna");
        assert_eq!(removed.len(), 1);
        assert_eq!(kept.width(), 1);
        assert_eq!(kept.height(), 2);
    }

    #[test]
    fn removes_only_completely_empty_columns_and_keeps_one_column() {
        let frame = df![
            "id" => &[1_i64, 2],
            "empty" => &[None::<String>, None],
            "partial" => &[Some("ok"), None]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_empty_columns_from_frame(&frame)
            .expect("las columnas vacías deben poder eliminarse");
        assert_eq!(removed_columns, vec!["empty"]);
        assert_eq!(cleaned.get_column_names(), vec!["id", "partial"]);

        let all_empty = df![
            "first" => &[None::<String>, None],
            "second" => &[None::<i64>, None]
        ]
        .unwrap();
        let (kept, removed) = remove_empty_columns_from_frame(&all_empty)
            .expect("el dataset debe conservar una columna");
        assert_eq!(removed.len(), 1);
        assert_eq!(kept.width(), 1);
        assert_eq!(kept.height(), 2);
    }

    #[test]
    fn removes_detected_identifier_columns_but_keeps_other_personal_columns() {
        let frame = df![
            "customer_id" => &["a-1", "b-2"],
            "email" => &["ana@example.com", "luis@example.com"],
            "amount" => &[10_i64, 20]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&frame)
            .expect("las columnas identificadoras deben poder retirarse");
        assert_eq!(removed_columns, vec!["customer_id"]);
        assert_eq!(cleaned.get_column_names(), vec!["email", "amount"]);
        assert_eq!(cleaned.height(), 2);
    }

    #[test]
    fn keeps_one_column_when_all_columns_are_identifiers() {
        let frame = df![
            "customer_id" => &["a-1", "b-2"],
            "order_id" => &["o-1", "o-2"]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&frame)
            .expect("el dataset debe conservar una columna");
        assert_eq!(removed_columns, vec!["customer_id"]);
        assert_eq!(cleaned.get_column_names(), vec!["order_id"]);
        assert_eq!(cleaned.height(), 2);
    }

    #[test]
    fn removes_personal_columns_by_category_without_touching_row_audit() {
        let frame = df![
            "email" => &["ana@example.com", "luis@example.com"],
            "phone" => &["555-0100", "555-0101"],
            "address" => &["Calle 1", "Calle 2"],
            "name" => &["Ana", "Luis"],
            "_cambios" => &[Some(""), Some("" )],
            "amount" => &[10_i64, 20]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_personal_columns_from_frame(&frame)
            .expect("las columnas personales deben poder retirarse");
        assert_eq!(removed_columns, vec!["email", "phone", "address", "name"]);
        assert_eq!(cleaned.get_column_names(), vec!["_cambios", "amount"]);
        assert_eq!(cleaned.height(), 2);
    }

    #[test]
    fn masks_personal_values_without_touching_identifiers_or_row_audit() {
        let frame = df![
            "email" => &[Some("ana@example.com"), None::<&str>],
            "phone" => &["555-0100", "555-0101"],
            "address" => &["Calle 1", "Calle 2"],
            "name" => &["Ana", "Luis"],
            "customer_id" => &["a-1", "b-2"],
            "_cambios" => &[Some(""), Some("" )],
            "amount" => &[10_i64, 20]
        ]
        .unwrap();

        let (masked, changed_cell_count, changed_column_count) =
            mask_personal_values_from_frame(&frame)
                .expect("los valores personales deben poder protegerse");
        assert_eq!(changed_cell_count, 7);
        assert_eq!(changed_column_count, 4);
        assert_eq!(
            masked.column("email").unwrap().str().unwrap().get(0),
            Some(REDACTED_VALUE)
        );
        assert_eq!(masked.column("email").unwrap().str().unwrap().get(1), None);
        assert_eq!(
            masked.column("phone").unwrap().str().unwrap().get(0),
            Some(REDACTED_VALUE)
        );
        assert_eq!(
            masked.column("address").unwrap().str().unwrap().get(1),
            Some(REDACTED_VALUE)
        );
        assert_eq!(
            masked.column("name").unwrap().str().unwrap().get(0),
            Some(REDACTED_VALUE)
        );
        assert_eq!(
            masked.column("customer_id").unwrap().str().unwrap().get(0),
            Some("a-1")
        );
        assert_eq!(
            masked.column("_cambios").unwrap().str().unwrap().get(0),
            Some("")
        );
        assert_eq!(
            masked.column("amount").unwrap().i64().unwrap().get(0),
            Some(10)
        );
    }

    #[test]
    fn masking_personal_values_is_idempotent_for_redacted_and_null_cells() {
        let frame = df![
            "email" => &[Some(REDACTED_VALUE), None::<&str>],
            "name" => &[Some("Luis"), None::<&str>],
            "amount" => &[1_i64, 2]
        ]
        .unwrap();

        let (masked, changed_cell_count, changed_column_count) =
            mask_personal_values_from_frame(&frame)
                .expect("la máscara debe poder repetirse sin cambiar lo ya protegido");
        assert_eq!(changed_cell_count, 1);
        assert_eq!(changed_column_count, 1);
        assert_eq!(
            masked.column("email").unwrap().str().unwrap().get(0),
            Some(REDACTED_VALUE)
        );
        assert_eq!(
            masked.column("name").unwrap().str().unwrap().get(0),
            Some(REDACTED_VALUE)
        );
    }

    #[test]
    fn keeps_one_personal_column_when_all_usable_columns_are_personal() {
        let frame = df![
            "email" => &["ana@example.com", "luis@example.com"],
            "nombre" => &["Ana", "Luis"]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_personal_columns_from_frame(&frame)
            .expect("el dataset debe conservar una columna");
        assert_eq!(removed_columns, vec!["email"]);
        assert_eq!(cleaned.get_column_names(), vec!["nombre"]);
        assert_eq!(cleaned.width(), 1);
    }

    #[test]
    fn removes_columns_at_or_above_eighty_percent_null_but_not_empty_or_below_threshold() {
        let frame = df![
            "id" => &[1_i64, 2, 3, 4, 5],
            "high" => &[Some("ok"), None, None, None, None],
            "below" => &[Some("a"), Some("b"), None, None, None],
            "empty" => &[None::<String>, None, None, None, None]
        ]
        .unwrap();

        let (cleaned, removed_columns) = remove_high_null_columns_from_frame(&frame)
            .expect("las columnas con alta nulidad deben poder eliminarse");
        assert_eq!(removed_columns, vec!["high"]);
        assert_eq!(cleaned.get_column_names(), vec!["id", "below", "empty"]);

        let all_high = df![
            "first" => &[Some("ok"), None, None, None, None],
            "second" => &[Some("ok"), None, None, None, None]
        ]
        .unwrap();
        let (kept, removed) = remove_high_null_columns_from_frame(&all_high)
            .expect("el dataset debe conservar una columna");
        assert_eq!(removed.len(), 1);
        assert_eq!(kept.width(), 1);
        assert_eq!(kept.height(), 5);
    }

    #[test]
    fn normalizes_known_text_sentinels_to_null_without_touching_other_types() {
        let frame = df![
            "status" => &[Some("N/A"), Some("Normal"), None, Some("Sin datos"), Some("n/a")],
            "amount" => &[1_i64, 2, 3, 4, 5]
        ]
        .unwrap();

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        assert_eq!(profile.columns[0].sentinel_count, Some(3));
        assert_eq!(profile.columns[1].sentinel_count, None);

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            clean_text_columns(&frame, None, TextCleaningMode::Sentinels)
                .expect("los centinelas deben poder normalizarse");
        let values = cleaned
            .column("status")
            .expect("la columna debe conservarse")
            .str()
            .expect("la columna debe seguir siendo texto");

        assert_eq!(affected_rows, 3);
        assert_eq!(changed_cells, 3);
        assert_eq!(changed_columns[0].name, "status");
        assert_eq!(values.get(0), None);
        assert_eq!(values.get(1), Some("Normal"));
        assert_eq!(values.get(2), None);
        assert_eq!(values.get(3), None);
        assert_eq!(values.get(4), None);
        assert_eq!(
            cleaned.column("amount").unwrap().i64().unwrap().get(0),
            Some(1)
        );
    }

    #[test]
    fn repairs_unambiguous_utf8_mojibake_without_touching_other_types() {
        let frame = df![
            "city" => &[Some("BogotÃ¡"), Some("â€™"), Some("Santo Domingo"), None::<&str>],
            "amount" => &[1_i64, 2, 3, 4]
        ]
        .unwrap();

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        assert_eq!(profile.columns[0].encoding_issue_count, Some(2));
        assert_eq!(profile.columns[1].encoding_issue_count, None);

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            clean_text_columns(&frame, None, TextCleaningMode::FixEncoding)
                .expect("la codificación debe poder corregirse");
        let city = cleaned.column("city").unwrap().str().unwrap();

        assert_eq!(affected_rows, 2);
        assert_eq!(changed_cells, 2);
        assert_eq!(changed_columns[0].name, "city");
        assert_eq!(city.get(0), Some("Bogotá"));
        assert_eq!(city.get(1), Some("’"));
        assert_eq!(city.get(2), Some("Santo Domingo"));
        assert_eq!(city.get(3), None);
        assert_eq!(
            cleaned.column("amount").unwrap().i64().unwrap().get(0),
            Some(1)
        );
    }

    #[test]
    fn nullifies_invalid_values_for_a_confident_suggested_type() {
        let frame = df![
            "created_at" => &[
                Some("2024-01-01"), Some("2024-01-02"), Some("2024-01-03"),
                Some("2024-01-04"), Some("2024-01-05"), Some("2024-01-06"),
                Some("2024-01-07"), Some("2024-01-08"), Some("2024-01-09"),
                Some("sin fecha"),
            ],
            "note" => &[Some("keep"), Some("keep"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave")]
        ]
        .unwrap();

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        assert_eq!(profile.columns[0].suggested_type, Some("date".to_owned()));
        assert_eq!(profile.columns[0].invalid_type_count, Some(1));

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            clean_text_columns(&frame, None, TextCleaningMode::NullifyInvalidTypes)
                .expect("los tipos incompatibles deben poder apartarse");
        let dates = cleaned.column("created_at").unwrap().str().unwrap();
        let notes = cleaned.column("note").unwrap().str().unwrap();

        assert_eq!(affected_rows, 1);
        assert_eq!(changed_cells, 1);
        assert_eq!(changed_columns[0].name, "created_at");
        assert_eq!(dates.get(0), Some("2024-01-01"));
        assert_eq!(dates.get(9), None);
        assert_eq!(notes.get(0), Some("keep"));
    }

    #[test]
    fn imputes_repeated_text_and_lower_median_numeric_nuls_only() {
        let frame = df![
            "status" => &[Some("ok"), None, Some("ok"), Some("review")],
            "amount" => &[Some(10_i64), None, Some(20), Some(30)],
            "notes" => &[Some(""), None, Some("draft"), Some("ready")]
        ]
        .unwrap();

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            impute_missing_values_in_frame(&frame)
                .expect("los nulos imputables deben poder completarse");
        let status = cleaned.column("status").unwrap().str().unwrap();
        let amount = cleaned.column("amount").unwrap().i64().unwrap();
        let notes = cleaned.column("notes").unwrap().str().unwrap();

        assert_eq!(affected_rows, 1);
        assert_eq!(changed_cells, 2);
        assert_eq!(changed_columns.len(), 2);
        assert_eq!(status.get(1), Some("ok"));
        assert_eq!(amount.get(1), Some(20));
        assert_eq!(notes.get(1), None);
    }

    #[test]
    fn normalizes_boolean_aliases_and_profiles_privacy_signals() {
        let frame = df![
            "active" => &[Some("YES"), Some("no"), Some("true"), Some("sí")],
            "email_address" => &[Some("a@example.com"), Some("b@example.com"), Some("c@example.com"), Some("d@example.com")],
            "customer_id" => &[Some("1"), Some("2"), Some("3"), Some("4")]
        ]
        .unwrap();

        let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
        assert_eq!(
            profile.columns[0].suggested_type,
            Some("boolean".to_owned())
        );
        assert_eq!(profile.columns[0].type_match_percentage, Some(100.0));
        assert_eq!(profile.columns[1].privacy_signal, Some("email".to_owned()));
        assert_eq!(
            profile.columns[2].privacy_signal,
            Some("identifier".to_owned())
        );

        let (cleaned, rows, cells, columns) =
            clean_text_columns(&frame, None, TextCleaningMode::Booleans)
                .expect("los alias booleanos deben poder normalizarse");
        let active = cleaned.column("active").unwrap().str().unwrap();
        assert_eq!(active.get(0), Some("true"));
        assert_eq!(active.get(1), Some("false"));
        assert_eq!(active.get(2), Some("true"));
        assert_eq!(active.get(3), Some("true"));
        assert_eq!(rows, 3);
        assert_eq!(cells, 3);
        assert_eq!(columns[0].name, "active");
        assert_eq!(columns[0].changed_cell_count, 3);
    }

    #[test]
    fn activates_row_audit_and_appends_future_change_labels() {
        let path = temporary_csv("value\n1\n2\n");
        let (original, _) = load_csv(&path).expect("el CSV debe cargar");
        let mut dataset = loaded_dataset(path.clone(), original);
        let (audited, added) = add_audit_column_to_frame(&dataset.frame)
            .expect("la columna de auditoría debe poder añadirse");
        assert!(added);
        publish_candidate(&mut dataset, audited, "Activar trazabilidad por fila")
            .expect("la activación debe publicarse");
        assert_eq!(
            dataset
                .frame
                .column("_cambios")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            None
        );

        let mut changed = dataset.frame.clone();
        changed
            .replace("value", Column::new("value".into(), ["3", "2"]))
            .expect("el cambio de prueba debe aplicarse");
        publish_candidate(&mut dataset, changed, "Normalizar booleanos")
            .expect("el cambio posterior debe publicarse");
        let audit = dataset.frame.column("_cambios").unwrap().str().unwrap();
        assert_eq!(audit.get(0), Some("Normalizar booleanos"));
        assert_eq!(audit.get(1), Some("Normalizar booleanos"));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn normalizes_column_names_and_resolves_collisions_deterministically() {
        let path = temporary_csv("Año Venta,ano-venta,2025 Total,/\n1,2,3,4\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let (names, renames) = normalized_column_names(&frame);

        assert_eq!(
            names,
            vec!["ano_venta", "ano_venta_2", "col_2025_total", "unnamed"]
        );
        assert_eq!(renames.len(), 4);
        assert_eq!(renames[0].from, "Año Venta");
        assert_eq!(renames[0].to, "ano_venta");
        assert_eq!(renames[1].to, "ano_venta_2");

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn trims_text_without_changing_internal_spaces_case_accents_or_nulls() {
        let path = temporary_csv("city,note\n\" Bogotá \",\"A  B\"\nLima,\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let (cleaned, rows, cells, columns) =
            clean_text_columns(&frame, None, TextCleaningMode::Trim)
                .expect("los espacios deben limpiarse");
        let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

        assert_eq!(page.rows[0][0].as_deref(), Some("Bogotá"));
        assert_eq!(page.rows[0][1].as_deref(), Some("A  B"));
        assert_eq!(page.rows[1][1], None);
        assert_eq!(rows, 1);
        assert_eq!(cells, 1);
        assert_eq!(columns[0].name, "city");
        assert_eq!(columns[0].changed_cell_count, 1);

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn normalizes_selected_text_and_preserves_unselected_columns() {
        let path = temporary_csv("city,code\n\"  BOGOTÁ\tNORTE  \",\" Ab C \"\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let selected = vec!["city".to_owned()];

        let (cleaned, rows, cells, columns) = clean_text_columns(
            &frame,
            Some(&selected),
            TextCleaningMode::Normalize {
                remove_accents: true,
            },
        )
        .expect("el texto seleccionado debe normalizarse");
        let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

        assert_eq!(page.rows[0][0].as_deref(), Some("bogota norte"));
        assert_eq!(page.rows[0][1].as_deref(), Some(" Ab C "));
        assert_eq!(rows, 1);
        assert_eq!(cells, 1);
        assert_eq!(columns.len(), 1);

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn undo_and_redo_restore_disk_backed_revisions() {
        let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
        let (original, _) = load_csv(&path).expect("el CSV debe cargar");
        let (cleaned, _) =
            remove_duplicate_rows(&original).expect("los duplicados deben eliminarse");
        let mut dataset = loaded_dataset(path.clone(), original.clone());
        publish_candidate(&mut dataset, cleaned, "Eliminar filas duplicadas").unwrap();
        dataset.profile = Some(profile_dataset(&original).expect("el perfil debe existir"));

        let undone = undo_dataset(&mut dataset).expect("el cambio debe deshacerse");
        assert_eq!(undone.dataset.row_count, 3);
        assert!(!undone.history.can_undo);
        assert!(undone.history.can_redo);
        assert!(dataset.profile.is_none());

        let redone = redo_dataset(&mut dataset).expect("el cambio debe rehacerse");
        assert_eq!(redone.dataset.row_count, 2);
        assert!(redone.history.can_undo);
        assert!(!redone.history.can_redo);
        assert!(redo_dataset(&mut dataset).is_err());

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn restored_project_keeps_original_file_name_across_changes_and_history() {
        let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
        let (original, _) = load_csv(&path).expect("el CSV debe cargar");
        let (cleaned, _) = remove_duplicate_rows(&original).unwrap();
        let mut dataset = loaded_dataset(path.clone(), original);
        dataset.source_path = None;
        dataset.file_name = "ventas originales.xlsx".to_owned();
        fs::remove_file(&path).expect("el snapshot persistente puede eliminarse del catálogo");

        let changed = publish_candidate(&mut dataset, cleaned, "Eliminar duplicados").unwrap();
        let undone = undo_dataset(&mut dataset).unwrap();
        let redone = redo_dataset(&mut dataset).unwrap();

        assert_eq!(changed.file_name, "ventas originales.xlsx");
        assert_eq!(undone.dataset.file_name, "ventas originales.xlsx");
        assert_eq!(redone.dataset.file_name, "ventas originales.xlsx");
    }

    #[test]
    fn history_supports_multiple_steps_and_truncates_redo_only_on_real_branch() {
        let path = temporary_csv("value\n1\n");
        let (original, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), original);
        for (value, label) in [(2_i64, "Paso dos"), (3, "Paso tres")] {
            let candidate =
                DataFrame::new(1, vec![Series::new("value".into(), [value]).into()]).unwrap();
            publish_candidate(&mut dataset, candidate, label).unwrap();
        }
        assert_eq!(dataset.history.state().entry_count, 3);
        undo_dataset(&mut dataset).unwrap();
        assert!(dataset.history.state().can_redo);

        // Una receta sin cambios no crea una etapa ni elimina la rama de rehacer.
        let result = apply_recipe_to_dataset(&mut dataset, &TransformRecipe::default()).unwrap();
        assert!(!result.changed);
        assert_eq!(dataset.history.state().entry_count, 3);
        assert!(dataset.history.state().can_redo);

        let branch = DataFrame::new(1, vec![Series::new("value".into(), [4_i64]).into()]).unwrap();
        publish_candidate(&mut dataset, branch, "Rama nueva").unwrap();
        let state = dataset.history.state();
        assert_eq!(state.entry_count, 3);
        assert_eq!(state.current_index, 2);
        assert!(!state.can_redo);
        assert_eq!(state.entries[2].label, "Rama nueva");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn history_evicts_old_snapshots_by_count_and_disables_an_oversize_snapshot() {
        let original =
            DataFrame::new(1, vec![Series::new("value".into(), [0_i64]).into()]).unwrap();
        let mut limited = HistoryManager::with_limits(&original, 3, u64::MAX).unwrap();
        for value in 1_i64..=4 {
            let frame =
                DataFrame::new(1, vec![Series::new("value".into(), [value]).into()]).unwrap();
            limited.record(&frame, &format!("Paso {value}")).unwrap();
        }
        let state = limited.state();
        assert_eq!(state.entry_count, 3);
        assert_eq!(state.current_index, 2);
        assert_eq!(state.entries[0].label, "Paso 2");

        let exact_snapshot_budget = HistoryManager::with_limits(&original, 12, u64::MAX)
            .unwrap()
            .disk_bytes();
        let mut budgeted =
            HistoryManager::with_limits(&original, 12, exact_snapshot_budget).unwrap();
        budgeted
            .record(&original, "Dentro del presupuesto")
            .unwrap();
        let state = budgeted.state();
        assert!(state.snapshots_enabled);
        assert_eq!(state.entry_count, 1);
        assert_eq!(state.entries[0].label, "Dentro del presupuesto");
        assert!(state.disk_bytes <= exact_snapshot_budget);

        let disabled = HistoryManager::with_limits(&original, 12, 0).unwrap();
        let state = disabled.state();
        assert!(!state.snapshots_enabled);
        assert!(!state.can_undo);
        assert_eq!(state.entry_count, 1);
        assert_eq!(state.disk_bytes, 0);
        assert!(state.degraded_reason.unwrap().contains("supera el límite"));
    }

    #[test]
    fn corrupt_restore_and_snapshot_io_failure_leave_dataset_and_cursor_unchanged() {
        let path = temporary_csv("value\n1\n");
        let (original, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), original.clone());
        let changed = DataFrame::new(1, vec![Series::new("value".into(), [2_i64]).into()]).unwrap();
        publish_candidate(&mut dataset, changed, "Cambio").unwrap();
        let before = dataset.frame.clone();
        let cursor = dataset.history.cursor;
        fs::write(&dataset.history.entries[0].path, b"not parquet").unwrap();
        assert!(undo_dataset(&mut dataset).is_err());
        assert_eq!(dataset.history.cursor, cursor);
        assert!(dataset.frame.equals_missing(&before));

        let fresh_path = temporary_csv("value\n1\n");
        let (fresh, _) = load_csv(&fresh_path).unwrap();
        let mut io_failure = loaded_dataset(fresh_path.clone(), fresh.clone());
        fs::remove_dir_all(io_failure.history.directory.path()).unwrap();
        let candidate =
            DataFrame::new(1, vec![Series::new("value".into(), [9_i64]).into()]).unwrap();
        assert!(publish_candidate(&mut io_failure, candidate, "No publicable").is_err());
        assert!(io_failure.frame.equals_missing(&fresh));
        assert_eq!(io_failure.history.cursor, 0);
        fs::remove_file(path).unwrap();
        fs::remove_file(fresh_path).unwrap();
    }

    #[test]
    fn replacing_a_loaded_dataset_removes_the_previous_snapshot_directory() {
        let first_path = temporary_csv("value\n1\n");
        let second_path = temporary_csv("value\n2\n");
        let (first, _) = load_csv(&first_path).unwrap();
        let (second, _) = load_csv(&second_path).unwrap();
        let mut current = Some(loaded_dataset(first_path.clone(), first));
        let old_directory = current
            .as_ref()
            .unwrap()
            .history
            .directory
            .path()
            .to_path_buf();
        assert!(old_directory.exists());
        current = Some(loaded_dataset(second_path.clone(), second));
        assert!(!old_directory.exists());
        drop(current);
        fs::remove_file(first_path).unwrap();
        fs::remove_file(second_path).unwrap();
    }

    #[test]
    fn applies_safe_corrections_in_one_candidate_frame() {
        let path = temporary_csv("Año Venta,city\n1,\" Bogotá \"\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

        let (corrected, rows, cells, renames) =
            safe_corrected_frame(&frame).expect("las correcciones deben aplicarse");
        let page = dataset_page(&corrected, 0, 50).expect("la vista previa debe generarse");

        assert_eq!(
            corrected
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["ano_venta", "city"]
        );
        assert_eq!(page.rows[0][1].as_deref(), Some("Bogotá"));
        assert_eq!(rows, 1);
        assert_eq!(cells, 1);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].to, "ano_venta");

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn loads_parquet_preserving_schema_nulls_and_unicode() {
        let source = temporary_csv("city,value\nSanto Domingo,10\nBogotá,\n");
        let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
        let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
        let parquet = directory.path().join("dataset.parquet");
        export_frame_atomic(&frame, &parquet, ExportFormat::Parquet, |_, _| {}, || false)
            .expect("el Parquet debe escribirse");
        let mut updates = Vec::new();

        let (loaded, preview) = load_dataset_with_progress(
            &parquet,
            |stage, percent| updates.push((stage, percent)),
            || false,
        )
        .expect("el Parquet debe cargarse");

        assert_eq!(loaded.dtypes(), frame.dtypes());
        assert_eq!(preview.file_name, "dataset.parquet");
        assert_eq!(preview.row_count, 2);
        assert_eq!(preview.rows[1][0].as_deref(), Some("Bogotá"));
        assert_eq!(preview.rows[1][1], None);
        assert_eq!(updates.first(), Some(&("Validando archivo", 10)));
        assert_eq!(updates.last(), Some(&("Preparando sesión", 95)));

        fs::remove_file(source).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn loads_tsv_with_tabs_and_preserves_lexical_values() {
        let path = temporary_delimited(
            "tsv",
            "id\tdescription\tamount\n00123\t\"Santo Domingo, RD\"\t1.00\n18446744073709551616\tBogotá\t\n",
        );

        let (frame, preview) =
            load_dataset_with_progress(&path, |_, _| {}, || false).expect("el TSV debe cargar");

        assert!(frame
            .dtypes()
            .iter()
            .all(|kind| *kind == polars::prelude::DataType::String));
        assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
        assert_eq!(preview.rows[0][1].as_deref(), Some("Santo Domingo, RD"));
        assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
        assert_eq!(preview.rows[1][0].as_deref(), Some("18446744073709551616"));
        assert_eq!(preview.rows[1][2], None);

        fs::remove_file(path).expect("se debe limpiar el TSV temporal");
    }

    #[test]
    fn detects_semicolon_csv_and_ignores_delimiters_inside_quotes() {
        let path = temporary_delimited(
            "csv",
            "id;place;amount\n00123;\"Santo Domingo, RD\";1.00\n00007;Santiago;2.50\n",
        );

        let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect("el CSV con punto y coma debe cargar");

        assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
        assert_eq!(preview.column_count, 3);
        assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
        assert_eq!(preview.rows[0][1].as_deref(), Some("Santo Domingo, RD"));
        assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn accepts_utf8_bom_without_including_it_in_the_header() {
        let path = temporary_delimited("csv", "\u{feff}id;city\n001;Santo Domingo\n002;Santiago\n");

        let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect("el CSV UTF-8 con BOM debe cargar");

        assert_eq!(frame.get_column_names()[0].as_str(), "id");
        assert_eq!(preview.rows[0][0].as_deref(), Some("001"));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn detects_pipe_delimited_txt_without_numeric_inference() {
        let path = temporary_delimited(
            "txt",
            "code|description|amount\n0001|alpha|1.00\n0002|beta|2.50\n",
        );

        let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect("el TXT delimitado debe cargar");

        assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
        assert_eq!(preview.column_count, 3);
        assert_eq!(preview.rows[0][0].as_deref(), Some("0001"));
        assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
        fs::remove_file(path).expect("se debe limpiar el TXT temporal");
    }

    #[test]
    fn rejects_invalid_utf8_instead_of_replacing_characters() {
        let path = temporary_delimited_bytes("csv", b"id,city\n001,Santo Domingo\n002,Bogot\xe1\n");

        let error = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect_err("los bytes que no son UTF-8 deben rechazarse");

        assert!(error.contains("UTF-8 válido"));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn converts_spreadsheet_range_with_unique_headers_and_safe_types() {
        let mut range = Range::<Data>::new((0, 0), (2, 3));
        range.set_value((0, 0), Data::String("id".to_owned()));
        range.set_value((0, 1), Data::String("id".to_owned()));
        range.set_value((0, 2), Data::Empty);
        range.set_value((0, 3), Data::String("active".to_owned()));
        range.set_value((1, 0), Data::Int(1));
        range.set_value((2, 0), Data::Int(2));
        range.set_value((1, 1), Data::String("001".to_owned()));
        range.set_value((2, 1), Data::Error(calamine::CellErrorType::Div0));
        range.set_value((1, 2), Data::Float(1.5));
        range.set_value((2, 2), Data::Empty);
        range.set_value((1, 3), Data::Bool(true));
        range.set_value((2, 3), Data::Bool(false));

        let frame = spreadsheet_range_to_frame(&range, SpreadsheetHeaderMode::FirstRow)
            .expect("la hoja debe convertirse");
        assert_eq!(frame.height(), 2);
        assert_eq!(
            frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "id_2", "column_3", "active"]
        );
        assert_eq!(frame.dtypes()[0], polars::prelude::DataType::Int64);
        assert_eq!(frame.dtypes()[1], polars::prelude::DataType::String);
        assert_eq!(frame.dtypes()[2], polars::prelude::DataType::Float64);
        assert_eq!(frame.dtypes()[3], polars::prelude::DataType::Boolean);
        let page = dataset_page(&frame, 0, 50).expect("debe generarse la vista previa");
        assert_eq!(page.rows[0][1].as_deref(), Some("001"));
        assert_eq!(page.rows[1][1].as_deref(), Some("#DIV/0!"));
    }

    #[test]
    fn generates_spreadsheet_headers_without_consuming_the_first_row() {
        let mut range = Range::<Data>::new((0, 0), (1, 1));
        range.set_value((0, 0), Data::String("001".to_owned()));
        range.set_value((0, 1), Data::String("Santo Domingo".to_owned()));
        range.set_value((1, 0), Data::String("002".to_owned()));
        range.set_value((1, 1), Data::String("Santiago".to_owned()));

        let frame = spreadsheet_range_to_frame(&range, SpreadsheetHeaderMode::Generated)
            .expect("la hoja debe usar encabezados generados");

        assert_eq!(frame.height(), 2);
        assert_eq!(
            frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["column_1", "column_2"]
        );
        let page = dataset_page(&frame, 0, 50).expect("debe conservar la primera fila");
        assert_eq!(page.rows[0][0].as_deref(), Some("001"));
        assert_eq!(page.rows[0][1].as_deref(), Some("Santo Domingo"));
    }

    #[test]
    fn loads_json_record_array_with_union_of_fields_and_nested_values() {
        let path = temporary_delimited(
            "json",
            r#"[
                {"id": 1, "active": true, "meta": {"city": "Santo Domingo"}},
                {"id": 2, "amount": 1.5, "active": null, "meta": ["a", "b"]}
            ]"#,
        );

        let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect("el arreglo JSON debe cargar");

        assert_eq!(frame.height(), 2);
        assert_eq!(
            frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["active", "id", "meta", "amount"]
        );
        assert_eq!(frame.dtypes()[0], polars::prelude::DataType::Boolean);
        assert_eq!(frame.dtypes()[1], polars::prelude::DataType::Int64);
        assert_eq!(frame.dtypes()[2], polars::prelude::DataType::String);
        assert_eq!(frame.dtypes()[3], polars::prelude::DataType::Float64);
        assert_eq!(
            preview.rows[0][2].as_deref(),
            Some(r#"{"city":"Santo Domingo"}"#)
        );
        assert_eq!(preview.rows[0][3], None);

        fs::remove_file(path).expect("se debe limpiar el JSON temporal");
    }

    #[test]
    fn loads_json_lines_and_rejects_non_object_records() {
        let valid = temporary_delimited(
            "jsonl",
            "{\"code\":\"001\",\"value\":10}\n{\"code\":\"002\",\"value\":20}\n",
        );
        let (_, preview) = load_dataset_with_progress(&valid, |_, _| {}, || false)
            .expect("JSON Lines debe cargar");
        assert_eq!(preview.rows[0][0].as_deref(), Some("001"));
        fs::remove_file(valid).expect("se debe limpiar JSON Lines");

        let invalid = temporary_delimited("json", "[{\"id\":1}, 2]");
        let error = load_dataset_with_progress(&invalid, |_, _| {}, || false)
            .expect_err("los registros escalares deben rechazarse");
        assert!(error.contains("registro JSON 2 no es un objeto"));
        fs::remove_file(invalid).expect("se debe limpiar el JSON inválido");
    }

    #[test]
    fn preserves_json_integers_larger_than_u64_as_text() {
        let path = temporary_delimited(
            "json",
            "[{\"identifier\":184467440737095516160},{\"identifier\":1}]",
        );

        let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
            .expect("el entero JSON grande debe preservarse");

        assert_eq!(frame.dtypes()[0], polars::prelude::DataType::String);
        assert_eq!(preview.rows[0][0].as_deref(), Some("184467440737095516160"));
        assert_eq!(preview.rows[1][0].as_deref(), Some("1"));
        fs::remove_file(path).expect("se debe limpiar el JSON temporal");
    }

    #[test]
    fn structural_recipe_applies_swapped_renames_strict_casts_and_dates_in_order() {
        let frame = DataFrame::new(
            2,
            vec![
                Series::new("amount".into(), [Some("10.50"), None]).into_column(),
                Series::new("count".into(), [Some("42"), Some("-7")]).into_column(),
                Series::new("enabled".into(), [Some("TRUE"), Some("false")]).into_column(),
                Series::new("day".into(), [Some("31/12/2025"), Some("01/01/2026")]).into_column(),
                Series::new("left".into(), ["L1", "L2"]).into_column(),
                Series::new("right".into(), ["R1", "R2"]).into_column(),
            ],
        )
        .expect("el frame debe ser válido");
        let recipe = TransformRecipe {
            renames: vec![
                RecipeRename {
                    from: "left".into(),
                    to: "right".into(),
                },
                RecipeRename {
                    from: "right".into(),
                    to: "left".into(),
                },
                RecipeRename {
                    from: "count".into(),
                    to: "units".into(),
                },
            ],
            casts: vec![
                RecipeCast {
                    column: "amount".into(),
                    target: RecipeCastTarget::Decimal,
                },
                // References the pre-rename name deliberately.
                RecipeCast {
                    column: "count".into(),
                    target: RecipeCastTarget::Integer,
                },
                RecipeCast {
                    column: "enabled".into(),
                    target: RecipeCastTarget::Boolean,
                },
            ],
            date_parses: vec![RecipeDateParse {
                column: "day".into(),
                format: RecipeDateFormat::Dmy,
                target: RecipeDateTarget::Date,
            }],
            ..Default::default()
        };

        let (
            result,
            renamed,
            converted,
            dates,
            removed,
            calculated,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
        ) = apply_recipe_to_frame(&frame, &recipe).expect("la receta debe ser atómica y válida");
        assert_eq!((renamed, converted, dates), (3, 3, 1));
        assert_eq!((removed, calculated), (0, 0));
        assert_eq!(
            result.column("units").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            result.column("amount").unwrap().dtype(),
            &polars::prelude::DataType::Float64
        );
        assert_eq!(
            result.column("enabled").unwrap().dtype(),
            &polars::prelude::DataType::Boolean
        );
        assert_eq!(
            result.column("day").unwrap().dtype(),
            &polars::prelude::DataType::Date
        );
        assert_eq!(
            dataset_page(&result, 0, 2).unwrap().rows[0][4].as_deref(),
            Some("L1")
        );
        assert_eq!(
            dataset_page(&result, 0, 2).unwrap().rows[0][5].as_deref(),
            Some("R1")
        );
    }

    #[test]
    fn structural_recipe_supports_all_explicit_date_formats_and_datetime_targets() {
        for (value, format) in [
            ("2025-12-31", RecipeDateFormat::Ymd),
            ("31/12/2025", RecipeDateFormat::Dmy),
            ("12/31/2025", RecipeDateFormat::Mdy),
            ("2025-12-31T23:15:30Z", RecipeDateFormat::Iso8601),
        ] {
            let column = Series::new("when".into(), [value]).into_column();
            let converted = strict_date_column(&column, format, RecipeDateTarget::Datetime)
                .expect("el formato explícito debe aceptarse");
            assert!(matches!(
                converted.dtype(),
                polars::prelude::DataType::Datetime(_, _)
            ));
        }
    }

    #[test]
    fn dataprep_date_cleaning_skips_ambiguous_columns_instead_of_creating_nulls() {
        let frame = df![
            "safe" => &["2025-01-02", "2025-01-03", "2025-01-04", "2025-01-05"],
            "ambiguous" => &["01/02/2025", "02/03/2025", "2025/04/05", "May 6, 2025"]
        ]
        .expect("la fixture de fechas debe construirse");

        let (cleaned, changed_rows, changed_cells, changed_columns) =
            parse_dataprep_date_columns(&frame).expect("el parseo conservador debe completarse");

        assert!(matches!(
            cleaned.column("safe").unwrap().dtype(),
            polars::prelude::DataType::Datetime(_, _)
        ));
        assert_eq!(
            cleaned.column("ambiguous").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(changed_rows, 4);
        assert_eq!(changed_cells, 4);
        assert_eq!(changed_columns.len(), 1);
        assert_eq!(changed_columns[0].name, "safe");
    }

    #[test]
    fn dataprep_numeric_cast_skips_identifiers_and_leading_zero_codes() {
        let frame = df![
            "amount" => &["10.5", "11.5", "12.5"],
            "code" => &["001", "002", "003"],
            "customer_id" => &["100", "101", "102"]
        ]
        .expect("la fixture numérica debe construirse");

        let (cast, changed_rows, changed_cells, changed_columns) =
            cast_dataprep_numeric_columns(&frame).expect("el cast seguro debe completarse");

        assert_eq!(cast.column("amount").unwrap().dtype(), &DataType::Float64);
        assert_eq!(cast.column("code").unwrap().dtype(), &DataType::String);
        assert_eq!(
            cast.column("customer_id").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(changed_rows, 3);
        assert_eq!(changed_cells, 3);
        assert_eq!(changed_columns.len(), 1);
        assert_eq!(changed_columns[0].name, "amount");
    }

    #[test]
    fn structural_recipe_rolls_back_fully_on_invalid_value_and_does_not_create_undo() {
        let path = temporary_csv("count\n1\nnot-an-integer\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let original = frame.clone();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let recipe = TransformRecipe {
            renames: vec![RecipeRename {
                from: "count".into(),
                to: "units".into(),
            }],
            casts: vec![RecipeCast {
                column: "count".into(),
                target: RecipeCastTarget::Integer,
            }],
            date_parses: vec![],
            ..Default::default()
        };

        let error = apply_recipe_to_dataset(&mut dataset, &recipe)
            .expect_err("un valor inválido debe abortar toda la receta");
        assert!(error.contains("fila 2"));
        assert!(dataset.frame.equals_missing(&original));
        assert!(!dataset.history.state().can_undo);
        assert!(!dataset.history.state().can_redo);

        let calculation_error = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                renames: vec![RecipeRename {
                    from: "count".into(),
                    to: "units".into(),
                }],
                calculated_column: Some(CalculatedColumnRecipe {
                    name: "ratio".into(),
                    source: "count".into(),
                    operation: CalculatedOperation::Divide,
                    operand: Some(CalculatedOperand {
                        kind: CalculatedOperandKind::Literal,
                        value: "0".into(),
                    }),
                }),
                ..Default::default()
            },
        )
        .expect_err("el cálculo inválido debe abortar también el renombrado");
        assert!(
            calculation_error.contains("División por cero") || calculation_error.contains("número")
        );
        assert!(dataset.frame.equals_missing(&original));
        assert!(!dataset.history.state().can_undo);
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");

        let metadata_error = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                renames: vec![RecipeRename {
                    from: "count".into(),
                    to: "units".into(),
                }],
                casts: vec![],
                date_parses: vec![],
                ..Default::default()
            },
        )
        .expect_err("un fallo al preparar la respuesta también debe abortar");
        assert!(metadata_error.contains("metadatos"));
        assert!(dataset.frame.equals_missing(&original));
        assert!(!dataset.history.state().can_undo);
    }

    #[test]
    fn empty_or_already_satisfied_recipe_is_a_noop_without_history() {
        let path = temporary_csv("value\n1\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let mut dataset = loaded_dataset(path.clone(), frame);
        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                renames: vec![RecipeRename {
                    from: "value".into(),
                    to: "value".into(),
                }],
                casts: vec![],
                date_parses: vec![],
                ..Default::default()
            },
        )
        .expect("una receta ya satisfecha debe ser válida");
        assert!(!result.changed);
        assert!(!dataset.history.state().can_undo);
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn structural_recipe_rejects_cross_step_conflicts_and_final_name_collisions() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("a".into(), ["2025-01-01"]).into_column(),
                Series::new("b".into(), ["value"]).into_column(),
            ],
        )
        .unwrap();
        let conflict = TransformRecipe {
            casts: vec![RecipeCast {
                column: "a".into(),
                target: RecipeCastTarget::String,
            }],
            date_parses: vec![RecipeDateParse {
                column: "a".into(),
                format: RecipeDateFormat::Ymd,
                target: RecipeDateTarget::Date,
            }],
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &conflict)
            .err()
            .unwrap()
            .contains("misma receta"));

        let collision = TransformRecipe {
            renames: vec![RecipeRename {
                from: "a".into(),
                to: "b".into(),
            }],
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &collision)
            .err()
            .unwrap()
            .contains("duplicados"));
    }

    #[test]
    fn recipe_filters_use_stable_and_null_safe_semantics() {
        let frame = DataFrame::new(
            4,
            vec![
                Series::new(
                    "city".into(),
                    [
                        Some("Santo Domingo"),
                        Some("Santiago"),
                        None,
                        Some("santo cielo"),
                    ],
                )
                .into_column(),
                Series::new(
                    "amount".into(),
                    [Some("10"), Some("20"), Some("30"), Some("40")],
                )
                .into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            filters: vec![
                RecipeFilter {
                    column: "city".into(),
                    operator: RecipeFilterOperator::Contains,
                    value: Some("SANTO".into()),
                },
                RecipeFilter {
                    column: "amount".into(),
                    operator: RecipeFilterOperator::Gte,
                    value: Some("20".into()),
                },
            ],
            ..Default::default()
        };
        let (result, _, _, _, removed, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            dataset_page(&result, 0, 10).unwrap().rows[0][0].as_deref(),
            Some("santo cielo")
        );

        let null_filter = TransformRecipe {
            filters: vec![RecipeFilter {
                column: "city".into(),
                operator: RecipeFilterOperator::IsNull,
                value: None,
            }],
            ..Default::default()
        };
        assert_eq!(
            apply_recipe_to_frame(&frame, &null_filter)
                .unwrap()
                .0
                .height(),
            1
        );
    }

    #[test]
    fn lazy_recipe_casts_filters_and_calculates_in_one_plan() {
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("amount".into(), ["5", "12", "20"]).into_column(),
                Series::new("segment".into(), ["discard", "keep", "keep"]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            casts: vec![RecipeCast {
                column: "amount".into(),
                target: RecipeCastTarget::Decimal,
            }],
            filters: vec![RecipeFilter {
                column: "amount".into(),
                operator: RecipeFilterOperator::Gte,
                value: Some("10".into()),
            }],
            calculated_column: Some(CalculatedColumnRecipe {
                name: "total".into(),
                source: "amount".into(),
                operation: CalculatedOperation::Multiply,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "2".into(),
                }),
            }),
            ..Default::default()
        };

        let (
            result,
            _,
            converted,
            _,
            removed,
            calculated,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
        ) = apply_recipe_to_frame(&frame, &recipe).expect("la receta simple debe usar lazy");
        assert_eq!(converted, 1);
        assert_eq!(removed, 1);
        assert_eq!(calculated, 1);
        assert_eq!(result.column("amount").unwrap().dtype(), &DataType::Float64);
        assert_eq!(
            dataset_page(&result, 0, 10).unwrap().rows[0][2].as_deref(),
            Some("24.0")
        );
        assert_eq!(
            dataset_page(&result, 0, 10).unwrap().rows[1][2].as_deref(),
            Some("40.0")
        );
    }

    #[test]
    fn lazy_group_summary_preserves_stable_groups_nulls_and_counts() {
        let frame = DataFrame::new(
            5,
            vec![
                Series::new(
                    "group".into(),
                    [Some("B"), None, Some("B"), None, Some("A")],
                )
                .into_column(),
                Series::new(
                    "value".into(),
                    [Some("2"), None, Some("4"), Some("8"), None],
                )
                .into_column(),
                Series::new(
                    "label".into(),
                    [Some("z"), Some("x"), Some("a"), Some("x"), None],
                )
                .into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            casts: vec![RecipeCast {
                column: "value".into(),
                target: RecipeCastTarget::Integer,
            }],
            group_summary: Some(GroupSummaryRecipe {
                group_by: vec!["group".into()],
                aggregations: vec![
                    SummaryAggregation {
                        column: "value".into(),
                        operation: SummaryOperation::Sum,
                    },
                    SummaryAggregation {
                        column: "value".into(),
                        operation: SummaryOperation::Mean,
                    },
                    SummaryAggregation {
                        column: "value".into(),
                        operation: SummaryOperation::Count,
                    },
                    SummaryAggregation {
                        column: "label".into(),
                        operation: SummaryOperation::CountUnique,
                    },
                    SummaryAggregation {
                        column: "label".into(),
                        operation: SummaryOperation::Min,
                    },
                ],
            }),
            ..Default::default()
        };
        let (
            result,
            _,
            cast_count,
            _,
            removed,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            groups,
            aggregations,
            collapsed,
            _,
            _,
            _,
        ) = apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(
            (cast_count, removed, groups, aggregations, collapsed),
            (1, 0, 3, 5, 2)
        );
        let rows = dataset_page(&result, 0, 10).unwrap().rows;
        assert_eq!(rows[0][0].as_deref(), Some("B"));
        assert_eq!(rows[1][0], None);
        assert_eq!(rows[0][1].as_deref(), Some("6"));
        assert_eq!(rows[1][1].as_deref(), Some("8"));
        assert_eq!(rows[2][1].as_deref(), Some("0"));
        assert_eq!(rows[1][3].as_deref(), Some("2"));
        assert_eq!(rows[1][4].as_deref(), Some("1"));
        assert_eq!(rows[2][5], None);
    }

    #[test]
    fn recipe_filters_validate_the_same_input_independent_of_order() {
        let frame = DataFrame::new(
            2,
            vec![
                Series::new("group".into(), ["keep", "discard"]).into_column(),
                Series::new("amount".into(), ["10", "invalid"]).into_column(),
            ],
        )
        .unwrap();
        let selective = RecipeFilter {
            column: "group".into(),
            operator: RecipeFilterOperator::Eq,
            value: Some("keep".into()),
        };
        let numeric = RecipeFilter {
            column: "amount".into(),
            operator: RecipeFilterOperator::Gt,
            value: Some("5".into()),
        };

        for filters in [
            vec![selective.clone(), numeric.clone()],
            vec![numeric.clone(), selective.clone()],
        ] {
            let error = apply_recipe_to_frame(
                &frame,
                &TransformRecipe {
                    filters,
                    ..Default::default()
                },
            )
            .err()
            .expect("el valor inválido debe rechazarse sin importar el orden");
            assert!(error.contains("fila 2"));
        }
    }

    #[test]
    fn recipe_calculation_remaps_column_operands_and_preserves_nulls() {
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("price".into(), [Some("10"), None, Some("5")]).into_column(),
                Series::new("quantity".into(), [Some("2"), Some("3"), Some("4")]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            renames: vec![RecipeRename {
                from: "quantity".into(),
                to: "units".into(),
            }],
            calculated_column: Some(CalculatedColumnRecipe {
                name: "total".into(),
                source: "price".into(),
                operation: CalculatedOperation::Multiply,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Column,
                    value: "quantity".into(),
                }),
            }),
            ..Default::default()
        };
        let (result, _, _, _, _, calculated, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(calculated, 1);
        let rows = dataset_page(&result, 0, 10).unwrap().rows;
        assert_eq!(rows[0][2].as_deref(), Some("20.0"));
        assert_eq!(rows[1][2], None);
        assert_eq!(rows[2][2].as_deref(), Some("20.0"));
    }

    #[test]
    fn recipe_rejects_division_by_zero_precision_loss_and_too_many_filters() {
        let frame = DataFrame::new(
            1,
            vec![Series::new("value".into(), ["9007199254740993"]).into_column()],
        )
        .unwrap();
        let divide = TransformRecipe {
            calculated_column: Some(CalculatedColumnRecipe {
                name: "result".into(),
                source: "value".into(),
                operation: CalculatedOperation::Divide,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "0".into(),
                }),
            }),
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &divide)
            .err()
            .unwrap()
            .contains("precisión"));

        let safe =
            DataFrame::new(1, vec![Series::new("value".into(), ["1"]).into_column()]).unwrap();
        assert!(apply_recipe_to_frame(&safe, &divide)
            .err()
            .unwrap()
            .contains("División por cero"));
        let too_many = TransformRecipe {
            filters: (0..4)
                .map(|_| RecipeFilter {
                    column: "value".into(),
                    operator: RecipeFilterOperator::Eq,
                    value: Some("1".into()),
                })
                .collect(),
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&safe, &too_many)
            .err()
            .unwrap()
            .contains("máximo tres"));
    }

    #[test]
    fn calculated_datetime_parts_support_values_before_unix_epoch() {
        let datetime = Series::new("when".into(), [Some(-1_i64)])
            .cast(&polars::prelude::DataType::Datetime(
                TimeUnit::Milliseconds,
                None,
            ))
            .unwrap()
            .into_column();
        let frame = DataFrame::new(1, vec![datetime]).unwrap();
        let recipe = TransformRecipe {
            calculated_column: Some(CalculatedColumnRecipe {
                name: "year".into(),
                source: "when".into(),
                operation: CalculatedOperation::Year,
                operand: None,
            }),
            ..Default::default()
        };
        let result = apply_recipe_to_frame(&frame, &recipe).unwrap().0;
        assert_eq!(
            dataset_page(&result, 0, 1).unwrap().rows[0][1].as_deref(),
            Some("1969")
        );

        let extreme = Series::new("when".into(), [Some(i64::MAX)])
            .cast(&polars::prelude::DataType::Datetime(
                TimeUnit::Milliseconds,
                None,
            ))
            .unwrap()
            .into_column();
        let extreme_frame = DataFrame::new(1, vec![extreme]).unwrap();
        assert!(apply_recipe_to_frame(&extreme_frame, &recipe)
            .err()
            .unwrap()
            .contains("fuera del rango"));
    }

    #[test]
    fn find_replace_is_literal_unicode_null_safe_and_counts_cells() {
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("text".into(), [Some("á-á"), Some("á"), None]).into_column(),
                Series::new("number".into(), [1_i64, 2, 3]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("text".into()),
                find: "á".into(),
                replace: "🙂".into(),
            }),
            ..Default::default()
        };
        let (result, _, _, _, _, _, replaced, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(replaced, 2);
        let rows = dataset_page(&result, 0, 10).unwrap().rows;
        assert_eq!(rows[0][0].as_deref(), Some("🙂-🙂"));
        assert_eq!(rows[2][0], None);
        assert_eq!(
            result.column("number").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        let identical = TransformRecipe {
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("text".into()),
                find: "á".into(),
                replace: "á".into(),
            }),
            ..Default::default()
        };
        assert_eq!(apply_recipe_to_frame(&frame, &identical).unwrap().6, 0);
    }

    #[test]
    fn find_replace_all_text_columns_skips_physical_non_text_and_remaps_rename() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("first".into(), ["x value"]).into_column(),
                Series::new("second".into(), ["x"]).into_column(),
                Series::new("count".into(), [10_i64]).into_column(),
            ],
        )
        .unwrap();
        let all = TransformRecipe {
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::AllTextColumns,
                column: None,
                find: "x".into(),
                replace: "y".into(),
            }),
            ..Default::default()
        };
        assert_eq!(apply_recipe_to_frame(&frame, &all).unwrap().6, 2);

        let renamed = TransformRecipe {
            renames: vec![RecipeRename {
                from: "first".into(),
                to: "title".into(),
            }],
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("first".into()),
                find: "x".into(),
                replace: "z".into(),
            }),
            ..Default::default()
        };
        let result = apply_recipe_to_frame(&frame, &renamed).unwrap().0;
        assert_eq!(
            dataset_page(&result, 0, 1).unwrap().rows[0][0].as_deref(),
            Some("z value")
        );
    }

    #[test]
    fn lazy_find_replace_counts_after_string_cast_and_preserves_nulls() {
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("code".into(), [12_i64, 20, 30]).into_column(),
                Series::new("note".into(), [Some("x"), None, Some("z")]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            casts: vec![RecipeCast {
                column: "code".into(),
                target: RecipeCastTarget::String,
            }],
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("code".into()),
                find: "2".into(),
                replace: "X".into(),
            }),
            ..Default::default()
        };

        let (result, _, converted, _, _, _, replaced, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(converted, 1);
        assert_eq!(replaced, 2);
        assert_eq!(result.column("code").unwrap().dtype(), &DataType::String);
        assert_eq!(
            dataset_page(&result, 0, 3).unwrap().rows,
            vec![
                vec![Some("1X".into()), Some("x".into())],
                vec![Some("X0".into()), None],
                vec![Some("30".into()), Some("z".into())],
            ]
        );
    }

    #[test]
    fn keep_columns_remaps_reorders_and_reports_drops() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("a".into(), ["A"]).into_column(),
                Series::new("b".into(), ["B"]).into_column(),
                Series::new("c".into(), ["C"]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            renames: vec![RecipeRename {
                from: "a".into(),
                to: "alpha".into(),
            }],
            keep_columns: Some(vec!["c".into(), "a".into()]),
            ..Default::default()
        };
        let (result, _, _, _, _, _, _, dropped, _, _, _, _, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(dropped, 1);
        assert_eq!(
            result
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["c", "alpha"]
        );
    }

    #[test]
    fn keep_columns_rejects_empty_duplicate_missing_and_dropped_calculation_source() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("a".into(), ["1"]).into_column(),
                Series::new("b".into(), ["2"]).into_column(),
            ],
        )
        .unwrap();
        for keep in [vec![], vec!["a".into(), "a".into()], vec!["missing".into()]] {
            assert!(apply_recipe_to_frame(
                &frame,
                &TransformRecipe {
                    keep_columns: Some(keep),
                    ..Default::default()
                }
            )
            .is_err());
        }
        let calculation = TransformRecipe {
            keep_columns: Some(vec!["b".into()]),
            calculated_column: Some(CalculatedColumnRecipe {
                name: "result".into(),
                source: "a".into(),
                operation: CalculatedOperation::Add,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "1".into(),
                }),
            }),
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &calculation)
            .err()
            .unwrap()
            .contains("descartada"));

        let renamed_success = TransformRecipe {
            renames: vec![RecipeRename {
                from: "a".into(),
                to: "alpha".into(),
            }],
            keep_columns: Some(vec!["a".into()]),
            calculated_column: Some(CalculatedColumnRecipe {
                name: "result".into(),
                source: "a".into(),
                operation: CalculatedOperation::Add,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "1".into(),
                }),
            }),
            ..Default::default()
        };
        let result = apply_recipe_to_frame(&frame, &renamed_success).unwrap().0;
        assert_eq!(
            result
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "result"]
        );
    }

    #[test]
    fn reorder_only_keep_columns_publishes_one_undo_revision() {
        let path = temporary_csv("a,b\nA,B\n");
        let (frame, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                keep_columns: Some(vec!["b".into(), "a".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(result.changed);
        assert_eq!(result.dropped_column_count, 0);
        assert_eq!(
            dataset
                .frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert!(dataset.history.state().can_undo);
        undo_dataset(&mut dataset).unwrap();
        assert_eq!(
            dataset
                .frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_is_literal_unicode_uses_remainder_and_preserves_missing_null_and_empty() {
        let frame = DataFrame::new(
            4,
            vec![Series::new(
                "path".into(),
                [
                    Some("uno🙂dos🙂tres🙂resto"),
                    Some("solo"),
                    None,
                    Some("a🙂"),
                ],
            )
            .into_column()],
        )
        .unwrap();
        let recipe = TransformRecipe {
            split_column: Some(SplitColumnRecipe {
                source: "path".into(),
                delimiter: "🙂".into(),
                names: vec!["first".into(), "second".into(), "third".into()],
                drop_source: false,
            }),
            ..Default::default()
        };
        let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(outcome.9, 3);
        let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
        assert_eq!(rows[0][3].as_deref(), Some("tres🙂resto"));
        assert_eq!(rows[1][1].as_deref(), Some("solo"));
        assert_eq!(rows[1][2], None);
        assert_eq!(rows[2][1], None);
        assert_eq!(rows[3][2].as_deref(), Some(""));
        assert_eq!(rows[3][3], None);
    }

    #[test]
    fn merge_preserves_source_order_nulls_empty_strings_and_separator() {
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("a".into(), [Some("A"), None, None]).into_column(),
                Series::new("b".into(), [Some(""), Some("B"), None]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            merge_columns: Some(MergeColumnsRecipe {
                sources: vec!["a".into(), "b".into()],
                name: "joined".into(),
                separator: "🙂".into(),
                drop_sources: true,
            }),
            ..Default::default()
        };
        let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!((outcome.10, outcome.11), (1, 2));
        let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
        assert_eq!(rows[0][0].as_deref(), Some("A🙂"));
        assert_eq!(rows[1][0].as_deref(), Some("B"));
        assert_eq!(rows[2][0], None);
    }

    #[test]
    fn lazy_merge_accepts_numeric_source_cast_to_text() {
        let frame = DataFrame::new(
            2,
            vec![
                Series::new("number".into(), [12_i64, 34]).into_column(),
                Series::new("label".into(), ["A", "B"]).into_column(),
            ],
        )
        .unwrap();
        let recipe = TransformRecipe {
            casts: vec![RecipeCast {
                column: "number".into(),
                target: RecipeCastTarget::String,
            }],
            merge_columns: Some(MergeColumnsRecipe {
                sources: vec!["number".into(), "label".into()],
                name: "joined".into(),
                separator: ":".into(),
                drop_sources: true,
            }),
            ..Default::default()
        };

        let (result, _, converted, _, _, _, _, _, _, _, merged, dropped, _, _, _, _, _, _, _, _, _) =
            apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!((converted, merged, dropped), (1, 1, 2));
        assert_eq!(
            dataset_page(&result, 0, 2).unwrap().rows,
            vec![vec![Some("12:A".into())], vec![Some("34:B".into())],]
        );
    }

    #[test]
    fn split_merge_remap_renames_and_validate_keep_and_drop_dependencies() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("full".into(), ["A-B"]).into_column(),
                Series::new("other".into(), ["C"]).into_column(),
            ],
        )
        .unwrap();
        let success = TransformRecipe {
            renames: vec![RecipeRename {
                from: "full".into(),
                to: "renamed".into(),
            }],
            keep_columns: Some(vec!["full".into(), "other".into()]),
            split_column: Some(SplitColumnRecipe {
                source: "full".into(),
                delimiter: "-".into(),
                names: vec!["left".into(), "right".into()],
                drop_source: false,
            }),
            merge_columns: Some(MergeColumnsRecipe {
                sources: vec!["full".into(), "other".into()],
                name: "joined".into(),
                separator: ":".into(),
                drop_sources: false,
            }),
            ..Default::default()
        };
        let result = apply_recipe_to_frame(&frame, &success).unwrap().0;
        assert_eq!(
            dataset_page(&result, 0, 1).unwrap().rows[0][4].as_deref(),
            Some("A-B:C")
        );

        let dropped = TransformRecipe {
            keep_columns: Some(vec!["other".into()]),
            split_column: success.split_column.clone(),
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &dropped)
            .err()
            .unwrap()
            .contains("keepColumns"));
        let conflict = TransformRecipe {
            split_column: Some(SplitColumnRecipe {
                drop_source: true,
                ..success.split_column.unwrap()
            }),
            merge_columns: success.merge_columns,
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &conflict)
            .err()
            .unwrap()
            .contains("descartaría"));
    }

    #[test]
    fn invalid_split_collision_rolls_back_combined_recipe_without_undo() {
        let path = temporary_csv("full,existing\nA-B,x\n");
        let (frame, _) = load_csv(&path).unwrap();
        let original = frame.clone();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let recipe = TransformRecipe {
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("full".into()),
                find: "A".into(),
                replace: "Z".into(),
            }),
            split_column: Some(SplitColumnRecipe {
                source: "full".into(),
                delimiter: "-".into(),
                names: vec!["existing".into(), "new".into()],
                drop_source: false,
            }),
            ..Default::default()
        };
        assert!(apply_recipe_to_dataset(&mut dataset, &recipe).is_err());
        assert!(dataset.frame.equals_missing(&original));
        assert!(!dataset.history.state().can_undo);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_and_merge_observe_casts_but_reject_non_text_physical_columns() {
        let frame = DataFrame::new(
            1,
            vec![
                Series::new("number".into(), [12_i64]).into_column(),
                Series::new("text".into(), ["3-4"]).into_column(),
            ],
        )
        .unwrap();
        let cast_to_text = TransformRecipe {
            casts: vec![RecipeCast {
                column: "number".into(),
                target: RecipeCastTarget::String,
            }],
            split_column: Some(SplitColumnRecipe {
                source: "number".into(),
                delimiter: "2".into(),
                names: vec!["one".into(), "two".into()],
                drop_source: false,
            }),
            ..Default::default()
        };
        assert_eq!(apply_recipe_to_frame(&frame, &cast_to_text).unwrap().9, 2);

        let cast_away = TransformRecipe {
            casts: vec![RecipeCast {
                column: "text".into(),
                target: RecipeCastTarget::Integer,
            }],
            split_column: Some(SplitColumnRecipe {
                source: "text".into(),
                delimiter: "-".into(),
                names: vec!["one".into(), "two".into()],
                drop_source: false,
            }),
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &cast_away).is_err());
    }

    #[test]
    fn split_and_merge_commit_as_one_undo_revision_with_metadata() {
        let path = temporary_csv("full,other\nA-B,C\n");
        let (frame, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                split_column: Some(SplitColumnRecipe {
                    source: "full".into(),
                    delimiter: "-".into(),
                    names: vec!["left".into(), "right".into()],
                    drop_source: true,
                }),
                merge_columns: Some(MergeColumnsRecipe {
                    sources: vec!["left".into(), "other".into()],
                    name: "joined".into(),
                    separator: ":".into(),
                    drop_sources: false,
                }),
                ..Default::default()
            },
        );
        // References produced by split are intentionally outside the contract.
        assert!(result.is_err());
        assert!(!dataset.history.state().can_undo);

        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                split_column: Some(SplitColumnRecipe {
                    source: "full".into(),
                    delimiter: "-".into(),
                    names: vec!["left".into(), "right".into()],
                    drop_source: true,
                }),
                merge_columns: None,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (
                result.split_column_count,
                result.dropped_source_column_count
            ),
            (2, 1)
        );
        assert!(dataset.history.state().can_undo);
        undo_dataset(&mut dataset).unwrap();
        assert_eq!(
            dataset
                .frame
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["full", "other"]
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn iqr_cap_uses_linear_quantiles_preserves_nulls_and_strict_boundaries() {
        let frame = DataFrame::new(
            6,
            vec![Series::new(
                "value".into(),
                [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
            )
            .into_column()],
        )
        .unwrap();
        let recipe = TransformRecipe {
            outlier_treatments: vec![OutlierTreatment {
                column: "value".into(),
                action: OutlierAction::Cap,
            }],
            ..Default::default()
        };
        let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!((outcome.12, outcome.13, outcome.14), (1, 0, 1));
        assert_eq!(
            outcome.0.column("value").unwrap().dtype(),
            &polars::prelude::DataType::Float64
        );
        let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
        assert_eq!(rows[4][0].as_deref(), Some("7.0"));
        assert_eq!(rows[5][0], None);

        let boundary = DataFrame::new(
            5,
            vec![Series::new("value".into(), [1_i64, 2, 3, 4, 7]).into_column()],
        )
        .unwrap();
        let no_op = apply_recipe_to_frame(&boundary, &recipe).unwrap();
        assert_eq!(no_op.12, 0);
        assert_eq!(
            no_op.0.column("value").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        let zero_iqr = DataFrame::new(
            4,
            vec![Series::new("value".into(), [5_i64; 4]).into_column()],
        )
        .unwrap();
        assert_eq!(apply_recipe_to_frame(&zero_iqr, &recipe).unwrap().12, 0);

        let imputed = apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                outlier_treatments: vec![OutlierTreatment {
                    column: "value".into(),
                    action: OutlierAction::Impute,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((imputed.12, imputed.13, imputed.14), (1, 0, 1));
        assert_eq!(
            imputed.0.column("value").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            imputed.0.column("value").unwrap().i64().unwrap().get(4),
            Some(3)
        );
        assert_eq!(
            imputed.0.column("value").unwrap().i64().unwrap().get(5),
            None
        );
    }

    #[test]
    fn direct_outlier_modes_report_affected_rows_and_preserve_nulls() {
        let frame = DataFrame::new(
            6,
            vec![Series::new(
                "amount".into(),
                [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
            )
            .into_column()],
        )
        .unwrap();

        let (capped, cap_rows, cap_cells, cap_columns) =
            apply_dataprep_outlier_mode(&frame, DataprepOutlierMode::Cap).unwrap();
        assert_eq!((cap_rows, cap_cells, cap_columns.len()), (1, 1, 1));
        assert_eq!(capped.column("amount").unwrap().dtype(), &DataType::Float64);
        assert_eq!(
            capped.column("amount").unwrap().f64().unwrap().get(4),
            Some(7.0)
        );
        assert_eq!(capped.column("amount").unwrap().f64().unwrap().get(5), None);

        let (dropped, drop_rows, drop_cells, drop_columns) =
            apply_dataprep_outlier_mode(&frame, DataprepOutlierMode::Drop).unwrap();
        assert_eq!((drop_rows, drop_cells, drop_columns.len()), (1, 1, 1));
        assert_eq!(dropped.height(), 5);
    }

    #[test]
    fn direct_outlier_imputation_preserves_numeric_types_and_audit_column() {
        let frame = DataFrame::new(
            6,
            vec![
                Series::new(
                    "amount".into(),
                    [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
                )
                .into_column(),
                Series::new(
                    "ratio".into(),
                    [
                        Some(1.0),
                        Some(2.0),
                        Some(3.0),
                        Some(4.0),
                        Some(100.0),
                        None,
                    ],
                )
                .into_column(),
                Series::new(
                    "_cambios".into(),
                    [Some(""), Some(""), Some(""), Some(""), Some("manual"), None],
                )
                .into_column(),
            ],
        )
        .unwrap();

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            impute_outlier_values_in_frame(&frame).unwrap();
        assert_eq!(affected_rows, 1);
        assert_eq!(changed_cells, 2);
        assert_eq!(changed_columns.len(), 2);
        assert_eq!(
            cleaned.column("amount").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            cleaned.column("ratio").unwrap().dtype(),
            &polars::prelude::DataType::Float64
        );
        assert_eq!(
            cleaned.column("amount").unwrap().i64().unwrap().get(4),
            Some(3)
        );
        assert_eq!(
            cleaned.column("ratio").unwrap().f64().unwrap().get(4),
            Some(3.0)
        );
        assert_eq!(
            cleaned.column("_cambios").unwrap().str().unwrap().get(4),
            Some("manual")
        );
        assert_eq!(
            cleaned.column("amount").unwrap().i64().unwrap().get(5),
            None
        );
    }

    #[test]
    fn categorical_imputation_uses_desconocido_without_touching_row_audit() {
        let frame = DataFrame::new(
            5,
            vec![
                Series::new(
                    "status".into(),
                    [Some("active"), None, Some("inactive"), None, None],
                )
                .into_column(),
                Series::new(
                    "_cambios".into(),
                    [Some("manual"), None, Some(""), None, None],
                )
                .into_column(),
            ],
        )
        .unwrap();

        let (cleaned, affected_rows, changed_cells, changed_columns) =
            impute_categorical_values_in_frame(&frame).unwrap();
        assert_eq!(affected_rows, 3);
        assert_eq!(changed_cells, 3);
        assert_eq!(changed_columns.len(), 1);
        assert_eq!(changed_columns[0].name, "status");
        assert_eq!(changed_columns[0].changed_cell_count, 3);
        assert_eq!(
            cleaned.column("status").unwrap().str().unwrap().get(1),
            Some("Desconocido")
        );
        assert_eq!(
            cleaned.column("_cambios").unwrap().str().unwrap().get(1),
            None
        );
    }

    #[test]
    fn iqr_drop_treatments_are_order_independent_and_share_one_baseline() {
        let frame = DataFrame::new(
            6,
            vec![
                Series::new("a".into(), [1.0, 2.0, 3.0, 4.0, 100.0, 3.0]).into_column(),
                Series::new("b".into(), [1.0, 2.0, 3.0, 4.0, 3.0, 100.0]).into_column(),
            ],
        )
        .unwrap();
        let treatments = vec![
            OutlierTreatment {
                column: "a".into(),
                action: OutlierAction::Drop,
            },
            OutlierTreatment {
                column: "b".into(),
                action: OutlierAction::Drop,
            },
        ];
        let first = apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                outlier_treatments: treatments.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let second = apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                outlier_treatments: treatments.into_iter().rev().collect(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((first.13, first.14), (2, 2));
        assert!(first.0.equals_missing(&second.0));

        let mixed = apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                outlier_treatments: vec![
                    OutlierTreatment {
                        column: "a".into(),
                        action: OutlierAction::Cap,
                    },
                    OutlierTreatment {
                        column: "b".into(),
                        action: OutlierAction::Drop,
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((mixed.12, mixed.13), (1, 1));
    }

    #[test]
    fn iqr_validates_minimum_duplicates_limits_nonfinite_and_precision() {
        let small = DataFrame::new(
            3,
            vec![Series::new("x".into(), [1_i64, 2, 100]).into_column()],
        )
        .unwrap();
        let treatment = OutlierTreatment {
            column: "x".into(),
            action: OutlierAction::Cap,
        };
        assert!(apply_recipe_to_frame(
            &small,
            &TransformRecipe {
                outlier_treatments: vec![treatment.clone()],
                ..Default::default()
            }
        )
        .is_err());
        assert!(apply_recipe_to_frame(
            &small,
            &TransformRecipe {
                outlier_treatments: vec![treatment.clone(), treatment.clone()],
                ..Default::default()
            }
        )
        .is_err());
        assert!(apply_recipe_to_frame(
            &small,
            &TransformRecipe {
                outlier_treatments: vec![treatment.clone(); 17],
                ..Default::default()
            }
        )
        .is_err());

        let nonfinite = DataFrame::new(
            4,
            vec![Series::new("x".into(), [1.0, 2.0, 3.0, f64::INFINITY]).into_column()],
        )
        .unwrap();
        assert!(apply_recipe_to_frame(
            &nonfinite,
            &TransformRecipe {
                outlier_treatments: vec![treatment.clone()],
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .contains("infinito"));
        let precision = DataFrame::new(
            4,
            vec![Series::new("x".into(), [1_i64, 2, 3, 9_007_199_254_740_993]).into_column()],
        )
        .unwrap();
        assert!(apply_recipe_to_frame(
            &precision,
            &TransformRecipe {
                outlier_treatments: vec![treatment],
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .contains("precisión"));
        let overflow = DataFrame::new(
            4,
            vec![Series::new("x".into(), [-1e308, -5e307, 5e307, 1e308]).into_column()],
        )
        .unwrap();
        assert!(apply_recipe_to_frame(
            &overflow,
            &TransformRecipe {
                outlier_treatments: vec![OutlierTreatment {
                    column: "x".into(),
                    action: OutlierAction::Drop
                }],
                ..Default::default()
            }
        )
        .err()
        .unwrap()
        .contains("rango numérico"));
    }

    #[test]
    fn iqr_remaps_rename_observes_cast_and_keep_and_commits_one_undo() {
        let path = temporary_csv("value,other\n1,a\n2,b\n3,c\n4,d\n100,e\n");
        let (frame, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                renames: vec![RecipeRename {
                    from: "value".into(),
                    to: "amount".into(),
                }],
                casts: vec![RecipeCast {
                    column: "value".into(),
                    target: RecipeCastTarget::Integer,
                }],
                keep_columns: Some(vec!["value".into()]),
                outlier_treatments: vec![OutlierTreatment {
                    column: "value".into(),
                    action: OutlierAction::Cap,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (
                result.adjusted_outlier_cell_count,
                result.outlier_column_count
            ),
            (1, 1)
        );
        assert!(dataset.history.state().can_undo);
        undo_dataset(&mut dataset).unwrap();
        assert_eq!(dataset.frame.width(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn group_summary_is_stable_supports_null_keys_and_count_semantics() {
        let frame = DataFrame::new(
            5,
            vec![
                Series::new(
                    "group".into(),
                    [Some("B"), None, Some("B"), None, Some("A")],
                )
                .into_column(),
                Series::new("value".into(), [Some(2_i64), None, Some(4), Some(8), None])
                    .into_column(),
                Series::new(
                    "label".into(),
                    [Some("z"), Some("x"), Some("a"), Some("x"), None],
                )
                .into_column(),
            ],
        )
        .unwrap();
        let summary = GroupSummaryRecipe {
            group_by: vec!["group".into()],
            aggregations: vec![
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Sum,
                },
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Mean,
                },
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Count,
                },
                SummaryAggregation {
                    column: "label".into(),
                    operation: SummaryOperation::CountUnique,
                },
                SummaryAggregation {
                    column: "label".into(),
                    operation: SummaryOperation::Min,
                },
            ],
        };
        let (result, groups, aggregations, collapsed) =
            apply_group_summary(frame, &summary, &HashMap::new()).unwrap();
        assert_eq!((groups, aggregations, collapsed), (3, 5, 2));
        assert_eq!(
            result
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "group",
                "value_sum",
                "value_mean",
                "value_count",
                "label_count_unique",
                "label_min"
            ]
        );
        let rows = dataset_page(&result, 0, 10).unwrap().rows;
        assert_eq!(rows[0][0].as_deref(), Some("B"));
        assert_eq!(rows[1][0], None);
        assert_eq!(rows[0][1].as_deref(), Some("6"));
        assert_eq!(rows[1][1].as_deref(), Some("8"));
        assert_eq!(rows[2][1].as_deref(), Some("0"));
        assert_eq!(rows[1][3].as_deref(), Some("2"));
        assert_eq!(rows[1][4].as_deref(), Some("1"));
        assert_eq!(rows[2][5], None);
    }

    #[test]
    fn group_summary_min_max_preserve_types_and_handle_negative_and_all_null_groups() {
        let dates = Series::new("date".into(), [Some(0_i32), Some(1), None, None])
            .cast(&polars::prelude::DataType::Date)
            .unwrap()
            .into_column();
        let datetimes = Series::new("time".into(), [Some(-1_i64), Some(1), None, None])
            .cast(&polars::prelude::DataType::Datetime(
                TimeUnit::Milliseconds,
                None,
            ))
            .unwrap()
            .into_column();
        let frame = DataFrame::new(
            4,
            vec![
                Series::new("g".into(), ["x", "x", "n", "n"]).into_column(),
                Series::new("number".into(), [Some(-2_i64), Some(-10), None, None]).into_column(),
                dates,
                datetimes,
            ],
        )
        .unwrap();
        let summary = GroupSummaryRecipe {
            group_by: vec!["g".into()],
            aggregations: vec![
                SummaryAggregation {
                    column: "number".into(),
                    operation: SummaryOperation::Min,
                },
                SummaryAggregation {
                    column: "number".into(),
                    operation: SummaryOperation::Max,
                },
                SummaryAggregation {
                    column: "date".into(),
                    operation: SummaryOperation::Min,
                },
                SummaryAggregation {
                    column: "time".into(),
                    operation: SummaryOperation::Max,
                },
            ],
        };
        let result = apply_group_summary(frame, &summary, &HashMap::new())
            .unwrap()
            .0;
        assert_eq!(
            result.column("number_min").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            result.column("date_min").unwrap().dtype(),
            &polars::prelude::DataType::Date
        );
        assert!(matches!(
            result.column("time_max").unwrap().dtype(),
            polars::prelude::DataType::Datetime(_, _)
        ));
        let rows = dataset_page(&result, 0, 10).unwrap().rows;
        assert_eq!(rows[0][1].as_deref(), Some("-10"));
        assert_eq!(rows[0][2].as_deref(), Some("-2"));
        assert_eq!(rows[1][1], None);
        assert_eq!(rows[1][3], None);
    }

    #[test]
    fn group_summary_rejects_overflow_precision_nonfinite_duplicates_and_missing_dependencies() {
        let overflow = DataFrame::new(
            2,
            vec![
                Series::new("g".into(), ["x", "x"]).into_column(),
                Series::new("v".into(), [i64::MAX, 1]).into_column(),
            ],
        )
        .unwrap();
        let sum = GroupSummaryRecipe {
            group_by: vec!["g".into()],
            aggregations: vec![SummaryAggregation {
                column: "v".into(),
                operation: SummaryOperation::Sum,
            }],
        };
        assert!(apply_group_summary(overflow, &sum, &HashMap::new())
            .err()
            .unwrap()
            .contains("desbordó"));
        let zeros = DataFrame::new(
            3,
            vec![
                Series::new("g".into(), ["x", "x", "x"]).into_column(),
                Series::new("v".into(), [-0.0, 0.0, f64::NAN]).into_column(),
            ],
        )
        .unwrap();
        let unique = GroupSummaryRecipe {
            group_by: vec!["g".into()],
            aggregations: vec![SummaryAggregation {
                column: "v".into(),
                operation: SummaryOperation::CountUnique,
            }],
        };
        assert!(apply_group_summary(zeros, &unique, &HashMap::new()).is_err());
        let signed_zero = DataFrame::new(
            3,
            vec![
                Series::new("g".into(), ["x", "x", "x"]).into_column(),
                Series::new("v".into(), [Some(-0.0), Some(0.0), None]).into_column(),
            ],
        )
        .unwrap();
        let result = apply_group_summary(signed_zero, &unique, &HashMap::new())
            .unwrap()
            .0;
        assert_eq!(
            dataset_page(&result, 0, 1).unwrap().rows[0][1].as_deref(),
            Some("1")
        );
        let precision = DataFrame::new(
            2,
            vec![
                Series::new("g".into(), ["x", "x"]).into_column(),
                Series::new("v".into(), [9_007_199_254_740_993_i64, 1]).into_column(),
            ],
        )
        .unwrap();
        let mean = GroupSummaryRecipe {
            group_by: vec!["g".into()],
            aggregations: vec![SummaryAggregation {
                column: "v".into(),
                operation: SummaryOperation::Mean,
            }],
        };
        assert!(apply_group_summary(precision, &mean, &HashMap::new())
            .err()
            .unwrap()
            .contains("precisión"));
    }

    #[test]
    fn group_summary_runs_after_outliers_and_commits_one_undo_revision() {
        let path = temporary_csv("g,v\na,1\na,2\na,3\na,4\na,100\n");
        let (frame, _) = load_csv(&path).unwrap();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let result = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                casts: vec![RecipeCast {
                    column: "v".into(),
                    target: RecipeCastTarget::Integer,
                }],
                outlier_treatments: vec![OutlierTreatment {
                    column: "v".into(),
                    action: OutlierAction::Drop,
                }],
                group_summary: Some(GroupSummaryRecipe {
                    group_by: vec!["g".into()],
                    aggregations: vec![SummaryAggregation {
                        column: "v".into(),
                        operation: SummaryOperation::Sum,
                    }],
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (
                result.group_count,
                result.aggregated_column_count,
                result.collapsed_row_count
            ),
            (1, 1, 3)
        );
        assert_eq!(
            dataset_page(&dataset.frame, 0, 1).unwrap().rows[0][1].as_deref(),
            Some("10")
        );
        assert!(dataset.history.state().can_undo);
        undo_dataset(&mut dataset).unwrap();
        assert_eq!(dataset.frame.height(), 5);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn contacts_normalize_unicode_phone_and_address_and_count_changed_cells() {
        let frame = DataFrame::new(
            2,
            vec![
                Series::new("email".into(), [Some(" İ@EXAMPLE.COM "), None]).into_column(),
                Series::new("phone".into(), [" +1 (809) 555-01 ", "1+2"]).into_column(),
                Series::new("address".into(), ["  Calle\u{a0}Uno\u{2003}Norte ", "ok"])
                    .into_column(),
            ],
        )
        .unwrap();
        let mut candidate = frame;
        let (cells, columns) = apply_contact_normalizations(
            &mut candidate,
            &[
                ContactNormalization {
                    column: "email".into(),
                    kind: ContactKind::Email,
                },
                ContactNormalization {
                    column: "phone".into(),
                    kind: ContactKind::Phone,
                },
                ContactNormalization {
                    column: "address".into(),
                    kind: ContactKind::Address,
                },
            ],
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!((cells, columns), (4, 3));
        let rows = dataset_page(&candidate, 0, 10).unwrap().rows;
        assert_eq!(rows[0][0].as_deref(), Some("i\u{307}@example.com"));
        assert_eq!(rows[0][1].as_deref(), Some("+180955501"));
        assert_eq!(rows[1][1].as_deref(), Some("12"));
        assert_eq!(rows[0][2].as_deref(), Some("Calle Uno Norte"));
        assert_eq!(rows[1][2].as_deref(), Some("ok"));
    }

    #[test]
    fn text_extractions_cover_unicode_tokens_runs_delimiters_and_all_null() {
        let mut frame = DataFrame::new(
            2,
            vec![
                Series::new("text".into(), [Some("  José Pérez 123🙂resto"), None]).into_column(),
                Series::new("arabic".into(), [Some("١٢ abc 45"), None]).into_column(),
            ],
        )
        .unwrap();
        let extractions = vec![
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::FirstToken,
                name: "first".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::LastToken,
                name: "last".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Letters,
                name: "letters".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "arabic".into(),
                kind: ExtractionKind::Digits,
                name: "digits".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Before,
                name: "before".into(),
                delimiter: Some("🙂".into()),
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::After,
                name: "after".into(),
                delimiter: Some("🙂".into()),
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Before,
                name: "missing".into(),
                delimiter: Some("NO".into()),
            },
        ];
        assert_eq!(
            apply_text_extractions(&mut frame, &extractions, &HashMap::new()).unwrap(),
            7
        );
        let rows = dataset_page(&frame, 0, 10).unwrap().rows;
        assert_eq!(rows[0][2].as_deref(), Some("José"));
        assert_eq!(rows[0][4].as_deref(), Some("José"));
        assert_eq!(rows[0][5].as_deref(), Some("45"));
        assert_eq!(rows[0][7].as_deref(), Some("resto"));
        assert_eq!(rows[0][8], None);
        assert_eq!(rows[1][8], None);
        assert_eq!(
            frame.column("missing").unwrap().dtype(),
            &polars::prelude::DataType::String
        );
    }

    #[test]
    fn contacts_and_extractions_validate_remap_keep_group_conflict_and_rollback() {
        let path = temporary_csv("contact,other\n A@B.COM ,x\n");
        let (frame, _) = load_csv(&path).unwrap();
        let original = frame.clone();
        let mut dataset = loaded_dataset(path.clone(), frame);
        let success = apply_recipe_to_dataset(
            &mut dataset,
            &TransformRecipe {
                renames: vec![RecipeRename {
                    from: "contact".into(),
                    to: "email".into(),
                }],
                keep_columns: Some(vec!["contact".into()]),
                contact_normalizations: vec![ContactNormalization {
                    column: "contact".into(),
                    kind: ContactKind::Email,
                }],
                text_extractions: vec![TextExtraction {
                    source: "contact".into(),
                    kind: ExtractionKind::Before,
                    name: "user".into(),
                    delimiter: Some("@".into()),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (
                success.normalized_contact_cell_count,
                success.normalized_contact_column_count,
                success.extracted_column_count
            ),
            (1, 1, 1)
        );
        assert!(dataset.history.state().can_undo);
        undo_dataset(&mut dataset).unwrap();
        assert!(dataset.frame.equals_missing(&original));

        let conflict = TransformRecipe {
            text_extractions: vec![TextExtraction {
                source: "contact".into(),
                kind: ExtractionKind::FirstToken,
                name: "new".into(),
                delimiter: None,
            }],
            group_summary: Some(GroupSummaryRecipe {
                group_by: vec!["contact".into()],
                aggregations: vec![SummaryAggregation {
                    column: "other".into(),
                    operation: SummaryOperation::Count,
                }],
            }),
            ..Default::default()
        };
        assert!(apply_recipe_to_dataset(&mut dataset, &conflict).is_err());
        assert!(!dataset.history.state().can_undo);
        fs::remove_file(path).unwrap();
    }

    fn quality_rule(column: &str, kind: QualityRuleKind) -> QualityRule {
        QualityRule {
            column: column.to_owned(),
            kind,
            max_invalid: Some(0),
            max_invalid_pct: None,
            min: None,
            max: None,
            values: None,
            reference_values: None,
            baseline: None,
            direction: None,
            expected: None,
            aggregate: None,
            tolerance_abs: None,
            tolerance_rel: None,
            threshold: None,
            pattern: None,
            dtype: None,
            columns: None,
            operator: None,
            min_date: None,
            max_date: None,
            when: None,
            then: None,
            allow_additional: None,
            required_order: None,
        }
    }

    #[test]
    fn quality_rules_apply_explicit_null_duplicate_empty_and_range_semantics() {
        let frame = df![
            "text" => &[Some("a"), Some("  "), None, Some("a"), Some("b")],
            "integer" => &[Some(1_i64), Some(2), None, Some(4), Some(5)],
            "decimal" => &[Some(1.0_f64), Some(f64::NAN), Some(f64::INFINITY), None, Some(5.0)]
        ]
        .unwrap();
        let mut non_empty = quality_rule("text", QualityRuleKind::NonEmpty);
        non_empty.max_invalid = Some(2);
        let mut unique = quality_rule("text", QualityRuleKind::Unique);
        unique.max_invalid = Some(2);
        let mut integer_range = quality_rule("integer", QualityRuleKind::NumericRange);
        integer_range.min = Some(1.0);
        integer_range.max = Some(4.0);
        integer_range.max_invalid = Some(2);
        let mut float_range = quality_rule("decimal", QualityRuleKind::NumericRange);
        float_range.min = Some(0.0);
        float_range.max_invalid = Some(3);

        let result = evaluate_quality_rules(
            &frame,
            &[
                quality_rule("text", QualityRuleKind::NotNull),
                non_empty,
                unique,
                integer_range,
                float_range,
            ],
        )
        .unwrap();

        assert_eq!(result.row_count, 5);
        assert_eq!(result.total_rules, 5);
        assert_eq!(result.rules[0].invalid_count, 1);
        assert!(!result.rules[0].passed);
        assert_eq!(result.rules[1].invalid_count, 2);
        assert_eq!(result.rules[2].invalid_count, 2);
        assert_eq!(result.rules[3].invalid_count, 2);
        assert_eq!(result.rules[4].invalid_count, 3);
        assert!(result.rules[1..].iter().all(|rule| rule.passed));
        assert_eq!(result.failed_rules, 1);
        assert!(!result.passed);
    }

    #[test]
    fn quality_rules_apply_the_first_advanced_v3_slice() {
        let frame = df![
            "status" => &[Some("ok"), Some("bad"), None, Some("ok")],
            "email" => &[Some("a@example.com"), Some("invalid"), Some("b@example.com"), Some("c@example.com")],
            "number" => &[1_i64, 2, 3, 4],
            "key_a" => &["a", "a", "a", "b"],
            "key_b" => &[1_i64, 1, 2, 1]
        ]
        .unwrap();

        let mut allowed_values = quality_rule("status", QualityRuleKind::AllowedValues);
        allowed_values.values = Some(vec!["ok".to_owned(), "pending".to_owned()]);
        allowed_values.max_invalid = Some(2);

        let mut regex = quality_rule("email", QualityRuleKind::Regex);
        regex.pattern = Some(r"^[^@]+@[^@]+$".to_owned());
        regex.max_invalid = Some(1);

        let mut dtype = quality_rule("number", QualityRuleKind::Dtype);
        dtype.dtype = Some("integer".to_owned());

        let mut unique_together = quality_rule("key_a", QualityRuleKind::UniqueTogether);
        unique_together.columns = Some(vec!["key_a".to_owned(), "key_b".to_owned()]);
        unique_together.max_invalid = Some(1);

        let mut row_count = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::RowCount);
        row_count.min = Some(4.0);
        row_count.max = Some(4.0);

        let result = evaluate_quality_rules(
            &frame,
            &[allowed_values, regex, dtype, unique_together, row_count],
        )
        .unwrap();

        assert!(result.passed);
        assert_eq!(
            result
                .rules
                .iter()
                .map(|rule| rule.invalid_count)
                .collect::<Vec<_>>(),
            vec![2, 1, 0, 1, 0]
        );
        assert_eq!(result.rules[4].checked_count, 1);
        assert_eq!(result.rules[4].invalid_pct, 0.0);
    }

    #[test]
    fn advanced_quality_rules_reject_invalid_parameters() {
        let frame = df!["text" => &["ok"], "number" => &[1_i64]].unwrap();

        let mut bad_regex = quality_rule("text", QualityRuleKind::Regex);
        bad_regex.pattern = Some("[".to_owned());
        assert!(evaluate_quality_rules(&frame, &[bad_regex]).is_err());

        let mut bad_values = quality_rule("number", QualityRuleKind::AllowedValues);
        bad_values.values = Some(vec!["1".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[bad_values]).is_err());

        let mut missing_key = quality_rule("text", QualityRuleKind::UniqueTogether);
        missing_key.columns = Some(vec!["text".to_owned(), "missing".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[missing_key]).is_err());

        let missing_row_bounds = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::RowCount);
        assert!(evaluate_quality_rules(&frame, &[missing_row_bounds]).is_err());

        let mut bad_dtype = quality_rule("text", QualityRuleKind::Dtype);
        bad_dtype.dtype = Some("money".to_owned());
        assert!(evaluate_quality_rules(&frame, &[bad_dtype]).is_err());

        let mut misplaced_operator = quality_rule("text", QualityRuleKind::NotNull);
        misplaced_operator.operator = Some(QualityComparison::Eq);
        assert!(evaluate_quality_rules(&frame, &[misplaced_operator]).is_err());
    }

    #[test]
    fn quality_rules_apply_simple_and_composite_referential_integrity() {
        let frame = df![
            "country" => &[Some("DO"), Some("US"), Some("MX"), None],
            "code" => &[Some(1_i64), Some(2), Some(3), Some(4)]
        ]
        .unwrap();
        let mut simple = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
        simple.columns = Some(vec!["country".to_owned()]);
        simple.reference_values = Some(vec!["DO".to_owned(), "US".to_owned()]);
        simple.max_invalid = Some(2);

        let mut composite = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
        composite.columns = Some(vec!["country".to_owned(), "code".to_owned()]);
        composite.reference_values = Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()]);
        composite.max_invalid = Some(2);

        let result = evaluate_quality_rules(&frame, &[simple, composite]).unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].invalid_count, 2);
        assert_eq!(result.rules[1].invalid_count, 2);
        assert_eq!(
            result.rules[1].reference_values,
            Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()])
        );
    }

    #[test]
    fn referential_integrity_rejects_missing_duplicate_or_unsupported_keys() {
        let date = Series::new("date".into(), [Some(0_i32)])
            .cast(&DataType::Date)
            .unwrap()
            .into_column();
        let frame = DataFrame::new(
            1,
            vec![Series::new("country".into(), ["DO"]).into_column(), date],
        )
        .unwrap();
        let mut missing_values = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
        missing_values.columns = Some(vec!["country".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[missing_values]).is_err());

        let mut duplicate_columns = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
        duplicate_columns.columns = Some(vec!["country".to_owned(), "country".to_owned()]);
        duplicate_columns.reference_values = Some(vec![r#"["DO","DO"]"#.to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[duplicate_columns]).is_err());

        let mut unsupported = quality_rule("date", QualityRuleKind::ReferentialIntegrity);
        unsupported.columns = Some(vec!["date".to_owned()]);
        unsupported.reference_values = Some(vec!["2024-01-01".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[unsupported]).is_err());
    }

    #[test]
    fn quality_rules_apply_non_strict_increasing_and_decreasing_sequences() {
        let frame = df![
            "value" => &[Some(1_i64), Some(2), Some(2), Some(1), Some(3), None, Some(2)]
        ]
        .unwrap();
        let mut increasing = quality_rule("value", QualityRuleKind::Monotonic);
        increasing.direction = Some(QualityMonotonicDirection::Increasing);
        increasing.max_invalid = Some(1);

        let mut decreasing = quality_rule("value", QualityRuleKind::Monotonic);
        decreasing.direction = Some(QualityMonotonicDirection::Decreasing);
        decreasing.max_invalid = Some(2);

        let result = evaluate_quality_rules(&frame, &[increasing, decreasing]).unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].checked_count, 7);
        assert_eq!(result.rules[0].invalid_count, 1);
        assert_eq!(result.rules[1].invalid_count, 2);
        assert_eq!(
            result.rules[0].direction,
            Some(QualityMonotonicDirection::Increasing)
        );
    }

    #[test]
    fn monotonic_rejects_extra_parameters_and_migrates_direction_aliases() {
        let frame = df!["value" => &[1_i64, 2]].unwrap();
        let mut extra = quality_rule("value", QualityRuleKind::Monotonic);
        extra.direction = Some(QualityMonotonicDirection::Increasing);
        extra.values = Some(vec!["1".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[extra]).is_err());

        let result = migrate_quality_rules_document(serde_json::json!([
            {"kind": "monotonic", "column": "value", "direction": "desc"},
            {"kind": "monotonic", "column": "value", "order": "sideways"}
        ]))
        .unwrap();
        assert_eq!(result.converted_rules.len(), 1);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(
            result.converted_rules[0].direction,
            Some(QualityMonotonicDirection::Decreasing)
        );
    }

    #[test]
    fn quality_rules_apply_aggregate_checks_and_reconciliation() {
        let frame = df![
            "amount" => &[Some(1_i64), Some(2), Some(3), None],
            "ledger" => &[Some(0_i64), Some(3), Some(3), None]
        ]
        .unwrap();
        let mut sum = quality_rule("amount", QualityRuleKind::AggregateCheck);
        sum.expected = Some(6.0);
        sum.aggregate = Some(QualityAggregate::Sum);

        let mut count = quality_rule("amount", QualityRuleKind::AggregateCheck);
        count.expected = Some(3.0);
        count.aggregate = Some(QualityAggregate::Count);

        let mut minimum = quality_rule("amount", QualityRuleKind::AggregateCheck);
        minimum.expected = Some(1.0);
        minimum.aggregate = Some(QualityAggregate::Min);

        let mut maximum = quality_rule("amount", QualityRuleKind::AggregateCheck);
        maximum.expected = Some(3.0);
        maximum.aggregate = Some(QualityAggregate::Max);

        let mut reconciliation = quality_rule("amount", QualityRuleKind::AggregateReconciliation);
        reconciliation.columns = Some(vec!["amount".to_owned(), "ledger".to_owned()]);
        reconciliation.tolerance_abs = Some(0.01);

        let result =
            evaluate_quality_rules(&frame, &[sum, count, minimum, maximum, reconciliation])
                .unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].checked_count, 4);
        assert_eq!(result.rules[1].checked_count, 4);
        assert_eq!(result.rules[4].invalid_count, 0);
        assert_eq!(result.rules[4].aggregate, None);
    }

    #[test]
    fn aggregate_rules_reject_invalid_payloads_and_migrate_legacy_fields() {
        let frame = df!["amount" => &[1_i64, 2], "ledger" => &[1_i64, 2]].unwrap();
        let mut extra = quality_rule("amount", QualityRuleKind::AggregateCheck);
        extra.expected = Some(3.0);
        extra.values = Some(vec!["3".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[extra]).is_err());

        let result = migrate_quality_rules_document(serde_json::json!([
            {
                "kind": "aggregate_check",
                "column": "amount",
                "aggregate": "total",
                "expected": 3,
                "toleranceAbs": 0.5
            },
            {
                "kind": "aggregate_reconciliation",
                "column": "amount",
                "columns": ["amount", "ledger"],
                "toleranceRel": 0.01
            },
            {
                "kind": "aggregate_check",
                "column": "amount",
                "aggregate": "sideways",
                "expected": 3
            }
        ]))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 2);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(
            result.converted_rules[0].aggregate,
            Some(QualityAggregate::Sum)
        );
        assert_eq!(result.converted_rules[0].tolerance_abs, Some(0.5));
        assert_eq!(
            result.converted_rules[1].columns,
            Some(vec!["amount".to_owned(), "ledger".to_owned()])
        );
        assert_eq!(result.converted_rules[1].tolerance_rel, Some(0.01));
    }

    #[test]
    fn quality_rules_apply_distribution_drift_to_numeric_means() {
        let frame = df!["amount" => &[Some(1_i64), Some(2), Some(3), None]].unwrap();
        let mut passing = quality_rule("amount", QualityRuleKind::DistributionDrift);
        passing.baseline = Some(vec!["1".to_owned(), "3".to_owned()]);
        passing.threshold = Some(0.0);

        let mut failing = quality_rule("amount", QualityRuleKind::DistributionDrift);
        failing.baseline = Some(vec!["4".to_owned(), "6".to_owned()]);
        failing.threshold = Some(2.0);

        let result = evaluate_quality_rules(&frame, &[passing, failing]).unwrap();

        assert_eq!(result.rules[0].checked_count, 4);
        assert_eq!(result.rules[0].invalid_count, 0);
        assert_eq!(result.rules[1].checked_count, 4);
        assert_eq!(result.rules[1].invalid_count, 1);
        assert!(!result.rules[1].passed);
        assert_eq!(result.failed_rules, 1);
    }

    #[test]
    fn distribution_drift_rejects_invalid_baselines_and_migrates_aliases() {
        let frame = df!["amount" => &[1_i64, 2]].unwrap();
        let mut invalid_baseline = quality_rule("amount", QualityRuleKind::DistributionDrift);
        invalid_baseline.baseline = Some(vec!["not-a-number".to_owned()]);
        assert!(evaluate_quality_rules(&frame, &[invalid_baseline]).is_err());

        let mut invalid_threshold = quality_rule("amount", QualityRuleKind::DistributionDrift);
        invalid_threshold.baseline = Some(vec!["1".to_owned()]);
        invalid_threshold.threshold = Some(-1.0);
        assert!(evaluate_quality_rules(&frame, &[invalid_threshold]).is_err());

        let result = migrate_quality_rules_document(serde_json::json!([
            {
                "kind": "drift",
                "column": "amount",
                "baseline": [1, 3],
                "threshold": 0.5
            },
            {
                "kind": "distribution_drift",
                "column": "amount",
                "referenceValues": [2, 2],
                "toleranceAbs": 0.1
            },
            {
                "kind": "distribution_drift",
                "column": "amount",
                "baseline": []
            }
        ]))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 2);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(
            result.converted_rules[0].kind,
            QualityRuleKind::DistributionDrift
        );
        assert_eq!(
            result.converted_rules[0].baseline,
            Some(vec!["1".to_owned(), "3".to_owned()])
        );
        assert_eq!(result.converted_rules[1].tolerance_abs, Some(0.1));
    }

    #[test]
    fn quality_rules_document_roundtrips_canonical_and_legacy_v1() {
        let directory = tempfile::tempdir().unwrap();
        let canonical_path = directory.path().join("quality.json");
        let legacy_path = directory.path().join("legacy.json");
        let first_rule = quality_rule("amount", QualityRuleKind::NotNull);
        let first_document = build_quality_rules_document(vec![first_rule.clone()]).unwrap();

        save_quality_rules_atomic(&first_document, &canonical_path).unwrap();
        let loaded = load_quality_rules_for_automation(&canonical_path).unwrap();
        assert_eq!(loaded, vec![first_rule]);
        let imported = load_quality_migration_file(&canonical_path).unwrap();
        assert_eq!(imported.source_format, "columnia");
        assert_eq!(imported.source_version.as_deref(), Some("1"));
        assert_eq!(imported.omitted_rules, 0);
        assert_eq!(imported.report.total_items, 1);
        assert_eq!(imported.report.converted_items, 1);
        assert_eq!(imported.report.omitted_items, 0);
        assert_eq!(imported.report.warning_count, 0);
        assert_eq!(imported.report.manual_actions.len(), 1);
        assert!(imported
            .report
            .artifact_sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64
                && hash.chars().all(|character| character.is_ascii_hexdigit())));

        let mut replacement = quality_rule("amount", QualityRuleKind::NumericRange);
        replacement.min = Some(0.0);
        let replacement_document = build_quality_rules_document(vec![replacement.clone()]).unwrap();
        save_quality_rules_atomic(&replacement_document, &canonical_path).unwrap();
        assert_eq!(
            load_quality_rules_for_automation(&canonical_path).unwrap(),
            vec![replacement]
        );

        fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "rules": [quality_rule("amount", QualityRuleKind::NotNull)]
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            load_quality_rules_for_automation(&legacy_path)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn quality_rules_document_rejects_future_or_ambiguous_contracts() {
        let canonical_future = serde_json::json!({
            "format": QUALITY_RULES_DOCUMENT_FORMAT,
            "version": 2,
            "rules": []
        });
        assert!(migrate_quality_rules_document(canonical_future)
            .unwrap_err()
            .contains("no es compatible"));

        let wrong_format = serde_json::json!({
            "format": "another-quality-format",
            "version": 1,
            "rules": []
        });
        assert!(migrate_quality_rules_document(wrong_format)
            .unwrap_err()
            .contains("formato"));

        let unknown_field = serde_json::json!({
            "format": QUALITY_RULES_DOCUMENT_FORMAT,
            "version": 1,
            "rules": [],
            "extra": true
        });
        assert!(migrate_quality_rules_document(unknown_field).is_err());

        let dataprep_future = serde_json::json!({
            "version": 4,
            "rules": []
        });
        assert!(migrate_quality_rules_document(dataprep_future)
            .unwrap_err()
            .contains("DataPrep 4"));

        let directory = tempfile::tempdir().unwrap();
        let legacy_future = directory.path().join("legacy-future.json");
        fs::write(&legacy_future, br#"{"version":2,"rules":[]}"#).unwrap();
        assert!(load_quality_rules_for_automation(&legacy_future).is_err());
    }

    #[test]
    fn migrates_supported_quality_rules_and_omits_unsupported_semantics() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {"kind": "allowed_values", "column": "status", "values": ["ok"], "max_invalid": 1},
                {"type": "regex", "column": "email", "pattern": "^.+@.+$", "maxInvalidPct": 5},
                {"kind": "column_compare", "column": "left", "other_column": "right", "operator": "lte"},
                {"kind": "not_null", "column": "id", "severity": "warning", "max_invalid": 0},
                {"kind": "unique_together", "column": "a", "columns": ["a", "b"]}
            ]
        }))
        .expect("el documento de migración debe ser válido");

        assert_eq!(result.source_version.as_deref(), Some("3"));
        assert_eq!(result.converted_rules.len(), 4);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(result.converted_rules[0].max_invalid, Some(1));
        assert_eq!(result.converted_rules[1].max_invalid_pct, Some(5.0));
        assert_eq!(
            result.converted_rules[2].kind,
            QualityRuleKind::ColumnCompare
        );
        assert_eq!(
            result.converted_rules[2].columns,
            Some(vec!["left".to_owned(), "right".to_owned()])
        );
        assert_eq!(
            result.converted_rules[2].operator,
            Some(QualityComparison::Lte)
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.severity == "omitted" && warning.source_kind == "not_null"));
    }

    #[test]
    fn migration_preserves_range_alias_and_numeric_tolerance_strings() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {
                    "kind": "range",
                    "column": "age",
                    "min": "0",
                    "max": "120",
                    "maxInvalid": "2"
                },
                {
                    "kind": "aggregate_check",
                    "column": "amount",
                    "aggregate": "sum",
                    "expected": "100.5",
                    "toleranceAbs": "0.5",
                    "toleranceRel": "0.01",
                    "max_invalid": 0
                }
            ]
        }))
        .expect("los aliases numéricos de DataPrep deben conservarse");

        assert_eq!(result.converted_rules.len(), 2);
        assert_eq!(
            result.converted_rules[0].kind,
            QualityRuleKind::NumericRange
        );
        assert_eq!(result.converted_rules[0].min, Some(0.0));
        assert_eq!(result.converted_rules[0].max, Some(120.0));
        assert_eq!(result.converted_rules[0].max_invalid, Some(2));
        assert_eq!(result.converted_rules[1].expected, Some(100.5));
        assert_eq!(result.converted_rules[1].tolerance_abs, Some(0.5));
        assert_eq!(result.converted_rules[1].tolerance_rel, Some(0.01));
        assert_eq!(result.omitted_rules, 0);
    }

    #[test]
    fn migration_preserves_v3_aliases_and_scalar_allowed_values() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "quality_rules": [
                {
                    "type": "allowed_values",
                    "column": "status",
                    "values": ["ok", 1, true],
                    "max_invalid": 0
                },
                {
                    "kind": "numeric_range",
                    "column": "amount",
                    "min_value": "1",
                    "maxValue": "3",
                    "maxInvalidPct": "5"
                },
                {
                    "kind": "dtype",
                    "column": "amount",
                    "expectedType": "integer",
                    "max_invalid": 0
                },
                {
                    "kind": "column_compare",
                    "column": "left",
                    "otherColumn": "right",
                    "operator": "lte",
                    "max_invalid": 0
                }
            ]
        }))
        .expect("los aliases v3 representables deben migrarse");

        assert_eq!(result.converted_rules.len(), 4);
        assert_eq!(
            result.converted_rules[0].values,
            Some(vec!["ok".to_owned(), "1".to_owned(), "true".to_owned()])
        );
        assert_eq!(result.converted_rules[1].min, Some(1.0));
        assert_eq!(result.converted_rules[1].max, Some(3.0));
        assert_eq!(result.converted_rules[1].max_invalid_pct, Some(5.0));
        assert_eq!(result.converted_rules[2].dtype.as_deref(), Some("integer"));
        assert_eq!(
            result.converted_rules[3].columns,
            Some(vec!["left".to_owned(), "right".to_owned()])
        );
    }

    #[test]
    fn migration_maps_blocking_alias_and_omits_unsafe_policy_metadata() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {
                    "kind": "not_null",
                    "column": "id",
                    "severity": "error",
                    "onMissing": "fail",
                    "nullPolicy": "invalid",
                    "max_invalid": 0
                },
                {
                    "kind": "unique",
                    "column": "id",
                    "blocking": false,
                    "max_invalid": 0
                },
                {
                    "kind": "not_null",
                    "column": "id",
                    "nullable": true,
                    "max_invalid": 0
                },
                {
                    "kind": "not_null",
                    "column": "id",
                    "referenceRevision": "rev-12",
                    "max_invalid": 0
                },
                {
                    "kind": "not_null",
                    "column": "id",
                    "severity": "warning",
                    "blocking": true,
                    "max_invalid": 0
                }
            ]
        }))
        .expect("las políticas deben aislarse por regla");

        assert_eq!(result.converted_rules.len(), 1);
        assert_eq!(result.converted_rules[0].kind, QualityRuleKind::NotNull);
        assert_eq!(result.omitted_rules, 4);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("no bloqueante")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("nullable/allowNulls=true")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("referencia externa")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("contradictorias")));
    }

    #[test]
    fn migration_accepts_condition_alias_but_keeps_nested_policies_blocking() {
        let result = migrate_quality_rules_document(serde_json::json!([{
            "kind": "conditional",
            "column": "amount",
            "condition": {"column": "status", "op": "eq", "val": "active"},
            "then": {"kind": "numeric_range", "minValue": 0, "maxValue": 10},
            "max_invalid": 0
        }]))
        .expect("la condición compatible debe migrarse");

        assert_eq!(result.converted_rules.len(), 1);
        let rule = &result.converted_rules[0];
        assert_eq!(
            rule.when.as_ref().map(|condition| condition.operator),
            Some(Some(QualityComparison::Eq))
        );
        assert_eq!(rule.then.as_deref().and_then(|then| then.min), Some(0.0));
        assert_eq!(rule.then.as_deref().and_then(|then| then.max), Some(10.0));
    }

    #[test]
    fn migration_omits_negative_tolerance_instead_of_returning_invalid_rule() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {
                    "kind": "aggregate_check",
                    "column": "amount",
                    "expected": 100,
                    "toleranceAbs": -0.5,
                    "max_invalid": 0
                },
                {
                    "kind": "distribution_drift",
                    "column": "amount",
                    "baseline": [100],
                    "threshold": -1,
                    "max_invalid": 0
                }
            ]
        }))
        .expect("una tolerancia negativa no debe invalidar todo el documento");

        assert_eq!(result.converted_rules.len(), 0);
        assert_eq!(result.omitted_rules, 2);
        assert!(result
            .warnings
            .iter()
            .all(|warning| warning.severity == "omitted"));
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("toleranceAbs no puede ser negativo")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("threshold no puede ser negativo")));
    }

    #[test]
    fn migrates_simple_and_composite_referential_integrity_values() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "rules": [
                {
                    "kind": "referential_integrity",
                    "column": "country",
                    "reference_values": ["DO", "US", 3, true],
                    "max_invalid": 0
                },
                {
                    "kind": "referential",
                    "columns": ["country", "code"],
                    "reference": [["DO", 1], ["US", 2]],
                    "max_invalid": 0
                },
                {
                    "kind": "referential_integrity",
                    "column": "country",
                    "reference_values": [null],
                    "max_invalid": 0
                }
            ]
        }))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 2);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(
            result.converted_rules[0].reference_values,
            Some(vec![
                "DO".to_owned(),
                "US".to_owned(),
                "3".to_owned(),
                "true".to_owned()
            ])
        );
        assert_eq!(result.converted_rules[1].column, "country");
        assert_eq!(
            result.converted_rules[1].reference_values,
            Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()])
        );
    }

    #[test]
    fn quality_rules_compare_columns_with_nulls_and_tolerance() {
        let frame = df![
            "start" => &[Some(1_i64), Some(2), Some(3), None],
            "end" => &[Some(1_i64), Some(1), Some(4), Some(4)]
        ]
        .unwrap();
        let mut compare = quality_rule("start", QualityRuleKind::ColumnCompare);
        compare.columns = Some(vec!["start".to_owned(), "end".to_owned()]);
        compare.operator = Some(QualityComparison::Lte);
        compare.max_invalid = Some(2);

        let result = evaluate_quality_rules(&frame, &[compare]).unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].checked_count, 4);
        assert_eq!(result.rules[0].invalid_count, 2);
        assert_eq!(result.rules[0].operator, Some(QualityComparison::Lte));
    }

    #[test]
    fn quality_rules_apply_conditional_row_rules_and_skip_null_conditions() {
        let frame = df![
            "status" => &[Some("ok"), Some("skip"), Some("ok"), Some("ok")],
            "amount" => &[Some(10_i64), None, Some(20), None]
        ]
        .unwrap();
        let mut conditional = quality_rule("amount", QualityRuleKind::Conditional);
        conditional.when = Some(QualityCondition {
            column: "status".to_owned(),
            operator: Some(QualityComparison::Eq),
            value: Some("ok".to_owned()),
        });
        conditional.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));

        let result = evaluate_quality_rules(&frame, &[conditional.clone()]).unwrap();

        assert!(!result.passed);
        assert_eq!(result.rules[0].checked_count, 4);
        assert_eq!(result.rules[0].invalid_count, 1);
        assert_eq!(result.rules[0].when, conditional.when);
        assert_eq!(result.rules[0].then, conditional.then);

        let mut allowed = conditional;
        allowed.max_invalid = Some(1);
        let result = evaluate_quality_rules(&frame, &[allowed]).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn quality_rules_match_numeric_conditions_across_integer_widths() {
        let frame = df![
            "status" => &["active", "inactive", "active"],
            "code" => &[1_i32, 2, 1],
            "amount" => &[Some(10_i64), None, Some(20)]
        ]
        .unwrap();
        let mut allowed = quality_rule("status", QualityRuleKind::AllowedValues);
        allowed.values = Some(vec!["active".to_owned(), "inactive".to_owned()]);
        allowed.max_invalid = Some(0);

        let mut conditional = quality_rule("amount", QualityRuleKind::Conditional);
        conditional.when = Some(QualityCondition {
            column: "code".to_owned(),
            operator: Some(QualityComparison::Eq),
            value: Some("1".to_owned()),
        });
        conditional.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));
        conditional.max_invalid = Some(0);

        let result = evaluate_quality_rules(&frame, &[allowed, conditional]).unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].invalid_count, 0);
        assert_eq!(result.rules[1].invalid_count, 0);
        assert_eq!(result.rules[1].checked_count, 3);
        assert_eq!(result.failed_rules, 0);
    }

    #[test]
    fn conditional_rules_reject_missing_or_global_then_checks() {
        let frame = df!["status" => &["ok"], "amount" => &[1_i64]].unwrap();
        let mut missing_when = quality_rule("amount", QualityRuleKind::Conditional);
        missing_when.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));
        assert!(evaluate_quality_rules(&frame, &[missing_when]).is_err());

        let mut global_then = quality_rule("amount", QualityRuleKind::Conditional);
        global_then.when = Some(QualityCondition {
            column: "status".to_owned(),
            operator: Some(QualityComparison::Eq),
            value: Some("ok".to_owned()),
        });
        global_then.then = Some(Box::new(quality_rule("amount", QualityRuleKind::Unique)));
        assert!(evaluate_quality_rules(&frame, &[global_then]).is_err());
    }

    #[test]
    fn migrates_conditional_rules_with_a_safe_nested_check() {
        let result = migrate_quality_rules_document(serde_json::json!([{
            "kind": "conditional",
            "column": "status",
            "when": {"column": "status", "operator": "eq", "value": "active"},
            "then": {"kind": "allowed_values", "values": ["active"]},
            "max_invalid": 0
        }]))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 1);
        let rule = &result.converted_rules[0];
        assert_eq!(rule.kind, QualityRuleKind::Conditional);
        assert_eq!(
            rule.when.as_ref().map(|when| when.column.as_str()),
            Some("status")
        );
        assert_eq!(
            rule.then.as_deref().map(|then| then.kind),
            Some(QualityRuleKind::AllowedValues)
        );
        assert_eq!(
            rule.then.as_deref().and_then(|then| then.values.clone()),
            Some(vec!["active".to_owned()])
        );
    }

    #[test]
    fn migration_omits_nonrepresentable_nested_policy_and_malformed_policy() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {
                    "kind": "conditional",
                    "column": "amount",
                    "when": {"column": "status", "value": "active"},
                    "then": {
                        "kind": "not_null",
                        "severity": "warning"
                    },
                    "max_invalid": 0
                },
                {
                    "kind": "not_null",
                    "column": "status",
                    "severity": true,
                    "max_invalid": 0
                }
            ]
        }))
        .expect("las políticas no representables deben aislarse por regla");

        assert_eq!(result.converted_rules.len(), 0);
        assert_eq!(result.omitted_rules, 2);
        assert!(result
            .warnings
            .iter()
            .all(|warning| warning.severity == "omitted"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("La severidad de la subregla then")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("severity' debe ser texto")));
    }

    #[test]
    fn quality_rules_validate_schema_required_additional_and_order_columns() {
        let frame = df![
            "status" => &["ok", "pending"],
            "amount" => &[1_i64, 2],
            "extra" => &[true, false]
        ]
        .unwrap();
        let mut schema = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::SchemaContract);
        schema.columns = Some(vec!["status".to_owned(), "amount".to_owned()]);
        schema.allow_additional = Some(false);

        let result = evaluate_quality_rules(&frame, &[schema.clone()]).unwrap();
        assert!(!result.passed);
        assert_eq!(result.rules[0].checked_count, 1);
        assert_eq!(result.rules[0].invalid_count, 1);

        schema.allow_additional = Some(true);
        schema.required_order = Some(vec![
            "status".to_owned(),
            "amount".to_owned(),
            "extra".to_owned(),
        ]);
        let result = evaluate_quality_rules(&frame, &[schema.clone()]).unwrap();
        assert!(result.passed);

        schema.required_order = Some(vec!["amount".to_owned(), "status".to_owned()]);
        let result = evaluate_quality_rules(&frame, &[schema]).unwrap();
        assert!(!result.passed);
        assert_eq!(result.rules[0].invalid_count, 1);
    }

    #[test]
    fn migrates_schema_contract_aliases_and_defaults_additional_columns() {
        let result = migrate_quality_rules_document(serde_json::json!([
            {
                "kind": "schema",
                "requiredColumns": ["status", "amount"],
                "allowAdditional": false,
                "required_order": ["status", "amount"],
                "max_invalid": 0
            },
            {
                "kind": "schema_contract",
                "columns": ["status", "status"],
                "max_invalid": 0
            }
        ]))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 1);
        assert_eq!(result.omitted_rules, 1);
        let rule = &result.converted_rules[0];
        assert_eq!(rule.kind, QualityRuleKind::SchemaContract);
        assert_eq!(rule.column, QUALITY_DATASET_COLUMN);
        assert_eq!(rule.allow_additional, Some(false));
        assert_eq!(
            rule.required_order,
            Some(vec!["status".to_owned(), "amount".to_owned()])
        );
        assert_eq!(
            rule.columns,
            Some(vec!["status".to_owned(), "amount".to_owned()])
        );
    }

    #[test]
    fn quality_rules_validate_date_ranges_and_count_unparseable_values() {
        let frame = df![
            "date" => &[
                Some("2024-01-01"),
                Some("2024-06-15"),
                Some("2025-01-01"),
                Some("not-a-date"),
                None
            ]
        ]
        .unwrap();
        let mut date_range = quality_rule("date", QualityRuleKind::DateRange);
        date_range.min_date = Some("2024-01-01".to_owned());
        date_range.max_date = Some("2024-12-31".to_owned());
        date_range.max_invalid = Some(3);

        let result = evaluate_quality_rules(&frame, &[date_range]).unwrap();

        assert!(result.passed);
        assert_eq!(result.rules[0].checked_count, 5);
        assert_eq!(result.rules[0].invalid_count, 3);
        assert_eq!(result.rules[0].min_date.as_deref(), Some("2024-01-01"));
        assert_eq!(result.rules[0].max_date.as_deref(), Some("2024-12-31"));
    }

    #[test]
    fn migration_defaults_tolerance_clamps_percentages_and_isolates_bad_rules() {
        let result = migrate_quality_rules_document(serde_json::json!([
            {"kind": "not_null", "column": "id"},
            {"kind": "allowed_values", "column": "status", "values": "not-a-list"},
            {"kind": "row_count", "min_value": 1, "maxInvalidPct": 120}
        ]))
        .expect("el documento de migración debe ser válido");

        assert_eq!(result.converted_rules.len(), 2);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(result.converted_rules[0].max_invalid, Some(0));
        assert_eq!(result.converted_rules[1].max_invalid_pct, Some(100.0));
        assert!(result.warnings.iter().any(|warning| {
            warning.severity == "warning" && warning.message.contains("no traía tolerancia")
        }));
        assert!(result.warnings.iter().any(|warning| {
            warning.severity == "warning" && warning.message.contains("ajustada")
        }));
        assert!(result.warnings.iter().any(|warning| {
            warning.severity == "omitted" && warning.message.contains("debe ser una lista")
        }));
    }

    #[test]
    fn migrates_date_range_bounds_and_rejects_invalid_dates() {
        let result = migrate_quality_rules_document(serde_json::json!([
            {
                "kind": "date_range",
                "column": "created_at",
                "min_value": "2024-01-01",
                "max_value": "2024-12-31",
                "max_invalid": 0
            },
            {
                "kind": "date_range",
                "column": "created_at",
                "min_value": "not-a-date",
                "max_invalid": 0
            }
        ]))
        .unwrap();

        assert_eq!(result.converted_rules.len(), 1);
        assert_eq!(result.omitted_rules, 1);
        assert_eq!(
            result.converted_rules[0].min_date.as_deref(),
            Some("2024-01-01")
        );
        assert_eq!(
            result.converted_rules[0].max_date.as_deref(),
            Some("2024-12-31")
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("límites de fecha válidos")));
    }

    #[test]
    fn quality_tolerances_are_inclusive_and_both_must_pass() {
        let frame = df!["value" => &[Some(1_i64), None, Some(3), Some(4)]].unwrap();
        let mut boundary = quality_rule("value", QualityRuleKind::NotNull);
        boundary.max_invalid = Some(1);
        boundary.max_invalid_pct = Some(25.0);
        let result = evaluate_quality_rules(&frame, &[boundary.clone()]).unwrap();
        assert!(result.passed);
        assert_eq!(result.rules[0].invalid_pct, 25.0);

        boundary.max_invalid_pct = Some(24.999);
        let result = evaluate_quality_rules(&frame, &[boundary]).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn quality_rule_definitions_reject_invalid_contracts_and_types() {
        let frame = df!["text" => &["a"], "number" => &[1_i64]].unwrap();
        let mut missing_tolerance = quality_rule("text", QualityRuleKind::NotNull);
        missing_tolerance.max_invalid = None;
        assert!(evaluate_quality_rules(&frame, &[missing_tolerance]).is_err());

        let mut bad_pct = quality_rule("text", QualityRuleKind::NotNull);
        bad_pct.max_invalid_pct = Some(100.1);
        assert!(evaluate_quality_rules(&frame, &[bad_pct]).is_err());

        let mut bad_bounds = quality_rule("number", QualityRuleKind::NumericRange);
        bad_bounds.min = Some(2.0);
        bad_bounds.max = Some(1.0);
        assert!(evaluate_quality_rules(&frame, &[bad_bounds]).is_err());

        let no_bounds = quality_rule("number", QualityRuleKind::NumericRange);
        assert!(evaluate_quality_rules(&frame, &[no_bounds]).is_err());

        let mut non_finite = quality_rule("number", QualityRuleKind::NumericRange);
        non_finite.min = Some(f64::NAN);
        assert!(evaluate_quality_rules(&frame, &[non_finite]).is_err());
        assert!(evaluate_quality_rules(
            &frame,
            &[quality_rule("missing", QualityRuleKind::NotNull)]
        )
        .is_err());
        assert!(evaluate_quality_rules(
            &frame,
            &[quality_rule("number", QualityRuleKind::NonEmpty)]
        )
        .is_err());
        assert!(evaluate_quality_rules(
            &frame,
            &[quality_rule("text", QualityRuleKind::NumericRange)]
        )
        .is_err());
    }

    #[test]
    fn integer_ranges_compare_large_values_exactly_and_reject_unsafe_bounds() {
        let frame = df![
            "number" => &[9_007_199_254_740_992_i64, 9_007_199_254_740_993_i64, i64::MAX, i64::MIN]
        ]
        .unwrap();
        let mut rule = quality_rule("number", QualityRuleKind::NumericRange);
        rule.max = Some(MAX_SAFE_INTEGER);
        rule.max_invalid = Some(4);
        let result = evaluate_quality_rules(&frame, &[rule]).unwrap();
        assert_eq!(result.rules[0].invalid_count, 3);
        assert!(result.rules[0].passed);

        let mut unsafe_bound = quality_rule("number", QualityRuleKind::NumericRange);
        unsafe_bound.max = Some(9_007_199_254_740_992.0);
        assert!(evaluate_quality_rules(&frame, &[unsafe_bound]).is_err());
    }

    #[test]
    fn quality_contract_limits_rules_and_denies_unknown_or_negative_fields() {
        let frame = df!["value" => &[1_i64]].unwrap();
        let rules = vec![quality_rule("value", QualityRuleKind::NotNull); MAX_QUALITY_RULES];
        assert!(evaluate_quality_rules(&frame, &rules).is_ok());
        let too_many = vec![quality_rule("value", QualityRuleKind::NotNull); MAX_QUALITY_RULES + 1];
        assert!(evaluate_quality_rules(&frame, &too_many).is_err());

        assert!(serde_json::from_str::<QualityRule>(
            r#"{"column":"value","kind":"not_null","maxInvalid":0,"extra":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<QualityRule>(
            r#"{"column":"value","kind":"not_null","maxInvalid":-1}"#
        )
        .is_err());
    }

    #[test]
    fn quality_semantic_budget_accepts_boundaries_and_rejects_validate_and_export_payloads() {
        let boundary_column = "c".repeat(MAX_QUALITY_COLUMN_CHARS);
        let boundary_rules = vec![
            quality_rule(&boundary_column, QualityRuleKind::NotNull);
            MAX_QUALITY_TOTAL_TEXT_CHARS / MAX_QUALITY_COLUMN_CHARS
        ];
        validate_quality_rules_payload(&boundary_rules)
            .expect("el presupuesto exacto de calidad debe admitirse");

        let oversized_column = "c".repeat(MAX_QUALITY_COLUMN_CHARS + 1);
        let field_error = validate_quality_rules_payload(&[quality_rule(
            &oversized_column,
            QualityRuleKind::NotNull,
        )])
        .expect_err("un nombre desproporcionado debe rechazarse");
        assert!(field_error.contains(&MAX_QUALITY_COLUMN_CHARS.to_string()));
        assert!(!field_error.contains(&"c".repeat(32)));

        let oversized_total = vec![
            quality_rule(&boundary_column, QualityRuleKind::NotNull);
            MAX_QUALITY_TOTAL_TEXT_CHARS / MAX_QUALITY_COLUMN_CHARS + 1
        ];
        let frame = df!["value" => &[1_i64]].unwrap();
        let validation_error = evaluate_quality_rules(&frame, &oversized_total)
            .expect_err("validar debe rechazar el total excedido antes de consultar columnas");
        assert!(validation_error.contains(&MAX_QUALITY_TOTAL_TEXT_CHARS.to_string()));
        let export_error =
            enforce_export_quality_with_cancel(&frame, &oversized_total, false, || false)
                .expect_err("exportar debe aplicar el mismo presupuesto");
        assert_eq!(export_error, validation_error);
        assert!(!export_error.contains(&"c".repeat(32)));
    }

    #[test]
    fn quality_result_exposes_counts_without_samples_or_cell_values() {
        let frame = df!["secret" => &[Some("private-value"), None]].unwrap();
        let result =
            evaluate_quality_rules(&frame, &[quality_rule("secret", QualityRuleKind::NotNull)])
                .unwrap();
        let json = serde_json::to_value(result).unwrap();
        let encoded = json.to_string();
        assert!(!encoded.contains("private-value"));
        assert!(!encoded.contains("sample"));
        assert_eq!(json["rules"][0]["checkedCount"], 2);
        assert_eq!(json["rules"][0]["invalidCount"], 1);
    }

    #[test]
    fn export_quality_gate_blocks_before_destination_and_requires_explicit_bypass() {
        use std::cell::Cell;

        let frame = df!["value" => &[Some(1_i64), None]].unwrap();
        assert!(enforce_export_quality_with_cancel(&frame, &[], false, || false).is_err());
        assert_eq!(
            enforce_export_quality_with_cancel(&frame, &[], true, || false).unwrap(),
            None
        );

        let failing = quality_rule("value", QualityRuleKind::NotNull);
        assert!(enforce_export_quality_with_cancel(&frame, &[failing], false, || false).is_err());

        let mut passing = quality_rule("value", QualityRuleKind::NotNull);
        passing.max_invalid = Some(1);
        assert!(
            enforce_export_quality_with_cancel(&frame, &[passing], false, || false)
                .unwrap()
                .is_some()
        );

        let checks = Cell::new(0_usize);
        let cancelled = enforce_export_quality_with_cancel(
            &frame,
            &[quality_rule("value", QualityRuleKind::NotNull)],
            false,
            || {
                checks.set(checks.get() + 1);
                checks.get() >= 2
            },
        );
        assert_eq!(cancelled.unwrap_err(), OPERATION_CANCELLED_MESSAGE);
    }
}
