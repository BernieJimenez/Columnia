use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use calamine::{open_workbook_auto, Data, DataType as CalamineDataType, Range, Reader};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use polars::io::json::{JsonFormat, JsonWriter};
use polars::lazy::dsl::{col, lit};
use polars::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

const PREVIEW_ROW_LIMIT: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const PROTOTYPE_FILE_LIMIT_BYTES: u64 = 500 * 1024 * 1024;
const OPERATION_CANCELLED_MESSAGE: &str = "Operación cancelada por el usuario.";
const DELIMITED_SAMPLE_BYTES: u64 = 64 * 1024;
const HISTORY_MAX_ENTRIES: usize = 12;
const HISTORY_DISK_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
const RECIPE_FILE_VERSION: u32 = 1;
const RECIPE_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const MAX_RECIPE_TEXT_FIELD_CHARS: usize = 4 * 1024;
const MAX_RECIPE_TOTAL_TEXT_CHARS: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    operation: &'static str,
    stage: &'static str,
    percent: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Parquet,
    Sql,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Parquet => "parquet",
            Self::Sql => "sql",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Parquet => "Parquet",
            Self::Sql => "SQL",
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub(crate) file_name: String,
    pub(crate) file_size_bytes: u64,
    pub(crate) format: &'static str,
}

const MAX_QUALITY_RULES: usize = 16;
const MAX_QUALITY_COLUMN_CHARS: usize = 256;
const MAX_QUALITY_TOTAL_TEXT_CHARS: usize = 2 * 1024;
const MAX_QUALITY_VALUES: usize = 128;
const MAX_QUALITY_COLUMNS_PER_RULE: usize = 16;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const QUALITY_DATASET_COLUMN: &str = "__dataset__";
const QUALITY_MIGRATION_FILE_LIMIT_BYTES: u64 = 1024 * 1024;

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
    RowCount,
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
    pattern: Option<String>,
    dtype: Option<String>,
    columns: Option<Vec<String>>,
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
    pattern: Option<String>,
    dtype: Option<String>,
    columns: Option<Vec<String>>,
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
pub struct QualityMigrationResult {
    source_version: Option<String>,
    converted_rules: Vec<QualityRule>,
    warnings: Vec<QualityMigrationWarning>,
    omitted_rules: usize,
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
    pub(crate) can_consolidate: bool,
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
    standard_deviation: Option<f64>,
    first_quartile: Option<f64>,
    median: Option<f64>,
    third_quartile: Option<f64>,
    outlier_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetProfile {
    row_count: usize,
    duplicate_row_count: usize,
    duplicate_percentage: f64,
    columns: Vec<ColumnProfile>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetMutation {
    dataset: DatasetPreview,
    affected_row_count: usize,
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
pub struct StoredTransformRecipe {
    pub version: u32,
    pub name: String,
    pub saved_at: String,
    pub recipe: TransformRecipe,
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
        let file = File::open(&entry.path)
            .map_err(|error| format!("No se pudo abrir el snapshot del historial: {error}"))?;
        ParquetReader::new(file)
            .set_low_memory(true)
            .read_parallel(ParallelStrategy::None)
            .finish()
            .map_err(|error| format!("No se pudo restaurar el snapshot del historial: {error}"))
    }
}

fn publish_candidate(
    dataset: &mut LoadedDataset,
    candidate: DataFrame,
    label: &str,
) -> Result<DatasetPreview, String> {
    let preview = loaded_dataset_preview(dataset, &candidate)?;
    dataset.history.record(&candidate, label)?;
    dataset.frame = candidate;
    dataset.profile = None;
    Ok(preview)
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
    comparison: Mutex<Option<PendingComparison>>,
    load_generation: AtomicU64,
    profile_generation: AtomicU64,
    export_generation: AtomicU64,
}

impl DatasetState {
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

    fn load_was_cancelled(&self, generation: u64) -> bool {
        self.load_generation.load(Ordering::SeqCst) != generation
    }

    fn profile_was_cancelled(&self, generation: u64) -> bool {
        self.profile_generation.load(Ordering::SeqCst) != generation
    }

    fn export_was_cancelled(&self, generation: u64) -> bool {
        self.export_generation.load(Ordering::SeqCst) != generation
    }

    fn cancel(&self, operation: &str) -> Result<(), String> {
        let generation = match operation {
            "load" => &self.load_generation,
            "profile" => &self.profile_generation,
            "export" => &self.export_generation,
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

    validate_file_size(size)?;

    Ok((canonical, size, extension))
}

fn validate_file_size(size: u64) -> Result<(), String> {
    if size > PROTOTYPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "El archivo supera el límite temporal de {} MB. La carga por streaming se incorporará en un próximo hito.",
            PROTOTYPE_FILE_LIMIT_BYTES / 1024 / 1024
        ));
    }

    Ok(())
}

fn preview_value(value: AnyValue<'_>) -> Option<String> {
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

fn linear_quantile(sorted_values: &[f64], quantile: f64) -> Option<f64> {
    if sorted_values.is_empty() {
        return None;
    }

    let position = (sorted_values.len() - 1) as f64 * quantile;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;
    let weight = position - lower_index as f64;
    Some(
        sorted_values[lower_index]
            + (sorted_values[upper_index] - sorted_values[lower_index]) * weight,
    )
}

fn numeric_statistics(column: &Column) -> Result<Option<NumericStatistics>, String> {
    let mut values = if column.dtype().is_primitive_numeric() {
        (0..column.len())
            .map(|index| {
                column
                    .get(index)
                    .map_err(|error| format!("No se pudo analizar la columna numérica: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(numeric_value)
            .collect::<Vec<_>>()
    } else if column.dtype() == &DataType::String {
        let text = column
            .str()
            .map_err(|error| format!("No se pudo analizar texto numérico: {error}"))?;
        let non_empty = text
            .iter()
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if non_empty.is_empty() {
            return Ok(None);
        }
        let parsed = non_empty
            .iter()
            .map(|value| semantic_numeric_value(value))
            .collect::<Option<Vec<_>>>();
        let Some(parsed) = parsed else {
            return Ok(None);
        };
        parsed
    } else {
        return Ok(None);
    };
    values.sort_by(f64::total_cmp);

    let minimum = values.first().copied();
    let maximum = values.last().copied();
    let mean = (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);

    let first_quartile = linear_quantile(&values, 0.25);
    let median = linear_quantile(&values, 0.5);
    let third_quartile = linear_quantile(&values, 0.75);
    let standard_deviation = if values.len() < 2 {
        None
    } else {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64;
        Some(variance.sqrt())
    };
    let outlier_count = if values.len() < 4 {
        0
    } else {
        let q1 = first_quartile.expect("cuatro valores siempre producen Q1");
        let q3 = third_quartile.expect("cuatro valores siempre producen Q3");
        let interquartile_range = q3 - q1;
        let lower_bound = q1 - 1.5 * interquartile_range;
        let upper_bound = q3 + 1.5 * interquartile_range;
        values
            .iter()
            .filter(|value| **value < lower_bound || **value > upper_bound)
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
    }))
}

struct TextStatistics {
    empty_count: usize,
    minimum_length: Option<usize>,
    maximum_length: Option<usize>,
    average_length: Option<f64>,
    suggested_type: Option<&'static str>,
    type_match_percentage: Option<f64>,
    invalid_type_count: Option<usize>,
}

fn is_supported_date(value: &str) -> bool {
    const DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%Y/%m/%d", "%B %d, %Y", "%b %d, %Y"];

    DATE_FORMATS
        .iter()
        .any(|format| NaiveDate::parse_from_str(value, format).is_ok())
}

fn suggest_text_type(values: &[&str]) -> (Option<&'static str>, Option<f64>, Option<usize>) {
    if values.len() < 3 {
        return (None, None, None);
    }

    let boolean_count = values
        .iter()
        .filter(|value| {
            matches!(
                value.to_lowercase().as_str(),
                "true" | "false" | "yes" | "no" | "si" | "sí"
            )
        })
        .count();
    let integer_count = values
        .iter()
        .filter(|value| !has_identifier_leading_zero(value) && value.parse::<i64>().is_ok())
        .count();
    let decimal_count = values
        .iter()
        .filter(|value| semantic_numeric_value(value).is_some())
        .count();
    let date_count = values
        .iter()
        .filter(|value| is_supported_date(value))
        .count();

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

    let match_percentage = (best.1 as f64 / values.len() as f64) * 100.0;
    if match_percentage < 90.0 {
        return (None, None, None);
    }

    (
        Some(best.0),
        Some(match_percentage),
        Some(values.len() - best.1),
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
    let mut value_count = 0;
    let mut total_length = 0;
    let mut minimum_length: Option<usize> = None;
    let mut maximum_length: Option<usize> = None;
    let mut non_empty_values = Vec::new();

    for value in values.iter().flatten() {
        let length = value.chars().count();
        let trimmed = value.trim();
        empty_count += usize::from(trimmed.is_empty());
        if !trimmed.is_empty() {
            non_empty_values.push(trimmed);
        }
        value_count += 1;
        total_length += length;
        minimum_length = Some(minimum_length.map_or(length, |current| current.min(length)));
        maximum_length = Some(maximum_length.map_or(length, |current| current.max(length)));
    }

    let average_length = (value_count > 0).then(|| total_length as f64 / value_count as f64);
    let (suggested_type, type_match_percentage, invalid_type_count) =
        suggest_text_type(&non_empty_values);
    Ok(Some(TextStatistics {
        empty_count,
        minimum_length,
        maximum_length,
        average_length,
        suggested_type,
        type_match_percentage,
        invalid_type_count,
    }))
}

fn profile_dataset_with_progress<F, C>(
    frame: &DataFrame,
    mut report: F,
    is_cancelled: C,
) -> Result<DatasetProfile, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    let row_count = frame.height();
    report("Detectando filas duplicadas", 10);
    let distinct_row_count = frame
        .unique::<Vec<String>, String>(None, UniqueKeepStrategy::First, None)
        .map_err(|error| format!("No se pudieron detectar las filas duplicadas: {error}"))?
        .height();
    let duplicate_row_count = row_count.saturating_sub(distinct_row_count);
    let duplicate_percentage = if row_count == 0 {
        0.0
    } else {
        (duplicate_row_count as f64 / row_count as f64) * 100.0
    };
    ensure_not_cancelled(is_cancelled())?;
    let source_columns = frame.columns();
    let mut columns = Vec::with_capacity(source_columns.len());
    for (column_index, column) in source_columns.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        let null_count = column.null_count();
        let unique_count = column
            .n_unique()
            .map_err(|error| format!("No se pudieron contar los valores únicos: {error}"))?
            .saturating_sub(usize::from(null_count > 0));
        let completeness_percentage = if row_count == 0 {
            100.0
        } else {
            ((row_count - null_count) as f64 / row_count as f64) * 100.0
        };

        let text_statistics = text_statistics(column)?;
        let numeric_statistics = numeric_statistics(column)?;
        let (minimum, maximum, mean) = if column.dtype().is_primitive_numeric() {
            let minimum = column
                .min_reduce()
                .map_err(|error| format!("No se pudo calcular el mínimo: {error}"))?
                .into_value();
            let maximum = column
                .max_reduce()
                .map_err(|error| format!("No se pudo calcular el máximo: {error}"))?
                .into_value();
            let mean = column
                .mean_reduce()
                .map_err(|error| format!("No se pudo calcular el promedio: {error}"))?
                .into_value();

            (
                preview_value(minimum),
                preview_value(maximum),
                numeric_value(mean),
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

        columns.push(ColumnProfile {
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
        });

        let completed_columns = column_index + 1;
        let percent = 20 + ((completed_columns * 80) / source_columns.len()) as u8;
        report("Analizando columnas", percent);
    }

    if source_columns.is_empty() {
        report("Perfil completado", 100);
    }

    Ok(DatasetProfile {
        row_count,
        duplicate_row_count,
        duplicate_percentage,
        columns,
    })
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
}

fn normalize_text_value(value: &str, remove_accents: bool) -> String {
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
        let mut column_changes = 0;
        let transformed: Vec<Option<String>> = values
            .iter()
            .enumerate()
            .map(|(row_index, value)| {
                value.map(|original| {
                    let next = match mode {
                        TextCleaningMode::Trim => original.trim().to_owned(),
                        TextCleaningMode::Normalize { remove_accents } => {
                            normalize_text_value(original, remove_accents)
                        }
                    };
                    if next != original {
                        column_changes += 1;
                        changed_cell_count += 1;
                        changed_rows[row_index] = true;
                    }
                    next
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

fn read_delimited_frame(path: &Path, extension: &str) -> Result<DataFrame, String> {
    let separator = detect_delimiter(path, extension)?;
    CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(0))
        .map_parse_options(|options| options.with_separator(separator))
        .try_into_reader_with_file_path(Some(path.to_path_buf()))
        .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))?
        .finish()
        .map_err(|error| {
            format!("No se pudo interpretar el archivo delimitado como UTF-8: {error}")
        })
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
        "parquet" => {
            let file = fs::File::open(path)
                .map_err(|error| format!("No se pudo abrir el Parquet: {error}"))?;
            ParquetReader::new(file)
                .set_low_memory(true)
                .read_parallel(ParallelStrategy::None)
                .finish()
                .map_err(|error| format!("No se pudo interpretar el Parquet: {error}"))?
        }
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
    };
    validate_stored_recipe(&document)?;
    Ok(document)
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
    let document: StoredTransformRecipe = serde_json::from_str(json)
        .map_err(|error| format!("La receta JSON no es válida: {error}"))?;
    validate_stored_recipe(&document)?;
    Ok(document)
}

fn migration_string_field(map: &JsonMap<String, JsonValue>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(JsonValue::as_str).map(str::to_owned))
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

fn migration_quality_kind(value: &str) -> Option<QualityRuleKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "not_null" => Some(QualityRuleKind::NotNull),
        "non_empty" => Some(QualityRuleKind::NonEmpty),
        "unique" => Some(QualityRuleKind::Unique),
        "numeric_range" => Some(QualityRuleKind::NumericRange),
        "allowed_values" => Some(QualityRuleKind::AllowedValues),
        "regex" => Some(QualityRuleKind::Regex),
        "dtype" => Some(QualityRuleKind::Dtype),
        "unique_together" => Some(QualityRuleKind::UniqueTogether),
        "row_count" => Some(QualityRuleKind::RowCount),
        _ => None,
    }
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

fn migrate_quality_rules_document(document: JsonValue) -> Result<QualityMigrationResult, String> {
    let (source_version, raw_rules) = match document {
        JsonValue::Array(rules) => (None, rules),
        JsonValue::Object(document) => {
            let source_version = document
                .get("version")
                .or_else(|| document.get("schema_version"))
                .map(|value| value.to_string().trim_matches('"').to_owned());
            let rules = document
                .get("rules")
                .or_else(|| document.get("quality_rules"))
                .and_then(JsonValue::as_array)
                .cloned()
                .ok_or_else(|| "El documento debe contener una lista 'rules'.".to_owned())?;
            (source_version, rules)
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
        let column = migration_string_field(&map, &["column"])
            .filter(|column| !column.trim().is_empty())
            .unwrap_or_else(|| QUALITY_DATASET_COLUMN.to_owned());
        if kind != QualityRuleKind::RowCount && column == QUALITY_DATASET_COLUMN {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La regla necesita una columna concreta.",
            ));
            continue;
        }

        let severity = migration_string_field(&map, &["severity"])
            .unwrap_or_else(|| "blocking".to_owned())
            .to_ascii_lowercase();
        if severity != "blocking" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La severidad no bloqueante no se convierte porque Columnia aún no tiene severidades por regla.",
            ));
            continue;
        }
        let on_missing = migration_string_field(&map, &["on_missing"])
            .unwrap_or_else(|| "fail".to_owned())
            .to_ascii_lowercase();
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
        let null_policy = migration_string_field(&map, &["null_policy"])
            .unwrap_or_else(|| "invalid".to_owned())
            .to_ascii_lowercase();
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
            pattern: None,
            dtype: None,
            columns: None,
        };

        let conversion_result = (|| -> Result<Option<String>, String> {
            Ok(match kind {
                QualityRuleKind::NumericRange => {
                    rule.min = migration_number_field(&map, &["min"])?;
                    rule.max = migration_number_field(&map, &["max"])?;
                    (rule.min.is_none() && rule.max.is_none())
                        .then(|| "numeric_range necesita min o max.".to_owned())
                }
                QualityRuleKind::AllowedValues => {
                    rule.values = migration_string_array(&map, &["values"])?;
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
                    let source_dtype = migration_string_field(&map, &["dtype"]);
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
                    rule.columns = migration_string_array(&map, &["columns", "required_columns"])?;
                    rule.columns
                        .as_ref()
                        .filter(|columns| columns.len() >= 2)
                        .is_none()
                        .then(|| "unique_together necesita al menos dos columnas.".to_owned())
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

    Ok(QualityMigrationResult {
        source_version,
        converted_rules,
        warnings,
        omitted_rules,
    })
}

fn load_quality_migration_file(path: &Path) -> Result<QualityMigrationResult, String> {
    let path = canonicalize_existing_file(path, "el contrato de calidad seleccionado")?;
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
    let document = serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))?;
    migrate_quality_rules_document(document)
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
        if let Some(columns) = rule.columns.as_deref() {
            for column in columns {
                text_fields.push(("columns", column.as_str()));
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
    if rule.column.trim().is_empty() && !is_dataset_rule {
        return Err("La columna de una regla de calidad no puede estar vacía.".to_owned());
    }
    if is_dataset_rule && rule.column != QUALITY_DATASET_COLUMN {
        return Err(format!(
            "La regla row_count debe usar la columna lógica '{}'.",
            QUALITY_DATASET_COLUMN
        ));
    }
    let column = (!is_dataset_rule)
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
    for (name, value) in [("min", rule.min), ("max", rule.max)] {
        if value.is_some_and(|number| !number.is_finite()) {
            return Err(format!(
                "{name} de '{}' debe ser un número finito.",
                rule.column
            ));
        }
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
            pattern: rule.pattern.clone(),
            dtype: rule.dtype.clone(),
            columns: rule.columns.clone(),
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

fn export_frame_atomic<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
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
    let mut output_frame = frame_for_export(frame, format)?;

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
            frame,
            temporary.as_file_mut(),
            |percent| report("Escribiendo SQL", percent),
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
    })
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
    for row_index in 0..frame.height() {
        let signature = row_signature(frame, columns, row_index)?;
        *counts.entry(signature).or_insert(0) += 1;
    }
    Ok(counts)
}

fn key_rows(
    frame: &DataFrame,
    key_columns: &[String],
) -> Result<HashMap<String, Vec<usize>>, String> {
    let mut rows_by_key = HashMap::new();
    for row_index in 0..frame.height() {
        let signature = row_signature(frame, key_columns, row_index)?;
        rows_by_key
            .entry(signature)
            .or_insert_with(Vec::new)
            .push(row_index);
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
    let current_keys = current_rows.keys().collect::<HashSet<_>>();
    let compared_keys = compared_rows.keys().collect::<HashSet<_>>();
    let shared_payload_columns = shared_columns
        .iter()
        .filter(|column| !key_columns.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let duplicate_keys = current_rows
        .iter()
        .filter(|(_, rows)| rows.len() > 1)
        .map(|(key, _)| key.clone())
        .chain(
            compared_rows
                .iter()
                .filter(|(_, rows)| rows.len() > 1)
                .map(|(key, _)| key.clone()),
        )
        .collect::<HashSet<_>>();
    let mut summary = KeyComparisonSummary {
        matched_key_count: current_keys.intersection(&compared_keys).count(),
        current_only_key_count: current_keys.difference(&compared_keys).count(),
        compared_only_key_count: compared_keys.difference(&current_keys).count(),
        duplicate_key_count: duplicate_keys.len(),
        ..Default::default()
    };

    for key in current_keys.intersection(&compared_keys) {
        let current_rows = &current_rows[*key];
        let compared_rows = &compared_rows[*key];
        if current_rows.len() != 1 || compared_rows.len() != 1 {
            continue;
        }
        let current_payload = row_signature(current, &shared_payload_columns, current_rows[0])?;
        let compared_payload = row_signature(compared, &shared_payload_columns, compared_rows[0])?;
        if current_payload != compared_payload {
            summary.conflicting_key_count += 1;
        }
    }

    Ok(summary)
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
    validate_key_columns(current, compared, key_columns)?;
    current
        .join(
            compared,
            key_columns.iter(),
            key_columns.iter(),
            JoinArgs::new(join_type.polars_type()).with_coalesce(JoinCoalesce::CoalesceColumns),
            None,
        )
        .map_err(|error| format!("No se pudieron unir los datasets por clave: {error}"))
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
    let key_summary = if key_columns.is_empty() {
        KeyComparisonSummary::default()
    } else {
        compare_keyed_frames(current, compared, key_columns, &shared_columns)?
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
    Ok(Some(DatasetSourceInspection {
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
    }))
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
            send_progress(&on_progress, "profile", "Perfil disponible", 100);
            return Ok(profile.clone());
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
    on_progress: Channel<OperationProgress>,
) -> Result<Option<ExportResult>, String> {
    validate_quality_rules_payload(&quality_rules)?;
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
    let (frame, suggested_name) = tauri::async_runtime::spawn_blocking(move || {
        enforce_export_quality_with_cancel(&frame, &quality_rules, allow_unvalidated, || {
            validation_app
                .state::<DatasetState>()
                .export_was_cancelled(generation)
        })?;
        ensure_not_cancelled(
            validation_app
                .state::<DatasetState>()
                .export_was_cancelled(generation),
        )?;
        Ok::<_, String>((frame, suggested_name))
    })
    .await
    .map_err(|error| format!("La validación previa a la exportación se interrumpió: {error}"))??;
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

    tauri::async_runtime::spawn_blocking(move || {
        export_frame_atomic(
            &frame,
            &destination,
            format,
            |stage, percent| send_progress(&on_progress, "export", stage, percent),
            || app.state::<DatasetState>().export_was_cancelled(generation),
        )
        .map(Some)
    })
    .await
    .map_err(|error| format!("La exportación se interrumpió: {error}"))?
}

#[tauri::command]
pub async fn save_transform_recipe(
    app: AppHandle,
    recipe: TransformRecipe,
    name: String,
) -> Result<Option<StoredTransformRecipe>, String> {
    let document = build_stored_recipe(recipe, name)?;
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
pub async fn pick_quality_rules_migration(
    app: AppHandle,
) -> Result<Option<QualityMigrationResult>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Reglas DataPrep", &["json"])
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

fn apply_keep_columns(
    frame: DataFrame,
    keep_columns: Option<&[String]>,
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, bool), String> {
    let Some(keep_columns) = keep_columns else {
        return Ok((frame, 0, false));
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
            recipe_column(&frame, &effective)?;
            Ok(effective)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let original_width = frame.width();
    let order_changed = frame
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .ne(names.iter().map(String::as_str));
    let selected = frame
        .select(&names)
        .map_err(|error| format!("No se pudieron conservar las columnas seleccionadas: {error}"))?;
    Ok((
        selected,
        original_width.saturating_sub(names.len()),
        order_changed,
    ))
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
                _ => Err(format!("La columna '{name}' debe ser Int64 o Float64 para tratar atípicos.")),
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
                "La columna '{name}' debe ser Int64 o Float64 para tratar atípicos."
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
        prepared.push((name, treatment.action, values, lower, upper));
    }

    let baseline_height = frame.height();
    let mut drop_mask = vec![false; baseline_height];
    let mut adjusted = 0;
    for (name, action, values, lower, upper) in prepared {
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

fn lazy_recipe_supported(recipe: &TransformRecipe) -> bool {
    recipe.date_parses.is_empty()
        && recipe.find_replace.is_none()
        && recipe.keep_columns.is_none()
        && recipe.split_column.is_none()
        && recipe.merge_columns.is_none()
        && recipe.outlier_treatments.is_empty()
        && recipe.group_summary.is_none()
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

    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        let source_name = remapped_name(&calculation.source, &rename_map);
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

    let candidate = plan
        .collect()
        .map_err(|error| format!("No se pudo ejecutar la receta lazy: {error}"))?;
    let removed_row_count = source.height().saturating_sub(candidate.height());
    Ok((
        candidate,
        renamed_count,
        cast_count,
        0,
        removed_row_count,
        calculated_column_count,
        0,
        0,
        false,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualityRulesDocument {
    version: u8,
    rules: Vec<QualityRule>,
}

pub(crate) fn load_quality_rules_for_automation(input: &Path) -> Result<Vec<QualityRule>, String> {
    let canonical = canonicalize_existing_file(input, "el contrato de calidad")?;
    if !canonical
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err("El contrato de calidad debe ser JSON.".to_owned());
    }
    validate_file_size(
        fs::metadata(&canonical)
            .map_err(|error| format!("No se pudo verificar el contrato de calidad: {error}"))?
            .len(),
    )?;
    let bytes = fs::read(canonical)
        .map_err(|error| format!("No se pudo leer el contrato de calidad: {error}"))?;
    let document: QualityRulesDocument = serde_json::from_slice(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))?;
    if document.version != 1 {
        return Err("La versión del contrato de calidad no es compatible.".to_owned());
    }
    validate_quality_rules_payload(&document.rules)?;
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
    let expected = profile_dataset_with_progress(frame, |_, _| {}, || false)
        .map_err(|_| "No se pudo validar el perfil guardado.".to_owned())?;
    if &expected != profile {
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
        let staged_frame = ParquetReader::new(
            File::open(&destination)
                .map_err(|_| "No se pudo validar el historial activo.".to_owned())?,
        )
        .set_low_memory(true)
        .read_parallel(ParallelStrategy::None)
        .finish()
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
    let mut restored_frames = Vec::with_capacity(history.entries.len());
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
        let frame = ParquetReader::new(
            File::open(&entry.path)
                .map_err(|_| "No se pudo abrir un snapshot del historial.".to_owned())?,
        )
        .set_low_memory(true)
        .read_parallel(ParallelStrategy::None)
        .finish()
        .map_err(|_| "Un snapshot del historial no contiene un Parquet válido.".to_owned())?;
        let destination = directory
            .path()
            .join(format!("snapshot-{index:020}.parquet"));
        let copied = fs::copy(&entry.path, &destination)
            .map_err(|_| "No se pudo copiar el historial restaurado.".to_owned())?;
        if copied != entry.bytes {
            return Err("Un snapshot del historial cambió durante la apertura.".to_owned());
        }
        restored_frames.push(frame);
        entries.push(HistoryEntry {
            label: entry.label,
            path: destination,
            bytes: copied,
        });
    }
    if history.snapshots_enabled && !restored_frames[history.cursor].equals_missing(current_frame) {
        return Err("El cursor del historial no coincide con el dataset actual.".to_owned());
    }
    Ok(HistoryManager {
        directory,
        entries,
        cursor: history.cursor,
        snapshots_enabled: history.snapshots_enabled,
        degraded_reason: history.degraded_reason,
        current_label: history.current_label,
        next_id: restored_frames.len() as u64,
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
        let file = File::open(&snapshot_path)
            .map_err(|_| "No se pudo abrir el snapshot del proyecto.".to_owned())?;
        let frame = ParquetReader::new(file)
            .set_low_memory(true)
            .read_parallel(ParallelStrategy::None)
            .finish()
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
    fn accepts_500_megabytes_and_rejects_the_next_byte() {
        assert_eq!(validate_file_size(PROTOTYPE_FILE_LIMIT_BYTES), Ok(()));

        let error = validate_file_size(PROTOTYPE_FILE_LIMIT_BYTES + 1)
            .expect_err("un byte sobre el límite debe rechazarse");

        assert!(error.contains("500 MB"));
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
        assert!(updates.windows(2).all(|pair| pair[0].1 <= pair[1].1));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn stops_profile_at_a_cooperative_cancellation_point() {
        use std::cell::Cell;

        let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let checks = Cell::new(0);

        let error = profile_dataset_with_progress(
            &frame,
            |_, _| {},
            || {
                checks.set(checks.get() + 1);
                checks.get() >= 2
            },
        )
        .expect_err("el perfil debe detenerse al cancelar");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
        assert_eq!(checks.get(), 2);
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }

    #[test]
    fn invalidates_only_the_requested_operation_generation() {
        let state = DatasetState::default();
        let load_generation = state.begin_load();
        let profile_generation = state.begin_profile();
        let export_generation = state.begin_export();

        state
            .cancel("profile")
            .expect("el perfil debe poder cancelarse");

        assert!(!state.load_was_cancelled(load_generation));
        assert!(state.profile_was_cancelled(profile_generation));
        assert!(!state.export_was_cancelled(export_generation));
        assert!(state.cancel("unknown").is_err());
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

        fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
        fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
        fs::remove_file(duplicate_path).expect("se debe limpiar el CSV duplicado");
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
            pattern: None,
            dtype: None,
            columns: None,
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
    }

    #[test]
    fn migrates_supported_quality_rules_and_omits_unsupported_semantics() {
        let result = migrate_quality_rules_document(serde_json::json!({
            "version": 3,
            "rules": [
                {"kind": "allowed_values", "column": "status", "values": ["ok"], "max_invalid": 1},
                {"type": "regex", "column": "email", "pattern": "^.+@.+$", "maxInvalidPct": 5},
                {"kind": "column_compare", "column": "left", "other_column": "right"},
                {"kind": "not_null", "column": "id", "severity": "warning", "max_invalid": 0},
                {"kind": "unique_together", "column": "a", "columns": ["a", "b"]}
            ]
        }))
        .expect("el documento de migración debe ser válido");

        assert_eq!(result.source_version.as_deref(), Some("3"));
        assert_eq!(result.converted_rules.len(), 3);
        assert_eq!(result.omitted_rules, 2);
        assert_eq!(result.converted_rules[0].max_invalid, Some(1));
        assert_eq!(result.converted_rules[1].max_invalid_pct, Some(5.0));
        assert!(result.warnings.iter().any(
            |warning| warning.severity == "omitted" && warning.source_kind == "column_compare"
        ));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.severity == "omitted" && warning.source_kind == "not_null"));
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
