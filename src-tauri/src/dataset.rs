use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use calamine::{open_workbook_auto, Data, DataType as CalamineDataType, Range, Reader};
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use polars::prelude::*;
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
    Parquet,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Parquet => "Parquet",
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    file_name: String,
    file_size_bytes: u64,
    format: &'static str,
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
    name: String,
    data_type: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetPreview {
    file_name: String,
    file_size_bytes: u64,
    row_count: usize,
    column_count: usize,
    columns: Vec<DatasetColumn>,
    rows: Vec<Vec<Option<String>>>,
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

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    suggested_type: Option<&'static str>,
    type_match_percentage: Option<f64>,
    invalid_type_count: Option<usize>,
    standard_deviation: Option<f64>,
    first_quartile: Option<f64>,
    median: Option<f64>,
    third_quartile: Option<f64>,
    outlier_count: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    can_undo: bool,
    can_redo: bool,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeRename {
    from: String,
    to: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeCastTarget {
    String,
    Integer,
    Decimal,
    Boolean,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeCast {
    column: String,
    target: RecipeCastTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeDateFormat {
    Ymd,
    Dmy,
    Mdy,
    Iso8601,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeDateTarget {
    Date,
    Datetime,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeDateParse {
    column: String,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeFilter {
    column: String,
    operator: RecipeFilterOperator,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CalculatedOperandKind {
    Literal,
    Column,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculatedOperand {
    kind: CalculatedOperandKind,
    value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculatedColumnRecipe {
    name: String,
    source: String,
    operation: CalculatedOperation,
    #[serde(default)]
    operand: Option<CalculatedOperand>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
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
    changed: bool,
}

struct LoadedDataset {
    path: PathBuf,
    frame: DataFrame,
    profile: Option<DatasetProfile>,
    undo_frame: Option<DataFrame>,
    redo_frame: Option<DataFrame>,
}

#[derive(Clone)]
struct PendingSelection {
    id: String,
    path: PathBuf,
    file_size_bytes: u64,
    sheets: Vec<String>,
}

#[derive(Default)]
pub struct DatasetState {
    current: Mutex<Option<LoadedDataset>>,
    pending_selection: Mutex<Option<PendingSelection>>,
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

fn validate_dataset_file(path: &Path) -> Result<(u64, String), String> {
    if !path.is_file() {
        return Err("El archivo seleccionado no existe o no es un archivo regular.".into());
    }
    let extension = dataset_extension(path)?;

    let size = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();

    validate_file_size(size)?;

    Ok((size, extension))
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
                .and_then(|statistics| statistics.suggested_type),
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
    let file_size_bytes = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();
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
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dataset.csv")
            .to_owned(),
        file_size_bytes,
        row_count: frame.height(),
        column_count: frame.width(),
        columns,
        rows,
    })
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
    let (_, extension) = validate_dataset_file(path)?;
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
    let (_, extension) = validate_dataset_file(path)?;
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
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?;
    report("Preparando archivo temporal", 10);
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo crear el archivo temporal: {error}"))?;
    let mut output_frame = frame.clone();

    report("Escribiendo dataset", 25);
    match format {
        ExportFormat::Csv => CsvWriter::new(temporary.as_file())
            .finish(&mut output_frame)
            .map_err(|error| format!("No se pudo escribir el CSV: {error}"))?,
        ExportFormat::Parquet => ParquetWriter::new(temporary.as_file())
            .finish(&mut output_frame)
            .map(|_| ())
            .map_err(|error| format!("No se pudo escribir Parquet: {error}"))?,
    }

    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la exportación: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    report("Publicando archivo completo", 90);
    temporary
        .persist(destination)
        .map_err(|error| format!("No se pudo publicar la exportación: {}", error.error))?;

    let file_size_bytes = fs::metadata(destination)
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
    let (file_size_bytes, extension) = validate_dataset_file(&path)?;
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
        let current_size = validate_dataset_file(&pending.path)?.0;
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
        *state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())? =
            Some(LoadedDataset {
                path: pending.path,
                frame,
                profile: None,
                undo_frame: None,
                redo_frame: None,
            });
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
pub async fn export_dataset(
    app: AppHandle,
    format: ExportFormat,
    on_progress: Channel<OperationProgress>,
) -> Result<Option<ExportResult>, String> {
    let generation = app.state::<DatasetState>().begin_export();
    let (frame, suggested_name) = {
        let state = app.state::<DatasetState>();
        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let stem = dataset
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("dataset");
        (
            dataset.frame.clone(),
            format!("{stem}-columnia.{}", format.extension()),
        )
    };

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

        if affected_row_count > 0 {
            dataset.undo_frame = Some(dataset.frame.clone());
            dataset.redo_frame = None;
            dataset.frame = cleaned;
            dataset.profile = None;
        }

        Ok(DatasetMutation {
            dataset: dataset_preview(&dataset.path, &dataset.frame)?,
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

        if !renames.is_empty() {
            let previous = dataset.frame.clone();
            dataset
                .frame
                .set_column_names(&names)
                .map_err(|error| format!("No se pudieron normalizar las columnas: {error}"))?;
            dataset.undo_frame = Some(previous);
            dataset.redo_frame = None;
            dataset.profile = None;
        }

        Ok(ColumnNormalizationResult {
            dataset: dataset_preview(&dataset.path, &dataset.frame)?,
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

    if changed_cell_count > 0 {
        dataset.undo_frame = Some(dataset.frame.clone());
        dataset.redo_frame = None;
        dataset.frame = cleaned;
        dataset.profile = None;
    }

    Ok(TextCleaningResult {
        dataset: dataset_preview(&dataset.path, &dataset.frame)?,
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

        if changed_cell_count > 0 || renamed_column_count > 0 {
            dataset.undo_frame = Some(dataset.frame.clone());
            dataset.redo_frame = None;
            dataset.frame = candidate;
            dataset.profile = None;
        }

        Ok(SafeCorrectionsResult {
            dataset: dataset_preview(&dataset.path, &dataset.frame)?,
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
    let previous = dataset
        .undo_frame
        .take()
        .ok_or_else(|| "No hay un cambio disponible para deshacer.".to_owned())?;
    let changed = std::mem::replace(&mut dataset.frame, previous);
    dataset.redo_frame = Some(changed);
    dataset.profile = None;
    Ok(HistoryResult {
        dataset: dataset_preview(&dataset.path, &dataset.frame)?,
        can_undo: dataset.undo_frame.is_some(),
        can_redo: true,
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
    let next = dataset
        .redo_frame
        .take()
        .ok_or_else(|| "No hay un cambio disponible para rehacer.".to_owned())?;
    let unchanged = std::mem::replace(&mut dataset.frame, next);
    dataset.undo_frame = Some(unchanged);
    dataset.profile = None;
    Ok(HistoryResult {
        dataset: dataset_preview(&dataset.path, &dataset.frame)?,
        can_undo: true,
        can_redo: dataset.redo_frame.is_some(),
    })
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

fn apply_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<(DataFrame, usize, usize, usize, usize, usize), String> {
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
    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        add_calculated_column(&mut candidate, calculation, &rename_map)?;
        1
    } else {
        0
    };

    Ok((
        candidate,
        renamed_count,
        cast_count,
        date_count,
        removed_row_count,
        calculated_column_count,
    ))
}

fn apply_recipe_to_dataset(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
) -> Result<TransformRecipeResult, String> {
    let (
        candidate,
        renamed_column_count,
        converted_column_count,
        parsed_date_column_count,
        removed_row_count,
        calculated_column_count,
    ) = apply_recipe_to_frame(&dataset.frame, recipe)?;
    let changed = renamed_column_count
        + converted_column_count
        + parsed_date_column_count
        + removed_row_count
        + calculated_column_count
        > 0;
    // Build every fallible response value before publishing the candidate. This keeps the
    // transaction atomic even if, for example, the source file disappeared after loading.
    let preview = dataset_preview(&dataset.path, &candidate)?;
    if changed {
        dataset.undo_frame = Some(dataset.frame.clone());
        dataset.redo_frame = None;
        dataset.frame = candidate;
        dataset.profile = None;
    }
    Ok(TransformRecipeResult {
        dataset: preview,
        renamed_column_count,
        converted_column_count,
        parsed_date_column_count,
        removed_row_count,
        calculated_column_count,
        changed,
    })
}

#[tauri::command]
pub async fn apply_transform_recipe(
    app: AppHandle,
    recipe: TransformRecipe,
) -> Result<TransformRecipeResult, String> {
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

    fn temporary_delimited(extension: &str, contents: &str) -> PathBuf {
        temporary_delimited_bytes(extension, contents.as_bytes())
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
        assert_eq!(temperature.suggested_type, Some("integer"));
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
        assert_eq!(profile.columns[1].suggested_type, Some("decimal"));
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

        assert_eq!(date.suggested_type, Some("date"));
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
    fn undo_and_redo_swap_the_single_reversible_revision() {
        let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
        let (original, _) = load_csv(&path).expect("el CSV debe cargar");
        let (cleaned, _) =
            remove_duplicate_rows(&original).expect("los duplicados deben eliminarse");
        let mut dataset = LoadedDataset {
            path: path.clone(),
            frame: cleaned,
            profile: Some(profile_dataset(&original).expect("el perfil debe existir")),
            undo_frame: Some(original),
            redo_frame: None,
        };

        let undone = undo_dataset(&mut dataset).expect("el cambio debe deshacerse");
        assert_eq!(undone.dataset.row_count, 3);
        assert!(!undone.can_undo);
        assert!(undone.can_redo);
        assert!(dataset.profile.is_none());

        let redone = redo_dataset(&mut dataset).expect("el cambio debe rehacerse");
        assert_eq!(redone.dataset.row_count, 2);
        assert!(redone.can_undo);
        assert!(!redone.can_redo);
        assert!(redo_dataset(&mut dataset).is_err());

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
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

        let (result, renamed, converted, dates, removed, calculated) =
            apply_recipe_to_frame(&frame, &recipe).expect("la receta debe ser atómica y válida");
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
        let mut dataset = LoadedDataset {
            path: path.clone(),
            frame,
            profile: None,
            undo_frame: None,
            redo_frame: None,
        };
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
        assert!(dataset.undo_frame.is_none());
        assert!(dataset.redo_frame.is_none());

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
        assert!(dataset.undo_frame.is_none());
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
        assert!(dataset.undo_frame.is_none());
    }

    #[test]
    fn empty_or_already_satisfied_recipe_is_a_noop_without_history() {
        let path = temporary_csv("value\n1\n");
        let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let mut dataset = LoadedDataset {
            path: path.clone(),
            frame,
            profile: None,
            undo_frame: None,
            redo_frame: None,
        };
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
        assert!(dataset.undo_frame.is_none());
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
            .unwrap_err()
            .contains("misma receta"));

        let collision = TransformRecipe {
            renames: vec![RecipeRename {
                from: "a".into(),
                to: "b".into(),
            }],
            ..Default::default()
        };
        assert!(apply_recipe_to_frame(&frame, &collision)
            .unwrap_err()
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
        let (result, _, _, _, removed, _) = apply_recipe_to_frame(&frame, &recipe).unwrap();
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
            .expect_err("el valor inválido debe rechazarse sin importar el orden");
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
        let (result, _, _, _, _, calculated) = apply_recipe_to_frame(&frame, &recipe).unwrap();
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
            .unwrap_err()
            .contains("precisión"));

        let safe =
            DataFrame::new(1, vec![Series::new("value".into(), ["1"]).into_column()]).unwrap();
        assert!(apply_recipe_to_frame(&safe, &divide)
            .unwrap_err()
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
            .unwrap_err()
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
            .unwrap_err()
            .contains("fuera del rango"));
    }
}
