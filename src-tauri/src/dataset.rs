use crate::crash_report::LockRecovering;
use ::zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
use calamine::{
    open_workbook_auto, Data, DataType as CalamineDataType, Dimensions, Range, Reader, Sheets,
};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use polars::io::json::{JsonFormat, JsonWriter};
use polars::lazy::dsl::{col, len, lit};
use polars::prelude::*;
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{
    de::{DeserializeSeed, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::BinaryHeap,
    collections::HashMap,
    collections::HashSet,
    collections::VecDeque,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use xxhash_rust::xxh3::xxh3_64;
#[path = "dataset/automation.rs"]
mod automation;
mod categorical_profile;
mod column_cleanup;
#[path = "dataset/comparison_engine.rs"]
mod comparison_engine;
#[path = "dataset/comparison_io.rs"]
mod comparison_io;
mod comparison_reader;
#[path = "dataset/csv_formula_safety.rs"]
mod csv_formula_safety;
#[path = "dataset/delimited_header_import.rs"]
mod delimited_header_import;
mod file_validation;
mod history;
#[allow(unused_imports)] // Mantiene la ruta pública del contrato serializado.
pub use history::HistoryEntryState;
use history::{
    get_history_state_impl, redo_last_change_impl, undo_last_change_impl, HistoryCommitResult,
    HistoryEntry, HistoryManager, PreparedHistorySnapshot,
};
#[cfg(test)]
use history::{redo_dataset, undo_dataset};
pub use history::{HistoryResult, HistoryState};
mod import_conventions;
mod import_loading;
#[path = "dataset/import_profile_validation.rs"]
mod import_profile_validation;
mod import_schema_preview;
mod import_source_inspection;
#[path = "dataset/json_reader.rs"]
mod json_reader;
#[path = "dataset/local_query.rs"]
mod local_query;
mod numeric_profile;
#[path = "dataset/operation_cancellation.rs"]
mod operation_cancellation;
#[path = "dataset/operation_state.rs"]
mod operation_state;
#[path = "dataset/page_reader.rs"]
mod page_reader;
#[path = "dataset/profile_engine.rs"]
mod profile_engine;
mod profile_reader;
#[path = "dataset/project_history.rs"]
mod project_history;
mod quality_contracts;
mod quality_documents;
mod quality_evaluation;
#[path = "dataset/query_execution.rs"]
mod query_execution;
#[path = "dataset/recipe_documents.rs"]
mod recipe_documents;
#[path = "dataset/recipe_eager.rs"]
mod recipe_eager;
#[path = "dataset/recipe_engine.rs"]
mod recipe_engine;
#[path = "dataset/recipe_source_projection.rs"]
mod recipe_source_projection;
mod temporal_profile;
pub(crate) use automation::*;
#[cfg(test)]
use csv_formula_safety::csv_formula_safe_frame;
use csv_formula_safety::csv_formula_safe_frame_with_cancel;
#[cfg(test)]
use delimited_header_import::delimited_header_review;
use delimited_header_import::delimited_header_review_with_cancel;
pub use delimited_header_import::DelimitedHeaderReview;
use file_validation::{
    canonicalize_existing_file, canonicalize_write_destination, dataset_extension,
    is_symbolic_link_or_reparse_point, validate_dataset_file,
};
pub(crate) use import_profile_validation::validate_import_exception_policy;
use import_profile_validation::{
    import_exception_schema_for_frame, validate_import_exception_policy_for_recipe,
};
pub use import_profile_validation::{
    import_profile_schema_mismatch, validate_import_profile, IMPORT_PROFILE_MISMATCH_PREFIX,
};
#[cfg(test)]
use local_query::prepare_duckdb_query;
use local_query::{
    duckdb_identifier, parse_local_join_query_spec, parse_local_query,
    prepare_duckdb_query_with_row_count, unique_duckdb_internal_name, LocalAggregate,
    LocalPredicate, LocalPredicateOperator, LocalProjection, LocalQueryPlan,
};
#[cfg(test)]
use page_reader::dataset_page_from_source;
use page_reader::{dataset_page, dataset_page_from_parquet};
#[allow(unused_imports)]
use profile_engine::*;
pub(crate) use project_history::*;
pub use quality_contracts::{
    QualityAggregate, QualityComparison, QualityCondition, QualityMigrationReport,
    QualityMigrationResult, QualityMigrationWarning, QualityMonotonicDirection, QualityRule,
    QualityRuleKind, QualityRuleResult, QualityRulesDocument, QualityValidationResult,
};
#[cfg(test)]
use quality_documents::migrate_quality_rules_document;
pub(crate) use quality_documents::validate_reusable_quality_rules;
use quality_documents::{
    build_quality_rules_document, load_quality_migration_file, parse_quality_datetime,
    parse_quality_rules_document, read_quality_rules_json, save_quality_rules_atomic,
    validate_quality_rules_payload,
};
use quality_evaluation::{
    enforce_export_quality_with_cancel, enforce_source_quality_with_cancel, evaluate_quality_rules,
    evaluate_quality_rules_with_cancel, evaluate_source_quality_rules_with_cancel,
    quality_aggregate_numeric_value, quality_aggregate_observation, quality_datetime_value,
    quality_monotonic_ordering, source_quality_rule_is_incremental,
    validate_quality_rule_definition,
};
#[allow(unused_imports)]
use query_execution::*;
pub(crate) use recipe_documents::validate_reusable_recipe;
use recipe_documents::{
    build_stored_recipe, load_recipe_file, migration_has_true_nullable, migration_policy_value,
    migration_severity_value, migration_string_field, recipe_path_with_extension,
    recipe_suggested_file_name, save_recipe_atomic, validate_recipe_structure,
    validate_semantic_text_budget, validate_stored_recipe,
};
#[allow(unused_imports)]
use recipe_eager::*;
#[cfg(test)]
use recipe_engine::{apply_lazy_recipe_to_frame, lazy_recipe_supported};
use recipe_engine::{apply_recipe_to_frame, lazy_renames_have_no_cycles};
use recipe_source_projection::{
    apply_source_backed_projection_recipe_with_cancellation, duckdb_iso8601_expression,
    duckdb_string_literal, source_backed_projection_recipe_supported,
};
#[path = "dataset/export_io.rs"]
mod export_io;
#[path = "dataset/snapshot_comparison.rs"]
mod snapshot_comparison;
#[path = "dataset/source_loading.rs"]
mod source_loading;
#[path = "dataset/spreadsheet_io.rs"]
mod spreadsheet_io;
use categorical_profile::{
    categorical_group_key, categorical_group_summaries, retain_group_candidate,
    source_categorical_group_summary, GroupKey,
};
#[allow(unused_imports)]
use comparison_engine::*;
#[cfg(test)]
use comparison_io::{
    load_compare_frame, persist_comparison_snapshot, persist_comparison_source_file,
    persist_delimited_comparison_source_file, persist_json_comparison_source_file,
    persist_spreadsheet_comparison_source_file,
};
use comparison_io::{
    load_compare_frame_with_cancel, persist_comparison_file_with_cancel,
    persist_comparison_snapshot_with_cancel, persist_delimited_comparison_source_file_with_cancel,
};
pub(crate) use export_io::copy_file_with_cancel;
use export_io::{
    export_frame_atomic, export_frame_atomic_with_privacy_and_quality_and_recipe,
    export_source_backed_bundle_atomic, export_source_backed_csv_atomic,
    export_source_backed_json_atomic, export_source_backed_parquet_atomic,
    export_source_backed_sql_atomic, export_source_backed_sqlite_atomic,
    export_source_backed_xlsx_atomic, path_with_extension, privacy_safe_frame,
    privacy_safe_frame_with_cancel, source_backed_privacy_snapshot,
};
#[cfg(test)]
use export_io::{export_frame_atomic_with_privacy_and_quality, frame_for_export, write_xlsx};
use numeric_profile::{
    numeric_correlation_matrix, numeric_statistics, numeric_value, semantic_numeric_value,
    source_numeric_correlation_matrix, source_numeric_statistics,
    validate_numeric_correlation_sample_rows, NumericRunWriter,
};
use operation_cancellation::{
    DatabasePreflightCancellation, DatasetComparisonCancellation, PrepareCancellation,
    QualityValidationCancellation, ReviewMutationCancellation,
};
use snapshot_comparison::compare_history_snapshots_impl;
use spreadsheet_io::{
    import_format_for_extension, inspect_workbook, load_spreadsheet_sheet_with_cancel,
    spreadsheet_extensions, spreadsheet_snapshot_plan_from_stream,
    write_spreadsheet_range_snapshot_with_cancel, write_streamed_spreadsheet_snapshot,
};
#[cfg(test)]
use spreadsheet_io::{
    load_spreadsheet_sheet, spreadsheet_range_to_frame, write_spreadsheet_range_snapshot,
};
use temporal_profile::{
    source_temporal_series_summary, temporal_period_key, temporal_period_label,
    temporal_periods_between, temporal_series_summaries, TemporalPeriodKey,
};
#[path = "dataset/project_validation.rs"]
mod project_validation;
pub(crate) use project_validation::{
    validate_project_profile, validate_project_profile_with_row_count, validate_project_workspace,
};
pub(crate) mod samples;

use crate::dataset_fingerprints::{
    normalized_fingerprint_columns, normalized_row_fingerprint, row_fingerprint,
    NormalizedRowFingerprint,
};
use crate::remote_databases::{self, DatabaseTarget};

const PREVIEW_ROW_LIMIT: usize = 50;
const HEADER_REVIEW_ROW_LIMIT: usize = 5;
const MAX_PAGE_SIZE: usize = 200;
const HISTORY_SNAPSHOT_BATCH_ROWS: usize = 4_096;
const SOURCE_BACKED_LOAD_THRESHOLD_BYTES: u64 = 512 * 1024 * 1024;
const MAX_QUERY_CHARS: usize = 2 * 1024;
const LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX: &str =
    "No se pudo usar el snapshot Parquet para la consulta local:";
const SOURCE_BACKED_QUERY_ERROR: &str = "La consulta source-backed no pudo ejecutarse sin materializar el dataset; revisa que la consulta sea compatible con el motor DuckDB y que la fuente siga intacta.";
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
const SAFE_MOJIBAKE_REPLACEMENTS: &[(&str, &str)] = &[
    ("â€™", "’"),
    ("â€œ", "“"),
    ("â€”", "—"),
    ("â€¦", "…"),
    ("Ã©", "é"),
    ("Ã¨", "è"),
    ("Ã ", "à"),
    ("Ã¢", "â"),
    ("Ã®", "î"),
    ("Ã´", "ô"),
    ("Ã³", "ó"),
    ("Ã±", "ñ"),
    ("Ã¼", "ü"),
    ("Ã¡", "á"),
    ("Ã\u{AD}", "í"),
    ("Ãº", "ú"),
];
pub(crate) const OPERATION_CANCELLED_MESSAGE: &str = "Operación cancelada por el usuario.";
const REDACTED_VALUE: &str = "[REDACTED]";
const DELIMITED_SAMPLE_BYTES: u64 = 64 * 1024;
const HISTORY_MAX_ENTRIES: usize = 12;
const HISTORY_DISK_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_AUDIT_CELL_CHARS: usize = 2048;
const RECIPE_FILE_VERSION: u32 = 2;
const PREVIOUS_RECIPE_FILE_VERSION: u32 = 1;
const RECIPE_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const MAX_RECIPE_TEXT_FIELD_CHARS: usize = 4 * 1024;
const MAX_RECIPE_TOTAL_TEXT_CHARS: usize = 64 * 1024;
const MAX_RECIPE_SOURCE_COLUMNS: usize = 2_000;
const NORMALIZED_DUPLICATE_CHUNK_ROWS: usize = 262_144;
// Fingerprints are spilled into fixed buckets before sorting. Equal values
// always land in the same bucket, while the in-memory sort only holds one
// bucket instead of one entry per dataset row.
const NORMALIZED_DUPLICATE_BUCKETS: usize = 256;
const NORMALIZED_FINGERPRINT_BYTES: usize = std::mem::size_of::<NormalizedRowFingerprint>();
const SPREADSHEET_SNAPSHOT_BLOCK_ROWS: usize = 16 * 1024;
const NUMERIC_HISTOGRAM_BUCKETS: usize = 12;
const MAX_NUMERIC_CORRELATION_COLUMNS: usize = 12;
const MIN_NUMERIC_CORRELATION_SAMPLE_ROWS: usize = 1_000;
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
const CANCELLABLE_READ_BATCH_ROWS: usize = 8 * 1024;
const SOURCE_PROFILE_BLOCK_ROWS: usize = 4 * LOCAL_QUERY_BLOCK_ROWS;
const LOCAL_QUERY_CANCEL_CHECK_ROWS: usize = 4096;
const LOCAL_QUERY_JOIN_MAX_INPUT_ROWS: usize = 2_000_000;
const LOCAL_QUERY_JOIN_MAX_RESULT_ROWS: usize = 2_000_000;
const MATERIALIZATION_GUARD_THRESHOLD_BYTES: u64 = 512 * 1024 * 1024;
const MATERIALIZATION_ESTIMATE_MULTIPLIER: u64 = 4;
const MATERIALIZATION_RESERVE_BYTES: u64 = 256 * 1024 * 1024;
const SOURCE_BACKED_CONSOLIDATION_CONFLICT_ERROR: &str = "No se pueden consolidar claves con conflictos o duplicados. Revisa la comparación antes de continuar.";
const SOURCE_BACKED_RESOLUTION_MAX_CONFLICTS: usize = 8_192;
const SOURCE_BACKED_RESOLUTION_LIMIT_REACHED: &str =
    "La resolución source-backed superó su límite seguro.";
const LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS: usize = 2_000_000;
const LOCAL_QUERY_MAX_GROUP_COLUMNS: usize = 8;
const LOCAL_QUERY_MAX_JOIN_COLUMNS: usize = 8;
// Key indexes are partitioned before comparison so only one bucket from each
// dataset needs to be materialized while computing the exact result.
const COMPARISON_KEY_BUCKETS: usize = 256;
const COMPARISON_KEY_RECORD_BYTES: usize = std::mem::size_of::<u64>() * 2;
const BUNDLE_MANIFEST_VERSION: u8 = 2;
const BUNDLE_DICTIONARY_VERSION: u8 = 1;
const BUNDLE_QUALITY_REPORT_VERSION: u8 = 1;
const BUNDLE_DELIVERY_SUMMARY_VERSION: u8 = 1;
const BUNDLE_DELIVERY_SUMMARY_FILE: &str = "delivery-summary.md";
static NEXT_HISTORY_ENTRY_ID: AtomicU64 = AtomicU64::new(1);

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
    #[serde(default)]
    delivery_summary_file: String,
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
const LEGACY_QUALITY_DOCUMENT_MAX_VERSION: u8 = 3;

pub(crate) fn send_progress(
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
    pub(crate) columns: Vec<DatasetColumn>,
    pub(crate) row_count: usize,
    pub(crate) offset: usize,
    pub(crate) rows: Vec<Vec<Option<String>>>,
    pub(crate) truncated: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DatasetQueryEngine {
    #[default]
    Polars,
    Duckdb,
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

type ConflictChoiceMap = HashMap<(usize, Option<String>), ConflictResolutionChoice>;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum ConflictResolutionChoice {
    Exclude,
    UseSource(ConflictSource),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConflictResolution {
    Exclude {
        conflict_index: usize,
    },
    UseSource {
        conflict_index: usize,
        source: ConflictSource,
        #[serde(default)]
        column: Option<String>,
    },
}

impl ConflictResolution {
    fn choice(&self) -> (usize, Option<String>, ConflictResolutionChoice) {
        match self {
            Self::Exclude { conflict_index } => {
                (*conflict_index, None, ConflictResolutionChoice::Exclude)
            }
            Self::UseSource {
                conflict_index,
                source,
                column,
            } => (
                *conflict_index,
                column.clone(),
                ConflictResolutionChoice::UseSource(*source),
            ),
        }
    }
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
    resource_estimate: DatasetResourceEstimate,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum DatasetLoadPath {
    InMemory,
    SourceBacked,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetResourceEstimate {
    processing_path: DatasetLoadPath,
    estimated_materialization_ram_bytes: u64,
    estimated_temporary_disk_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SampleDatasetDescriptor {
    id: String,
    name: String,
    format: String,
    description: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpreadsheetHeaderMode {
    FirstRow,
    Generated,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDateConvention {
    Unresolved,
    Iso8601,
    Ymd,
    Dmy,
    Mdy,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportNumberConvention {
    Unresolved,
    DotDecimalCommaGrouping,
    CommaDecimalDotGrouping,
    DotDecimalSpaceGrouping,
    CommaDecimalSpaceGrouping,
    Integer,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportProfileColumn {
    pub name: String,
    pub data_type: String,
}

/// Reusable, versioned import guidance. This deliberately contains schema metadata
/// only: never a native selection ID, source path, file name, or data sample.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportProfile {
    pub version: u8,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_mode: Option<SpreadsheetHeaderMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_convention: Option<ImportDateConvention>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_convention: Option<ImportNumberConvention>,
    pub schema: Vec<ImportProfileColumn>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportExceptionBaseline {
    Lexical,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InvalidConversionAction {
    Review,
    Nullify,
    ExcludeRow,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ImportExceptionConversion {
    Cast {
        column: String,
        target: RecipeCastTarget,
        on_invalid: InvalidConversionAction,
    },
    Date {
        column: String,
        format: RecipeDateFormat,
        target: RecipeDateTarget,
        on_invalid: InvalidConversionAction,
    },
}

/// Column conversion decisions are metadata only and are bound to one exact schema.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportExceptionPolicy {
    pub version: u8,
    pub baseline: ImportExceptionBaseline,
    pub schema: Vec<ImportProfileColumn>,
    pub conversions: Vec<ImportExceptionConversion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportProfileTypeChange {
    pub column: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportProfileMismatch {
    pub code: String,
    pub missing_columns: Vec<String>,
    pub added_columns: Vec<String>,
    pub changed_types: Vec<ImportProfileTypeChange>,
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TemporalAggregationKind {
    Sum,
    Mean,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemporalAggregationPeriod {
    period: String,
    row_count: usize,
    value_count: usize,
    value: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemporalAggregationSeries {
    date_column: String,
    value_column: String,
    aggregation: TemporalAggregationKind,
    granularity: String,
    periods: Vec<TemporalAggregationPeriod>,
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

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRevisionComparison {
    before_snapshot_id: String,
    after_snapshot_id: String,
    before_label: String,
    after_label: String,
    before: SnapshotRevisionSummary,
    after: SnapshotRevisionSummary,
    deltas: SnapshotRevisionDeltas,
    columns: Vec<SnapshotColumnComparison>,
    quality: SnapshotQualityComparison,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotRevisionSummary {
    row_count: usize,
    column_count: usize,
    null_count: usize,
    invalid_type_count: usize,
    duplicate_row_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotRevisionDeltas {
    row_count: Option<i64>,
    column_count: Option<i64>,
    null_count: Option<i64>,
    invalid_type_count: Option<i64>,
    duplicate_row_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotColumnSummary {
    data_type: String,
    null_count: usize,
    invalid_type_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotColumnComparison {
    name: String,
    comparable: bool,
    reason: Option<String>,
    before: Option<SnapshotColumnSummary>,
    after: Option<SnapshotColumnSummary>,
    type_changed: Option<bool>,
    null_count_delta: Option<i64>,
    invalid_type_count_delta: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotQualityComparison {
    configured_rule_count: usize,
    comparable_rule_count: usize,
    non_comparable_rule_count: usize,
    improved_rule_count: usize,
    degraded_rule_count: usize,
    before_passed_rule_count: usize,
    after_passed_rule_count: usize,
    rules: Vec<SnapshotQualityRuleComparison>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SnapshotQualityRuleComparison {
    rule_index: usize,
    kind: QualityRuleKind,
    column: String,
    comparable: bool,
    reason: Option<String>,
    before_invalid_count: Option<usize>,
    after_invalid_count: Option<usize>,
    before_invalid_percentage: Option<f64>,
    after_invalid_percentage: Option<f64>,
    before_passed: Option<bool>,
    after_passed: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SafeCorrectionsResult {
    dataset: DatasetPreview,
    changed_cell_count: usize,
    affected_row_count: usize,
    removed_row_count: usize,
    renamed_column_count: usize,
    renames: Vec<ColumnRename>,
    /// Cells filled by the optional conservative imputation of the same plan.
    imputed_cell_count: usize,
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
    #[serde(default)]
    regex: bool,
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
pub struct RecipeSourceColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredTransformRecipe {
    pub version: u32,
    pub name: String,
    pub saved_at: String,
    pub recipe: TransformRecipe,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_schema: Option<Vec<RecipeSourceColumn>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_options: Option<RecipeExportOptions>,
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
    row_count: usize,
    frame: DataFrame,
    source_backed: bool,
    delimited_header_mode: Option<SpreadsheetHeaderMode>,
    profile: Option<DatasetProfile>,
    history: HistoryManager,
}

#[derive(Clone)]
struct DatasetMutationStamp {
    source_path: Option<PathBuf>,
    history_source_snapshot_path: Option<PathBuf>,
    history_cursor_path: Option<PathBuf>,
    file_name: String,
    file_size_bytes: u64,
    row_count: usize,
    source_backed: bool,
    frame: DataFrame,
    history_cursor: usize,
    history_entry_count: usize,
    history_next_id: u64,
    history_current_label: String,
}

impl DatasetMutationStamp {
    fn capture(dataset: &LoadedDataset) -> Self {
        Self {
            source_path: dataset.source_path.clone(),
            history_source_snapshot_path: dataset.history.source_snapshot_path.clone(),
            history_cursor_path: dataset
                .history
                .entries
                .get(dataset.history.cursor)
                .map(|entry| entry.path.clone()),
            file_name: dataset.file_name.clone(),
            file_size_bytes: dataset.file_size_bytes,
            row_count: dataset.row_count,
            source_backed: dataset.source_backed,
            frame: dataset.frame.clone(),
            history_cursor: dataset.history.cursor,
            history_entry_count: dataset.history.entries.len(),
            history_next_id: dataset.history.next_id,
            history_current_label: dataset.history.current_label.clone(),
        }
    }

    fn matches(&self, dataset: &LoadedDataset) -> bool {
        self.source_path == dataset.source_path
            && self.history_source_snapshot_path == dataset.history.source_snapshot_path
            && self.history_cursor_path
                == dataset
                    .history
                    .entries
                    .get(dataset.history.cursor)
                    .map(|entry| entry.path.clone())
            && self.file_name == dataset.file_name
            && self.file_size_bytes == dataset.file_size_bytes
            && self.row_count == dataset.row_count
            && self.source_backed == dataset.source_backed
            && self.frame.equals_missing(&dataset.frame)
            && self.history_cursor == dataset.history.cursor
            && self.history_entry_count == dataset.history.entries.len()
            && self.history_next_id == dataset.history.next_id
            && self.history_current_label == dataset.history.current_label
    }
}

fn fresh_history_entry_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_HISTORY_ENTRY_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "rev-{timestamp:032x}-{:08x}-{sequence:016x}",
        std::process::id()
    )
}

#[cfg(test)]
fn publish_candidate(
    dataset: &mut LoadedDataset,
    candidate: DataFrame,
    label: &str,
) -> Result<DatasetPreview, String> {
    publish_candidate_with_cancellation(dataset, candidate, label, None)
}

fn publish_candidate_with_cancellation(
    dataset: &mut LoadedDataset,
    mut candidate: DataFrame,
    label: &str,
    cancellation: Option<&PrepareCancellation>,
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
    let prepared_snapshot = if dataset.history.snapshots_enabled {
        Some(dataset.history.prepare_frame_snapshot(&candidate, || {
            cancellation.is_some_and(PrepareCancellation::is_cancelled)
        })?)
    } else {
        None
    };
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    let previous_source_path = dataset.source_path.clone();
    let previous_source_snapshot = dataset.history.source_snapshot_path.clone();
    let mut retired_paths = Vec::new();
    let publish = || {
        if let Some(prepared_snapshot) = prepared_snapshot {
            retired_paths = dataset
                .history
                .record_prepared_snapshot(prepared_snapshot, label)?;
        } else {
            dataset.history.current_label = label.to_owned();
        }
        // La fuente original solo representa el cursor actual antes de una
        // mutación. Después de publicar una candidata, las consultas deben usar
        // el snapshot/historial o el frame transformado, nunca el archivo viejo.
        dataset.source_path = None;
        dataset.history.source_snapshot_path = None;
        dataset.row_count = candidate.height();
        dataset.frame = candidate;
        dataset.source_backed = false;
        dataset.profile = None;
        Ok(())
    };
    if let Some(cancellation) = cancellation {
        cancellation.commit(publish)?;
    } else {
        publish()?;
    }
    for path in retired_paths {
        if !dataset
            .history
            .entries
            .iter()
            .any(|entry| entry.path == path)
            && previous_source_snapshot.as_deref() != Some(path.as_path())
        {
            let _ = fs::remove_file(path);
        }
    }
    for path in [previous_source_path, previous_source_snapshot]
        .into_iter()
        .flatten()
    {
        if path.parent() == Some(dataset.history.directory.path())
            && dataset.source_path.as_deref() != Some(path.as_path())
            && !dataset
                .history
                .entries
                .iter()
                .any(|entry| entry.path == path)
        {
            let _ = fs::remove_file(path);
        }
    }
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
    generation: u64,
    path: PathBuf,
    file_size_bytes: u64,
    sheets: Vec<String>,
}

struct PendingComparison {
    file_name: String,
    file_size_bytes: u64,
    row_count: usize,
    _directory: tempfile::TempDir,
    snapshot_path: PathBuf,
    key_columns: Vec<String>,
}

struct ReviewComparisonSnapshot {
    _directory: tempfile::TempDir,
    path: PathBuf,
    identity_path: PathBuf,
    file_name: String,
    file_size_bytes: u64,
    row_count: usize,
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
    load_commit_lock: Mutex<()>,
    profile_generation: AtomicU64,
    temporal_generation: AtomicU64,
    export_generation: AtomicU64,
    query_generation: AtomicU64,
    dataset_page_generation: AtomicU64,
    snapshot_comparison_generation: AtomicU64,
    project_open_generation: std::sync::Arc<AtomicU64>,
    project_open_commit_lock: Mutex<()>,
    project_save_generation: std::sync::Arc<AtomicU64>,
    project_save_commit_lock: Mutex<()>,
    project_delete_generation: std::sync::Arc<AtomicU64>,
    project_delete_commit_lock: Mutex<()>,
    project_catalog_generation: std::sync::Arc<AtomicU64>,
    project_versions_generation: std::sync::Arc<AtomicU64>,
    dataset_comparison_generation: AtomicU64,
    dataset_comparison_commit_lock: Mutex<()>,
    quality_validation_generation: AtomicU64,
    database_preflight_generation: AtomicU64,
    database_connection_generation: AtomicU64,
    prepare_generation: AtomicU64,
    prepare_commit_lock: Mutex<()>,
    review_mutation_generation: AtomicU64,
    review_mutation_in_flight: AtomicBool,
    review_mutation_commit_lock: Mutex<()>,
}

fn snapshot_pending_comparison_for_review(
    state: &DatasetState,
    cancellation: &ReviewMutationCancellation,
) -> Result<ReviewComparisonSnapshot, String> {
    cancellation.ensure()?;
    let comparison = state.comparison.lock_recovering();
    let pending = comparison
        .as_ref()
        .ok_or_else(|| "No hay un dataset comparado listo para consolidar.".to_owned())?;
    let identity_path = pending.snapshot_path.clone();
    let file_name = pending.file_name.clone();
    let row_count = pending.row_count;
    let key_columns = pending.key_columns.clone();
    let source_size = fs::metadata(&pending.snapshot_path)
        .map_err(|error| format!("No se pudo inspeccionar el snapshot comparado: {error}"))?
        .len();
    if source_size != pending.file_size_bytes {
        return Err("El snapshot comparado cambió antes de consolidar.".to_owned());
    }

    // A private copy lets the review operation release the comparison lock
    // while DuckDB reads it. If another comparison replaces it, publication
    // checks the original path and rejects the stale candidate.
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot de Review: {error}"))?;
    let mut input = File::open(&pending.snapshot_path)
        .map_err(|error| format!("No se pudo leer el snapshot comparado: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory.path())
        .map_err(|error| format!("No se pudo copiar el snapshot comparado: {error}"))?;
    let mut buffer = [0; 64 * 1024];
    loop {
        cancellation.ensure()?;
        let bytes_read = input
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo leer el snapshot comparado: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        temporary
            .as_file_mut()
            .write_all(&buffer[..bytes_read])
            .map_err(|error| format!("No se pudo copiar el snapshot comparado: {error}"))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot comparado: {error}"))?;
    let path = directory.path().join("compared.parquet");
    temporary
        .persist(&path)
        .map_err(|error| format!("No se pudo publicar el snapshot de Review: {}", error.error))?;
    let copied_size = fs::metadata(&path)
        .map_err(|error| format!("No se pudo verificar el snapshot comparado: {error}"))?
        .len();
    if copied_size != source_size {
        return Err("La copia del snapshot comparado quedó incompleta.".to_owned());
    }
    cancellation.ensure()?;
    Ok(ReviewComparisonSnapshot {
        _directory: directory,
        path,
        identity_path,
        file_name,
        file_size_bytes: copied_size,
        row_count,
        key_columns,
    })
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
    let mut current = state.current.lock_recovering();
    if current.is_some() {
        return Err("El probe nativo requiere una sesión de datos vacía.".to_owned());
    }
    *current = Some(LoadedDataset {
        source_path: None,
        file_name: "native-probe.csv".to_owned(),
        file_size_bytes: 96,
        row_count: frame.height(),
        frame,
        source_backed: false,
        delimited_header_mode: None,
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
    let generation = app.state::<DatasetState>().begin_export();
    let (frame, suggested_name) = {
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current
            .as_mut()
            .ok_or_else(|| "No hay un dataset activo para el probe nativo.".to_owned())?;
        materialize_loaded_dataset_with_cancel(dataset, || state.export_was_cancelled(generation))?;
        let stem = Path::new(&dataset.file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("dataset");
        (
            dataset.frame.clone(),
            format!("{stem}-native-probe.{}", format.extension()),
        )
    };

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
        let mut current = self.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        publish_candidate_with_cancellation(dataset, frame, label, None).map(|_| ())
    }

    pub(crate) fn project_test_undo(&self) -> Result<DataFrame, String> {
        let mut current = self.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        undo_dataset(dataset)?;
        Ok(dataset.frame.clone())
    }

    pub(crate) fn project_test_redo(&self) -> Result<DataFrame, String> {
        let mut current = self.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        redo_dataset(dataset)?;
        Ok(dataset.frame.clone())
    }

    pub(crate) fn project_test_cache_profile(&self) -> Result<DatasetProfile, String> {
        let mut current = self.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        materialize_loaded_dataset(dataset)?;
        let profile = profile_dataset(&dataset.frame)?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    }

    pub(crate) fn project_test_degrade_history(&self) -> Result<(), String> {
        let mut current = self.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| "missing".to_owned())?;
        materialize_loaded_dataset(dataset)?;
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

pub(crate) fn preview_value(value: AnyValue<'_>) -> Option<String> {
    match value {
        AnyValue::Null => None,
        AnyValue::String(value) => Some(value.to_owned()),
        AnyValue::StringOwned(value) => Some(value.as_str().to_owned()),
        value => Some(value.to_string()),
    }
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

fn dataset_preview_from_schema_and_page(
    file_name: &str,
    file_size_bytes: u64,
    row_count: usize,
    schema: &DataFrame,
    page: &DataFrame,
) -> Result<DatasetPreview, String> {
    let columns = schema
        .columns()
        .iter()
        .map(|column| DatasetColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
        })
        .collect();
    let rows = dataset_page(page, 0, PREVIEW_ROW_LIMIT)?.rows;

    Ok(DatasetPreview {
        file_name: file_name.to_owned(),
        file_size_bytes,
        row_count,
        column_count: schema.width(),
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

fn source_backed_projection(columns: &[String], audit_label: &str) -> Result<String, String> {
    if columns.is_empty() {
        return Err("La fuente source-backed no contiene columnas utilizables.".to_owned());
    }
    let audit_literal = duckdb_string_literal(audit_label);
    let projection = columns
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name == "_cambios" {
                format!(
                    "CASE WHEN {identifier} IS NULL OR TRIM(CAST({identifier} AS VARCHAR)) = '' THEN {audit_literal} ELSE LEFT(CAST({identifier} AS VARCHAR) || '; ' || {audit_literal}, {MAX_AUDIT_CELL_CHARS}) END AS {identifier}"
                )
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>();
    Ok(projection.join(", "))
}

fn source_backed_empty_row_projection(
    schema: &DataFrame,
    audit_label: &str,
) -> Result<String, String> {
    let columns = schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    source_backed_projection(&columns, audit_label)
}

fn source_backed_empty_row_query(schema: &DataFrame) -> Result<String, String> {
    if schema.width() == 0 {
        return Err("La fuente source-backed no contiene columnas utilizables.".to_owned());
    }
    let conditions = schema
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            format!("COALESCE(TRIM(CAST({identifier} AS VARCHAR)), '') <> ''")
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "SELECT * FROM dataset WHERE {}",
        conditions.join(" OR ")
    ))
}

fn source_backed_duplicate_query(schema: &DataFrame, projection: &str) -> Result<String, String> {
    if schema.width() == 0 {
        return Err("La fuente source-backed no contiene columnas utilizables.".to_owned());
    }
    let columns = schema
        .get_column_names()
        .iter()
        .map(|name| duckdb_identifier(name))
        .collect::<Vec<_>>();
    let schema_names = schema
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>();
    let mut order_name = "__columnia_row_order".to_owned();
    while schema_names.iter().any(|name| *name == order_name) {
        order_name.push('_');
    }
    let mut rank_name = "__columnia_duplicate_rank".to_owned();
    while schema_names.iter().any(|name| *name == rank_name) || rank_name == order_name {
        rank_name.push('_');
    }
    let order_column = duckdb_identifier(&order_name);
    let rank_column = duckdb_identifier(&rank_name);
    Ok(format!(
        "SELECT {projection} FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY {} ORDER BY {order_column}) AS {rank_column} FROM (SELECT dataset.*, ROW_NUMBER() OVER () AS {order_column} FROM dataset) AS ordered) AS ranked WHERE {rank_column} = 1 ORDER BY {order_column}",
        columns.join(", ")
    ))
}

fn source_backed_near_duplicate_query(
    schema: &DataFrame,
    projection: &str,
) -> Result<String, String> {
    if schema.width() == 0 {
        return Err("La fuente source-backed no contiene columnas utilizables.".to_owned());
    }
    let schema_names = schema
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>();
    let mut used = HashSet::new();
    let mut internal_name = |base: &str| {
        let mut candidate = base.to_owned();
        while schema_names.iter().any(|name| *name == candidate) || used.contains(&candidate) {
            candidate.push('_');
        }
        used.insert(candidate.clone());
        candidate
    };
    let order_name = internal_name("__columnia_near_order");
    let normalized_name = internal_name("__columnia_near_normalized");
    let exact_name = internal_name("__columnia_near_exact");
    let normalized_rank_name = internal_name("__columnia_near_normalized_rank");
    let exact_rank_name = internal_name("__columnia_near_exact_rank");
    let normalized_components = schema
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            let normalized = source_backed_normalized_text_expression(&identifier, true);
            format!(
                "CASE WHEN {identifier} IS NULL THEN '0' ELSE concat('1', CAST(length({normalized}) AS VARCHAR), ':', {normalized}) END"
            )
        })
        .collect::<Vec<_>>();
    let exact_components = schema
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            let raw = format!("CAST({identifier} AS VARCHAR)");
            format!(
                "CASE WHEN {identifier} IS NULL THEN '0' ELSE concat('1', CAST(length({raw}) AS VARCHAR), ':', {raw}) END"
            )
        })
        .collect::<Vec<_>>();
    let order = duckdb_identifier(&order_name);
    let normalized = duckdb_identifier(&normalized_name);
    let exact = duckdb_identifier(&exact_name);
    let normalized_rank = duckdb_identifier(&normalized_rank_name);
    let exact_rank = duckdb_identifier(&exact_rank_name);
    let normalized_key = format!("concat({})", normalized_components.join(", "));
    let exact_key = format!("concat({})", exact_components.join(", "));
    let keyed = format!(
        "SELECT dataset.*, ROW_NUMBER() OVER () AS {order}, {normalized_key} AS {normalized}, {exact_key} AS {exact} FROM dataset"
    );
    let ranked = format!(
        "SELECT *, ROW_NUMBER() OVER (PARTITION BY {normalized} ORDER BY {order}) AS {normalized_rank}, ROW_NUMBER() OVER (PARTITION BY {normalized}, {exact} ORDER BY {order}) AS {exact_rank} FROM ({keyed}) AS keyed"
    );
    Ok(format!(
        "SELECT {projection} FROM ({ranked}) AS ranked WHERE {normalized_rank} = 1 OR {exact_rank} > 1 ORDER BY {order}"
    ))
}

#[cfg(test)]
fn initialize_source_backed_history(
    dataset: &mut LoadedDataset,
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
) -> Result<bool, String> {
    if dataset.history.snapshots_enabled {
        return Ok(true);
    }

    let existing_snapshot = dataset.history.source_snapshot_path.clone();
    let (snapshot_source, cleanup_snapshot_source) = if let Some(snapshot) = existing_snapshot {
        (snapshot, false)
    } else if matches!(
        source_format,
        crate::duckdb_query::DuckDbFileFormat::Parquet
    ) {
        (source_path.to_owned(), false)
    } else {
        let snapshot = dataset
            .history
            .directory
            .path()
            .join("source-initial.parquet");
        if let Err(error) = crate::duckdb_query::materialize_file_to_parquet_with_projection(
            source_path,
            source_format,
            &snapshot,
            "*",
            || false,
        ) {
            let _ = fs::remove_file(&snapshot);
            return Err(error);
        }
        (snapshot, true)
    };

    let history = &mut dataset.history;
    let previous_snapshots_enabled = history.snapshots_enabled;
    let previous_degraded_reason = history.degraded_reason.clone();
    let previous_next_id = history.next_id;
    let mut candidate_id = previous_next_id;
    loop {
        let candidate_path = history
            .directory
            .path()
            .join(format!("snapshot-{candidate_id:020}.parquet"));
        let already_referenced = history
            .entries
            .iter()
            .any(|entry| entry.path == candidate_path);
        if !already_referenced && !candidate_path.exists() {
            break;
        }
        candidate_id = candidate_id.checked_add(1).ok_or_else(|| {
            "No se pudo reservar un identificador único para el historial.".to_owned()
        })?;
    }
    let previous_entries = std::mem::take(&mut history.entries);
    let previous_cursor = history.cursor;
    let previous_label = history.current_label.clone();

    history.snapshots_enabled = true;
    history.degraded_reason = None;
    history.cursor = 0;
    history.next_id = candidate_id;
    let record_result = history.record_parquet(&snapshot_source, "Dataset original");
    if cleanup_snapshot_source {
        let _ = fs::remove_file(snapshot_source);
    }
    if let Err(error) = record_result {
        // `record_parquet` only changes the cursor after the snapshot has been
        // persisted. Restore every in-memory field if copying or persisting the
        // original fails. Preserve the prior next ID so a retry cannot collide
        // with any snapshot already referenced by the restored entries.
        history.snapshots_enabled = previous_snapshots_enabled;
        history.degraded_reason = previous_degraded_reason;
        history.entries = previous_entries;
        history.cursor = previous_cursor;
        history.next_id = previous_next_id;
        history.current_label = previous_label;
        return Err(error);
    }

    for entry in previous_entries {
        let _ = fs::remove_file(entry.path);
    }
    Ok(history.snapshots_enabled)
}

fn prepare_source_backed_baseline_snapshot<C>(
    history: &HistoryManager,
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    is_cancelled: C,
) -> Result<PreparedHistorySnapshot, String>
where
    C: Fn() -> bool + Send + Sync + Clone + 'static,
{
    let mut generated_snapshot_guard = None;
    let snapshot_source = if let Some(snapshot) = history.source_snapshot_path.as_deref() {
        snapshot.to_owned()
    } else if matches!(
        source_format,
        crate::duckdb_query::DuckDbFileFormat::Parquet
    ) {
        source_path.to_owned()
    } else {
        let temporary =
            tempfile::NamedTempFile::with_suffix_in(".parquet", history.directory.path()).map_err(
                |error| format!("No se pudo preparar la fuente original del historial: {error}"),
            )?;
        let snapshot = temporary.path().to_owned();
        drop(temporary);
        generated_snapshot_guard = Some(SourceBackedJoinOutputGuard::new(&snapshot));
        let materialized = crate::duckdb_query::materialize_file_to_parquet_with_projection(
            source_path,
            source_format,
            &snapshot,
            "*",
            is_cancelled.clone(),
        );
        materialized?;
        snapshot
    };

    let prepared = history.prepare_parquet_snapshot(&snapshot_source, is_cancelled);
    drop(generated_snapshot_guard);
    prepared
}

fn commit_source_backed_recipe_history(
    history: &mut HistoryManager,
    baseline: Option<PreparedHistorySnapshot>,
    output: PreparedHistorySnapshot,
    label: &str,
) -> Result<HistoryCommitResult, String> {
    commit_source_backed_recipe_history_with_persist(
        history,
        baseline,
        output,
        label,
        |prepared, destination| {
            prepared
                .temporary
                .persist(destination)
                .map(|_| ())
                .map_err(|error| error.error.to_string())
        },
    )
}

fn commit_source_backed_recipe_history_with_persist<P>(
    history: &mut HistoryManager,
    baseline: Option<PreparedHistorySnapshot>,
    output: PreparedHistorySnapshot,
    label: &str,
    mut persist: P,
) -> Result<HistoryCommitResult, String>
where
    P: FnMut(PreparedHistorySnapshot, &Path) -> Result<(), String>,
{
    let output_bytes = output.bytes;
    if baseline.is_none() && !history.snapshots_enabled {
        return Err("No se pudo preparar el historial source-backed.".to_owned());
    }
    if let Some(baseline) = baseline.as_ref() {
        if baseline.bytes > history.disk_budget_bytes {
            return Ok(HistoryCommitResult {
                active_snapshot: None,
                retired_paths: history
                    .disable_for_size_deferred("Dataset original", baseline.bytes),
            });
        }
    }
    if output.bytes > history.disk_budget_bytes {
        return Ok(HistoryCommitResult {
            active_snapshot: None,
            retired_paths: history.disable_for_size_deferred(label, output.bytes),
        });
    }

    let (baseline_destination, output_id, output_destination) = if let Some(baseline) =
        baseline.as_ref()
    {
        let (baseline_id, baseline_path) =
            history.next_snapshot_destination(history.next_id, &[])?;
        let (output_id, output_path) = history.next_snapshot_destination(
            baseline_id.checked_add(1).ok_or_else(|| {
                "No se pudo reservar un identificador único para el historial.".to_owned()
            })?,
            std::slice::from_ref(&baseline_path),
        )?;
        (
            Some((baseline_id, baseline_path, baseline.bytes)),
            output_id,
            output_path,
        )
    } else {
        let (output_id, output_path) = history.next_snapshot_destination(history.next_id, &[])?;
        (None, output_id, output_path)
    };

    let mut installed = Vec::with_capacity(2);
    if let Some((_, path, _)) = baseline_destination.as_ref() {
        let baseline = baseline.expect("la línea base debe acompañar su destino");
        if let Err(error) = persist(baseline, path) {
            return Err(format!(
                "No se pudo publicar el snapshot inicial del historial: {error}"
            ));
        }
        installed.push(path.clone());
    }
    if let Err(error) = persist(output, &output_destination) {
        for path in installed {
            let _ = fs::remove_file(path);
        }
        return Err(format!(
            "No se pudo publicar el snapshot de la receta: {error}"
        ));
    }

    let mut retired_paths = Vec::new();
    if let Some((id, path, bytes)) = baseline_destination {
        let previous_entries = std::mem::take(&mut history.entries);
        history.snapshots_enabled = true;
        history.degraded_reason = None;
        history.entries.push(HistoryEntry {
            id: fresh_history_entry_id(),
            label: "Dataset original".to_owned(),
            path: path.clone(),
            bytes,
        });
        history.cursor = 0;
        history.next_id = id.wrapping_add(1);
        for entry in previous_entries {
            if entry.path != path && entry.path != output_destination {
                retired_paths.push(entry.path);
            }
        }
    } else {
        let branch_start = history.cursor.saturating_add(1).min(history.entries.len());
        let removed = history.entries.split_off(branch_start);
        retired_paths.extend(removed.into_iter().map(|entry| entry.path));
    }
    history.entries.push(HistoryEntry {
        id: fresh_history_entry_id(),
        label: label.to_owned(),
        path: output_destination,
        bytes: output_bytes,
    });
    history.cursor = history.entries.len() - 1;
    history.next_id = output_id.wrapping_add(1);
    history.current_label = label.to_owned();

    while history.entries.len() > history.max_entries
        || history.disk_bytes() > history.disk_budget_bytes
    {
        let entry = history.entries.remove(0);
        retired_paths.push(entry.path);
        history.cursor = history.cursor.saturating_sub(1);
    }

    Ok(HistoryCommitResult {
        active_snapshot: history
            .snapshots_enabled
            .then(|| history.entries.get(history.cursor))
            .flatten()
            .map(|entry| (entry.path.clone(), entry.bytes)),
        retired_paths,
    })
}

fn publish_source_backed_query(
    dataset: &mut LoadedDataset,
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    query: &str,
    label: &str,
    force_publish: bool,
    cancellation: PrepareCancellation,
) -> Result<Option<DatasetMutation>, String> {
    cancellation.ensure()?;
    let original_source_path = dataset
        .source_path
        .clone()
        .ok_or_else(|| "La fuente source-backed ya no está disponible.".to_owned())?;
    let (_, source_size_before, _) = validate_dataset_file(&original_source_path)?;
    if source_size_before != dataset.file_size_bytes {
        return Err("El archivo source-backed cambió antes de aplicar la limpieza.".to_owned());
    }

    let temporary =
        tempfile::NamedTempFile::with_suffix_in(".parquet", dataset.history.directory.path())
            .map_err(|error| format!("No se pudo preparar la salida source-backed: {error}"))?;
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let mut output_guard = SourceBackedJoinOutputGuard::new(&output_path);
    match crate::duckdb_query::materialize_file_query_to_parquet_with_cancel(
        source_path,
        source_format,
        query,
        &output_path,
        cancellation.callback(),
    ) {
        Ok(()) => {}
        Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
        Err(_) => {
            let _ = fs::remove_file(&output_path);
            return Ok(None);
        }
    }

    cancellation.ensure()?;
    let output_size = fs::metadata(&output_path)
        .map_err(|error| format!("No se pudo verificar la salida source-backed: {error}"))?
        .len();
    let output_schema = read_parquet_schema_frame(&output_path)?;
    cancellation.ensure()?;
    let output_row_count = crate::duckdb_query::count_file_rows(
        &output_path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        cancellation.callback(),
    )?;
    cancellation.ensure()?;
    if output_row_count > dataset.row_count {
        let _ = fs::remove_file(&output_path);
        return Err(
            "La limpieza source-backed aumentó inesperadamente el conteo de filas.".to_owned(),
        );
    }

    let output_columns = output_schema
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>();
    let current_columns = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>();
    let schema_changed = output_columns != current_columns;
    let affected_row_count = dataset.row_count.saturating_sub(output_row_count);
    let page = if output_row_count == 0 {
        output_schema.slice(0, 0)
    } else {
        collect_lazy_frame_streaming(
            parquet_scan(&output_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa de la limpieza source-backed",
        )?
    };
    cancellation.ensure()?;
    let (_, source_size_after, _) = validate_dataset_file(&original_source_path)?;
    if source_size_before != source_size_after {
        let _ = fs::remove_file(&output_path);
        return Err("El archivo source-backed cambió durante la limpieza.".to_owned());
    }

    if !force_publish && affected_row_count == 0 && !schema_changed {
        let _ = fs::remove_file(&output_path);
        return Ok(Some(DatasetMutation {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
        }));
    }

    let preview = dataset_preview_from_schema_and_page(
        &dataset.file_name,
        output_size,
        output_row_count,
        &output_schema,
        &page,
    )?;
    cancellation.ensure()?;
    let baseline_snapshot = if dataset.history.snapshots_enabled {
        None
    } else {
        let cancellation_for_baseline = cancellation.clone();
        let is_cancelled = move || cancellation_for_baseline.is_cancelled();
        Some(prepare_source_backed_baseline_snapshot(
            &dataset.history,
            source_path,
            source_format,
            is_cancelled,
        )?)
    };
    let prepared_output = dataset
        .history
        .prepare_parquet_snapshot(&output_path, || cancellation.is_cancelled())?;
    cancellation.ensure()?;
    let previous_source_path = dataset.source_path.clone();
    let previous_source_snapshot = dataset.history.source_snapshot_path.clone();
    let mut retired_paths = Vec::new();
    let mut keep_output = false;
    let committed = cancellation.commit(|| {
        let history_commit = commit_source_backed_recipe_history(
            &mut dataset.history,
            baseline_snapshot,
            prepared_output,
            label,
        )?;
        retired_paths = history_commit.retired_paths;
        let (current_path, current_size) = history_commit.active_snapshot.unwrap_or_else(|| {
            keep_output = true;
            (output_path.clone(), output_size)
        });
        dataset.source_path = Some(current_path);
        dataset.file_size_bytes = current_size;
        dataset.row_count = output_row_count;
        dataset.frame = output_schema;
        dataset.source_backed = true;
        dataset.history.source_snapshot_path = None;
        dataset.history.current_label = label.to_owned();
        dataset.profile = None;
        Ok(Some(DatasetMutation {
            dataset: preview,
            affected_row_count,
        }))
    })?;
    if committed.is_some() {
        for path in retired_paths {
            if !dataset
                .history
                .entries
                .iter()
                .any(|entry| entry.path == path)
                && previous_source_snapshot.as_deref() != Some(path.as_path())
                && previous_source_path.as_deref() != Some(path.as_path())
            {
                let _ = fs::remove_file(path);
            }
        }
        for path in [previous_source_path, previous_source_snapshot]
            .into_iter()
            .flatten()
        {
            if path.parent() == Some(dataset.history.directory.path())
                && !dataset
                    .history
                    .entries
                    .iter()
                    .any(|entry| entry.path == path)
            {
                let _ = fs::remove_file(path);
            }
        }
    }
    if keep_output {
        output_guard.keep();
    }
    Ok(committed)
}

#[cfg(test)]
fn publish_source_backed_result_output(
    dataset: &mut LoadedDataset,
    context: &SourceBackedJoinContext,
    output: SourceBackedResultOutput<'_>,
) -> Result<Option<DatasetPreview>, String> {
    let _output_guard = SourceBackedJoinOutputGuard::new(output.output_path);
    let original_source_path = if context.snapshot_only {
        let Some(current_entry) = dataset.history.entries.get(dataset.history.cursor) else {
            let _ = fs::remove_file(output.output_path);
            return Err("El snapshot activo ya no está disponible.".to_owned());
        };
        let current_entry_path = fs::canonicalize(&current_entry.path)
            .map_err(|_| "El snapshot activo ya no está disponible.".to_owned())?;
        if current_entry_path != context.source_path {
            let _ = fs::remove_file(output.output_path);
            return Err(
                "El dataset activo cambió durante la preparación source-backed.".to_owned(),
            );
        }
        None
    } else {
        let Some(original_source_path) = dataset.source_path.as_ref() else {
            let _ = fs::remove_file(output.output_path);
            return Err("La fuente source-backed ya no está disponible.".to_owned());
        };
        if original_source_path != &context.original_source_path
            || dataset.file_size_bytes != context.original_file_size_bytes
        {
            let _ = fs::remove_file(output.output_path);
            return Err(
                "El dataset activo cambió durante la preparación source-backed.".to_owned(),
            );
        }
        Some(original_source_path)
    };

    if let Some(original_source_path) = original_source_path {
        let (_, original_source_size_before, _) = validate_dataset_file(original_source_path)?;
        if original_source_size_before != context.original_file_size_bytes {
            let _ = fs::remove_file(output.output_path);
            return Err(
                "El archivo source-backed cambió antes de aplicar la operación.".to_owned(),
            );
        }
    }
    let (_, source_size_before, _) = validate_dataset_file(&context.source_path)?;
    if source_size_before != context.source_size_bytes {
        let _ = fs::remove_file(output.output_path);
        return Err("La fuente source-backed cambió antes de ejecutar la operación.".to_owned());
    }
    let (_, compared_size_before, _) = validate_dataset_file(output.compared_path)?;
    if compared_size_before != output.compared_size_bytes {
        let _ = fs::remove_file(output.output_path);
        return Err("El dataset comparado cambió antes de aplicar la operación.".to_owned());
    }

    let output_size = fs::metadata(output.output_path)
        .map_err(|error| format!("No se pudo verificar la salida source-backed: {error}"))?
        .len();
    let output_schema = read_parquet_schema_frame(output.output_path)?;
    let verified_row_count = crate::duckdb_query::count_file_rows(
        output.output_path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        || false,
    )?;
    if verified_row_count != output.output_row_count {
        let _ = fs::remove_file(output.output_path);
        return Err("El conteo del resultado source-backed no coincide con DuckDB.".to_owned());
    }
    let page = if output.output_row_count == 0 {
        output_schema.slice(0, 0)
    } else {
        collect_lazy_frame_streaming(
            parquet_scan(output.output_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa source-backed",
        )?
    };

    let original_source_size_after = original_source_path
        .map(|path| validate_dataset_file(path).map(|(_, size, _)| size))
        .transpose()?;
    let (_, source_size_after, _) = validate_dataset_file(&context.source_path)?;
    let (_, compared_size_after, _) = validate_dataset_file(output.compared_path)?;
    if original_source_size_after.is_some_and(|size| size != context.original_file_size_bytes)
        || source_size_after != context.source_size_bytes
        || compared_size_after != output.compared_size_bytes
    {
        let _ = fs::remove_file(output.output_path);
        return Err("Una fuente cambió durante la ejecución source-backed.".to_owned());
    }

    let preview = dataset_preview_from_schema_and_page(
        output.file_name,
        output_size,
        output.output_row_count,
        &output_schema,
        &page,
    )?;
    if !context.snapshot_only
        && !initialize_source_backed_history(dataset, &context.source_path, context.source_format)?
    {
        let _ = fs::remove_file(output.output_path);
        return Ok(None);
    }
    dataset
        .history
        .record_parquet(output.output_path, output.label)?;
    if !dataset.history.snapshots_enabled {
        let _ = fs::remove_file(output.output_path);
        return Ok(None);
    }
    let current_path = dataset
        .history
        .entries
        .last()
        .map(|entry| entry.path.clone())
        .ok_or_else(|| "No se pudo publicar el resultado source-backed.".to_owned())?;
    let current_size = fs::metadata(&current_path)
        .map_err(|error| format!("No se pudo verificar el historial source-backed: {error}"))?
        .len();
    let _ = fs::remove_file(output.output_path);
    // The result is now backed by the newly published history snapshot in both
    // cases. Snapshot-backed joins used to leave only a schema in `frame` while
    // marking the dataset as materialized; the next operation then treated the
    // empty schema frame as the complete dataset and lost all active rows.
    // Keep the cursor explicit so the next operation can materialize this exact
    // snapshot on demand, without reading the original source again.
    dataset.source_path = Some(current_path);
    dataset.file_name = output.file_name.to_owned();
    dataset.file_size_bytes = current_size;
    dataset.row_count = output.output_row_count;
    dataset.frame = output_schema;
    dataset.source_backed = true;
    dataset.history.source_snapshot_path = None;
    dataset.history.current_label = output.label.to_owned();
    dataset.profile = None;
    Ok(Some(DatasetPreview {
        file_size_bytes: current_size,
        ..preview
    }))
}

#[allow(clippy::too_many_arguments)]
fn publish_review_source_backed_result_output(
    dataset: &mut LoadedDataset,
    comparison: &mut Option<PendingComparison>,
    context: &SourceBackedJoinContext,
    expected_stamp: &DatasetMutationStamp,
    output: SourceBackedResultOutput<'_>,
    expected_comparison_path: Option<&Path>,
    expected_comparison_must_be_absent: bool,
    cancellation: &ReviewMutationCancellation,
) -> Result<Option<DatasetPreview>, String> {
    let _output_guard = SourceBackedJoinOutputGuard::new(output.output_path);
    cancellation.ensure()?;
    if !expected_stamp.matches(dataset) {
        return Err("El dataset activo cambió durante la operación de Review.".to_owned());
    }
    if let Some(expected_path) = expected_comparison_path {
        let matches = comparison
            .as_ref()
            .is_some_and(|pending| pending.snapshot_path == expected_path);
        if !matches {
            return Err("La comparación cambió durante la operación de Review.".to_owned());
        }
    } else if expected_comparison_must_be_absent && comparison.is_some() {
        return Err("La comparación cambió durante la operación de Review.".to_owned());
    }

    let original_source_path = if context.snapshot_only {
        let Some(current_entry) = dataset.history.entries.get(dataset.history.cursor) else {
            return Err("El snapshot activo ya no está disponible.".to_owned());
        };
        let current_entry_path = fs::canonicalize(&current_entry.path)
            .map_err(|_| "El snapshot activo ya no está disponible.".to_owned())?;
        if current_entry_path != context.source_path {
            return Err(
                "El dataset activo cambió durante la preparación source-backed.".to_owned(),
            );
        }
        None
    } else {
        let Some(original_source_path) = dataset.source_path.as_ref() else {
            return Err("La fuente source-backed ya no está disponible.".to_owned());
        };
        if original_source_path != &context.original_source_path
            || dataset.file_size_bytes != context.original_file_size_bytes
        {
            return Err(
                "El dataset activo cambió durante la preparación source-backed.".to_owned(),
            );
        }
        Some(original_source_path)
    };

    if let Some(original_source_path) = original_source_path {
        let (_, original_source_size_before, _) = validate_dataset_file(original_source_path)?;
        if original_source_size_before != context.original_file_size_bytes {
            return Err(
                "El archivo source-backed cambió antes de aplicar la operación.".to_owned(),
            );
        }
    }
    let (_, source_size_before, _) = validate_dataset_file(&context.source_path)?;
    if source_size_before != context.source_size_bytes {
        return Err("La fuente source-backed cambió antes de ejecutar la operación.".to_owned());
    }
    let (_, compared_size_before, _) = validate_dataset_file(output.compared_path)?;
    if compared_size_before != output.compared_size_bytes {
        return Err("El dataset comparado cambió antes de aplicar la operación.".to_owned());
    }

    cancellation.ensure()?;
    let output_size = fs::metadata(output.output_path)
        .map_err(|error| format!("No se pudo verificar la salida source-backed: {error}"))?
        .len();
    let output_schema = read_parquet_schema_frame(output.output_path)?;
    cancellation.ensure()?;
    let verified_row_count = crate::duckdb_query::count_file_rows(
        output.output_path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        cancellation.callback(),
    )?;
    if verified_row_count != output.output_row_count {
        return Err("El conteo del resultado source-backed no coincide con DuckDB.".to_owned());
    }
    let page = if output.output_row_count == 0 {
        output_schema.slice(0, 0)
    } else {
        cancellation.ensure()?;
        let page = collect_lazy_frame_streaming(
            parquet_scan(output.output_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa source-backed",
        )?;
        cancellation.ensure()?;
        page
    };

    let original_source_size_after = original_source_path
        .map(|path| validate_dataset_file(path).map(|(_, size, _)| size))
        .transpose()?;
    let (_, source_size_after, _) = validate_dataset_file(&context.source_path)?;
    let (_, compared_size_after, _) = validate_dataset_file(output.compared_path)?;
    if original_source_size_after.is_some_and(|size| size != context.original_file_size_bytes)
        || source_size_after != context.source_size_bytes
        || compared_size_after != output.compared_size_bytes
    {
        return Err("Una fuente cambió durante la ejecución source-backed.".to_owned());
    }

    let preview = dataset_preview_from_schema_and_page(
        output.file_name,
        output_size,
        output.output_row_count,
        &output_schema,
        &page,
    )?;
    cancellation.ensure()?;

    let baseline = if !context.snapshot_only && !dataset.history.snapshots_enabled {
        let cancellation_for_baseline = cancellation.clone();
        let is_cancelled = move || cancellation_for_baseline.is_cancelled();
        Some(prepare_source_backed_baseline_snapshot(
            &dataset.history,
            &context.source_path,
            context.source_format,
            is_cancelled,
        )?)
    } else {
        None
    };
    let prepared_output = dataset
        .history
        .prepare_parquet_snapshot(output.output_path, || cancellation.is_cancelled())?;
    if baseline
        .as_ref()
        .is_some_and(|snapshot| snapshot.bytes > dataset.history.disk_budget_bytes)
        || prepared_output.bytes > dataset.history.disk_budget_bytes
    {
        // Let the caller use the eager route if history cannot safely retain a
        // reversible source-backed cursor. No live history state changed.
        return Ok(None);
    }

    let history_commit = cancellation.commit(|| {
        let history_commit = commit_source_backed_recipe_history(
            &mut dataset.history,
            baseline,
            prepared_output,
            output.label,
        )?;
        let Some((current_path, current_size)) = history_commit.active_snapshot.clone() else {
            return Err("No se pudo preparar el historial source-backed.".to_owned());
        };
        dataset.source_path = Some(current_path);
        dataset.file_name = output.file_name.to_owned();
        dataset.file_size_bytes = current_size;
        dataset.row_count = output.output_row_count;
        dataset.frame = output_schema;
        dataset.source_backed = true;
        dataset.history.source_snapshot_path = None;
        dataset.profile = None;
        *comparison = None;
        Ok(history_commit)
    })?;

    for path in history_commit.retired_paths {
        if !dataset
            .history
            .entries
            .iter()
            .any(|entry| entry.path == path)
        {
            let _ = fs::remove_file(path);
        }
    }
    let current_size = dataset.file_size_bytes;
    Ok(Some(DatasetPreview {
        file_size_bytes: current_size,
        ..preview
    }))
}

#[allow(clippy::too_many_arguments)]
fn publish_review_eager_candidate(
    state: &DatasetState,
    expected_stamp: &DatasetMutationStamp,
    expected_comparison_path: Option<&Path>,
    expected_comparison_must_be_absent: bool,
    mut candidate: DataFrame,
    file_name: &str,
    file_size_bytes: u64,
    label: &str,
    cancellation: &ReviewMutationCancellation,
) -> Result<DatasetPreview, String> {
    cancellation.ensure()?;
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if !expected_stamp.matches(dataset) {
        return Err("El dataset activo cambió durante la operación de Review.".to_owned());
    }
    let mut comparison = state.comparison.lock_recovering();
    if let Some(expected_path) = expected_comparison_path {
        if !comparison
            .as_ref()
            .is_some_and(|pending| pending.snapshot_path == expected_path)
        {
            return Err("La comparación cambió durante la operación de Review.".to_owned());
        }
    } else if expected_comparison_must_be_absent && comparison.is_some() {
        return Err("La comparación cambió durante la operación de Review.".to_owned());
    }

    if dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios")
    {
        append_audit_label(&mut candidate, label)?;
    }
    let preview = dataset_preview_with_size(file_name, file_size_bytes, &candidate)?;
    let prepared_snapshot = if dataset.history.snapshots_enabled {
        Some(
            dataset
                .history
                .prepare_frame_snapshot(&candidate, || cancellation.is_cancelled())?,
        )
    } else {
        None
    };
    cancellation.ensure()?;

    let previous_source_path = dataset.source_path.clone();
    let previous_source_snapshot = dataset.history.source_snapshot_path.clone();
    let mut retired_paths = Vec::new();
    cancellation.commit(|| {
        if let Some(prepared_snapshot) = prepared_snapshot {
            retired_paths = dataset
                .history
                .record_prepared_snapshot(prepared_snapshot, label)?;
        } else {
            dataset.history.current_label = label.to_owned();
        }
        dataset.source_path = None;
        dataset.history.source_snapshot_path = None;
        dataset.file_name = file_name.to_owned();
        dataset.file_size_bytes = file_size_bytes;
        dataset.row_count = candidate.height();
        dataset.frame = candidate;
        dataset.source_backed = false;
        dataset.profile = None;
        *comparison = None;
        Ok(())
    })?;

    for path in retired_paths {
        if !dataset
            .history
            .entries
            .iter()
            .any(|entry| entry.path == path)
            && previous_source_snapshot.as_deref() != Some(path.as_path())
        {
            let _ = fs::remove_file(path);
        }
    }
    for path in [previous_source_path, previous_source_snapshot]
        .into_iter()
        .flatten()
    {
        if path.parent() == Some(dataset.history.directory.path())
            && dataset.source_path.as_deref() != Some(path.as_path())
            && !dataset
                .history
                .entries
                .iter()
                .any(|entry| entry.path == path)
        {
            let _ = fs::remove_file(path);
        }
    }
    Ok(preview)
}

fn join_source_backed_dataset(
    state: &DatasetState,
    request: SourceBackedJoinRequest,
    expected_stamp: &DatasetMutationStamp,
    expected_comparison_path: Option<&Path>,
    expected_comparison_must_be_absent: bool,
    cancellation: &ReviewMutationCancellation,
) -> Result<Option<DatasetPreview>, String> {
    let SourceBackedJoinRequest {
        context,
        compared_path,
        compared_format,
        compared_schema,
        compared_file_name,
        compared_size_bytes,
        key_columns,
        join_type,
    } = request;
    cancellation.ensure()?;
    let (
        _joined_schema,
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ) = source_backed_join_plan(
        &context.schema,
        &compared_schema,
        &key_columns,
        join_type,
        context.row_count,
    )?;
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .map_err(|error| {
            format!("No se pudo preparar la salida del JOIN source-backed: {error}")
        })?;
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let _output_guard = SourceBackedJoinOutputGuard::new(&output_path);
    let output_row_count =
        match crate::duckdb_query::materialize_file_sources_query_to_parquet_with_cancel(
            crate::duckdb_query::DuckDbFileSourcesQuery {
                current_path: &context.source_path,
                current_format: context.source_format,
                compared_path: &compared_path,
                compared_format,
                dataset_view_query: &dataset_view_query,
                query: &output_query,
                destination: &output_path,
                current_order_column: &current_order_column,
                compared_order_column: &compared_order_column,
                max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
            },
            cancellation.callback(),
        ) {
            Ok(row_count) => row_count,
            Err(error) if error.contains("supera el límite local") => {
                let _ = fs::remove_file(&output_path);
                return Err(error);
            }
            Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
            Err(_) => {
                let _ = fs::remove_file(&output_path);
                return Ok(None);
            }
        };

    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        let _ = fs::remove_file(&output_path);
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let file_name = format!(
        "Join {} · {} + {compared_file_name}",
        join_type.label(),
        context.file_name
    );
    let label = format!("Unir datasets ({})", join_type.label());
    let mut comparison = state.comparison.lock_recovering();
    let preview = publish_review_source_backed_result_output(
        dataset,
        &mut comparison,
        &context,
        expected_stamp,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes,
            output_path: &output_path,
            output_row_count,
            file_name: &file_name,
            label: &label,
        },
        expected_comparison_path,
        expected_comparison_must_be_absent,
        cancellation,
    )?;
    Ok(preview)
}

fn consolidate_source_backed_dataset(
    state: &DatasetState,
    request: SourceBackedConsolidationRequest,
    expected_stamp: &DatasetMutationStamp,
    expected_comparison_path: &Path,
    cancellation: &ReviewMutationCancellation,
) -> Result<Option<DatasetPreview>, String> {
    let SourceBackedConsolidationRequest {
        context,
        compared_path,
        compared_format,
        compared_schema,
        compared_file_name,
        compared_size_bytes,
        key_columns,
    } = request;
    cancellation.ensure()?;
    let (_, current_size_before, _) = validate_dataset_file(&context.source_path)?;
    if current_size_before != context.source_size_bytes {
        return Err(
            "La fuente source-backed cambió antes de ejecutar la consolidación.".to_owned(),
        );
    }
    let (_, compared_size_before, _) = validate_dataset_file(&compared_path)?;
    if compared_size_before != compared_size_bytes {
        return Err("El dataset comparado cambió antes de consolidar.".to_owned());
    }
    let (dataset_view_query, output_query, current_order_column, compared_order_column) =
        source_backed_consolidation_plan(
            &context.schema,
            &compared_schema,
            &key_columns,
            context.row_count,
        )?;
    let cancellation_for_validation = cancellation.clone();
    let is_cancelled = move || cancellation_for_validation.is_cancelled();
    match validate_source_backed_consolidation_with_cancellation(
        &context.source_path,
        context.source_format,
        &compared_path,
        compared_format,
        &context.schema,
        &key_columns,
        is_cancelled,
    ) {
        Ok(()) => {}
        Err(error) if error == SOURCE_BACKED_CONSOLIDATION_CONFLICT_ERROR => return Err(error),
        Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
        Err(_) => return Ok(None),
    }
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .map_err(|error| {
            format!("No se pudo preparar la salida de la consolidación source-backed: {error}")
        })?;
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let _output_guard = SourceBackedJoinOutputGuard::new(&output_path);
    let output_row_count =
        match crate::duckdb_query::materialize_file_sources_query_to_parquet_with_cancel(
            crate::duckdb_query::DuckDbFileSourcesQuery {
                current_path: &context.source_path,
                current_format: context.source_format,
                compared_path: &compared_path,
                compared_format,
                dataset_view_query: &dataset_view_query,
                query: &output_query,
                destination: &output_path,
                current_order_column: &current_order_column,
                compared_order_column: &compared_order_column,
                max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
            },
            cancellation.callback(),
        ) {
            Ok(row_count) => row_count,
            Err(error) if error.contains("supera el límite local") => return Err(error),
            Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
            Err(_) => return Ok(None),
        };

    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if !expected_stamp.matches(dataset) {
        return Err("El dataset activo cambió durante la consolidación.".to_owned());
    }
    let mut comparison = state.comparison.lock_recovering();
    let file_name = format!("Consolidado · {} + {compared_file_name}", context.file_name);
    let preview = publish_review_source_backed_result_output(
        dataset,
        &mut comparison,
        &context,
        expected_stamp,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes,
            output_path: &output_path,
            output_row_count,
            file_name: &file_name,
            label: "Consolidar datasets",
        },
        Some(expected_comparison_path),
        false,
        cancellation,
    )?;
    Ok(preview)
}

fn resolve_source_backed_conflicts(
    state: &DatasetState,
    request: SourceBackedConflictResolutionRequest,
    expected_stamp: &DatasetMutationStamp,
    expected_comparison_path: &Path,
    cancellation: &ReviewMutationCancellation,
) -> Result<Option<DatasetPreview>, String> {
    let SourceBackedConflictResolutionRequest {
        context,
        compared_path,
        compared_size_bytes,
        compared_row_count,
        compared_schema,
        compared_file_name,
        key_columns,
        decisions,
    } = request;
    cancellation.ensure()?;
    let (_, source_size_before, _) = validate_dataset_file(&context.source_path)?;
    if source_size_before != context.source_size_bytes {
        return Err("La fuente source-backed cambió antes de resolver conflictos.".to_owned());
    }
    let (_, compared_size_before, _) = validate_dataset_file(&compared_path)?;
    if compared_size_before != compared_size_bytes {
        return Err("El dataset comparado cambió antes de resolver conflictos.".to_owned());
    }
    let Some((current_path, current_row_count, _current_snapshot_directory)) =
        source_backed_parquet_snapshot_with_cancel(&context, cancellation.callback())?
    else {
        return Ok(None);
    };
    cancellation.ensure()?;
    let current_schema = read_parquet_schema_frame(&current_path)?;
    if current_schema.get_column_names() != context.schema.get_column_names()
        || current_schema
            .columns()
            .iter()
            .zip(context.schema.columns())
            .any(|(left, right)| left.dtype() != right.dtype())
    {
        return Err("El esquema source-backed cambió antes de resolver conflictos.".to_owned());
    }
    let shared_columns = current_schema
        .get_column_names()
        .iter()
        .filter(|name| compared_schema.get_column_index(name).is_some())
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let is_cancelled = cancellation.callback();
    let Some(choices) = validate_source_backed_conflict_decisions(
        &current_path,
        current_row_count,
        &compared_path,
        compared_row_count,
        &key_columns,
        &shared_columns,
        &decisions,
        &is_cancelled,
    )?
    else {
        return Ok(None);
    };
    let (dataset_view_query, output_query, current_order_column, compared_order_column) =
        source_backed_conflict_resolution_plan(
            &current_schema,
            &compared_schema,
            &key_columns,
            &choices,
        )?;
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .map_err(|error| {
            format!("No se pudo preparar la salida de conflictos source-backed: {error}")
        })?;
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let _output_guard = SourceBackedJoinOutputGuard::new(&output_path);
    cancellation.ensure()?;
    let output_row_count =
        match crate::duckdb_query::materialize_file_sources_query_to_parquet_with_cancel(
            crate::duckdb_query::DuckDbFileSourcesQuery {
                current_path: &current_path,
                current_format: crate::duckdb_query::DuckDbFileFormat::Parquet,
                compared_path: &compared_path,
                compared_format: crate::duckdb_query::DuckDbFileFormat::Parquet,
                dataset_view_query: &dataset_view_query,
                query: &output_query,
                destination: &output_path,
                current_order_column: &current_order_column,
                compared_order_column: &compared_order_column,
                max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
            },
            cancellation.callback(),
        ) {
            Ok(row_count) => row_count,
            Err(error) => {
                cancellation.ensure()?;
                let _ = error;
                return Ok(None);
            }
        };
    cancellation.ensure()?;
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let mut comparison = state.comparison.lock_recovering();
    let file_name = format!("Resuelto · {} + {compared_file_name}", context.file_name);
    let preview = publish_review_source_backed_result_output(
        dataset,
        &mut comparison,
        &context,
        expected_stamp,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes,
            output_path: &output_path,
            output_row_count,
            file_name: &file_name,
            label: "Resolver conflictos por clave",
        },
        Some(expected_comparison_path),
        false,
        cancellation,
    )?;
    Ok(preview)
}

#[cfg(test)]
fn remove_empty_rows_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<DatasetMutation>, String> {
    let cancellation = PrepareCancellation::disabled();
    remove_empty_rows_source_backed_with_cancellation(dataset, &cancellation)
}

fn remove_empty_rows_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<DatasetMutation>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let projection =
        source_backed_empty_row_projection(&dataset.frame, "Eliminar filas completamente vacías")?;
    let query = source_backed_empty_row_query(&dataset.frame)?;
    let query = query.replacen("SELECT *", &format!("SELECT {projection}"), 1);
    publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Eliminar filas completamente vacías",
        false,
        cancellation.clone(),
    )
}

#[derive(Clone, Copy)]
enum SourceBackedColumnCleanup {
    Constant,
    Empty,
    HighNull,
    Identifier,
    Personal,
}

fn source_backed_removable_columns(
    dataset: &LoadedDataset,
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    cleanup: SourceBackedColumnCleanup,
    cancellation: &PrepareCancellation,
) -> Result<Vec<String>, String> {
    if dataset.row_count == 0 || dataset.frame.width() <= 1 {
        return Ok(Vec::new());
    }
    let columns = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let needs_null_counts = matches!(
        cleanup,
        SourceBackedColumnCleanup::Constant
            | SourceBackedColumnCleanup::Empty
            | SourceBackedColumnCleanup::HighNull
    );
    let null_counts = if needs_null_counts {
        Some(crate::duckdb_query::count_file_nulls(
            source_path,
            source_format,
            &columns,
            cancellation.callback(),
        )?)
    } else {
        None
    };
    let distinct_counts = if matches!(cleanup, SourceBackedColumnCleanup::Constant) {
        Some(crate::duckdb_query::count_file_distinct_non_null(
            source_path,
            source_format,
            &columns,
            cancellation.callback(),
        )?)
    } else {
        None
    };
    let mut candidates = columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            if column == "_cambios" {
                return None;
            }
            let null_count = null_counts
                .as_ref()
                .and_then(|counts| counts.get(index))
                .copied()
                .unwrap_or_default();
            let removable = match cleanup {
                SourceBackedColumnCleanup::Constant => {
                    distinct_counts.as_ref()?.get(index).copied()? <= 1
                        && null_count < dataset.row_count
                }
                SourceBackedColumnCleanup::Empty => null_count == dataset.row_count,
                SourceBackedColumnCleanup::HighNull => {
                    null_count > 0
                        && null_count < dataset.row_count
                        && null_count.saturating_mul(100)
                            >= dataset
                                .row_count
                                .saturating_mul(HIGH_NULL_COLUMN_THRESHOLD_PERCENTAGE)
                }
                SourceBackedColumnCleanup::Identifier => {
                    matches!(privacy_signal(column), Some("identifier"))
                }
                SourceBackedColumnCleanup::Personal => is_personal_privacy_signal(column),
            };
            removable.then(|| column.clone())
        })
        .collect::<Vec<_>>();
    candidates.truncate(dataset.frame.width().saturating_sub(1));
    Ok(candidates)
}

#[cfg(test)]
fn remove_duplicates_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<DatasetMutation>, String> {
    let cancellation = PrepareCancellation::disabled();
    remove_duplicates_source_backed_with_cancellation(dataset, &cancellation)
}

fn remove_duplicates_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<DatasetMutation>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let projection =
        source_backed_empty_row_projection(&dataset.frame, "Eliminar filas duplicadas")?;
    let query = source_backed_duplicate_query(&dataset.frame, &projection)?;
    publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Eliminar filas duplicadas",
        false,
        cancellation.clone(),
    )
}

#[cfg(test)]
fn remove_near_duplicates_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<DatasetMutation>, String> {
    let cancellation = PrepareCancellation::disabled();
    remove_near_duplicates_source_backed_with_cancellation(dataset, &cancellation)
}

fn remove_near_duplicates_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<DatasetMutation>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let projection =
        source_backed_empty_row_projection(&dataset.frame, "Eliminar filas duplicadas parecidas")?;
    let query = source_backed_near_duplicate_query(&dataset.frame, &projection)?;
    publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Eliminar filas duplicadas parecidas",
        false,
        cancellation.clone(),
    )
}

#[cfg(test)]
fn remove_columns_source_backed(
    dataset: &mut LoadedDataset,
    cleanup: SourceBackedColumnCleanup,
    label: &str,
) -> Result<Option<ColumnRemovalResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    remove_columns_source_backed_with_cancellation(dataset, cleanup, label, &cancellation)
}

fn remove_columns_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cleanup: SourceBackedColumnCleanup,
    label: &str,
    cancellation: &PrepareCancellation,
) -> Result<Option<ColumnRemovalResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let removed_columns = match source_backed_removable_columns(
        dataset,
        &source_path,
        source_format,
        cleanup,
        cancellation,
    ) {
        Ok(columns) => columns,
        Err(_) => return Ok(None),
    };
    if removed_columns.is_empty() {
        return Ok(Some(ColumnRemovalResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            removed_column_count: 0,
            removed_columns,
        }));
    }
    let remaining_columns = dataset
        .frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let projection = source_backed_projection(&remaining_columns, label)?;
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        label,
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(ColumnRemovalResult {
        dataset: mutation.dataset,
        removed_column_count: removed_columns.len(),
        removed_columns,
    }))
}

fn source_backed_mask_projection(
    schema: &DataFrame,
    personal_columns: &[String],
    audit_label: &str,
) -> Result<String, String> {
    if schema.width() == 0 {
        return Err("La fuente source-backed no contiene columnas utilizables.".to_owned());
    }
    let audit_literal = duckdb_string_literal(audit_label);
    let mask_literal = duckdb_string_literal(REDACTED_VALUE);
    Ok(schema
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name.as_str() == "_cambios" {
                format!(
                    "CASE WHEN {identifier} IS NULL OR TRIM(CAST({identifier} AS VARCHAR)) = '' THEN {audit_literal} ELSE LEFT(CAST({identifier} AS VARCHAR) || '; ' || {audit_literal}, {MAX_AUDIT_CELL_CHARS}) END AS {identifier}"
                )
            } else if personal_columns.iter().any(|column| column == name.as_str()) {
                format!(
                    "CASE WHEN {identifier} IS NULL THEN NULL ELSE {mask_literal} END AS {identifier}"
                )
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
}

#[cfg(test)]
fn mask_personal_values_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<PersonalDataMaskResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    mask_personal_values_source_backed_with_cancellation(dataset, &cancellation)
}

fn mask_personal_values_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<PersonalDataMaskResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let personal_columns = dataset
        .frame
        .get_column_names()
        .iter()
        .filter(|name| name.as_str() != "_cambios" && is_personal_privacy_signal(name))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    if personal_columns.is_empty() {
        return Ok(Some(PersonalDataMaskResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            changed_cell_count: 0,
            changed_column_count: 0,
        }));
    }
    let changed_counts = match crate::duckdb_query::count_file_values_not_equal(
        &source_path,
        source_format,
        &personal_columns,
        REDACTED_VALUE,
        cancellation.callback(),
    ) {
        Ok(counts) => counts,
        Err(_) => return Ok(None),
    };
    let changed_cell_count = changed_counts.iter().copied().sum::<usize>();
    let changed_column_count = changed_counts.iter().filter(|count| **count > 0).count();
    if changed_cell_count == 0 {
        return Ok(Some(PersonalDataMaskResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            changed_cell_count: 0,
            changed_column_count: 0,
        }));
    }
    let projection = source_backed_mask_projection(
        &dataset.frame,
        &personal_columns,
        "Proteger valores personales detectados",
    )?;
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Proteger valores personales detectados",
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(PersonalDataMaskResult {
        dataset: mutation.dataset,
        changed_cell_count,
        changed_column_count,
    }))
}

fn source_backed_rename_projection(
    schema: &DataFrame,
    names: &[String],
    audit_label: &str,
) -> Result<String, String> {
    if schema.width() != names.len() || names.is_empty() {
        return Err("La fuente source-backed no contiene un esquema renombrable.".to_owned());
    }
    let audit_literal = duckdb_string_literal(audit_label);
    Ok(schema
        .get_column_names()
        .iter()
        .zip(names)
        .map(|(original, renamed)| {
            let source = duckdb_identifier(original);
            let destination = duckdb_identifier(renamed);
            if original.as_str() == "_cambios" {
                format!(
                    "CASE WHEN {source} IS NULL OR TRIM(CAST({source} AS VARCHAR)) = '' THEN {audit_literal} ELSE LEFT(CAST({source} AS VARCHAR) || '; ' || {audit_literal}, {MAX_AUDIT_CELL_CHARS}) END AS {destination}"
                )
            } else if original.as_str() == renamed {
                source
            } else {
                format!("{source} AS {destination}")
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
}

#[cfg(test)]
fn normalize_column_names_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<ColumnNormalizationResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    normalize_column_names_source_backed_with_cancellation(dataset, &cancellation)
}

fn normalize_column_names_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<ColumnNormalizationResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let (names, renames) = normalized_column_names(&dataset.frame);
    if renames.is_empty() {
        return Ok(Some(ColumnNormalizationResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            renamed_column_count: 0,
            renames,
        }));
    }
    let projection =
        source_backed_rename_projection(&dataset.frame, &names, "Normalizar nombres de columnas")?;
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Normalizar nombres de columnas",
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(ColumnNormalizationResult {
        dataset: mutation.dataset,
        renamed_column_count: renames.len(),
        renames,
    }))
}

#[cfg(test)]
fn enable_row_audit_source_backed(
    dataset: &mut LoadedDataset,
) -> Result<Option<DatasetMutation>, String> {
    let cancellation = PrepareCancellation::disabled();
    enable_row_audit_source_backed_with_cancellation(dataset, &cancellation)
}

fn enable_row_audit_source_backed_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<DatasetMutation>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    if dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios")
    {
        return Ok(Some(DatasetMutation {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
        }));
    }
    let mut projection = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| duckdb_identifier(name))
        .collect::<Vec<_>>();
    projection.push(format!(
        "CAST(NULL AS VARCHAR) AS {}",
        duckdb_identifier("_cambios")
    ));
    let query = format!("SELECT {} FROM dataset", projection.join(", "));
    publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Activar trazabilidad por fila",
        true,
        cancellation.clone(),
    )
}

fn source_backed_normalized_text_expression(identifier: &str, remove_accents: bool) -> String {
    let collapsed = format!(
        "trim(regexp_replace(CAST({identifier} AS VARCHAR), {}, ' ', 'g'))",
        duckdb_string_literal(r"[\s\p{Z}]+"),
    );
    let lowered = format!("lower({collapsed})");
    if remove_accents {
        format!("strip_accents({lowered})")
    } else {
        lowered
    }
}

fn source_backed_safe_mojibake_pattern() -> String {
    let alternatives = SAFE_MOJIBAKE_REPLACEMENTS
        .iter()
        .map(|(source, _)| *source)
        .collect::<Vec<_>>()
        .join("|");
    format!("^([[:ascii:]]|{alternatives})*$")
}

fn source_backed_fix_encoding_expression(identifier: &str) -> String {
    let pattern = duckdb_string_literal(&source_backed_safe_mojibake_pattern());
    let replaced = SAFE_MOJIBAKE_REPLACEMENTS.iter().fold(
        identifier.to_owned(),
        |expression, (source, replacement)| {
            format!(
                "replace({expression}, {}, {})",
                duckdb_string_literal(source),
                duckdb_string_literal(replacement)
            )
        },
    );
    format!(
        "CASE WHEN regexp_matches(CAST({identifier} AS VARCHAR), {pattern}) THEN {replaced} ELSE {identifier} END"
    )
}

fn source_backed_fix_encoding_unsafe_predicate(identifier: &str) -> String {
    let marker_predicate = MOJIBAKE_MARKERS
        .iter()
        .map(|marker| {
            format!(
                "strpos(CAST({identifier} AS VARCHAR), {}) > 0",
                duckdb_string_literal(marker)
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let pattern = duckdb_string_literal(&source_backed_safe_mojibake_pattern());
    format!("({marker_predicate}) AND NOT regexp_matches(CAST({identifier} AS VARCHAR), {pattern})")
}

fn source_backed_supported_date_expression(identifier: &str) -> String {
    let expressions = INFERRED_DATE_FORMATS
        .iter()
        .copied()
        .filter_map(|format| source_backed_inferred_date_expression(identifier, format))
        .collect::<Vec<_>>();
    format!("COALESCE({})", expressions.join(", "))
}

fn source_backed_text_type_expressions(
    columns: &[String],
) -> Vec<crate::duckdb_query::FileTextTypeExpressions> {
    columns
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            let normalized = source_backed_normalized_text_expression(&identifier, true);
            let trimmed = format!("TRIM(CAST({identifier} AS VARCHAR))");
            let floating = format!("TRY_CAST({trimmed} AS DOUBLE)");
            let leading_zero = format!(
                "regexp_matches({trimmed}, {})",
                duckdb_string_literal(r"^[+-]?0[0-9]")
            );
            let no_decimal_or_exponent = format!(
                "strpos({trimmed}, '.') = 0 AND strpos(lower({trimmed}), 'e') = 0"
            );
            let integer = format!(
                "{trimmed} <> '' AND TRY_CAST({trimmed} AS BIGINT) IS NOT NULL AND NOT ({leading_zero}) AND {no_decimal_or_exponent} AND ABS({floating}) <= 9007199254740992"
            );
            let decimal = format!(
                "{trimmed} <> '' AND {floating} IS NOT NULL AND isfinite({floating}) AND NOT ({leading_zero}) AND ((strpos({trimmed}, '.') > 0 OR strpos(lower({trimmed}), 'e') > 0) OR ABS({floating}) <= 9007199254740992)"
            );
            crate::duckdb_query::FileTextTypeExpressions {
                column: name.clone(),
                normalized,
                integer,
                decimal,
                date: source_backed_supported_date_expression(&identifier),
            }
        })
        .collect()
}

fn source_backed_text_suggested_types(
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    columns: &[String],
    cancellation: &PrepareCancellation,
) -> Option<HashMap<String, &'static str>> {
    let expressions = source_backed_text_type_expressions(columns);
    let stats = crate::duckdb_query::count_file_text_type_stats(
        source_path,
        source_format,
        &expressions,
        cancellation.callback(),
    )
    .ok()?;
    Some(
        expressions
            .into_iter()
            .zip(stats)
            .filter_map(|(expression, stats)| {
                let (suggested_type, _, _) = suggest_text_type(
                    stats.non_empty_count,
                    stats.boolean_count,
                    stats.integer_count,
                    stats.decimal_count,
                    stats.date_count,
                );
                suggested_type.map(|suggested_type| (expression.column, suggested_type))
            })
            .collect(),
    )
}

fn source_backed_text_type_validity_expression(
    identifier: &str,
    suggested_type: &str,
) -> Option<String> {
    let trimmed = format!("TRIM(CAST({identifier} AS VARCHAR))");
    let valid = match suggested_type {
        "boolean" => {
            let normalized = source_backed_normalized_text_expression(identifier, true);
            format!("{normalized} IN ('true', 'yes', 'si', 'false', 'no')")
        }
        "integer" => {
            let floating = format!("TRY_CAST({trimmed} AS DOUBLE)");
            let leading_zero = format!(
                "regexp_matches({trimmed}, {})",
                duckdb_string_literal(r"^[+-]?0[0-9]")
            );
            format!(
                "TRY_CAST({trimmed} AS BIGINT) IS NOT NULL AND NOT ({leading_zero}) AND strpos({trimmed}, '.') = 0 AND strpos(lower({trimmed}), 'e') = 0 AND ABS({floating}) <= 9007199254740992"
            )
        }
        "decimal" => {
            let floating = format!("TRY_CAST({trimmed} AS DOUBLE)");
            let leading_zero = format!(
                "regexp_matches({trimmed}, {})",
                duckdb_string_literal(r"^[+-]?0[0-9]")
            );
            format!(
                "{floating} IS NOT NULL AND isfinite({floating}) AND NOT ({leading_zero}) AND ((strpos({trimmed}, '.') > 0 OR strpos(lower({trimmed}), 'e') > 0) OR ABS({floating}) <= 9007199254740992)"
            )
        }
        "date" => format!(
            "({}) IS NOT NULL",
            source_backed_supported_date_expression(identifier)
        ),
        _ => return None,
    };
    Some(valid)
}

fn source_backed_text_expression(
    identifier: &str,
    mode: TextCleaningMode,
    suggested_type: Option<&str>,
) -> Option<String> {
    match mode {
        TextCleaningMode::Trim => Some(format!(
            "trim(regexp_replace(CAST({identifier} AS VARCHAR), {}, '', 'g'))",
            duckdb_string_literal(r"^[\s\p{Z}]+|[\s\p{Z}]+$"),
        )),
        TextCleaningMode::Normalize { remove_accents } => Some(
            source_backed_normalized_text_expression(identifier, remove_accents),
        ),
        TextCleaningMode::Sentinels => {
            let normalized = source_backed_normalized_text_expression(identifier, true);
            let values = SENTINEL_VALUES
                .iter()
                .map(|value| duckdb_string_literal(value))
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!(
                "CASE WHEN {normalized} IN ({values}) THEN NULL ELSE {identifier} END"
            ))
        }
        TextCleaningMode::Booleans => {
            let normalized = source_backed_normalized_text_expression(identifier, true);
            Some(format!(
                "CASE WHEN {normalized} IN ('true', 'yes', 'si') THEN 'true' WHEN {normalized} IN ('false', 'no') THEN 'false' ELSE {identifier} END"
            ))
        }
        TextCleaningMode::FixEncoding => Some(source_backed_fix_encoding_expression(identifier)),
        TextCleaningMode::NullifyInvalidTypes => {
            let Some(suggested_type) = suggested_type else {
                return Some(identifier.to_owned());
            };
            let valid = source_backed_text_type_validity_expression(identifier, suggested_type)?;
            Some(format!(
                "CASE WHEN {identifier} IS NULL OR TRIM(CAST({identifier} AS VARCHAR)) = '' OR ({valid}) THEN {identifier} ELSE NULL END"
            ))
        }
    }
}

fn source_backed_text_cleaning_columns(
    dataset: &LoadedDataset,
    selected_columns: Option<&[String]>,
) -> Option<Vec<String>> {
    let columns = selected_columns.map_or_else(
        || {
            dataset
                .frame
                .columns()
                .iter()
                .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
                .map(|column| column.name().to_string())
                .collect::<Vec<_>>()
        },
        |columns| columns.to_vec(),
    );
    if selected_columns.is_some() && columns.is_empty() {
        return None;
    }
    if columns.iter().any(|name| {
        name == "_cambios"
            || dataset
                .frame
                .column(name)
                .map_or(true, |column| column.dtype() != &DataType::String)
    }) {
        return None;
    }
    Some(columns)
}

fn source_backed_boolean_candidate_columns(
    dataset: &LoadedDataset,
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    cancellation: &PrepareCancellation,
) -> Option<Vec<String>> {
    let columns = dataset
        .frame
        .columns()
        .iter()
        .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Some(Vec::new());
    }
    let normalized = columns
        .iter()
        .map(|name| {
            (
                name.clone(),
                source_backed_normalized_text_expression(&duckdb_identifier(name), true),
            )
        })
        .collect::<Vec<_>>();
    let counts = crate::duckdb_query::count_file_boolean_candidates(
        source_path,
        source_format,
        &normalized,
        cancellation.callback(),
    )
    .ok()?;
    Some(
        columns
            .into_iter()
            .zip(counts)
            .filter(|(_, (non_empty, recognized))| {
                *non_empty >= 3 && recognized.saturating_mul(100) >= non_empty.saturating_mul(90)
            })
            .map(|(name, _)| name)
            .collect(),
    )
}

fn source_backed_text_audit_expression(identifier: &str, audit_label: &str) -> String {
    let audit_literal = duckdb_string_literal(audit_label);
    format!(
        "CASE WHEN {identifier} IS NULL OR TRIM(CAST({identifier} AS VARCHAR)) = '' THEN {audit_literal} ELSE LEFT(CAST({identifier} AS VARCHAR) || '; ' || {audit_literal}, {MAX_AUDIT_CELL_CHARS}) END AS {identifier}"
    )
}

fn source_backed_text_cleaning_projection(
    schema: &DataFrame,
    selected_columns: &[String],
    mode: TextCleaningMode,
    audit_label: &str,
    suggested_types: &HashMap<String, &'static str>,
) -> Result<(String, Vec<(String, String)>), String> {
    let mut expressions = Vec::with_capacity(selected_columns.len());
    let mut projection = Vec::with_capacity(schema.width());
    for name in schema.get_column_names() {
        let identifier = duckdb_identifier(name);
        if name.as_str() == "_cambios" {
            projection.push(source_backed_text_audit_expression(
                &identifier,
                audit_label,
            ));
            continue;
        }
        let suggested_type = suggested_types.get(name.as_str()).copied();
        let Some(expression) = source_backed_text_expression(&identifier, mode, suggested_type)
        else {
            return Err(
                "La limpieza de texto source-backed no es compatible con este modo.".to_owned(),
            );
        };
        if selected_columns
            .iter()
            .any(|selected| selected == name.as_str())
        {
            projection.push(format!("{expression} AS {identifier}"));
        } else {
            projection.push(identifier.clone());
        }
    }
    for name in selected_columns {
        let identifier = duckdb_identifier(name);
        let suggested_type = suggested_types.get(name.as_str()).copied();
        let expression = source_backed_text_expression(&identifier, mode, suggested_type)
            .ok_or_else(|| {
                "La limpieza de texto source-backed no es compatible con este modo.".to_owned()
            })?;
        expressions.push((name.clone(), expression));
    }
    Ok((projection.join(", "), expressions))
}

#[cfg(test)]
fn source_backed_text_cleaning(
    dataset: &mut LoadedDataset,
    selected_columns: Option<&[String]>,
    mode: TextCleaningMode,
) -> Result<Option<TextCleaningResult>, String> {
    source_backed_text_cleaning_with_cancellation(
        dataset,
        selected_columns,
        mode,
        PrepareCancellation::disabled(),
    )
}

fn source_backed_text_cleaning_with_cancellation(
    dataset: &mut LoadedDataset,
    selected_columns: Option<&[String]>,
    mode: TextCleaningMode,
    cancellation: PrepareCancellation,
) -> Result<Option<TextCleaningResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let selected_columns = if matches!(mode, TextCleaningMode::Booleans) {
        if selected_columns.is_some() {
            return Ok(None);
        }
        let Some(columns) = source_backed_boolean_candidate_columns(
            dataset,
            &source_path,
            source_format,
            &cancellation,
        ) else {
            return Ok(None);
        };
        columns
    } else {
        let Some(columns) = source_backed_text_cleaning_columns(dataset, selected_columns) else {
            return Ok(None);
        };
        columns
    };
    if matches!(mode, TextCleaningMode::FixEncoding) && !selected_columns.is_empty() {
        let unsafe_predicates = selected_columns
            .iter()
            .map(|name| source_backed_fix_encoding_unsafe_predicate(&duckdb_identifier(name)))
            .collect::<Vec<_>>();
        let unsafe_counts = match crate::duckdb_query::count_file_predicate_matches(
            &source_path,
            source_format,
            &unsafe_predicates,
            cancellation.callback(),
        ) {
            Ok((_, counts)) => counts,
            Err(_) => return Ok(None),
        };
        if unsafe_counts.iter().any(|count| *count > 0) {
            return Ok(None);
        }
    }
    let suggested_types = if matches!(mode, TextCleaningMode::NullifyInvalidTypes) {
        let Some(suggested_types) = source_backed_text_suggested_types(
            &source_path,
            source_format,
            &selected_columns,
            &cancellation,
        ) else {
            return Ok(None);
        };
        suggested_types
    } else {
        HashMap::new()
    };
    let label = match mode {
        TextCleaningMode::Trim => "Recortar espacios",
        TextCleaningMode::Normalize { .. } => "Normalizar texto",
        TextCleaningMode::Sentinels => "Normalizar valores centinela",
        TextCleaningMode::Booleans => "Normalizar booleanos",
        TextCleaningMode::FixEncoding => "Corregir codificación UTF-8",
        TextCleaningMode::NullifyInvalidTypes => "Apartar tipos incompatibles",
    };
    if selected_columns.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let (projection, expressions) = source_backed_text_cleaning_projection(
        &dataset.frame,
        &selected_columns,
        mode,
        label,
        &suggested_types,
    )?;
    let (affected_row_count, changed_counts) =
        match crate::duckdb_query::count_file_expression_changes(
            &source_path,
            source_format,
            &expressions,
            cancellation.callback(),
        ) {
            Ok(counts) => counts,
            Err(_) => return Ok(None),
        };
    let changed_cell_count = changed_counts.iter().copied().sum::<usize>();
    let changed_columns = selected_columns
        .iter()
        .zip(changed_counts)
        .filter(|(_, changed_cell_count)| *changed_cell_count > 0)
        .map(|(name, changed_cell_count)| ChangedTextColumn {
            name: name.clone(),
            changed_cell_count,
        })
        .collect::<Vec<_>>();
    if changed_cell_count == 0 {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns,
        }));
    }
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        label,
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TextCleaningResult {
        dataset: mutation.dataset,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    }))
}

#[cfg(test)]
fn source_backed_numeric_cast(
    dataset: &mut LoadedDataset,
) -> Result<Option<TextCleaningResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    source_backed_numeric_cast_with_cancellation(dataset, &cancellation)
}

fn source_backed_numeric_cast_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<TextCleaningResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let columns = dataset
        .frame
        .columns()
        .iter()
        .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let stats = match crate::duckdb_query::count_file_numeric_candidate_stats(
        &source_path,
        source_format,
        &columns,
        cancellation.callback(),
    ) {
        Ok(stats) => stats,
        Err(_) => return Ok(None),
    };
    let mut conversions = Vec::<(String, String, String)>::new();
    for (name, stats) in columns.iter().zip(stats) {
        if stats.non_null_count == 0
            || privacy_signal(name) == Some("identifier")
            || stats.leading_zero_count > 0
        {
            continue;
        }
        let identifier = duckdb_identifier(name);
        let trimmed = format!("TRIM(CAST({identifier} AS VARCHAR))");
        if stats.decimal_token_count == 0
            && stats.integer_count.saturating_mul(10) > stats.non_null_count.saturating_mul(9)
        {
            let expression = format!("TRY_CAST({trimmed} AS BIGINT)");
            conversions.push((
                name.clone(),
                expression.clone(),
                format!("({expression}) IS NOT NULL"),
            ));
            continue;
        }
        if stats.float_count.saturating_mul(10) <= stats.non_null_count.saturating_mul(9) {
            continue;
        }
        if stats.precision_loss_count > 0 {
            return Err(format!(
                "La columna '{name}' contiene números que perderían precisión al convertirse."
            ));
        }
        let floating = format!("TRY_CAST({trimmed} AS DOUBLE)");
        let expression = format!(
            "CASE WHEN {floating} IS NOT NULL AND isfinite({floating}) THEN {floating} ELSE NULL END"
        );
        conversions.push((
            name.clone(),
            expression.clone(),
            format!("({expression}) IS NOT NULL"),
        ));
    }
    if conversions.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let predicates = conversions
        .iter()
        .map(|(_, _, predicate)| predicate.clone())
        .collect::<Vec<_>>();
    let (affected_row_count, changed_counts) =
        match crate::duckdb_query::count_file_predicate_matches(
            &source_path,
            source_format,
            &predicates,
            cancellation.callback(),
        ) {
            Ok(counts) => counts,
            Err(_) => return Ok(None),
        };
    let changed_columns = conversions
        .iter()
        .zip(changed_counts)
        .filter(|(_, count)| *count > 0)
        .map(|((name, _, _), count)| ChangedTextColumn {
            name: name.clone(),
            changed_cell_count: count,
        })
        .collect::<Vec<_>>();
    let changed_cell_count = changed_columns
        .iter()
        .map(|column| column.changed_cell_count)
        .sum::<usize>();
    if changed_cell_count == 0 {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns,
        }));
    }
    let projection = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name.as_str() == "_cambios" {
                source_backed_text_audit_expression(&identifier, "Convertir números detectados")
            } else if let Some((_, expression, _)) = conversions
                .iter()
                .find(|(column, _, _)| column == name.as_str())
            {
                format!("{expression} AS {identifier}")
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Convertir números detectados",
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TextCleaningResult {
        dataset: mutation.dataset,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    }))
}

fn source_backed_inferred_date_expression(
    identifier: &str,
    format: InferredDateFormat,
) -> Option<String> {
    let formats: &[&str] = match format {
        InferredDateFormat::Ymd => &["%Y-%m-%d"],
        InferredDateFormat::DmySlash => &["%d/%m/%Y"],
        InferredDateFormat::MdySlash => &["%m/%d/%Y"],
        InferredDateFormat::DmyDash => &["%d-%m-%Y"],
        InferredDateFormat::YmdSlash => &["%Y/%m/%d"],
        InferredDateFormat::DmyShort => &["%d %b %Y"],
        InferredDateFormat::DmyLong => &["%d %B %Y"],
        InferredDateFormat::CompactYmd => &["%Y%m%d"],
        InferredDateFormat::DmyShortDash => &["%d-%b-%Y"],
        InferredDateFormat::MdyShort => &["%b %d, %Y"],
        InferredDateFormat::MdyLong => &["%B %d, %Y"],
        InferredDateFormat::YmdTime => &["%Y-%m-%dT%H:%M:%S.%f", "%Y-%m-%dT%H:%M:%S"],
        InferredDateFormat::YmdSpaceTime => &["%Y-%m-%d %H:%M:%S.%f", "%Y-%m-%d %H:%M:%S"],
        InferredDateFormat::DmySlashTime => &["%d/%m/%Y %H:%M"],
        InferredDateFormat::Iso8601 => {
            return Some(duckdb_iso8601_expression(
                identifier,
                RecipeDateTarget::Datetime,
            ));
        }
    };
    let value = format!("NULLIF(TRIM(CAST({identifier} AS VARCHAR)), '')");
    let parsed = formats
        .iter()
        .map(|format| format!("try_strptime({value}, {})", duckdb_string_literal(format)))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("COALESCE({parsed})"))
}

#[cfg(test)]
fn source_backed_date_parsing(
    dataset: &mut LoadedDataset,
) -> Result<Option<TextCleaningResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    source_backed_date_parsing_with_cancellation(dataset, &cancellation)
}

fn source_backed_date_parsing_with_cancellation(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
) -> Result<Option<TextCleaningResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let columns = dataset
        .frame
        .columns()
        .iter()
        .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }

    let mut candidates = Vec::<(String, String, usize)>::new();
    for name in &columns {
        let sample = match crate::duckdb_query::sample_file_column_values_with_cancel(
            &source_path,
            source_format,
            name,
            50,
            cancellation.callback(),
        ) {
            Ok(sample) => sample,
            Err(_) => return Ok(None),
        };
        if sample.is_empty() {
            continue;
        }
        let Some(inferred_format) = INFERRED_DATE_FORMATS.iter().copied().find(|format| {
            let parsed = sample
                .iter()
                .filter_map(|value| parse_inferred_datetime(value, *format))
                .collect::<Vec<_>>();
            parsed.len().saturating_mul(100) > sample.len().saturating_mul(80)
                && parsed
                    .iter()
                    .all(|value| (1900..=2100).contains(&value.year()))
        }) else {
            continue;
        };
        let identifier = duckdb_identifier(name);
        let Some(expression) = source_backed_inferred_date_expression(&identifier, inferred_format)
        else {
            return Ok(None);
        };
        candidates.push((name.clone(), expression, sample.len()));
    }
    if candidates.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }

    let candidate_expressions = candidates
        .iter()
        .map(|(name, expression, _)| (name.clone(), expression.clone()))
        .collect::<Vec<_>>();
    let stats = match crate::duckdb_query::count_file_date_parse_stats(
        &source_path,
        source_format,
        &candidate_expressions,
        cancellation.callback(),
    ) {
        Ok(stats) => stats,
        Err(_) => return Ok(None),
    };
    let conversions = candidates
        .into_iter()
        .zip(stats)
        .filter(|((_, _, sample_len), stats)| {
            let extra_null_count = stats.non_null_count.saturating_sub(stats.parsed_count);
            stats.parsed_count.saturating_mul(100) > sample_len.saturating_mul(80)
                && extra_null_count.saturating_mul(100) <= dataset.row_count.saturating_mul(1)
                && stats.in_range_count == stats.parsed_count
        })
        .map(|((name, expression, _), stats)| (name, expression, stats.parsed_count))
        .collect::<Vec<_>>();
    if conversions.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let predicates = conversions
        .iter()
        .map(|(_, expression, _)| format!("({expression}) IS NOT NULL"))
        .collect::<Vec<_>>();
    let (affected_row_count, changed_counts) =
        match crate::duckdb_query::count_file_predicate_matches(
            &source_path,
            source_format,
            &predicates,
            cancellation.callback(),
        ) {
            Ok(counts) => counts,
            Err(_) => return Ok(None),
        };
    let changed_columns = conversions
        .iter()
        .zip(changed_counts)
        .filter(|(_, count)| *count > 0)
        .map(|((name, _, _), count)| ChangedTextColumn {
            name: name.clone(),
            changed_cell_count: count,
        })
        .collect::<Vec<_>>();
    let changed_cell_count = changed_columns
        .iter()
        .map(|column| column.changed_cell_count)
        .sum::<usize>();
    if changed_cell_count == 0 {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns,
        }));
    }
    let projection = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name.as_str() == "_cambios" {
                source_backed_text_audit_expression(&identifier, "Interpretar fechas detectadas")
            } else if let Some((_, expression, _)) = conversions
                .iter()
                .find(|(column, _, _)| column == name.as_str())
            {
                format!("{expression} AS {identifier}")
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Interpretar fechas detectadas",
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TextCleaningResult {
        dataset: mutation.dataset,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    }))
}

fn source_backed_numeric_imputation_type(dtype: &DataType) -> Option<&'static str> {
    match dtype {
        DataType::Int64 => Some("BIGINT"),
        DataType::Float64 => Some("DOUBLE"),
        _ => None,
    }
}

fn source_backed_imputation_projection(
    schema: &DataFrame,
    replacements: &HashMap<String, String>,
    numeric_types: &HashMap<String, &'static str>,
    label: &str,
) -> String {
    schema
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name.as_str() == "_cambios" {
                source_backed_text_audit_expression(&identifier, label)
            } else if let Some(replacement) = replacements.get(name.as_str()) {
                let replacement = duckdb_string_literal(replacement);
                if let Some(dtype) = numeric_types.get(name.as_str()) {
                    format!(
                        "COALESCE({identifier}, CAST({replacement} AS {dtype})) AS {identifier}"
                    )
                } else {
                    format!("COALESCE({identifier}, {replacement}) AS {identifier}")
                }
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
fn source_backed_imputation(
    dataset: &mut LoadedDataset,
    categorical_only: bool,
) -> Result<Option<TextCleaningResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    source_backed_imputation_with_cancellation(dataset, categorical_only, &cancellation)
}

fn source_backed_imputation_with_cancellation(
    dataset: &mut LoadedDataset,
    categorical_only: bool,
    cancellation: &PrepareCancellation,
) -> Result<Option<TextCleaningResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let columns = dataset
        .frame
        .columns()
        .iter()
        .filter(|column| {
            column.name() != "_cambios"
                && if categorical_only {
                    column.dtype() == &DataType::String
                } else {
                    column.dtype() == &DataType::String
                        || source_backed_numeric_imputation_type(column.dtype()).is_some()
                }
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let typed_columns = columns
        .iter()
        .map(|name| {
            let numeric = !categorical_only
                && dataset
                    .frame
                    .column(name)
                    .ok()
                    .and_then(|column| source_backed_numeric_imputation_type(column.dtype()))
                    .is_some();
            (name.clone(), numeric)
        })
        .collect::<Vec<_>>();
    let stats = match crate::duckdb_query::count_file_imputation_stats(
        &source_path,
        source_format,
        &typed_columns,
        cancellation.callback(),
    ) {
        Ok(stats) => stats,
        Err(_) => return Ok(None),
    };
    let mut replacements = HashMap::new();
    let mut numeric_types = HashMap::new();
    let mut changed_columns = Vec::new();
    let mut predicates = Vec::new();
    for ((name, numeric), stats) in typed_columns.iter().zip(stats) {
        if stats.null_count == 0 {
            continue;
        }
        let replacement = if categorical_only {
            Some("Desconocido".to_owned())
        } else {
            stats.replacement
        };
        let Some(replacement) = replacement else {
            continue;
        };
        replacements.insert(name.clone(), replacement);
        if *numeric {
            let dtype = dataset
                .frame
                .column(name)
                .ok()
                .and_then(|column| source_backed_numeric_imputation_type(column.dtype()))
                .ok_or_else(|| format!("La columna numérica '{name}' no es compatible."))?;
            numeric_types.insert(name.clone(), dtype);
        }
        predicates.push(format!("{} IS NULL", duckdb_identifier(name)));
        changed_columns.push(ChangedTextColumn {
            name: name.clone(),
            changed_cell_count: stats.null_count,
        });
    }
    if replacements.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }
    let (affected_row_count, _) = match crate::duckdb_query::count_file_predicate_matches(
        &source_path,
        source_format,
        &predicates,
        cancellation.callback(),
    ) {
        Ok(counts) => counts,
        Err(_) => return Ok(None),
    };
    let changed_cell_count = changed_columns
        .iter()
        .map(|column| column.changed_cell_count)
        .sum::<usize>();
    let label = if categorical_only {
        "Imputación categórica"
    } else {
        "Imputación conservadora"
    };
    let projection =
        source_backed_imputation_projection(&dataset.frame, &replacements, &numeric_types, label);
    let query = format!("SELECT {projection} FROM dataset");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        label,
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TextCleaningResult {
        dataset: mutation.dataset,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    }))
}

struct SourceBackedDirectOutlierPlan {
    name: String,
    dtype: DataType,
    lower: f64,
    upper: f64,
    median: f64,
}

fn source_backed_outlier_literal(value: f64) -> String {
    format!("{value:.17}")
}

fn source_backed_direct_outlier_condition(name: &str, lower: f64, upper: f64) -> String {
    let identifier = duckdb_identifier(name);
    let value = format!("CAST({identifier} AS DOUBLE)");
    format!(
        "{identifier} IS NOT NULL AND ({value} < {} OR {value} > {})",
        source_backed_outlier_literal(lower),
        source_backed_outlier_literal(upper),
    )
}

#[cfg(test)]
fn source_backed_direct_outlier(
    dataset: &mut LoadedDataset,
    action: OutlierAction,
    label: &str,
) -> Result<Option<TextCleaningResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    source_backed_direct_outlier_with_cancellation(dataset, action, label, &cancellation)
}

fn source_backed_direct_outlier_with_cancellation(
    dataset: &mut LoadedDataset,
    action: OutlierAction,
    label: &str,
    cancellation: &PrepareCancellation,
) -> Result<Option<TextCleaningResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let numeric_columns = dataset
        .frame
        .columns()
        .iter()
        .filter(|column| {
            column.name() != "_cambios"
                && matches!(column.dtype(), DataType::Int64 | DataType::Float64)
        })
        .map(|column| {
            (
                column.name().to_string(),
                column.dtype().clone(),
                column.dtype() == &DataType::Int64,
            )
        })
        .collect::<Vec<_>>();
    if numeric_columns.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }

    let typed_columns = numeric_columns
        .iter()
        .map(|(name, _, integer)| (name.clone(), *integer))
        .collect::<Vec<_>>();
    let stats = match crate::duckdb_query::count_file_outlier_stats(
        &source_path,
        source_format,
        &typed_columns,
        cancellation.callback(),
    ) {
        Ok(stats) => stats,
        Err(_) => return Ok(None),
    };
    let mut plans = Vec::new();
    for ((name, dtype, _), stats) in numeric_columns.iter().zip(stats) {
        if stats.non_finite_count > 0 || stats.precision_loss_count > 0 {
            return Ok(None);
        }
        if stats.valid_count < 4 {
            continue;
        }
        let (Some(q1), Some(q3), Some(median)) = (stats.q1, stats.q3, stats.median) else {
            return Ok(None);
        };
        let iqr = q3 - q1;
        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;
        if ![q1, q3, iqr, lower, upper, median]
            .into_iter()
            .all(f64::is_finite)
        {
            return Ok(None);
        }
        plans.push(SourceBackedDirectOutlierPlan {
            name: name.clone(),
            dtype: dtype.clone(),
            lower,
            upper,
            median,
        });
    }
    if plans.is_empty() {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns: Vec::new(),
        }));
    }

    let predicates = plans
        .iter()
        .map(|plan| source_backed_direct_outlier_condition(&plan.name, plan.lower, plan.upper))
        .collect::<Vec<_>>();
    let (affected_row_count, changed_counts) =
        match crate::duckdb_query::count_file_predicate_matches(
            &source_path,
            source_format,
            &predicates,
            cancellation.callback(),
        ) {
            Ok(counts) => counts,
            Err(_) => return Ok(None),
        };
    let changed_columns = plans
        .iter()
        .zip(changed_counts)
        .filter(|(_, count)| *count > 0)
        .map(|(plan, count)| ChangedTextColumn {
            name: plan.name.clone(),
            changed_cell_count: count,
        })
        .collect::<Vec<_>>();
    let changed_cell_count = changed_columns
        .iter()
        .map(|column| column.changed_cell_count)
        .sum::<usize>();
    if changed_cell_count == 0 {
        return Ok(Some(TextCleaningResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            affected_row_count: 0,
            changed_cell_count: 0,
            changed_columns,
        }));
    }

    let projection = dataset
        .frame
        .get_column_names()
        .iter()
        .map(|name| {
            let identifier = duckdb_identifier(name);
            if name.as_str() == "_cambios" {
                return source_backed_text_audit_expression(&identifier, label);
            }
            let Some(plan) = plans.iter().find(|plan| plan.name == name.as_str()) else {
                return identifier;
            };
            let value = format!("CAST({identifier} AS DOUBLE)");
            let condition = source_backed_direct_outlier_condition(
                &plan.name,
                plan.lower,
                plan.upper,
            );
            let lower = source_backed_outlier_literal(plan.lower);
            let upper = source_backed_outlier_literal(plan.upper);
            let expression = match action {
                OutlierAction::Cap => format!(
                    "CASE WHEN {value} IS NULL THEN NULL WHEN {value} < {lower} THEN {lower} WHEN {value} > {upper} THEN {upper} ELSE {value} END"
                ),
                OutlierAction::Impute => {
                    let replacement = if plan.dtype == DataType::Int64 {
                        format!(
                            "CAST({} AS BIGINT)",
                            source_backed_outlier_literal(plan.median)
                        )
                    } else {
                        source_backed_outlier_literal(plan.median)
                    };
                    let original = if plan.dtype == DataType::Int64 {
                        identifier.clone()
                    } else {
                        value.clone()
                    };
                    format!(
                        "CASE WHEN {value} IS NULL THEN NULL WHEN {condition} THEN {replacement} ELSE {original} END"
                    )
                }
                OutlierAction::Drop => identifier.clone(),
            };
            format!("{expression} AS {identifier}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let where_clause = if action == OutlierAction::Drop {
        format!(" WHERE NOT ({})", predicates.join(" OR "))
    } else {
        String::new()
    };
    let query = format!("SELECT {projection} FROM dataset{where_clause}");
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        label,
        true,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TextCleaningResult {
        dataset: mutation.dataset,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    }))
}

#[cfg(test)]
fn source_backed_safe_corrections(
    dataset: &mut LoadedDataset,
    trim_text: bool,
    normalize_column_names: bool,
    normalize_sentinels: bool,
    remove_duplicates: bool,
) -> Result<Option<SafeCorrectionsResult>, String> {
    let cancellation = PrepareCancellation::disabled();
    source_backed_safe_corrections_with_cancellation(
        dataset,
        trim_text,
        normalize_column_names,
        normalize_sentinels,
        remove_duplicates,
        &cancellation,
    )
}

fn source_backed_safe_corrections_with_cancellation(
    dataset: &mut LoadedDataset,
    trim_text: bool,
    normalize_column_names: bool,
    normalize_sentinels: bool,
    remove_duplicates: bool,
    cancellation: &PrepareCancellation,
) -> Result<Option<SafeCorrectionsResult>, String> {
    cancellation.ensure()?;
    let Some((source_path, source_format)) = current_duckdb_file_source(dataset) else {
        return Ok(None);
    };
    let (names, renames) = if normalize_column_names {
        normalized_column_names(&dataset.frame)
    } else {
        (
            dataset
                .frame
                .get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect(),
            Vec::new(),
        )
    };
    let text_expressions = if trim_text || normalize_sentinels {
        dataset
            .frame
            .columns()
            .iter()
            .filter(|column| column.dtype() == &DataType::String && column.name() != "_cambios")
            .map(|column| {
                let name = column.name().to_string();
                let mut expression = duckdb_identifier(&name);
                if trim_text {
                    expression =
                        source_backed_text_expression(&expression, TextCleaningMode::Trim, None)
                            .expect("el modo Trim tiene expresión source-backed");
                }
                if normalize_sentinels {
                    expression = source_backed_text_expression(
                        &expression,
                        TextCleaningMode::Sentinels,
                        None,
                    )
                    .expect("el modo Sentinels tiene expresión source-backed");
                }
                (name, expression)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let (affected_row_count, changed_counts) = if text_expressions.is_empty() {
        (0, Vec::new())
    } else {
        match crate::duckdb_query::count_file_expression_changes(
            &source_path,
            source_format,
            &text_expressions,
            cancellation.callback(),
        ) {
            Ok(counts) => counts,
            Err(_) => return Ok(None),
        }
    };
    let changed_cell_count = changed_counts.iter().copied().sum::<usize>();
    if changed_cell_count == 0 && renames.is_empty() && !remove_duplicates {
        return Ok(Some(SafeCorrectionsResult {
            dataset: loaded_dataset_preview(dataset, &dataset.frame)?,
            changed_cell_count: 0,
            affected_row_count: 0,
            removed_row_count: 0,
            renamed_column_count: 0,
            renames,
            imputed_cell_count: 0,
        }));
    }

    let projection = dataset
        .frame
        .get_column_names()
        .iter()
        .zip(&names)
        .map(|(original, renamed)| {
            let source = duckdb_identifier(original);
            let destination = duckdb_identifier(renamed);
            if let Some((_, expression)) = text_expressions
                .iter()
                .find(|(name, _)| name == original.as_str())
            {
                format!("{expression} AS {destination}")
            } else if original.as_str() == renamed {
                source
            } else {
                format!("{source} AS {destination}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let columns = names
        .iter()
        .map(|name| duckdb_identifier(name))
        .collect::<Vec<_>>();
    let prepared_query = format!("SELECT {projection} FROM dataset");
    let query = if remove_duplicates {
        let names = dataset
            .frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<HashSet<_>>();
        let mut order_name = "__columnia_plan_order".to_owned();
        while names.contains(order_name.as_str()) {
            order_name.push('_');
        }
        let mut rank_name = "__columnia_plan_rank".to_owned();
        while names.contains(rank_name.as_str()) || rank_name == order_name {
            rank_name.push('_');
        }
        let order_column = duckdb_identifier(&order_name);
        let rank_column = duckdb_identifier(&rank_name);
        format!(
            "WITH prepared AS ({prepared_query}), ordered AS (SELECT prepared.*, ROW_NUMBER() OVER () AS {order_column} FROM prepared), ranked AS (SELECT *, ROW_NUMBER() OVER (PARTITION BY {} ORDER BY {order_column}) AS {rank_column} FROM ordered) SELECT {} FROM ranked WHERE {rank_column} = 1 ORDER BY {order_column}",
            columns.join(", "),
            columns.join(", "),
        )
    } else {
        prepared_query
    };
    let force_publish = changed_cell_count > 0 || !renames.is_empty();
    let Some(mutation) = publish_source_backed_query(
        dataset,
        &source_path,
        source_format,
        &query,
        "Aplicar correcciones recomendadas",
        force_publish,
        cancellation.clone(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(SafeCorrectionsResult {
        removed_row_count: mutation.affected_row_count,
        dataset: mutation.dataset,
        changed_cell_count,
        affected_row_count,
        renamed_column_count: renames.len(),
        renames,
        imputed_cell_count: 0,
    }))
}

fn leading_zero_code(value: &str) -> bool {
    let value = value.trim().trim_start_matches(['+', '-']);
    value.starts_with('0')
        && value
            .chars()
            .nth(1)
            .is_some_and(|character| character.is_ascii_digit())
}

fn remove_constant_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    column_cleanup::remove_constant_columns_from_frame(frame)
}

fn remove_empty_columns_from_frame(frame: &DataFrame) -> Result<(DataFrame, Vec<String>), String> {
    column_cleanup::remove_empty_columns_from_frame(frame)
}

fn remove_high_null_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    column_cleanup::remove_high_null_columns_from_frame(frame)
}

fn remove_identifier_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    column_cleanup::remove_identifier_columns_from_frame(frame)
}

fn remove_personal_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    column_cleanup::remove_personal_columns_from_frame(frame)
}

fn is_personal_privacy_signal(column_name: &str) -> bool {
    column_cleanup::is_personal_privacy_signal(column_name)
}

fn mask_personal_values_from_frame(frame: &DataFrame) -> Result<(DataFrame, usize, usize), String> {
    column_cleanup::mask_personal_values_from_frame(frame)
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
    if value.is_ascii() {
        let mut normalized = String::with_capacity(value.len());
        let mut pending_space = false;
        for byte in value.bytes() {
            if byte.is_ascii_whitespace() {
                pending_space = !normalized.is_empty();
                continue;
            }
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(byte.to_ascii_lowercase() as char);
        }
        return normalized;
    }
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

fn parse_inferred_date_columns(
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

        let inferred_format = INFERRED_DATE_FORMATS.iter().copied().find(|format| {
            let parsed = sample
                .iter()
                .filter_map(|value| parse_inferred_datetime(value, *format))
                .collect::<Vec<_>>();
            parsed.len() * 100 > sample.len() * INFERENCE_THRESHOLD_PERCENTAGE
                && parsed
                    .iter()
                    .all(|value| (1900..=2100).contains(&value.year()))
        });
        let Some(inferred_format) = inferred_format else {
            continue;
        };

        let parse_value = |value: &str| parse_inferred_datetime(value, inferred_format);
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

fn cast_inferred_numeric_columns(
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
                .any(|value| leading_zero_code(value))
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
enum OutlierMode {
    Cap,
    Drop,
}

fn apply_outlier_mode(
    frame: &DataFrame,
    mode: OutlierMode,
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
            OutlierMode::Cap => {
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
            OutlierMode::Drop => {
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

    if matches!(mode, OutlierMode::Drop) {
        affected_row_count = drop_mask.iter().filter(|drop| **drop).count();
        if affected_row_count > 0 {
            let keep = drop_mask.iter().map(|drop| !drop).collect::<Vec<_>>();
            candidate = candidate
                .filter(&BooleanChunked::from_slice("outliers".into(), &keep))
                .map_err(|error| format!("No se pudieron retirar filas atípicas: {error}"))?;
        }
    }
    if matches!(mode, OutlierMode::Cap) {
        affected_row_count = changed_rows.iter().filter(|changed| **changed).count();
    }

    Ok((
        candidate,
        affected_row_count,
        changed_cell_count,
        changed_columns,
    ))
}

pub(super) struct SafeCorrectionPlanFrame {
    pub(super) frame: DataFrame,
    pub(super) affected_row_count: usize,
    pub(super) changed_cell_count: usize,
    pub(super) removed_row_count: usize,
    pub(super) renames: Vec<ColumnRename>,
    pub(super) imputed_cell_count: usize,
}

/// Safe corrections plus the optional conservative imputation, as one candidate
/// so the whole plan is published and undone as a single revision.
pub(super) fn safe_corrected_plan_frame(
    frame: &DataFrame,
    trim_text: bool,
    normalize_column_names: bool,
    normalize_sentinels: bool,
    remove_duplicates: bool,
    impute_missing: bool,
) -> Result<SafeCorrectionPlanFrame, String> {
    let (corrected, mut affected_row_count, changed_cell_count, removed_row_count, renames) =
        safe_corrected_frame(
            frame,
            trim_text,
            normalize_column_names,
            normalize_sentinels,
            remove_duplicates,
        )?;
    let (frame, imputed_cell_count) = if impute_missing {
        let (imputed, imputed_rows, imputed_cells, _) = impute_missing_values_in_frame(&corrected)?;
        affected_row_count = affected_row_count.max(imputed_rows);
        (imputed, imputed_cells)
    } else {
        (corrected, 0)
    };
    Ok(SafeCorrectionPlanFrame {
        frame,
        affected_row_count,
        changed_cell_count,
        removed_row_count,
        renames,
        imputed_cell_count,
    })
}

fn safe_corrected_frame(
    frame: &DataFrame,
    trim_text: bool,
    normalize_column_names: bool,
    normalize_sentinels: bool,
    remove_duplicates: bool,
) -> Result<(DataFrame, usize, usize, usize, Vec<ColumnRename>), String> {
    let mut candidate = frame.clone();
    if trim_text {
        candidate = clean_text_columns(&candidate, None, TextCleaningMode::Trim)?.0;
    }
    if normalize_sentinels {
        candidate = clean_text_columns(&candidate, None, TextCleaningMode::Sentinels)?.0;
    }
    let (affected_row_count, changed_cell_count) = count_changed_text_cells(frame, &candidate)?;
    let (names, renames) = if normalize_column_names {
        normalized_column_names(&candidate)
    } else {
        (Vec::new(), Vec::new())
    };
    if !renames.is_empty() {
        candidate
            .set_column_names(&names)
            .map_err(|error| format!("No se pudieron normalizar las columnas: {error}"))?;
    }
    let (candidate, removed_row_count) = if remove_duplicates {
        remove_duplicate_rows(&candidate)?
    } else {
        (candidate, 0)
    };
    Ok((
        candidate,
        affected_row_count,
        changed_cell_count,
        removed_row_count,
        renames,
    ))
}

fn count_changed_text_cells(
    before: &DataFrame,
    after: &DataFrame,
) -> Result<(usize, usize), String> {
    let mut changed_rows = vec![false; before.height()];
    let mut changed_cell_count = 0;
    for column in before.columns() {
        if column.dtype() != &DataType::String || column.name() == "_cambios" {
            continue;
        }
        let name = column.name().as_str();
        let before_values = column
            .str()
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        let after_values = after
            .column(name)
            .and_then(|column| column.str())
            .map_err(|error| format!("No se pudo leer la columna '{name}' corregida: {error}"))?;
        for (row_index, changed) in changed_rows.iter_mut().enumerate() {
            if before_values.get(row_index) != after_values.get(row_index) {
                changed_cell_count += 1;
                *changed = true;
            }
        }
    }
    Ok((
        changed_rows.into_iter().filter(|changed| *changed).count(),
        changed_cell_count,
    ))
}

#[cfg(test)]
use json_reader::{json_record_column_names, load_json_records};
use json_reader::{json_record_column_names_with_cancel, load_json_records_with_cancel};

fn read_utf8_delimited_sample_with_cancel<C>(
    path: &Path,
    is_cancelled: C,
) -> Result<(String, bool), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let file_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar el archivo delimitado: {error}"))?
        .len();
    ensure_not_cancelled(is_cancelled())?;
    let mut bytes = Vec::with_capacity(DELIMITED_SAMPLE_BYTES as usize);
    let mut file = fs::File::open(path)
        .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))?
        .take(DELIMITED_SAMPLE_BYTES);
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo inspeccionar el archivo delimitado: {error}"))?;
        ensure_not_cancelled(is_cancelled())?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
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
    detect_delimiter_with_cancel(path, extension, || false)
}

fn detect_delimiter_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<u8, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if extension == "tsv" {
        // La extensión explícita tiene prioridad sobre cualquier carácter que
        // aparezca dentro de los valores.
        read_utf8_delimited_sample_with_cancel(path, &is_cancelled)?;
        return Ok(b'\t');
    }

    let (sample, complete) = read_utf8_delimited_sample_with_cancel(path, &is_cancelled)?;
    let candidates = [(b',', ','), (b';', ';'), (b'\t', '\t'), (b'|', '|')];
    let mut valid = Vec::new();
    for (byte, delimiter) in candidates {
        ensure_not_cancelled(is_cancelled())?;
        let counts = delimited_field_counts(&sample, delimiter, complete);
        let Some(first) = counts.first().copied() else {
            continue;
        };
        if counts.len() >= 2 && first > 1 && counts.iter().all(|count| *count == first) {
            valid.push((byte, first));
        }
    }
    ensure_not_cancelled(is_cancelled())?;
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

fn collect_lazy_frame_streaming_with_cancel<C>(
    plan: LazyFrame,
    context: &str,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    let schema = plan
        .clone()
        .collect_schema()
        .map_err(|error| format!("{context}: {error}"))?;
    let collected = Arc::new(Mutex::new(DataFrame::empty_with_schema(&schema)));
    let callback_collected = Arc::clone(&collected);
    let callback_cancelled = Arc::new(AtomicBool::new(false));
    let callback_cancelled_state = Arc::clone(&callback_cancelled);
    let callback_cancellation = Arc::clone(&callback_cancelled);
    let plan = plan
        .sink_batches(
            PlanCallback::new(move |batch| {
                if callback_cancellation.load(Ordering::Acquire) {
                    return Ok(true);
                }
                let mut collected = callback_collected.lock().map_err(|_| {
                    PolarsError::ComputeError(
                        "No se pudo acumular un bloque leído del dataset.".into(),
                    )
                })?;
                collected.vstack_mut(&batch)?;
                Ok(callback_cancelled_state.load(Ordering::Acquire))
            }),
            true,
            std::num::NonZeroUsize::new(CANCELLABLE_READ_BATCH_ROWS),
        )
        .map_err(|error| format!("{context}: {error}"))?;

    let collect_result = std::thread::scope(|scope| {
        let watcher_cancelled = Arc::clone(&callback_cancelled);
        let watcher_stop = Arc::new(AtomicBool::new(false));
        let watcher_stop_signal = Arc::clone(&watcher_stop);
        let watcher = scope.spawn(move || {
            while !watcher_stop_signal.load(Ordering::Acquire) {
                if is_cancelled() {
                    watcher_cancelled.store(true, Ordering::Release);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        let result = plan
            .collect_with_engine(Engine::Streaming)
            .map(|_| ())
            .map_err(|error| format!("{context}: {error}"));
        watcher_stop.store(true, Ordering::Release);
        if watcher.join().is_err() {
            return Err("El monitor de cancelación de Polars terminó inesperadamente.".to_owned());
        }
        result
    });
    ensure_not_cancelled(callback_cancelled.load(Ordering::Acquire) || is_cancelled())?;
    collect_result?;
    collected
        .lock()
        .map(|frame| frame.clone())
        .map_err(|_| "No se pudo recuperar el dataset leído por bloques.".to_owned())
}

#[cfg(test)]
fn delimited_scan(path: &Path, extension: &str) -> Result<LazyFrame, String> {
    delimited_scan_with_header(path, extension, true)
}

fn delimited_scan_with_header(
    path: &Path,
    extension: &str,
    has_header: bool,
) -> Result<LazyFrame, String> {
    delimited_scan_with_header_and_cancel(path, extension, has_header, || false)
}

fn delimited_scan_with_header_and_cancel<C>(
    path: &Path,
    extension: &str,
    has_header: bool,
    is_cancelled: C,
) -> Result<LazyFrame, String>
where
    C: Fn() -> bool + Sync,
{
    let separator = detect_delimiter_with_cancel(path, extension, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    delimited_scan_with_separator(path, separator, has_header)
}

fn delimited_scan_with_separator(
    path: &Path,
    separator: u8,
    has_header: bool,
) -> Result<LazyFrame, String> {
    let source = PlRefPath::try_from_path(path)
        .map_err(|error| format!("No se pudo preparar el lector delimitado: {error}"))?;
    LazyCsvReader::new(source)
        .with_has_header(has_header)
        .with_infer_schema_length(Some(0))
        .with_low_memory(true)
        .with_rechunk(false)
        .with_separator(separator)
        .finish()
        .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))
}

#[cfg(test)]
fn read_delimited_frame(path: &Path, extension: &str) -> Result<DataFrame, String> {
    let plan = delimited_scan(path, extension)?;
    collect_lazy_frame_streaming(
        plan,
        "No se pudo interpretar el archivo delimitado como UTF-8",
    )
}

fn read_delimited_frame_with_header_and_cancel<C>(
    path: &Path,
    extension: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    let has_header = header_mode == SpreadsheetHeaderMode::FirstRow;
    let plan = delimited_scan_with_header_and_cancel(path, extension, has_header, &is_cancelled)?;
    collect_lazy_frame_streaming_with_cancel(
        plan,
        "No se pudo interpretar el archivo delimitado como UTF-8",
        &is_cancelled,
    )
}

fn parquet_scan(path: &Path) -> Result<LazyFrame, String> {
    let source = PlRefPath::try_from_path(path)
        .map_err(|error| format!("No se pudo preparar el lector Parquet: {error}"))?;
    let options = ScanArgsParquet {
        parallel: ParallelStrategy::Columns,
        low_memory: true,
        rechunk: false,
        ..Default::default()
    };
    LazyFrame::scan_parquet(source, options)
        .map_err(|error| format!("No se pudo abrir el Parquet: {error}"))
}

#[cfg(test)]
fn read_parquet_frame(path: &Path) -> Result<DataFrame, String> {
    read_parquet_frame_with_cancel(path, || false)
}

fn read_parquet_frame_with_cancel<C>(path: &Path, is_cancelled: C) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_materialization_budget_for_path(path)?;
    let plan = parquet_scan(path)?;
    collect_lazy_frame_streaming_with_cancel(
        plan,
        "No se pudo interpretar el Parquet",
        &is_cancelled,
    )
}

fn validate_staged_history_snapshot_with_cancel<C>(
    path: &Path,
    current_frame: &DataFrame,
    is_cursor: bool,
    error_message: &str,
    is_cancelled: C,
) -> Result<bool, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if is_cursor {
        let frame = read_parquet_frame_with_cancel(path, &is_cancelled).map_err(|error| {
            if error == OPERATION_CANCELLED_MESSAGE {
                error
            } else {
                error_message.to_owned()
            }
        })?;
        ensure_not_cancelled(is_cancelled())?;
        return Ok(frame.equals_missing(current_frame));
    }

    // Los snapshots que no son el cursor se conservan byte a byte y solo se
    // valida su esquema/footer durante la apertura. Sus páginas se leen bajo
    // demanda cuando el usuario hace undo/redo, evitando materializar todo el
    // historial en RAM.
    read_parquet_schema_frame(path).map_err(|_| error_message.to_owned())?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(false)
}

fn read_parquet_schema_frame(path: &Path) -> Result<DataFrame, String> {
    let mut plan = parquet_scan(path)?;
    let schema = plan
        .collect_schema()
        .map_err(|error| format!("No se pudo leer el esquema Parquet: {error}"))?;
    Ok(DataFrame::empty_with_schema(&schema))
}

use source_loading::{
    dataset_resource_estimate, ensure_materialization_budget,
    ensure_materialization_budget_for_path, load_dataset_with_header_mode,
    load_dataset_with_progress, load_source_backed_dataset_for_automation,
    materialize_current_dataset_with_cancel, materialize_loaded_dataset,
    materialize_loaded_dataset_with_cancel, materialized_dataset_frame,
    materialized_dataset_frame_with_cancel, should_defer_source_load, source_backed_join_source,
    source_backed_json_load, source_backed_load, source_backed_load_with_header_mode,
    source_backed_spreadsheet_load, source_scan,
};
#[cfg(test)]
use source_loading::{load_csv, load_csv_with_progress, materialization_budget_error};

#[tauri::command]
pub async fn compare_dataset(
    app: AppHandle,
    key_columns: Option<Vec<String>>,
) -> Result<Option<DatasetComparison>, String> {
    comparison_reader::compare_dataset_impl(app, key_columns).await
}

#[tauri::command]
pub async fn get_dataset_conflict_page(
    app: AppHandle,
    offset: usize,
    limit: usize,
) -> Result<Option<DatasetConflictPage>, String> {
    comparison_reader::get_dataset_conflict_page_impl(app, offset, limit).await
}

#[tauri::command]
pub async fn join_dataset(
    app: AppHandle,
    key_columns: Vec<String>,
    join_type: DatasetJoinType,
) -> Result<Option<DatasetPreview>, String> {
    let (_review_guard, cancellation) = ReviewMutationCancellation::begin(&app)?;
    let key_columns = normalize_key_columns(Some(key_columns))?;
    if key_columns.is_empty() {
        return Err("Selecciona al menos una columna clave para unir datasets.".to_owned());
    }
    cancellation.ensure()?;
    let (
        source_join_context,
        initial_eager_frame,
        expected_stamp,
        current_file_name,
        current_file_size,
        expected_comparison_path,
    ) = {
        let state = app.state::<DatasetState>();
        let current = state.current.lock_recovering();
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let context =
            source_backed_join_context(dataset).or_else(|| snapshot_backed_join_context(dataset));
        let eager_frame: Option<DataFrame> = None;
        let stamp = DatasetMutationStamp::capture(dataset);
        let file_name = dataset.file_name.clone();
        let file_size = dataset.file_size_bytes;
        drop(current);
        let comparison_path = state
            .comparison
            .lock_recovering()
            .as_ref()
            .map(|pending| pending.snapshot_path.clone());
        (
            context,
            eager_frame,
            stamp,
            file_name,
            file_size,
            comparison_path,
        )
    };
    cancellation.ensure()?;
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
    cancellation.ensure()?;
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let compared_file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();

    cancellation.ensure()?;
    let compared_source = source_backed_join_source(&path, &extension)?;
    cancellation.ensure()?;
    if let (Some(context), Some((compared_format, compared_schema))) =
        (source_join_context.clone(), compared_source)
    {
        let app_for_source_join = app.clone();
        let cancellation_for_source_join = cancellation.clone();
        let expected_stamp_for_source_join = expected_stamp.clone();
        let expected_comparison_path_for_source_join = expected_comparison_path.clone();
        let compared_path_for_source_join = path.clone();
        let compared_file_name_for_source_join = compared_file_name.clone();
        let key_columns_for_source_join = key_columns.clone();
        let source_result = tauri::async_runtime::spawn_blocking(move || {
            join_source_backed_dataset(
                &app_for_source_join.state::<DatasetState>(),
                SourceBackedJoinRequest {
                    context,
                    compared_path: compared_path_for_source_join,
                    compared_format,
                    compared_schema,
                    compared_file_name: compared_file_name_for_source_join,
                    compared_size_bytes: file_size_bytes,
                    key_columns: key_columns_for_source_join,
                    join_type,
                },
                &expected_stamp_for_source_join,
                expected_comparison_path_for_source_join.as_deref(),
                expected_comparison_path_for_source_join.is_none(),
                &cancellation_for_source_join,
            )
        })
        .await
        .map_err(|error| format!("La unión source-backed se interrumpió: {error}"))??;
        if source_result.is_some() {
            return Ok(source_result);
        }
    }

    let cancellation_for_eager = cancellation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancellation_for_eager.ensure()?;
        let current_frame = match initial_eager_frame {
            Some(frame) => frame,
            None => {
                let state = app.state::<DatasetState>();
                let current = state.current.lock_recovering();
                let dataset = current.as_ref().ok_or_else(|| {
                    "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
                })?;
                if !expected_stamp.matches(dataset) {
                    return Err(
                        "El dataset activo cambió durante la operación de Review.".to_owned()
                    );
                }
                materialized_dataset_frame_with_cancel(dataset, || {
                    cancellation_for_eager.is_cancelled()
                })?
            }
        };
        cancellation_for_eager.ensure()?;
        let compared_frame =
            load_compare_frame_with_cancel(&path, &extension, cancellation_for_eager.callback())?;
        let joined = join_frames_on_keys_in_blocks_with_cancel(
            &current_frame,
            &compared_frame,
            &key_columns,
            &key_columns,
            join_type,
            &cancellation_for_eager.callback(),
        )?;
        cancellation_for_eager.ensure()?;
        let file_name = format!(
            "Join {} · {} + {compared_file_name}",
            join_type.label(),
            current_file_name
        );
        let output_size = current_file_size.saturating_add(file_size_bytes);
        publish_review_eager_candidate(
            &app.state::<DatasetState>(),
            &expected_stamp,
            expected_comparison_path.as_deref(),
            expected_comparison_path.is_none(),
            joined,
            &file_name,
            output_size,
            &format!("Unir datasets ({})", join_type.label()),
            &cancellation_for_eager,
        )
        .map(Some)
    })
    .await
    .map_err(|error| format!("La unión se interrumpió: {error}"))?
}

fn validate_conflict_decisions_with_cancel<C>(
    conflicts: &[KeyConflictRows],
    decisions: &[ConflictResolution],
    is_cancelled: &C,
) -> Result<ConflictChoiceMap, String>
where
    C: Fn() -> bool + Sync,
{
    let mut choices = HashMap::new();
    for (decision_index, decision) in decisions.iter().enumerate() {
        if decision_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let (conflict_index, column, choice) = decision.choice();
        let Some(conflict) = conflicts.get(conflict_index) else {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        };
        if let Some(column) = column.as_ref() {
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
        if choices.insert((conflict_index, column), choice).is_some() {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        }
    }

    for (conflict_index, conflict) in conflicts.iter().enumerate() {
        if conflict_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let conflict_choices = choices
            .iter()
            .filter(|((index, _), _)| *index == conflict_index)
            .collect::<Vec<_>>();
        let excludes = conflict_choices
            .iter()
            .any(|(_, choice)| **choice == ConflictResolutionChoice::Exclude);
        if excludes {
            if conflict_choices.len() != 1 {
                return Err(
                    "La exclusión debe ser la única decisión para cada conflicto.".to_owned(),
                );
            }
            continue;
        }
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
    ensure_not_cancelled(is_cancelled())?;
    Ok(choices)
}

#[allow(clippy::too_many_arguments)]
fn validate_source_backed_conflict_decisions(
    current_path: &Path,
    current_row_count: usize,
    compared_path: &Path,
    compared_row_count: usize,
    key_columns: &[String],
    shared_columns: &[String],
    decisions: &[ConflictResolution],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> Result<Option<ConflictChoiceMap>, String> {
    ensure_not_cancelled(is_cancelled())?;
    if decisions.len() > SOURCE_BACKED_RESOLUTION_MAX_CONFLICTS {
        return Ok(None);
    }

    let mut choices = ConflictChoiceMap::new();
    let mut choices_by_conflict =
        HashMap::<usize, HashMap<Option<String>, ConflictResolutionChoice>>::new();
    for (decision_index, decision) in decisions.iter().enumerate() {
        if decision_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        let (conflict_index, column, choice) = decision.choice();
        let choice_key = (conflict_index, column.clone());
        if choices.insert(choice_key.clone(), choice).is_some() {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        }
        if choices_by_conflict
            .entry(conflict_index)
            .or_default()
            .insert(column, choice)
            .is_some()
        {
            return Err(
                "La selección de resolución contiene conflictos repetidos o inválidos.".to_owned(),
            );
        }
    }

    match for_each_key_conflict_between_parquet_with_cancel(
        ParquetComparisonSource {
            path: current_path,
            row_count: current_row_count,
        },
        ParquetComparisonSource {
            path: compared_path,
            row_count: compared_row_count,
        },
        key_columns,
        shared_columns,
        Some(SOURCE_BACKED_RESOLUTION_MAX_CONFLICTS),
        is_cancelled,
        |conflict_index,
         _current_block,
         _current_row_in_block,
         _compared_block,
         _compared_row_in_block,
         _current_row_index,
         _compared_row_index,
         shape| {
            let conflict_choices =
                choices_by_conflict.remove(&conflict_index).ok_or_else(|| {
                    "Debes elegir un origen para cada conflicto detectado.".to_owned()
                })?;
            let excludes = conflict_choices
                .values()
                .any(|choice| *choice == ConflictResolutionChoice::Exclude);
            if excludes {
                if conflict_choices.len() != 1
                    || !matches!(
                        conflict_choices.get(&None),
                        Some(ConflictResolutionChoice::Exclude)
                    )
                {
                    return Err(
                        "La exclusión debe ser la única decisión para cada conflicto.".to_owned(),
                    );
                }
                return Ok(());
            }
            for column in conflict_choices.keys().filter_map(|column| column.as_ref()) {
                if !shape.columns.iter().any(|expected| expected == column) {
                    return Err(format!(
                        "La columna '{column}' no pertenece al conflicto seleccionado."
                    ));
                }
            }
            let uses_columns = conflict_choices.keys().any(Option::is_some);
            if conflict_choices
                .keys()
                .any(|column| column.is_some() != uses_columns)
            {
                return Err(
                    "Cada conflicto debe resolverse por fila completa o por todas sus columnas."
                        .to_owned(),
                );
            }
            if uses_columns {
                let selected_columns = conflict_choices
                    .keys()
                    .filter_map(Option::as_deref)
                    .collect::<HashSet<_>>();
                let expected_columns = shape
                    .columns
                    .iter()
                    .map(String::as_str)
                    .collect::<HashSet<_>>();
                if selected_columns != expected_columns {
                    return Err(
                        "Faltan decisiones de columna para alguno de los conflictos visibles."
                            .to_owned(),
                    );
                }
            } else if conflict_choices.len() != 1 {
                return Err("Debes elegir un origen para cada conflicto detectado.".to_owned());
            }
            Ok(())
        },
    ) {
        Ok(_) => {}
        Err(error) if error == SOURCE_BACKED_RESOLUTION_LIMIT_REACHED => return Ok(None),
        Err(error) => return Err(error),
    }
    if !choices_by_conflict.is_empty() {
        return Err("La selección de resolución contiene conflictos inválidos.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(Some(choices))
}

fn source_backed_conflict_resolution_plan(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    choices: &ConflictChoiceMap,
) -> Result<(String, String, String, String), String> {
    if current.get_column_names() != compared.get_column_names()
        || current
            .columns()
            .iter()
            .zip(compared.columns())
            .any(|(left, right)| left.dtype() != right.dtype())
    {
        return Err("Los esquemas no son compatibles para resolver conflictos.".to_owned());
    }
    validate_key_columns(current, compared, key_columns)?;
    let mut used_names = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let current_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_current_order");
    let compared_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_compared_order");
    let current_count_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_current_count");
    let compared_count_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_compared_count");
    let conflict_index_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_conflict_index");
    let result_order_column =
        unique_duckdb_internal_name(&mut used_names, "__columnia_resolution_result_order");
    let keys = key_columns
        .iter()
        .map(|key| duckdb_identifier(key))
        .collect::<Vec<_>>();
    let partition = keys.join(", ");
    let key_conditions = key_columns
        .iter()
        .map(|key| {
            let identifier = duckdb_identifier(key);
            format!("c.{identifier} IS NOT DISTINCT FROM r.{identifier}")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let payload_columns = current
        .get_column_names()
        .iter()
        .filter(|name| !key_columns.iter().any(|key| key == name.as_str()))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let payload_equal = payload_columns
        .iter()
        .map(|column| {
            let identifier = duckdb_identifier(column);
            format!("c.{identifier} IS NOT DISTINCT FROM r.{identifier}")
        })
        .collect::<Vec<_>>();
    let conflict_predicate = if payload_equal.is_empty() {
        "FALSE".to_owned()
    } else {
        format!("NOT ({})", payload_equal.join(" AND "))
    };

    let current_columns = current
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let mut choices_by_column = HashMap::<String, Vec<(usize, ConflictSource)>>::new();
    let mut excluded_conflicts = Vec::new();
    for ((conflict_index, column), choice) in choices {
        if *choice == ConflictResolutionChoice::Exclude {
            excluded_conflicts.push(*conflict_index);
            continue;
        }
        let ConflictResolutionChoice::UseSource(source) = choice else {
            unreachable!("exclusion cases are handled above")
        };
        if let Some(column) = column {
            choices_by_column
                .entry(column.clone())
                .or_default()
                .push((*conflict_index, *source));
        } else {
            for column in &current_columns {
                choices_by_column
                    .entry(column.clone())
                    .or_default()
                    .push((*conflict_index, *source));
            }
        }
    }
    let projection = current_columns
        .iter()
        .map(|column| {
            let identifier = duckdb_identifier(column);
            let mut column_choices = choices_by_column.remove(column).unwrap_or_default();
            column_choices.sort_by_key(|(conflict_index, _)| *conflict_index);
            let compared_cases = column_choices
                .into_iter()
                .filter(|(_, source)| matches!(source, ConflictSource::Compared))
                .map(|(conflict_index, _)| {
                    format!(
                        "WHEN x.{} = {conflict_index} THEN r.{identifier}",
                        duckdb_identifier(&conflict_index_column)
                    )
                })
                .collect::<Vec<_>>();
            if compared_cases.is_empty() {
                format!("c.{identifier} AS {identifier}")
            } else {
                format!(
                    "CASE {} ELSE c.{identifier} END AS {identifier}",
                    compared_cases.join(" ")
                )
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let current_order = duckdb_identifier(&current_order_column);
    let current_count = duckdb_identifier(&current_count_column);
    let compared_count = duckdb_identifier(&compared_count_column);
    let conflict_index = duckdb_identifier(&conflict_index_column);
    let result_order = duckdb_identifier(&result_order_column);
    excluded_conflicts.sort_unstable();
    excluded_conflicts.dedup();
    let conflict_exclusion_filter = if excluded_conflicts.is_empty() {
        String::new()
    } else {
        let excluded = excluded_conflicts
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!(" WHERE x.{conflict_index} IS NULL OR x.{conflict_index} NOT IN ({excluded})")
    };
    let dataset_view_query = format!(
        "CREATE VIEW dataset AS WITH current_ranked AS (SELECT c.*, COUNT(*) OVER (PARTITION BY {partition}) AS {current_count} FROM __columnia_current AS c), compared_ranked AS (SELECT r.*, COUNT(*) OVER (PARTITION BY {partition}) AS {compared_count} FROM __columnia_compared AS r), conflicts AS (SELECT c.{current_order}, ROW_NUMBER() OVER (ORDER BY c.{current_order}) - 1 AS {conflict_index} FROM current_ranked AS c JOIN compared_ranked AS r ON {key_conditions} WHERE c.{current_count} = 1 AND r.{compared_count} = 1 AND {conflict_predicate}) SELECT {projection}, c.{current_order} AS {result_order} FROM current_ranked AS c LEFT JOIN compared_ranked AS r ON {key_conditions} AND c.{current_count} = 1 AND r.{compared_count} = 1 LEFT JOIN conflicts AS x ON x.{current_order} = c.{current_order}{conflict_exclusion_filter}"
    );
    let output_projection = current_columns
        .iter()
        .map(|column| duckdb_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let output_query = format!("SELECT {output_projection} FROM dataset ORDER BY {result_order}");
    Ok((
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ))
}

#[cfg(test)]
fn resolved_conflict_frame(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    decisions: &[ConflictResolution],
) -> Result<DataFrame, String> {
    resolved_conflict_frame_with_cancel(current, compared, key_columns, decisions, &|| false)
}

fn resolved_conflict_frame_with_cancel<C>(
    current: &DataFrame,
    compared: &DataFrame,
    key_columns: &[String],
    decisions: &[ConflictResolution],
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
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
    let conflicts = collect_all_key_conflicts_with_cancel(
        current,
        compared,
        key_columns,
        &shared_columns,
        is_cancelled,
    )?;
    let choices = validate_conflict_decisions_with_cancel(&conflicts, decisions, is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;

    let conflict_rows = conflicts
        .iter()
        .enumerate()
        .map(|(index, conflict)| (conflict.current_row_index, index))
        .collect::<HashMap<_, _>>();
    let excluded_conflicts = choices
        .iter()
        .filter_map(|((conflict_index, _), choice)| {
            (*choice == ConflictResolutionChoice::Exclude).then_some(*conflict_index)
        })
        .collect::<HashSet<_>>();
    let excluded_rows = conflicts
        .iter()
        .enumerate()
        .filter_map(|(conflict_index, conflict)| {
            excluded_conflicts
                .contains(&conflict_index)
                .then_some(conflict.current_row_index)
        })
        .collect::<HashSet<_>>();
    let mut active_rows = Vec::with_capacity(current.height().saturating_sub(excluded_rows.len()));
    for row_index in 0..current.height() {
        if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
            ensure_not_cancelled(is_cancelled())?;
        }
        if !excluded_rows.contains(&row_index) {
            active_rows.push(row_index);
        }
    }
    let mut resolved_columns = Vec::with_capacity(current.width());
    for current_column in current.columns() {
        let name = current_column.name().to_string();
        let compared_column = compared
            .column(&name)
            .map_err(|error| format!("No se pudo leer la columna comparada '{name}': {error}"))?;
        let mut values = Vec::with_capacity(active_rows.len());
        for (active_index, row_index) in active_rows.iter().copied().enumerate() {
            if active_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            let conflict_index = conflict_rows.get(&row_index).copied();
            let source = conflict_index.and_then(|conflict_index| {
                match choices
                    .get(&(conflict_index, Some(name.clone())))
                    .or_else(|| choices.get(&(conflict_index, None)))
                {
                    Some(ConflictResolutionChoice::UseSource(source)) => Some(*source),
                    Some(ConflictResolutionChoice::Exclude) | None => None,
                }
            });
            let source_row = match (source, conflict_index) {
                (Some(ConflictSource::Compared), Some(conflict_index)) => conflicts
                    .get(conflict_index)
                    .map(|conflict| conflict.compared_row_index)
                    .ok_or_else(|| "No se encontró la fila comparada en conflicto.".to_owned())?,
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
            values.push(value.clone());
        }
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
    ensure_not_cancelled(is_cancelled())?;
    DataFrame::new(active_rows.len(), resolved_columns)
        .map_err(|error| format!("No se pudo construir el dataset resuelto: {error}"))
}

#[tauri::command]
pub async fn resolve_dataset_conflicts(
    app: AppHandle,
    decisions: Vec<ConflictResolution>,
) -> Result<DatasetPreview, String> {
    let (_review_guard, cancellation) = ReviewMutationCancellation::begin(&app)?;
    let cancellation_for_work = cancellation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let comparison = snapshot_pending_comparison_for_review(&state, &cancellation_for_work)?;
        cancellation_for_work.ensure()?;
        let (source_context, initial_eager_frame, expected_stamp) = {
            let current = state.current.lock_recovering();
            let dataset = current.as_ref().ok_or_else(|| {
                "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
            })?;
            (
                current_join_context(dataset),
                (!dataset.source_backed).then(|| dataset.frame.clone()),
                DatasetMutationStamp::capture(dataset),
            )
        };

        if let Some(context) = source_context.as_ref() {
            cancellation_for_work.ensure()?;
            let compared_schema = read_parquet_schema_frame(&comparison.path)?;
            let compared_snapshot_size = fs::metadata(&comparison.path)
                .map_err(|error| format!("No se pudo verificar el snapshot comparado: {error}"))?
                .len();
            if compared_snapshot_size != comparison.file_size_bytes {
                return Err("El snapshot comparado cambió antes de resolver conflictos.".to_owned());
            }
            if let Some(preview) = resolve_source_backed_conflicts(
                &state,
                SourceBackedConflictResolutionRequest {
                    context: context.clone(),
                    compared_path: comparison.path.clone(),
                    compared_size_bytes: compared_snapshot_size,
                    compared_row_count: comparison.row_count,
                    compared_schema,
                    compared_file_name: comparison.file_name.clone(),
                    key_columns: comparison.key_columns.clone(),
                    decisions: decisions.clone(),
                },
                &expected_stamp,
                &comparison.identity_path,
                &cancellation_for_work,
            )? {
                return Ok(preview);
            }
        }
        cancellation_for_work.ensure()?;
        let compared_frame =
            read_parquet_frame_with_cancel(&comparison.path, || cancellation.is_cancelled())?;
        cancellation_for_work.ensure()?;
        let current_frame = match initial_eager_frame {
            Some(frame) => frame,
            None => {
                let current = state.current.lock_recovering();
                let dataset = current.as_ref().ok_or_else(|| {
                    "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
                })?;
                if !expected_stamp.matches(dataset) {
                    return Err(
                        "El dataset activo cambió durante la operación de Review.".to_owned()
                    );
                }
                materialized_dataset_frame_with_cancel(dataset, || {
                    cancellation_for_work.is_cancelled()
                })?
            }
        };
        cancellation_for_work.ensure()?;
        let is_cancelled = cancellation_for_work.callback();
        let resolved = resolved_conflict_frame_with_cancel(
            &current_frame,
            &compared_frame,
            &comparison.key_columns,
            &decisions,
            &is_cancelled,
        )?;
        let file_name = format!(
            "Resuelto · {} + {}",
            expected_stamp.file_name, comparison.file_name
        );
        let file_size_bytes = expected_stamp
            .file_size_bytes
            .saturating_add(comparison.file_size_bytes);
        publish_review_eager_candidate(
            &state,
            &expected_stamp,
            Some(&comparison.identity_path),
            false,
            resolved,
            &file_name,
            file_size_bytes,
            "Resolver conflictos por clave",
            &cancellation_for_work,
        )
    })
    .await
    .map_err(|error| format!("La resolución de conflictos se interrumpió: {error}"))?
}

#[tauri::command]
pub fn clear_dataset_comparison(state: State<'_, DatasetState>) -> Result<(), String> {
    *state.comparison.lock_recovering() = None;
    Ok(())
}

#[tauri::command]
pub async fn use_consolidated_dataset(app: AppHandle) -> Result<DatasetPreview, String> {
    let (_review_guard, cancellation) = ReviewMutationCancellation::begin(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let comparison = snapshot_pending_comparison_for_review(&state, &cancellation)?;
        cancellation.ensure()?;
        let (expected_stamp, source_context) = {
            let current = state
                .current
                .lock_recovering();
            let dataset = current.as_ref().ok_or_else(|| {
                "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
            })?;
            (
                DatasetMutationStamp::capture(dataset),
                current_join_context(dataset),
            )
        };

        if let Some(context) = source_context {
            cancellation.ensure()?;
            let compared_schema = read_parquet_schema_frame(&comparison.path)?;
            let source_result = consolidate_source_backed_dataset(
                &state,
                SourceBackedConsolidationRequest {
                    context,
                    compared_path: comparison.path.clone(),
                    compared_format: crate::duckdb_query::DuckDbFileFormat::Parquet,
                    compared_schema,
                    compared_file_name: comparison.file_name.clone(),
                    compared_size_bytes: comparison.file_size_bytes,
                    key_columns: comparison.key_columns.clone(),
                },
                &expected_stamp,
                &comparison.identity_path,
                &cancellation,
            )?;
            if let Some(preview) = source_result {
                return Ok(preview);
            }
        }

        cancellation.ensure()?;
        let compared_frame = read_parquet_frame_with_cancel(&comparison.path, || cancellation.is_cancelled())?;
        if compared_frame.height() != comparison.row_count {
            return Err("El snapshot comparado cambió antes de consolidar.".to_owned());
        }
        cancellation.ensure()?;
        let current_frame = {
            let current = state
                .current
                .lock_recovering();
            let dataset = current.as_ref().ok_or_else(|| {
                "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
            })?;
            if !expected_stamp.matches(dataset) {
                return Err("El dataset activo cambió durante la consolidación.".to_owned());
            }
            materialized_dataset_frame_with_cancel(dataset, || cancellation.is_cancelled())?
        };
        if current_frame.get_column_names() != compared_frame.get_column_names()
            || current_frame
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
        if !comparison.key_columns.is_empty() {
            let shared_columns = current_frame
                .get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let key_summary = compare_keyed_frames_with_cancel(
                &current_frame,
                &compared_frame,
                &comparison.key_columns,
                &shared_columns,
                cancellation.callback(),
            )?;
            if key_summary.conflicting_key_count > 0 || key_summary.duplicate_key_count > 0 {
                return Err(
                    "No se pueden consolidar claves con conflictos o duplicados. Revisa la comparación antes de continuar."
                        .to_owned(),
                );
            }
        }
        cancellation.ensure()?;
        let additions = if comparison.key_columns.is_empty() {
            compared_frame.clone()
        } else {
            rows_with_new_keys_with_cancel(
                &current_frame,
                &compared_frame,
                &comparison.key_columns,
                cancellation.callback(),
            )?
        };
        let mut consolidated = current_frame;
        consolidated
            .vstack_mut(&additions)
            .map_err(|error| format!("No se pudieron unir los datasets: {error}"))?;
        cancellation.ensure()?;
        let file_name = format!(
            "Consolidado · {} + {}",
            expected_stamp.file_name, comparison.file_name
        );
        let file_size_bytes = expected_stamp
            .file_size_bytes
            .saturating_add(comparison.file_size_bytes);
        publish_review_eager_candidate(
            &state,
            &expected_stamp,
            Some(&comparison.identity_path),
            false,
            consolidated,
            &file_name,
            file_size_bytes,
            "Consolidar datasets",
            &cancellation,
        )
    })
    .await
    .map_err(|error| format!("La consolidación se interrumpió: {error}"))?
}

async fn inspect_dataset_path(
    app: &AppHandle,
    path: PathBuf,
) -> Result<DatasetSourceInspection, String> {
    import_source_inspection::inspect_dataset_path_impl(app, path).await
}
#[tauri::command]
pub async fn pick_dataset_source(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    import_source_inspection::pick_dataset_source_impl(app).await
}

#[tauri::command]
pub async fn inspect_workbook_sheets(
    app: AppHandle,
    selection_id: String,
) -> Result<Vec<WorkbookSheet>, String> {
    import_source_inspection::inspect_workbook_sheets_impl(app, selection_id).await
}

/// Consumes a path captured by Tauri native drag and drop.
/// The path never crosses IPC; React receives only the resulting inspection.
#[tauri::command]
pub async fn inspect_dropped_dataset(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    import_source_inspection::inspect_dropped_dataset_impl(app).await
}

#[tauri::command]
pub async fn preview_delimited_header_review(
    app: AppHandle,
    selection_id: String,
) -> Result<DelimitedHeaderReview, String> {
    import_source_inspection::preview_delimited_header_review_impl(app, selection_id).await
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetImportSchemaPreview {
    row_count: usize,
    columns: Vec<DatasetColumn>,
    schema_mismatch: Option<ImportProfileMismatch>,
}

#[tauri::command]
pub async fn preview_dataset_selection(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
    expected_profile: Option<ImportProfile>,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
) -> Result<DatasetImportSchemaPreview, String> {
    import_schema_preview::preview_dataset_selection_impl(
        app,
        selection_id,
        sheet_id,
        header_mode,
        expected_profile,
        date_convention,
        number_convention,
    )
    .await
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn load_dataset_selection(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
    expected_profile: Option<ImportProfile>,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
    on_progress: Channel<OperationProgress>,
) -> Result<DatasetPreview, String> {
    import_loading::load_dataset_selection_impl(
        app,
        selection_id,
        sheet_id,
        header_mode,
        expected_profile,
        date_convention,
        number_convention,
        on_progress,
    )
    .await
}

#[tauri::command]
pub fn discard_dataset_selection(
    state: State<'_, DatasetState>,
    selection_id: String,
) -> Result<(), String> {
    import_loading::discard_dataset_selection_impl(state, selection_id)
}
#[tauri::command]
pub async fn get_dataset_page(
    app: AppHandle,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    page_reader::get_dataset_page_impl(app, offset, limit).await
}

#[tauri::command]
pub async fn query_dataset(
    app: AppHandle,
    query: String,
    engine: Option<DatasetQueryEngine>,
) -> Result<DatasetQueryResult, String> {
    let generation = app.state::<DatasetState>().begin_query();
    let engine = engine.unwrap_or_default();
    let query_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || match engine {
        DatasetQueryEngine::Polars => {
            let state = query_app.state::<DatasetState>();
            let mut current = state.current.lock_recovering();
            let dataset = current
                .as_mut()
                .ok_or_else(|| "No hay un dataset activo para consultar.".to_owned())?;
            let comparison = state.comparison.lock_recovering();
            let current_snapshot =
                current_history_parquet_snapshot(dataset).map(|(path, _, _)| path);
            let current_file_source = current_duckdb_file_source(dataset);
            let source_backed_query = dataset.source_backed;
            let compared_snapshot = comparison
                .as_ref()
                .map(|pending| pending.snapshot_path.clone());
            if should_route_join_to_duckdb(&query, compared_snapshot.is_some()) {
                let compared_path = compared_snapshot
                    .as_ref()
                    .expect("la ruta DuckDB requiere un snapshot comparado");
                let current_schema = current_snapshot
                    .as_ref()
                    .map(|path| read_parquet_schema_frame(path).ok())
                    .unwrap_or_else(|| Some(dataset.frame.slice(0, 0)));
                let compared_schema = read_parquet_schema_frame(compared_path).ok();
                if let (Some(current_schema), Some(compared_schema)) =
                    (current_schema.as_ref(), compared_schema.as_ref())
                {
                    if let Ok(spec) = prepare_duckdb_query_with_row_count(
                        &query,
                        current_schema,
                        Some(compared_schema),
                        dataset.row_count,
                    ) {
                        let result = if let Some(current_path) = current_snapshot.as_ref() {
                            let cancellation_app = query_app.clone();
                            Some(
                                crate::duckdb_query::execute_duckdb_query_from_parquet_sources(
                                    current_path,
                                    Some(compared_path),
                                    &spec,
                                    move || {
                                        cancellation_app
                                            .state::<DatasetState>()
                                            .query_was_cancelled(generation)
                                    },
                                ),
                            )
                        } else if let Some((current_path, current_format)) =
                            current_file_source.as_ref()
                        {
                            let cancellation_app = query_app.clone();
                            Some(crate::duckdb_query::execute_duckdb_query_from_file_sources(
                                current_path,
                                *current_format,
                                Some(compared_path),
                                &spec,
                                move || {
                                    cancellation_app
                                        .state::<DatasetState>()
                                        .query_was_cancelled(generation)
                                },
                            ))
                        } else {
                            let cancellation_app = query_app.clone();
                            Some(
                                crate::duckdb_query::execute_duckdb_query_from_frame_and_parquet(
                                    &dataset.frame,
                                    compared_path,
                                    &spec,
                                    move || {
                                        cancellation_app
                                            .state::<DatasetState>()
                                            .query_was_cancelled(generation)
                                    },
                                ),
                            )
                        };
                        if let Some(result) = result {
                            match result {
                                Ok(result) => return Ok(result),
                                Err(error) if error == OPERATION_CANCELLED_MESSAGE => {
                                    return Err(error);
                                }
                                Err(_) => {}
                            }
                        }
                    }
                }
            }
            if !dataset.history.snapshots_enabled && current_snapshot.is_none() {
                if let Some((current_path, current_format)) = current_file_source.as_ref() {
                    let current_schema = dataset.frame.slice(0, 0);
                    let compared_schema = compared_snapshot
                        .as_ref()
                        .map(|path| read_parquet_schema_frame(path))
                        .transpose()?;
                    if let Ok(spec) = prepare_duckdb_query_with_row_count(
                        &query,
                        &current_schema,
                        compared_schema.as_ref(),
                        dataset.row_count,
                    ) {
                        let cancellation_app = query_app.clone();
                        match crate::duckdb_query::execute_duckdb_query_from_file_sources(
                            current_path,
                            *current_format,
                            compared_snapshot.as_deref(),
                            &spec,
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .query_was_cancelled(generation)
                            },
                        ) {
                            Ok(result) => return Ok(result),
                            Err(error) if error == OPERATION_CANCELLED_MESSAGE => {
                                return Err(error);
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
            if comparison.is_none() && !local_query_has_join(&query) {
                if let Some(path) = current_snapshot {
                    match execute_local_query_from_parquet_with_cancel(
                        &path,
                        dataset.row_count,
                        &query,
                        &|| state.query_was_cancelled(generation),
                    ) {
                        Ok(result) => return Ok(result),
                        Err(error) if error.starts_with(LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX) => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            if source_backed_query {
                return Err(SOURCE_BACKED_QUERY_ERROR.to_owned());
            }
            materialize_loaded_dataset_with_cancel(dataset, || {
                state.query_was_cancelled(generation)
            })?;
            let compared_frame = comparison
                .as_ref()
                .map(|pending| {
                    read_parquet_frame_with_cancel(&pending.snapshot_path, || {
                        state.query_was_cancelled(generation)
                    })
                })
                .transpose()?;
            let compared = compared_frame.as_ref();
            execute_local_query_with_comparison_and_cancel(
                &dataset.frame,
                compared,
                &query,
                &|| state.query_was_cancelled(generation),
            )
        }
        DatasetQueryEngine::Duckdb => {
            let state = query_app.state::<DatasetState>();
            let mut current = state.current.lock_recovering();
            let dataset = current
                .as_mut()
                .ok_or_else(|| "No hay un dataset activo para consultar.".to_owned())?;
            let comparison = state.comparison.lock_recovering();
            let compared_snapshot = comparison
                .as_ref()
                .map(|pending| pending.snapshot_path.as_path());
            let current_snapshot =
                current_history_parquet_snapshot(dataset).map(|(path, _, _)| path);
            let current_file_source = current_duckdb_file_source(dataset);
            if dataset.source_backed && current_file_source.is_none() && current_snapshot.is_none()
            {
                return Err(SOURCE_BACKED_QUERY_ERROR.to_owned());
            }
            let current_validation_frame = current_snapshot
                .as_deref()
                .map(read_parquet_schema_frame)
                .transpose()?;
            let current_frame = current_validation_frame.as_ref().unwrap_or(&dataset.frame);
            let compared_frame = comparison
                .as_ref()
                .map(|pending| read_parquet_schema_frame(&pending.snapshot_path))
                .transpose()?;
            let compared = compared_frame.as_ref();
            let spec = prepare_duckdb_query_with_row_count(
                &query,
                current_frame,
                compared,
                dataset.row_count,
            )?;
            let cancellation_app = query_app.clone();
            let fallback_cancellation_app = query_app.clone();
            if let Some(path) = current_snapshot {
                if let Some(compared_path) = compared_snapshot {
                    crate::duckdb_query::execute_duckdb_query_from_parquet_sources(
                        &path,
                        Some(compared_path),
                        &spec,
                        move || {
                            cancellation_app
                                .state::<DatasetState>()
                                .query_was_cancelled(generation)
                        },
                    )
                } else {
                    crate::duckdb_query::execute_duckdb_query_from_parquet(
                        &path,
                        None,
                        &spec,
                        move || {
                            cancellation_app
                                .state::<DatasetState>()
                                .query_was_cancelled(generation)
                        },
                    )
                }
            } else if let Some((path, format)) = current_file_source {
                if let Some(compared_path) = compared_snapshot {
                    crate::duckdb_query::execute_duckdb_query_from_file_sources(
                        &path,
                        format,
                        Some(compared_path),
                        &spec,
                        move || {
                            fallback_cancellation_app
                                .state::<DatasetState>()
                                .query_was_cancelled(generation)
                        },
                    )
                } else {
                    crate::duckdb_query::execute_duckdb_query_from_file(
                        &path,
                        format,
                        compared,
                        &spec,
                        move || {
                            fallback_cancellation_app
                                .state::<DatasetState>()
                                .query_was_cancelled(generation)
                        },
                    )
                }
            } else if let Some(compared_path) = compared_snapshot {
                crate::duckdb_query::execute_duckdb_query_from_frame_and_parquet(
                    &dataset.frame,
                    compared_path,
                    &spec,
                    move || {
                        fallback_cancellation_app
                            .state::<DatasetState>()
                            .query_was_cancelled(generation)
                    },
                )
            } else {
                crate::duckdb_query::execute_duckdb_query(
                    &dataset.frame,
                    compared,
                    &spec,
                    move || {
                        fallback_cancellation_app
                            .state::<DatasetState>()
                            .query_was_cancelled(generation)
                    },
                )
            }
        }
    })
    .await
    .map_err(|error| format!("La consulta local se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn get_dataset_profile(
    app: AppHandle,
    on_progress: Channel<OperationProgress>,
    correlation_sample_rows: Option<usize>,
) -> Result<DatasetProfile, String> {
    profile_reader::get_dataset_profile_impl(app, on_progress, correlation_sample_rows).await
}

#[tauri::command]
pub async fn get_temporal_aggregation(
    app: AppHandle,
    date_column: String,
    value_column: String,
    aggregation: TemporalAggregationKind,
) -> Result<TemporalAggregationSeries, String> {
    profile_reader::get_temporal_aggregation_impl(app, date_column, value_column, aggregation).await
}

#[tauri::command]
pub fn cancel_operation(app: AppHandle, operation: String) -> Result<(), String> {
    if operation == crate::reusable_tasks::REUSABLE_TASK_CATALOG_OPERATION {
        app.state::<crate::reusable_tasks::ReusableTaskState>()
            .cancel_catalog();
        Ok(())
    } else if operation == crate::delivery_presets::DELIVERY_PRESET_CATALOG_OPERATION {
        app.state::<crate::delivery_presets::DeliveryPresetState>()
            .cancel_catalog();
        Ok(())
    } else if operation == "updateCheck" {
        #[cfg(desktop)]
        {
            crate::updater::cancel_update_check(&app)
        }
        #[cfg(not(desktop))]
        {
            Err("La comprobación de actualizaciones no está disponible.".to_owned())
        }
    } else {
        app.state::<DatasetState>().cancel(&operation)
    }
}

#[tauri::command]
pub async fn validate_quality_rules(
    app: AppHandle,
    quality_rules: Vec<QualityRule>,
) -> Result<QualityValidationResult, String> {
    validate_quality_rules_payload(&quality_rules)?;
    let cancellation = QualityValidationCancellation::begin(&app);
    cancellation.ensure()?;
    let is_cancelled = cancellation.callback();
    let source_context = {
        let state = app.state::<DatasetState>();
        let current = state.current.lock_recovering();
        current.as_ref().and_then(|dataset| {
            if dataset.source_backed {
                current_source_backed_context(dataset)
            } else {
                current_history_parquet_snapshot(dataset)
            }
        })
    };
    cancellation.ensure()?;
    if quality_rules.iter().all(source_quality_rule_is_incremental) {
        if let Some((source_path, expected_file_size, row_count)) = source_context {
            let cancellation = cancellation.clone();
            return tauri::async_runtime::spawn_blocking(move || {
                cancellation.ensure()?;
                let (source_path, source_size, extension) = validate_dataset_file(&source_path)?;
                cancellation.ensure()?;
                if source_size != expected_file_size {
                    return Err(
                        "El archivo source-backed cambió desde la carga; vuelve a seleccionarlo."
                            .to_owned(),
                    );
                }
                let result = evaluate_source_quality_rules_with_cancel(
                    &source_path,
                    &extension,
                    expected_file_size,
                    row_count,
                    &quality_rules,
                    cancellation.callback(),
                )?;
                cancellation.ensure()?;
                let current_context = {
                    let state = app.state::<DatasetState>();
                    let current = state.current.lock_recovering();
                    current.as_ref().and_then(|dataset| {
                        if dataset.source_backed {
                            current_source_backed_context(dataset)
                        } else {
                            current_history_parquet_snapshot(dataset)
                        }
                    })
                };
                cancellation.ensure()?;
                if current_context != Some((source_path.clone(), expected_file_size, row_count)) {
                    return Err(
                        "El dataset activo cambió durante la validación de calidad.".to_owned()
                    );
                }
                Ok(result)
            })
            .await
            .map_err(|error| format!("La validación de calidad se interrumpió: {error}"))?;
        }
    }
    let state = app.state::<DatasetState>();
    let (frame, _) = materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
    cancellation.ensure()?;
    let cancellation = cancellation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            evaluate_quality_rules_with_cancel(&frame, &quality_rules, cancellation.callback())?;
        cancellation.ensure()?;
        Ok(result)
    })
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
    let source_context = {
        let state = app.state::<DatasetState>();
        let current = state.current.lock_recovering();
        current.as_ref().and_then(|dataset| {
            if dataset.source_backed {
                let (source_path, source_size, row_count) = current_source_backed_context(dataset)?;
                let original_source_path = dataset.source_path.as_ref()?.clone();
                Some((
                    source_path,
                    dataset.file_name.clone(),
                    source_size,
                    row_count,
                    original_source_path,
                    dataset.file_size_bytes,
                    false,
                ))
            } else {
                let (source_path, source_size, row_count) =
                    current_history_parquet_snapshot(dataset)?;
                Some((
                    source_path.clone(),
                    dataset.file_name.clone(),
                    source_size,
                    row_count,
                    source_path,
                    source_size,
                    true,
                ))
            }
        })
    };
    // Privacy is applied into a temporary Parquet snapshot below, so mask/hash
    // remain source-backed instead of forcing the active dataset into memory.
    // A recipe is Bundle metadata, not a second transformation pass; it must
    // not disable the streaming export path.
    if matches!(
        format,
        ExportFormat::Csv
            | ExportFormat::Json
            | ExportFormat::Parquet
            | ExportFormat::Sql
            | ExportFormat::Excel
            | ExportFormat::Sqlite
            | ExportFormat::Bundle
    ) && quality_rules.iter().all(source_quality_rule_is_incremental)
    {
        if let Some((
            source_path,
            file_name,
            expected_file_size,
            row_count,
            original_source_path,
            expected_original_file_size,
            snapshot_only,
        )) = source_context
        {
            let generation = app.state::<DatasetState>().begin_export();
            let validation_app = app.clone();
            let source_quality_rules = quality_rules.clone();
            let validation_source_path = source_path.clone();
            let validation_original_source_path = original_source_path.clone();
            let quality_validation = tauri::async_runtime::spawn_blocking(move || {
                let (_, original_source_size, _) =
                    validate_dataset_file(&validation_original_source_path)?;
                if original_source_size != expected_original_file_size {
                    return Err(
                        "El archivo source-backed original cambió desde la carga; vuelve a seleccionarlo."
                            .to_owned(),
                    );
                }
                let (source_path, source_size, extension) =
                    validate_dataset_file(&validation_source_path)?;
                if source_size != expected_file_size {
                    return Err(
                        "El archivo source-backed cambió desde la carga; vuelve a seleccionarlo."
                            .to_owned(),
                    );
                }
                let quality_validation_app = validation_app.clone();
                enforce_source_quality_with_cancel(
                    &source_path,
                    &extension,
                    expected_file_size,
                    row_count,
                    &source_quality_rules,
                    allow_unvalidated,
                    move || {
                        quality_validation_app
                            .state::<DatasetState>()
                            .export_was_cancelled(generation)
                    },
                )
            })
            .await
            .map_err(|error| {
                format!("La validación previa a la exportación se interrumpió: {error}")
            })??;
            ensure_not_cancelled(app.state::<DatasetState>().export_was_cancelled(generation))?;

            let stem = Path::new(&file_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("dataset");
            send_progress(&on_progress, "export", "Esperando destino", 0);
            let selection = app
                .dialog()
                .file()
                .add_filter(format.label(), &[format.extension()])
                .set_file_name(format!("{stem}-columnia.{}", format.extension()))
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
            let export_original_source_path = original_source_path.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let mut effective_source_path = source_path.clone();
                let mut effective_source_size = expected_file_size;
                let mut protected_columns = Vec::new();
                let _privacy_scratch = if privacy_mode == PrivacyMode::None {
                    None
                } else {
                    let scratch = tempfile::tempdir_in(destination.parent().ok_or_else(|| {
                        "No se pudo resolver la carpeta de exportación.".to_owned()
                    })?)
                    .map_err(|error| {
                        format!("No se pudo preparar el snapshot protegido: {error}")
                    })?;
                    let privacy_snapshot = scratch.path().join("protected.parquet");
                    let cancellation_app = app.clone();
                    protected_columns = source_backed_privacy_snapshot(
                        &source_path,
                        expected_file_size,
                        &privacy_snapshot,
                        privacy_mode,
                        move || {
                            cancellation_app
                                .state::<DatasetState>()
                                .export_was_cancelled(generation)
                        },
                    )?;
                    if !protected_columns.is_empty() {
                        effective_source_size = fs::metadata(&privacy_snapshot)
                            .map_err(|error| {
                                format!("No se pudo verificar el snapshot protegido: {error}")
                            })?
                            .len();
                        effective_source_path = privacy_snapshot;
                    }
                    Some(scratch)
                };
                let result = match format {
                    ExportFormat::Csv => {
                        let cancellation_app = app.clone();
                        export_source_backed_csv_atomic(
                            &effective_source_path,
                            effective_source_size,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Json => {
                        let cancellation_app = app.clone();
                        export_source_backed_json_atomic(
                            &effective_source_path,
                            effective_source_size,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Parquet => {
                        let cancellation_app = app.clone();
                        export_source_backed_parquet_atomic(
                            &effective_source_path,
                            effective_source_size,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Sql => {
                        let cancellation_app = app.clone();
                        export_source_backed_sql_atomic(
                            &effective_source_path,
                            effective_source_size,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Excel => {
                        let cancellation_app = app.clone();
                        export_source_backed_xlsx_atomic(
                            &effective_source_path,
                            effective_source_size,
                            row_count,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Sqlite => {
                        let cancellation_app = app.clone();
                        export_source_backed_sqlite_atomic(
                            &effective_source_path,
                            effective_source_size,
                            row_count,
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                    ExportFormat::Bundle => {
                        let cancellation_app = app.clone();
                        export_source_backed_bundle_atomic(
                            &effective_source_path,
                            effective_source_size,
                            row_count,
                            quality_validation.as_ref(),
                            recipe.as_ref(),
                            &destination,
                            |stage, percent| send_progress(&on_progress, "export", stage, percent),
                            move || {
                                cancellation_app
                                    .state::<DatasetState>()
                                    .export_was_cancelled(generation)
                            },
                        )
                    }
                };
                let result = result?;
                if snapshot_only {
                    let current_snapshot = {
                        let state = app.state::<DatasetState>();
                        let current = state.current.lock_recovering();
                        current.as_ref().and_then(current_history_parquet_snapshot)
                    };
                    let snapshot_is_current = current_snapshot.is_some_and(
                        |(current_path, current_size, current_row_count)| {
                            current_path == source_path
                                && current_size == expected_file_size
                                && current_row_count == row_count
                        },
                    );
                    if !snapshot_is_current {
                        return Err(
                            "El snapshot Parquet activo cambió durante la exportación.".to_owned()
                        );
                    }
                }
                let (_, final_original_source_size, _) =
                    validate_dataset_file(&export_original_source_path)?;
                if final_original_source_size != expected_original_file_size {
                    return Err(
                        "El archivo source-backed original cambió durante la exportación."
                            .to_owned(),
                    );
                }
                let mut result = result;
                result.protected_column_count = protected_columns.len();
                result.protected_columns = protected_columns;
                Ok(Some(result))
            })
            .await
            .map_err(|error| format!("La exportación se interrumpió: {error}"))??;
            if result.is_some() {
                export_state
                    .state::<DatasetState>()
                    .remember_last_export(remembered_destination);
            }
            return Ok(result);
        }
    }
    let generation = app.state::<DatasetState>().begin_export();
    let preparation_app = app.clone();
    let output_extension = format.extension().to_owned();
    let (frame, suggested_name) = tauri::async_runtime::spawn_blocking(move || {
        let state = preparation_app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        materialize_loaded_dataset_with_cancel(dataset, || state.export_was_cancelled(generation))?;
        let stem = Path::new(&dataset.file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("dataset");
        Ok::<_, String>((
            dataset.frame.clone(),
            format!("{stem}-columnia.{output_extension}"),
        ))
    })
    .await
    .map_err(|error| format!("La preparación previa a la exportación se interrumpió: {error}"))??;

    // La compuerta se evalúa sobre el mismo snapshot que después será escrito y
    // antes de abrir el selector, para que una exportación bloqueada no solicite destino.
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

/// The native dialog blocks, so it runs on a blocking worker, not the async runtime.
async fn confirm_remote_target_off_main_thread(
    app: &AppHandle,
    target: &DatabaseTarget,
) -> Result<(), String> {
    let app = app.clone();
    let target = target.clone();
    tauri::async_runtime::spawn_blocking(move || {
        remote_databases::confirm_remote_target(&app, &target)
    })
    .await
    .map_err(|_| "La confirmación de la conexión se interrumpió.".to_owned())?
}

#[tauri::command]
pub async fn preflight_database_export(
    app: AppHandle,
    target: DatabaseTarget,
    privacy_mode: PrivacyMode,
) -> Result<remote_databases::RemoteExportPreflight, String> {
    remote_databases::validate_database_target(&target)?;
    confirm_remote_target_off_main_thread(&app, &target).await?;
    let cancellation = DatabasePreflightCancellation::begin(&app);
    cancellation.ensure()?;
    let is_cancelled = cancellation.callback();
    let (source_context, frame) = {
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let source_context = if dataset.source_backed {
            let (_, source_size, row_count) =
                current_source_backed_context(dataset).ok_or_else(|| {
                    "La fuente source-backed cambió o ya no está disponible.".to_owned()
                })?;
            let (source_path, source_format) =
                current_duckdb_file_source(dataset).ok_or_else(|| {
                    "La fuente source-backed ya no está disponible para el preflight.".to_owned()
                })?;
            let original_source_path = dataset.source_path.as_ref().cloned().ok_or_else(|| {
                "La fuente source-backed ya no está disponible para el preflight.".to_owned()
            })?;
            Some((
                source_path,
                source_format,
                dataset.frame.clone(),
                source_size,
                row_count,
                original_source_path,
                dataset.file_size_bytes,
            ))
        } else if let Some((snapshot_path, snapshot_size, row_count)) =
            current_history_parquet_snapshot(dataset)
        {
            Some((
                snapshot_path.clone(),
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                dataset.frame.slice(0, 0),
                snapshot_size,
                row_count,
                snapshot_path,
                snapshot_size,
            ))
        } else {
            None
        };
        let frame = if source_context.is_some() {
            None
        } else {
            materialize_loaded_dataset_with_cancel(dataset, &is_cancelled)?;
            cancellation.ensure()?;
            Some(dataset.frame.clone())
        };
        (source_context, frame)
    };

    let worker_cancellation = cancellation.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let is_cancelled = worker_cancellation.callback();
        worker_cancellation.ensure()?;
        if let Some((
            source_path,
            source_format,
            schema,
            expected_source_size,
            row_count,
            original_source_path,
            expected_original_file_size,
        )) = source_context
        {
            let (_, source_size, _) = validate_dataset_file(&source_path)?;
            if source_size != expected_source_size {
                return Err(
                    "La fuente cambió desde su carga; vuelve a revisarla antes de entregar."
                        .to_owned(),
                );
            }
            worker_cancellation.ensure()?;
            let (_, original_source_size, _) = validate_dataset_file(&original_source_path)?;
            if original_source_size != expected_original_file_size {
                return Err(
                    "El archivo original cambió desde su carga; vuelve a seleccionarlo.".to_owned(),
                );
            }
            worker_cancellation.ensure()?;
            if privacy_mode == PrivacyMode::None {
                return remote_databases::preflight_source_backed(
                    &source_path,
                    source_format,
                    &schema,
                    row_count,
                    &target,
                    is_cancelled.clone(),
                );
            }
            let scratch = tempfile::tempdir()
                .map_err(|error| format!("No se pudo preparar el preflight protegido: {error}"))?;
            let protected_snapshot = scratch.path().join("preflight-protected.parquet");
            let protected_columns = source_backed_privacy_snapshot(
                &source_path,
                expected_source_size,
                &protected_snapshot,
                privacy_mode,
                is_cancelled.clone(),
            )?;
            worker_cancellation.ensure()?;
            if protected_columns.is_empty() {
                return remote_databases::preflight_source_backed(
                    &source_path,
                    source_format,
                    &schema,
                    row_count,
                    &target,
                    is_cancelled.clone(),
                );
            }
            let protected_schema = read_parquet_schema_frame(&protected_snapshot)?;
            remote_databases::preflight_source_backed(
                &protected_snapshot,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                &protected_schema,
                row_count,
                &target,
                is_cancelled.clone(),
            )
        } else {
            let frame = frame
                .ok_or_else(|| "No se pudo preparar el dataset para el preflight.".to_owned())?;
            let (protected_frame, _) =
                privacy_safe_frame_with_cancel(&frame, privacy_mode, &is_cancelled)?;
            let input_columns = remote_databases::input_shape_from_frame_with_cancel(
                &protected_frame,
                &is_cancelled,
            )?;
            worker_cancellation.ensure()?;
            remote_databases::preflight_export_with_cancel(&target, &input_columns, &is_cancelled)
        }
    })
    .await
    .map_err(|error| format!("El preflight remoto se interrumpió: {error}"))?;
    cancellation.ensure()?;
    result
}

#[tauri::command]
pub async fn export_dataset_to_database(
    app: AppHandle,
    target: DatabaseTarget,
    quality_rules: Vec<QualityRule>,
    allow_unvalidated: bool,
    privacy_mode: PrivacyMode,
    on_progress: Channel<OperationProgress>,
) -> Result<ExportResult, String> {
    remote_databases::validate_database_target(&target)?;
    validate_quality_rules_payload(&quality_rules)?;
    confirm_remote_target_off_main_thread(&app, &target).await?;

    let source_context = {
        let state = app.state::<DatasetState>();
        let current = state.current.lock_recovering();
        current.as_ref().and_then(|dataset| {
            if dataset.source_backed {
                let (_, source_size, row_count) = current_source_backed_context(dataset)?;
                let (source_path, source_format) = current_duckdb_file_source(dataset)?;
                let original_source_path = dataset.source_path.as_ref()?.clone();
                Some((
                    source_path,
                    source_format,
                    dataset.frame.clone(),
                    dataset.file_name.clone(),
                    source_size,
                    row_count,
                    original_source_path,
                    dataset.file_size_bytes,
                    false,
                ))
            } else {
                let (source_path, source_size, row_count) =
                    current_history_parquet_snapshot(dataset)?;
                Some((
                    source_path.clone(),
                    crate::duckdb_query::DuckDbFileFormat::Parquet,
                    dataset.frame.slice(0, 0),
                    dataset.file_name.clone(),
                    source_size,
                    row_count,
                    source_path,
                    source_size,
                    true,
                ))
            }
        })
    };

    // Reuse the same source-backed quality and privacy boundaries as local
    // exports. Only the unsupported quality/privacy combinations fall through
    // to the materialized path below.
    let source_privacy_supported = |source_format: crate::duckdb_query::DuckDbFileFormat| {
        privacy_mode == PrivacyMode::None
            || matches!(
                source_format,
                crate::duckdb_query::DuckDbFileFormat::Parquet
                    | crate::duckdb_query::DuckDbFileFormat::Delimited { .. }
            )
    };
    if quality_rules.iter().all(source_quality_rule_is_incremental) {
        if let Some((
            source_path,
            source_format,
            schema,
            _file_name,
            expected_file_size,
            row_count,
            original_source_path,
            expected_original_file_size,
            snapshot_only,
        )) = source_context.filter(|(_, source_format, _, _, _, _, _, _, _)| {
            source_privacy_supported(*source_format)
        }) {
            let generation = app.state::<DatasetState>().begin_export();
            let validation_app = app.clone();
            let source_quality_rules = quality_rules.clone();
            let target = target.clone();
            let source_progress = on_progress.clone();
            send_progress(
                &source_progress,
                "export",
                "Validando fuente source-backed",
                0,
            );
            let result = tauri::async_runtime::spawn_blocking(move || {
                let (_, original_source_size, _) = validate_dataset_file(&original_source_path)?;
                if original_source_size != expected_original_file_size {
                    return Err(
                        "El archivo source-backed original cambió desde la carga; vuelve a seleccionarlo."
                            .to_owned(),
                    );
                }
                let (source_path, source_size, extension) = validate_dataset_file(&source_path)?;
                if source_size != expected_file_size {
                    return Err(
                        "El archivo source-backed cambió desde la carga; vuelve a seleccionarlo."
                            .to_owned(),
                    );
                }
                let quality_validation_app = validation_app.clone();
                enforce_source_quality_with_cancel(
                    &source_path,
                    &extension,
                    expected_file_size,
                    row_count,
                    &source_quality_rules,
                    allow_unvalidated,
                    move || {
                        quality_validation_app
                            .state::<DatasetState>()
                            .export_was_cancelled(generation)
                    },
                )?;
                ensure_not_cancelled(
                    validation_app
                        .state::<DatasetState>()
                        .export_was_cancelled(generation),
                )?;

                let mut effective_source_path = source_path.clone();
                let mut effective_source_format = source_format;
                let mut effective_schema = schema;
                let mut protected_columns = Vec::new();
                let _privacy_scratch = if privacy_mode == PrivacyMode::None {
                    None
                } else {
                    let scratch = tempfile::tempdir().map_err(|error| {
                        format!("No se pudo preparar el snapshot protegido: {error}")
                    })?;
                    let privacy_snapshot = scratch.path().join("protected.parquet");
                    let cancellation_app = validation_app.clone();
                    protected_columns = source_backed_privacy_snapshot(
                        &source_path,
                        expected_file_size,
                        &privacy_snapshot,
                        privacy_mode,
                        move || {
                            cancellation_app
                                .state::<DatasetState>()
                                .export_was_cancelled(generation)
                        },
                    )?;
                    if !protected_columns.is_empty() {
                        fs::metadata(&privacy_snapshot)
                            .map_err(|error| {
                                format!("No se pudo verificar el snapshot protegido: {error}")
                            })?;
                        effective_source_path = privacy_snapshot;
                        effective_source_format = crate::duckdb_query::DuckDbFileFormat::Parquet;
                        effective_schema = read_parquet_schema_frame(&effective_source_path)?;
                    }
                    Some(scratch)
                };

                let cancellation_app = validation_app.clone();
                let remote_result = remote_databases::export_source_backed(
                    &effective_source_path,
                    effective_source_format,
                    &effective_schema,
                    row_count,
                    &target,
                    |stage, percent| send_progress(&source_progress, "export", stage, percent),
                    move || {
                        cancellation_app
                            .state::<DatasetState>()
                            .export_was_cancelled(generation)
                    },
                )?;
                if snapshot_only {
                    let current_snapshot = {
                        let state = validation_app.state::<DatasetState>();
                        let current = state.current.lock_recovering();
                        current.as_ref().and_then(current_history_parquet_snapshot)
                    };
                    let snapshot_is_current = current_snapshot.is_some_and(
                        |(current_path, current_size, current_row_count)| {
                            current_path == source_path
                                && current_size == expected_file_size
                                && current_row_count == row_count
                        },
                    );
                    if !snapshot_is_current {
                        return Err(
                            "El snapshot Parquet activo cambió durante la entrega ODBC."
                                .to_owned(),
                        );
                    }
                }
                let (_, final_original_source_size, _) =
                    validate_dataset_file(&original_source_path)?;
                if final_original_source_size != expected_original_file_size {
                    return Err(
                        "El archivo source-backed original cambió durante la entrega ODBC."
                            .to_owned(),
                    );
                }
                Ok(ExportResult {
                    file_name: remote_result.table_name,
                    file_size_bytes: 0,
                    format: remote_result.format,
                    protected_column_count: protected_columns.len(),
                    protected_columns,
                })
            })
            .await
            .map_err(|error| format!("La entrega remota se interrumpió: {error}"))??;
            return Ok(result);
        }
    }

    let generation = app.state::<DatasetState>().begin_export();
    let preparation_app = app.clone();
    let (protected_frame, protected_columns) = tauri::async_runtime::spawn_blocking(move || {
        let frame = {
            let state = preparation_app.state::<DatasetState>();
            let mut current = state.current.lock_recovering();
            let dataset = current.as_mut().ok_or_else(|| {
                "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
            })?;
            materialize_loaded_dataset_with_cancel(dataset, || {
                preparation_app
                    .state::<DatasetState>()
                    .export_was_cancelled(generation)
            })?;
            dataset.frame.clone()
        };
        let validation_app = preparation_app.clone();
        enforce_export_quality_with_cancel(&frame, &quality_rules, allow_unvalidated, || {
            validation_app
                .state::<DatasetState>()
                .export_was_cancelled(generation)
        })?;
        ensure_not_cancelled(
            preparation_app
                .state::<DatasetState>()
                .export_was_cancelled(generation),
        )?;
        privacy_safe_frame(&frame, privacy_mode)
    })
    .await
    .map_err(|error| format!("La preparación de la entrega remota se interrumpió: {error}"))??;
    ensure_not_cancelled(app.state::<DatasetState>().export_was_cancelled(generation))?;

    send_progress(&on_progress, "export", "Conectando con destino remoto", 0);
    let export_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        remote_databases::export_frame(
            &protected_frame,
            &target,
            |stage, percent| send_progress(&on_progress, "export", stage, percent),
            || {
                export_app
                    .state::<DatasetState>()
                    .export_was_cancelled(generation)
            },
        )
    })
    .await
    .map_err(|error| format!("La entrega remota se interrumpió: {error}"))??;

    Ok(ExportResult {
        file_name: result.table_name,
        file_size_bytes: 0,
        format: result.format,
        protected_column_count: protected_columns.len(),
        protected_columns,
    })
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
    source_schema: Vec<RecipeSourceColumn>,
    export_options: Option<RecipeExportOptions>,
) -> Result<Option<StoredTransformRecipe>, String> {
    let mut document = build_stored_recipe(recipe, name)?;
    document.source_schema = Some(source_schema);
    document.export_options = export_options;
    validate_stored_recipe(&document)?;
    let suggested_name = recipe_suggested_file_name(&document.name);
    let selection = app
        .dialog()
        .file()
        .add_filter("Receta Columnia", &["json"])
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
        .add_filter("Receta Columnia", &["json"])
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
pub async fn remove_duplicates(app: AppHandle) -> Result<DatasetMutation, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                remove_duplicates_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count) = remove_duplicate_rows(&dataset.frame)?;
        cancellation.ensure()?;

        let preview = if affected_row_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar filas duplicadas",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                remove_near_duplicates_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count) = remove_near_duplicate_rows(&dataset.frame)?;
        cancellation.ensure()?;

        let preview = if affected_row_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar filas duplicadas parecidas",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                remove_empty_rows_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count) = remove_empty_rows_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if affected_row_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar filas completamente vacías",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                enable_row_audit_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (candidate, added) = add_audit_column_to_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if added {
            publish_candidate_with_cancellation(
                dataset,
                candidate,
                "Activar trazabilidad por fila",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) = remove_columns_source_backed_with_cancellation(
                dataset,
                SourceBackedColumnCleanup::Constant,
                "Eliminar columnas constantes",
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, removed_columns) = remove_constant_columns_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar columnas constantes",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) = remove_columns_source_backed_with_cancellation(
                dataset,
                SourceBackedColumnCleanup::Empty,
                "Eliminar columnas completamente vacías",
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, removed_columns) = remove_empty_columns_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar columnas completamente vacías",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) = remove_columns_source_backed_with_cancellation(
                dataset,
                SourceBackedColumnCleanup::HighNull,
                "Eliminar columnas con alta nulidad",
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, removed_columns) = remove_high_null_columns_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Eliminar columnas con alta nulidad",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) = remove_columns_source_backed_with_cancellation(
                dataset,
                SourceBackedColumnCleanup::Identifier,
                "Retirar columnas identificadoras",
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Retirar columnas identificadoras",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(mut result) = remove_columns_source_backed_with_cancellation(
                dataset,
                SourceBackedColumnCleanup::Personal,
                "Retirar datos personales detectados",
                &cancellation,
            )? {
                result.removed_columns.clear();
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, removed_columns) = remove_personal_columns_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let removed_column_count = removed_columns.len();
        let preview = if removed_columns.is_empty() {
            loaded_dataset_preview(dataset, &dataset.frame)?
        } else {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Retirar datos personales detectados",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                mask_personal_values_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (masked, changed_cell_count, changed_column_count) =
            mask_personal_values_from_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if changed_cell_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                masked,
                "Proteger valores personales detectados",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                normalize_column_names_source_backed_with_cancellation(dataset, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (names, renames) = normalized_column_names(&dataset.frame);
        cancellation.ensure()?;

        let preview = if !renames.is_empty() {
            let mut candidate = dataset.frame.clone();
            candidate
                .set_column_names(&names)
                .map_err(|error| format!("No se pudieron normalizar las columnas: {error}"))?;
            publish_candidate_with_cancellation(
                dataset,
                candidate,
                "Normalizar nombres de columnas",
                Some(&cancellation),
            )?
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
    cancellation: PrepareCancellation,
) -> Result<TextCleaningResult, String> {
    cancellation.ensure()?;
    let state = app.state::<DatasetState>();
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if dataset.source_backed {
        if let Some(result) = source_backed_text_cleaning_with_cancellation(
            dataset,
            selected_columns.as_deref(),
            mode,
            cancellation.clone(),
        )? {
            return Ok(result);
        }
    }
    cancellation.ensure()?;
    materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
    let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
        clean_text_columns(&dataset.frame, selected_columns.as_deref(), mode)?;
    cancellation.ensure()?;

    let label = match mode {
        TextCleaningMode::Trim => "Recortar espacios",
        TextCleaningMode::Normalize { .. } => "Normalizar texto",
        TextCleaningMode::Sentinels => "Normalizar valores centinela",
        TextCleaningMode::Booleans => "Normalizar booleanos",
        TextCleaningMode::FixEncoding => "Corregir codificación UTF-8",
        TextCleaningMode::NullifyInvalidTypes => "Apartar tipos incompatibles",
    };
    let preview = if changed_cell_count > 0 {
        publish_candidate_with_cancellation(dataset, cleaned, label, Some(&cancellation))?
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

fn apply_date_parsing(
    app: AppHandle,
    cancellation: PrepareCancellation,
) -> Result<TextCleaningResult, String> {
    cancellation.ensure()?;
    let state = app.state::<DatasetState>();
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if dataset.source_backed {
        if let Some(result) = source_backed_date_parsing_with_cancellation(dataset, &cancellation)?
        {
            return Ok(result);
        }
    }
    cancellation.ensure()?;
    materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
    let (parsed, affected_row_count, changed_cell_count, changed_columns) =
        parse_inferred_date_columns(&dataset.frame)?;
    cancellation.ensure()?;
    let preview = if changed_cell_count > 0 {
        publish_candidate_with_cancellation(
            dataset,
            parsed,
            "Interpretar fechas detectadas",
            Some(&cancellation),
        )?
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

fn apply_numeric_cast(
    app: AppHandle,
    cancellation: PrepareCancellation,
) -> Result<TextCleaningResult, String> {
    cancellation.ensure()?;
    let state = app.state::<DatasetState>();
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if dataset.source_backed {
        if let Some(result) = source_backed_numeric_cast_with_cancellation(dataset, &cancellation)?
        {
            return Ok(result);
        }
    }
    cancellation.ensure()?;
    materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
    let (cast, affected_row_count, changed_cell_count, changed_columns) =
        cast_inferred_numeric_columns(&dataset.frame)?;
    cancellation.ensure()?;
    let preview = if changed_cell_count > 0 {
        publish_candidate_with_cancellation(
            dataset,
            cast,
            "Convertir números detectados",
            Some(&cancellation),
        )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Trim, cancellation)
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(
            app,
            Some(columns),
            TextCleaningMode::Normalize { remove_accents },
            cancellation,
        )
    })
    .await
    .map_err(|error| format!("La normalización de texto se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn parse_date_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || apply_date_parsing(app, cancellation))
        .await
        .map_err(|error| format!("La interpretación de fechas se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn cast_numeric_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || apply_numeric_cast(app, cancellation))
        .await
        .map_err(|error| format!("La conversión numérica se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_sentinel_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Sentinels, cancellation)
    })
    .await
    .map_err(|error| format!("La normalización de valores centinela se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn normalize_boolean_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::Booleans, cancellation)
    })
    .await
    .map_err(|error| format!("La normalización de booleanos se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn fix_encoding_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(app, None, TextCleaningMode::FixEncoding, cancellation)
    })
    .await
    .map_err(|error| format!("La corrección de codificación se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn nullify_invalid_type_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_text_cleaning(
            app,
            None,
            TextCleaningMode::NullifyInvalidTypes,
            cancellation,
        )
    })
    .await
    .map_err(|error| format!("La corrección de tipos incompatibles se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn impute_missing_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                source_backed_imputation_with_cancellation(dataset, false, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_missing_values_in_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if changed_cell_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Imputación conservadora",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) =
                source_backed_imputation_with_cancellation(dataset, true, &cancellation)?
            {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_categorical_values_in_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if changed_cell_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Imputación categórica",
                Some(&cancellation),
            )?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        if dataset.source_backed {
            if let Some(result) = source_backed_direct_outlier_with_cancellation(
                dataset,
                OutlierAction::Impute,
                "Imputación de outliers",
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
            impute_outlier_values_in_frame(&dataset.frame)?;
        cancellation.ensure()?;
        let preview = if changed_cell_count > 0 {
            publish_candidate_with_cancellation(
                dataset,
                cleaned,
                "Imputación de outliers",
                Some(&cancellation),
            )?
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
    mode: OutlierMode,
    label: &'static str,
    cancellation: PrepareCancellation,
) -> Result<TextCleaningResult, String> {
    cancellation.ensure()?;
    let state = app.state::<DatasetState>();
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    if dataset.source_backed {
        if let Some(result) = source_backed_direct_outlier_with_cancellation(
            dataset,
            match mode {
                OutlierMode::Cap => OutlierAction::Cap,
                OutlierMode::Drop => OutlierAction::Drop,
            },
            label,
            &cancellation,
        )? {
            return Ok(result);
        }
    }
    cancellation.ensure()?;
    materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
    let (cleaned, affected_row_count, changed_cell_count, changed_columns) =
        apply_outlier_mode(&dataset.frame, mode)?;
    cancellation.ensure()?;
    let preview = if changed_cell_count > 0 || affected_row_count > 0 {
        publish_candidate_with_cancellation(dataset, cleaned, label, Some(&cancellation))?
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
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_direct_outlier_mode(
            app,
            OutlierMode::Cap,
            "Limitar outliers con IQR",
            cancellation,
        )
    })
    .await
    .map_err(|error| format!("La limitación de outliers se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn drop_outlier_values(app: AppHandle) -> Result<TextCleaningResult, String> {
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        apply_direct_outlier_mode(
            app,
            OutlierMode::Drop,
            "Eliminar filas atípicas",
            cancellation,
        )
    })
    .await
    .map_err(|error| format!("La eliminación de outliers se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn apply_safe_corrections(
    app: AppHandle,
    trim_text: bool,
    normalize_column_names: bool,
    normalize_sentinels: Option<bool>,
    remove_duplicates: Option<bool>,
    impute_missing: Option<bool>,
) -> Result<SafeCorrectionsResult, String> {
    let normalize_sentinels = normalize_sentinels.unwrap_or(false);
    let remove_duplicates = remove_duplicates.unwrap_or(false);
    let impute_missing = impute_missing.unwrap_or(false);
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        // Imputation needs the materialized frame, so a plan that includes it
        // skips the source-backed shortcut and publishes one revision.
        if dataset.source_backed && !impute_missing {
            if let Some(result) = source_backed_safe_corrections_with_cancellation(
                dataset,
                trim_text,
                normalize_column_names,
                normalize_sentinels,
                remove_duplicates,
                &cancellation,
            )? {
                return Ok(result);
            }
        }
        cancellation.ensure()?;
        materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;

        let SafeCorrectionPlanFrame {
            frame: candidate,
            affected_row_count,
            changed_cell_count,
            removed_row_count,
            renames,
            imputed_cell_count,
        } = safe_corrected_plan_frame(
            &dataset.frame,
            trim_text,
            normalize_column_names,
            normalize_sentinels,
            remove_duplicates,
            impute_missing,
        )?;
        cancellation.ensure()?;
        let renamed_column_count = renames.len();

        let preview = if changed_cell_count > 0
            || renamed_column_count > 0
            || removed_row_count > 0
            || imputed_cell_count > 0
        {
            publish_candidate_with_cancellation(
                dataset,
                candidate,
                "Aplicar correcciones recomendadas",
                Some(&cancellation),
            )?
        } else {
            loaded_dataset_preview(dataset, &dataset.frame)?
        };

        Ok(SafeCorrectionsResult {
            dataset: preview,
            changed_cell_count,
            affected_row_count,
            removed_row_count,
            renamed_column_count,
            renames,
            imputed_cell_count,
        })
    })
    .await
    .map_err(|error| format!("Las correcciones recomendadas se interrumpieron: {error}"))?
}

#[tauri::command]
pub async fn compare_history_snapshots(
    app: AppHandle,
    before_snapshot_id: String,
    after_snapshot_id: String,
    quality_rules: Vec<QualityRule>,
    on_progress: Channel<OperationProgress>,
) -> Result<SnapshotRevisionComparison, String> {
    compare_history_snapshots_impl(
        app,
        before_snapshot_id,
        after_snapshot_id,
        quality_rules,
        on_progress,
    )
    .await
}

#[tauri::command]
pub async fn undo_last_change(app: AppHandle) -> Result<HistoryResult, String> {
    undo_last_change_impl(app).await
}

#[tauri::command]
pub async fn redo_last_change(app: AppHandle) -> Result<HistoryResult, String> {
    redo_last_change_impl(app).await
}

#[tauri::command]
pub fn get_history_state(state: State<'_, DatasetState>) -> Result<HistoryState, String> {
    get_history_state_impl(state)
}

impl DatasetState {
    pub(crate) fn begin_project_open(&self) -> Result<u64, String> {
        let _guard = self.project_open_commit_lock.lock_recovering();
        Ok(self
            .project_open_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(crate) fn begin_project_save(&self) -> Result<u64, String> {
        let _guard = self.project_save_commit_lock.lock_recovering();
        Ok(self
            .project_save_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(crate) fn project_save_was_cancelled(&self, generation: u64) -> bool {
        self.project_save_generation.load(Ordering::SeqCst) != generation
    }

    pub(crate) fn project_save_cancellation(
        &self,
        generation: u64,
    ) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        let current_generation = std::sync::Arc::clone(&self.project_save_generation);
        std::sync::Arc::new(move || current_generation.load(Ordering::SeqCst) != generation)
    }

    pub(crate) fn commit_project_save<T>(
        &self,
        generation: u64,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.project_save_commit_lock.lock_recovering();
        ensure_not_cancelled(self.project_save_was_cancelled(generation))?;
        operation()
    }

    pub(crate) fn begin_project_delete(&self) -> Result<u64, String> {
        let _guard = self.project_delete_commit_lock.lock_recovering();
        Ok(self
            .project_delete_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(crate) fn project_delete_was_cancelled(&self, generation: u64) -> bool {
        self.project_delete_generation.load(Ordering::SeqCst) != generation
    }

    pub(crate) fn project_delete_cancellation(
        &self,
        generation: u64,
    ) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        let current_generation = std::sync::Arc::clone(&self.project_delete_generation);
        std::sync::Arc::new(move || current_generation.load(Ordering::SeqCst) != generation)
    }

    pub(crate) fn commit_project_delete<T>(
        &self,
        generation: u64,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.project_delete_commit_lock.lock_recovering();
        ensure_not_cancelled(self.project_delete_was_cancelled(generation))?;
        operation()
    }

    pub(crate) fn begin_project_catalog(&self) -> Result<u64, String> {
        Ok(self
            .project_catalog_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(crate) fn project_catalog_cancellation(
        &self,
        generation: u64,
    ) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        let current_generation = std::sync::Arc::clone(&self.project_catalog_generation);
        std::sync::Arc::new(move || current_generation.load(Ordering::SeqCst) != generation)
    }

    pub(crate) fn begin_project_versions(&self) -> Result<u64, String> {
        Ok(self
            .project_versions_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(crate) fn project_versions_cancellation(
        &self,
        generation: u64,
    ) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        let current_generation = std::sync::Arc::clone(&self.project_versions_generation);
        std::sync::Arc::new(move || current_generation.load(Ordering::SeqCst) != generation)
    }

    pub(crate) fn project_open_was_cancelled(&self, generation: u64) -> bool {
        self.project_open_generation.load(Ordering::SeqCst) != generation
    }

    pub(crate) fn project_open_cancellation(
        &self,
        generation: u64,
    ) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        let current_generation = std::sync::Arc::clone(&self.project_open_generation);
        std::sync::Arc::new(move || current_generation.load(Ordering::SeqCst) != generation)
    }

    pub(crate) fn commit_project_open<T>(
        &self,
        generation: u64,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.project_open_commit_lock.lock_recovering();
        ensure_not_cancelled(self.project_open_was_cancelled(generation))?;
        operation()
    }

    pub(crate) fn commit_project_open_with_candidate<T>(
        &self,
        generation: u64,
        candidate: ProjectDatasetCandidate,
        commit_catalog: impl FnOnce() -> Result<T, String>,
    ) -> Result<(T, DatasetPreview), String> {
        ensure_not_cancelled(self.project_open_was_cancelled(generation))?;
        // Esperar aquí deja que cancelar gane mientras la sesión esté ocupada.
        let mut current = self.current.lock_recovering();
        let _guard = self.project_open_commit_lock.lock_recovering();
        ensure_not_cancelled(self.project_open_was_cancelled(generation))?;

        let ProjectDatasetCandidate { loaded, preview } = candidate;
        let catalog_result = commit_catalog()?;
        *current = Some(loaded);
        drop(current);

        // Catalog and active dataset have committed. Clearing a poisoned pending
        // selection must not turn that successful commit into a reported failure.
        if let Ok(mut pending_selection) = self.pending_selection.lock() {
            pending_selection.take();
        }
        self.profile_generation.fetch_add(1, Ordering::SeqCst);
        self.export_generation.fetch_add(1, Ordering::SeqCst);
        Ok((catalog_result, preview))
    }

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
                row_count: frame.height(),
                frame,
                source_backed: false,
                delimited_header_mode: None,
                profile: None,
                history,
            })),
            ..Self::default()
        })
    }

    pub(crate) fn for_source_backed_project_import(
        input: &Path,
        sheet_name: Option<&str>,
        header_mode: Option<SpreadsheetHeaderMode>,
    ) -> Result<(Self, DatasetPreview), String> {
        let (loaded, preview) =
            load_source_backed_dataset_for_automation(input, sheet_name, header_mode)?;
        Ok((
            Self {
                current: Mutex::new(Some(loaded)),
                ..Self::default()
            },
            preview,
        ))
    }

    pub(crate) fn apply_project_import_recipe(
        &self,
        recipe: &TransformRecipe,
    ) -> Result<bool, String> {
        let mut current = self.current.lock_recovering();
        let dataset = current
            .as_mut()
            .ok_or_else(|| "La importación no contiene un dataset.".to_owned())?;
        apply_recipe_to_dataset(dataset, recipe).map(|result| result.changed)
    }

    pub(crate) fn evaluate_source_backed_quality_for_automation(
        &self,
        quality_rules: &[QualityRule],
    ) -> Result<QualityValidationResult, String> {
        let current = self.current.lock_recovering();
        let dataset = current
            .as_ref()
            .ok_or_else(|| "El proyecto no contiene un dataset activo.".to_owned())?;
        let (source_path, _) = current_duckdb_file_source(dataset).ok_or_else(|| {
            "La fuente source-backed del proyecto ya no está disponible.".to_owned()
        })?;
        let (source_path, expected_file_size, extension) = validate_dataset_file(&source_path)?;
        evaluate_source_quality_rules_with_cancel(
            &source_path,
            &extension,
            expected_file_size,
            dataset.row_count,
            quality_rules,
            || false,
        )
    }

    pub(crate) fn dimensions_for_automation(&self) -> Result<(usize, usize), String> {
        let current = self.current.lock_recovering();
        let dataset = current
            .as_ref()
            .ok_or_else(|| "El proyecto no contiene un dataset activo.".to_owned())?;
        Ok((dataset.row_count, dataset.frame.width()))
    }

    pub(crate) fn export_source_backed_project_for_automation(
        &self,
        output: &Path,
        format: ExportFormat,
        quality_validation: Option<&QualityValidationResult>,
        recipe: Option<&StoredTransformRecipe>,
    ) -> Result<ExportResult, String> {
        let current = self.current.lock_recovering();
        let dataset = current
            .as_ref()
            .ok_or_else(|| "El proyecto no contiene un dataset activo.".to_owned())?;
        export_source_backed_for_automation(dataset, output, format, quality_validation, recipe)
    }

    pub(crate) fn cache_project_import_profile(&self) -> Result<DatasetProfile, String> {
        self.cache_project_import_profile_with_progress(|_, _| {}, || false)
    }

    pub(crate) fn cache_project_import_profile_with_progress<F, C>(
        &self,
        report: F,
        is_cancelled: C,
    ) -> Result<DatasetProfile, String>
    where
        F: FnMut(&'static str, u8),
        C: Fn() -> bool + Sync,
    {
        let mut current = self.current.lock_recovering();
        let dataset = current
            .as_mut()
            .ok_or_else(|| "La importación no contiene un dataset.".to_owned())?;
        let profile = if dataset.source_backed {
            let (source_path, _) = current_duckdb_file_source(dataset).ok_or_else(|| {
                "La fuente source-backed ya no está disponible para perfilar el proyecto."
                    .to_owned()
            })?;
            let (source_path, expected_file_size, extension) = validate_dataset_file(&source_path)?;
            profile_source_backed_with_progress(
                &source_path,
                &extension,
                expected_file_size,
                dataset.row_count,
                report,
                is_cancelled,
                MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
            )?
        } else {
            materialize_loaded_dataset_with_cancel(dataset, &is_cancelled)?;
            profile_dataset_with_progress(
                &dataset.frame,
                report,
                is_cancelled,
                MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
            )?
        };
        dataset.profile = Some(profile.clone());
        Ok(profile)
    }

    #[cfg(test)]
    pub(crate) fn active_project_snapshot(&self) -> Result<ActiveDatasetSnapshot, String> {
        self.active_project_snapshot_with_cancel(|| false)
    }

    pub(crate) fn active_project_snapshot_with_cancel<C>(
        &self,
        is_cancelled: C,
    ) -> Result<ActiveDatasetSnapshot, String>
    where
        C: Fn() -> bool + Clone + Send + Sync + 'static,
    {
        ensure_not_cancelled(is_cancelled())?;
        let mut current = self.current.lock_recovering();
        ensure_not_cancelled(is_cancelled())?;
        let dataset = current
            .as_mut()
            .ok_or_else(|| "Carga un dataset antes de guardar un proyecto.".to_owned())?;
        let mut snapshot_temporary_guard = None;
        let current_snapshot_path = if dataset.source_backed {
            let (source_path, source_format) =
                current_duckdb_file_source(dataset).ok_or_else(|| {
                    "La fuente source-backed ya no está disponible para guardar el proyecto."
                        .to_owned()
                })?;
            let snapshot_temporary = tempfile::Builder::new()
                .prefix("project-current-")
                .suffix(".parquet")
                .tempfile_in(dataset.history.directory.path())
                .map_err(|_| {
                    "No se pudo preparar el snapshot source-backed del proyecto.".to_owned()
                })?
                .into_temp_path();
            let snapshot_path = snapshot_temporary.to_path_buf();
            fs::remove_file(&snapshot_path).map_err(|_| {
                "No se pudo preparar el snapshot source-backed del proyecto.".to_owned()
            })?;
            crate::duckdb_query::materialize_file_to_parquet_with_cancel(
                &source_path,
                source_format,
                &snapshot_path,
                None,
                is_cancelled.clone(),
            )?;
            ensure_not_cancelled(is_cancelled())?;
            let snapshot_row_count = crate::duckdb_query::count_file_rows(
                &snapshot_path,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                is_cancelled.clone(),
            )?;
            if snapshot_row_count != dataset.row_count {
                return Err(
                    "El conteo del dataset source-backed cambió durante el guardado.".to_owned(),
                );
            }
            ensure_not_cancelled(is_cancelled())?;
            snapshot_temporary_guard = Some(snapshot_temporary);
            Some(snapshot_path)
        } else {
            materialize_loaded_dataset_with_cancel(dataset, &is_cancelled)?;
            None
        };
        ensure_not_cancelled(is_cancelled())?;
        let frame = if let Some(snapshot_path) = current_snapshot_path.as_deref() {
            read_parquet_schema_frame(snapshot_path)?
        } else {
            dataset.frame.clone()
        };
        let history =
            capture_project_history_with_cancel(&dataset.history, &dataset.frame, || {
                is_cancelled()
            })?;
        ensure_not_cancelled(is_cancelled())?;
        Ok(ActiveDatasetSnapshot {
            column_count: frame.width(),
            frame,
            current_snapshot_path,
            _current_snapshot_guard: snapshot_temporary_guard,
            file_name: dataset.file_name.clone(),
            row_count: dataset.row_count,
            profile: dataset.profile.clone(),
            history,
        })
    }

    #[cfg(test)]
    pub(crate) fn prepare_project_candidate(
        snapshot_path: PathBuf,
        file_name: String,
    ) -> Result<ProjectDatasetCandidate, String> {
        Self::prepare_durable_project_candidate(snapshot_path, file_name, None, None)
    }

    #[cfg(test)]
    pub(crate) fn prepare_durable_project_candidate(
        snapshot_path: PathBuf,
        file_name: String,
        profile: Option<DatasetProfile>,
        history: Option<ProjectHistoryRestore>,
    ) -> Result<ProjectDatasetCandidate, String> {
        Self::prepare_durable_project_candidate_with_cancel(
            snapshot_path,
            file_name,
            profile,
            history,
            std::sync::Arc::new(|| false),
        )
    }

    pub(crate) fn prepare_durable_project_candidate_with_cancel(
        snapshot_path: PathBuf,
        file_name: String,
        profile: Option<DatasetProfile>,
        history: Option<ProjectHistoryRestore>,
        is_cancelled: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<ProjectDatasetCandidate, String> {
        ensure_not_cancelled(is_cancelled())?;
        let snapshot_size_bytes = fs::metadata(&snapshot_path)
            .map_err(|_| "No se pudo verificar el snapshot del proyecto.".to_owned())?
            .len();
        ensure_not_cancelled(is_cancelled())?;
        let can_defer_history = history
            .as_ref()
            .is_none_or(|value| !value.snapshots_enabled && value.entries.is_empty());
        if snapshot_size_bytes >= SOURCE_BACKED_LOAD_THRESHOLD_BYTES && can_defer_history {
            let schema = read_parquet_schema_frame(&snapshot_path)
                .map_err(|_| "No se pudo leer el esquema del proyecto.".to_owned())?;
            ensure_not_cancelled(is_cancelled())?;
            let cancellation = std::sync::Arc::clone(&is_cancelled);
            let row_count = crate::duckdb_query::count_file_rows(
                &snapshot_path,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                move || cancellation(),
            )?;
            ensure_not_cancelled(is_cancelled())?;
            let page = if row_count == 0 {
                schema.slice(0, 0)
            } else {
                read_parquet_query_block_with_cancel(
                    &snapshot_path,
                    0,
                    row_count.min(PREVIEW_ROW_LIMIT),
                    is_cancelled.as_ref(),
                )?
            };
            ensure_not_cancelled(is_cancelled())?;
            let preview = dataset_preview_from_schema_and_page(
                &file_name,
                snapshot_size_bytes,
                row_count,
                &schema,
                &page,
            )?;
            if let Some(profile) = profile.as_ref() {
                ensure_not_cancelled(is_cancelled())?;
                validate_project_profile_with_row_count(&schema, row_count, profile)?;
                ensure_not_cancelled(is_cancelled())?;
            }
            let history = match history {
                Some(history) => {
                    restore_project_history_with_cancel(&schema, history, || is_cancelled())?
                }
                None => HistoryManager::deferred()?,
            };
            ensure_not_cancelled(is_cancelled())?;
            return Ok(ProjectDatasetCandidate {
                loaded: LoadedDataset {
                    source_path: Some(snapshot_path),
                    file_name,
                    file_size_bytes: snapshot_size_bytes,
                    row_count,
                    frame: schema,
                    source_backed: true,
                    delimited_header_mode: None,
                    profile,
                    history,
                },
                preview,
            });
        }
        ensure_materialization_budget(snapshot_size_bytes)?;
        let frame =
            read_parquet_frame_with_cancel(&snapshot_path, || is_cancelled()).map_err(|error| {
                if error == OPERATION_CANCELLED_MESSAGE {
                    error
                } else {
                    "No se pudo restaurar el dataset del proyecto.".to_owned()
                }
            })?;
        ensure_not_cancelled(is_cancelled())?;
        let file_size_bytes = snapshot_size_bytes;
        let preview = dataset_preview_with_size(&file_name, file_size_bytes, &frame)
            .map_err(|_| "No se pudo preparar el dataset del proyecto.".to_owned())?;
        if let Some(profile) = profile.as_ref() {
            ensure_not_cancelled(is_cancelled())?;
            validate_project_profile(&frame, profile)?;
            ensure_not_cancelled(is_cancelled())?;
        }
        let history = match history {
            Some(history) => {
                restore_project_history_with_cancel(&frame, history, || is_cancelled())?
            }
            None => HistoryManager::new(&frame)
                .map_err(|_| "No se pudo iniciar el historial temporal del proyecto.".to_owned())?,
        };
        ensure_not_cancelled(is_cancelled())?;
        Ok(ProjectDatasetCandidate {
            loaded: LoadedDataset {
                source_path: None,
                file_name,
                file_size_bytes,
                row_count: frame.height(),
                frame,
                source_backed: false,
                delimited_header_mode: None,
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
        *self.current.lock_recovering() = Some(loaded);
        self.pending_selection.lock_recovering().take();
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

fn source_backed_iso8601_values_are_supported(
    dataset: &LoadedDataset,
    recipe: &TransformRecipe,
) -> Result<bool, String> {
    let iso_columns = recipe
        .date_parses
        .iter()
        .filter(|parse| {
            parse.format == RecipeDateFormat::Iso8601
                && recipe_column(&dataset.frame, &parse.column).is_ok_and(|column| {
                    !matches!(
                        (column.dtype(), parse.target),
                        (DataType::Date, RecipeDateTarget::Date)
                            | (DataType::Datetime(_, _), RecipeDateTarget::Datetime)
                    )
                })
        })
        .collect::<Vec<_>>();
    if iso_columns.is_empty() {
        return Ok(true);
    }
    let original_source_path = dataset
        .source_path
        .as_deref()
        .ok_or_else(|| "La fuente source-backed ya no está disponible.".to_owned())?;
    let (_, original_source_size, _) = validate_dataset_file(original_source_path)?;
    if original_source_size != dataset.file_size_bytes {
        return Err("El archivo source-backed cambió después de la carga.".to_owned());
    }
    // Después de una mutación source-backed, `source_path` sigue apuntando al
    // archivo original, pero `source_backed_projection_recipe` ejecutará la
    // siguiente receta sobre el snapshot Parquet publicado. Validar el
    // original aquí podía aprobar una receta con datos que ya no existen o
    // rechazar innecesariamente una transformación que seguía siendo segura.
    let (source_path, source_format) = current_duckdb_file_source(dataset)
        .ok_or_else(|| "La receta source-backed requiere una fuente compatible.".to_owned())?;
    let invalid_terms = iso_columns
        .iter()
        .map(|parse| {
            let identifier = duckdb_identifier(&parse.column);
            let value =
                duckdb_iso8601_expression(&format!("t.{identifier}"), RecipeDateTarget::Datetime);
            format!("CASE WHEN t.{identifier} IS NOT NULL AND {value} IS NULL THEN 1 ELSE 0 END")
        })
        .collect::<Vec<_>>();
    let query = format!(
        "SELECT CAST(COALESCE(SUM({}), 0) AS BIGINT) FROM dataset AS t",
        invalid_terms.join(" + ")
    );
    let invalid = crate::duckdb_query::query_file_scalar(&source_path, source_format, &query)?;
    Ok(invalid == 0)
}

fn apply_recipe_to_dataset(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
) -> Result<TransformRecipeResult, String> {
    apply_recipe_to_dataset_with_cancellation(dataset, recipe, None)
}

fn apply_recipe_to_dataset_with_cancellation(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
    cancellation: Option<&PrepareCancellation>,
) -> Result<TransformRecipeResult, String> {
    apply_recipe_to_dataset_with_policy_and_cancellation(dataset, recipe, None, cancellation)
}

fn apply_recipe_to_dataset_with_policy_and_cancellation(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
    exception_policy: Option<&ImportExceptionPolicy>,
    cancellation: Option<&PrepareCancellation>,
) -> Result<TransformRecipeResult, String> {
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    validate_recipe_structure(recipe)?;
    if let Some(policy) = exception_policy {
        let input_schema = import_exception_schema_for_frame(&dataset.frame);
        validate_import_exception_policy_for_recipe(policy, &input_schema, recipe)?;
    }
    let requires_eager_exception_handling = exception_policy.is_some_and(|policy| {
        policy
            .conversions
            .iter()
            .any(|conversion| match conversion {
                ImportExceptionConversion::Cast { on_invalid, .. }
                | ImportExceptionConversion::Date { on_invalid, .. } => {
                    *on_invalid != InvalidConversionAction::Review
                }
            })
    });
    if dataset.source_backed
        && !requires_eager_exception_handling
        && source_backed_projection_recipe_supported(&dataset.frame, recipe)
        && source_backed_iso8601_values_are_supported(dataset, recipe)?
    {
        return apply_source_backed_projection_recipe_with_cancellation(
            dataset,
            recipe,
            cancellation,
        );
    }
    let source_frame = if requires_eager_exception_handling && dataset.source_backed {
        Some(if let Some(cancellation) = cancellation {
            materialized_dataset_frame_with_cancel(dataset, || cancellation.is_cancelled())?
        } else {
            materialized_dataset_frame(dataset)?
        })
    } else {
        if let Some(cancellation) = cancellation {
            materialize_loaded_dataset_with_cancel(dataset, || cancellation.is_cancelled())?;
        } else {
            materialize_loaded_dataset(dataset)?;
        }
        None
    };
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    let source = source_frame.as_ref().unwrap_or(&dataset.frame);
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
    ) = if requires_eager_exception_handling {
        apply_eager_recipe_to_frame_with_exception_policy(source, recipe, exception_policy)?
    } else {
        apply_recipe_to_frame(source, recipe)?
    };
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
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
        publish_candidate_with_cancellation(
            dataset,
            candidate,
            "Aplicar receta de transformación",
            cancellation,
        )?
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
    exception_policy: Option<ImportExceptionPolicy>,
) -> Result<TransformRecipeResult, String> {
    validate_recipe_structure(&recipe)?;
    let cancellation = PrepareCancellation::begin(&app);
    tauri::async_runtime::spawn_blocking(move || {
        cancellation.ensure()?;
        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        apply_recipe_to_dataset_with_policy_and_cancellation(
            dataset,
            &recipe,
            exception_policy.as_ref(),
            Some(&cancellation),
        )
    })
    .await
    .map_err(|error| format!("La receta estructural se interrumpió: {error}"))?
}

#[cfg(test)]
#[path = "dataset/delivery_summary_tests.rs"]
mod delivery_summary_tests;

#[cfg(test)]
#[path = "dataset/snapshot_comparison_tests.rs"]
mod snapshot_comparison_tests;

#[cfg(test)]
mod tests;
