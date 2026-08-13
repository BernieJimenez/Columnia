use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use calamine::{open_workbook_auto, Data, DataType as CalamineDataType, Range, Reader};
use chrono::NaiveDate;
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

const PREVIEW_ROW_LIMIT: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const PROTOTYPE_FILE_LIMIT_BYTES: u64 = 500 * 1024 * 1024;
const OPERATION_CANCELLED_MESSAGE: &str = "Operación cancelada por el usuario.";

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
                "csv" | "tsv" | "parquet" | "xlsx" | "xls" | "xlsb" | "ods"
            )
        })
        .ok_or_else(|| {
            "Columnia admite CSV, TSV, Parquet y libros Excel/ODS en esta versión.".to_owned()
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
    standard_deviation: Option<f64>,
    first_quartile: Option<f64>,
    median: Option<f64>,
    third_quartile: Option<f64>,
    outlier_count: usize,
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
    if !column.dtype().is_primitive_numeric() {
        return Ok(None);
    }

    let mut values = (0..column.len())
        .map(|index| {
            column
                .get(index)
                .map_err(|error| format!("No se pudo analizar la columna numérica: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(numeric_value)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);

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
        .filter(|value| value.parse::<i64>().is_ok())
        .count();
    let decimal_count = values
        .iter()
        .filter(|value| value.parse::<f64>().is_ok_and(|number| number.is_finite()))
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
        } else {
            (None, None, None)
        };
        let text_statistics = text_statistics(column)?;
        let numeric_statistics = numeric_statistics(column)?;

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

fn spreadsheet_range_to_frame(range: &Range<Data>) -> Result<DataFrame, String> {
    if range.is_empty() {
        return Err("La hoja seleccionada está vacía.".to_owned());
    }
    let width = range.width();
    let height = range.height();
    let headers = unique_spreadsheet_headers(
        (0..width)
            .map(|column| {
                range
                    .get((0, column))
                    .map(ToString::to_string)
                    .unwrap_or_default()
            })
            .collect(),
    );
    let mut columns = Vec::with_capacity(width);
    for (column_index, name) in headers.iter().enumerate() {
        let cells = (1..height)
            .map(|row| range.get((row, column_index)).expect("rango rectangular"))
            .collect::<Vec<_>>();
        columns.push(spreadsheet_cells_to_column(name, &cells)?);
    }
    DataFrame::new(height.saturating_sub(1), columns)
        .map_err(|error| format!("No se pudo construir el dataset desde la hoja: {error}"))
}

fn load_spreadsheet_sheet(path: &Path, sheet_name: &str) -> Result<DataFrame, String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
    if !workbook.sheet_names().iter().any(|name| name == sheet_name) {
        return Err("La hoja seleccionada ya no está disponible en el libro.".to_owned());
    }
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
    spreadsheet_range_to_frame(&range)
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
    let frame = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.to_path_buf()))
        .map_err(|error| format!("No se pudo abrir el CSV: {error}"))?
        .finish()
        .map_err(|error| format!("No se pudo interpretar el CSV: {error}"))?;

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
        "csv" | "tsv" => {
            let separator = if extension == "tsv" { b'\t' } else { b',' };
            CsvReadOptions::default()
                .with_has_header(true)
                .with_infer_schema_length(if extension == "tsv" {
                    Some(0)
                } else {
                    Some(100)
                })
                .map_parse_options(|options| options.with_separator(separator))
                .try_into_reader_with_file_path(Some(path.to_path_buf()))
                .map_err(|error| format!("No se pudo abrir el archivo delimitado: {error}"))?
                .finish()
                .map_err(|error| format!("No se pudo interpretar el archivo delimitado: {error}"))?
        }
        "parquet" => {
            let file = fs::File::open(path)
                .map_err(|error| format!("No se pudo abrir el Parquet: {error}"))?;
            ParquetReader::new(file)
                .set_low_memory(true)
                .read_parallel(ParallelStrategy::None)
                .finish()
                .map_err(|error| format!("No se pudo interpretar el Parquet: {error}"))?
        }
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
            &["csv", "tsv", "parquet", "xlsx", "xls", "xlsb", "ods"],
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
    }))
}

#[tauri::command]
pub async fn load_dataset_selection(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
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
            let frame = load_spreadsheet_sheet(&pending.path, sheet_name)?;
            ensure_not_cancelled(app.state::<DatasetState>().load_was_cancelled(generation))?;
            send_progress(&on_progress, "load", "Preparando vista previa", 85);
            let preview = dataset_preview(&pending.path, &frame)?;
            (frame, preview)
        } else {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj del sistema debe ser válido")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "columnia-delimited-test-{}-{nonce}.{extension}",
            std::process::id()
        ));
        let mut file = File::create(&path).expect("se debe crear el archivo temporal");
        file.write_all(contents.as_bytes())
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
        let path = std::env::temp_dir().join("columnia-invalid-dataset.txt");
        File::create(&path).expect("se debe poder crear el archivo temporal");

        let error = validate_dataset_file(&path)
            .expect_err("un archivo que no es un dataset compatible debe rechazarse");

        assert!(error.contains("admite CSV, TSV, Parquet y libros Excel/ODS"));
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
        assert_eq!(temperature.empty_count, None);
        assert_eq!(temperature.first_quartile, Some(27.25));
        assert_eq!(temperature.median, Some(28.0));
        assert_eq!(temperature.third_quartile, Some(28.5));
        assert!((temperature.standard_deviation.unwrap() - 2.061_552).abs() < 0.001);
        assert_eq!(temperature.outlier_count, Some(1));

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

        let frame = spreadsheet_range_to_frame(&range).expect("la hoja debe convertirse");
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
}
