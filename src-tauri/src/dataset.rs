use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::NaiveDate;
use polars::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

const PREVIEW_ROW_LIMIT: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const PROTOTYPE_FILE_LIMIT_BYTES: u64 = 100 * 1024 * 1024;

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

struct LoadedDataset {
    _path: PathBuf,
    frame: DataFrame,
    profile: Option<DatasetProfile>,
}

#[derive(Default)]
pub struct DatasetState {
    current: Mutex<Option<LoadedDataset>>,
}

fn validate_csv(path: &Path) -> Result<u64, String> {
    if !path.is_file() {
        return Err("El archivo seleccionado no existe o no es un archivo regular.".into());
    }

    let is_csv = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"));

    if !is_csv {
        return Err("Columnia solo admite archivos CSV en este primer hito.".into());
    }

    let size = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();

    if size > PROTOTYPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "El CSV supera el límite temporal de {} MB. La carga por streaming se incorporará en un próximo hito.",
            PROTOTYPE_FILE_LIMIT_BYTES / 1024 / 1024
        ));
    }

    Ok(size)
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

fn profile_dataset(frame: &DataFrame) -> Result<DatasetProfile, String> {
    let row_count = frame.height();
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
    let columns = frame
        .columns()
        .iter()
        .map(|column| {
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
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(DatasetProfile {
        row_count,
        duplicate_row_count,
        duplicate_percentage,
        columns,
    })
}

fn load_csv(path: &Path) -> Result<(DataFrame, DatasetPreview), String> {
    let file_size_bytes = validate_csv(path)?;
    let frame = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.to_path_buf()))
        .map_err(|error| format!("No se pudo abrir el CSV: {error}"))?
        .finish()
        .map_err(|error| format!("No se pudo interpretar el CSV: {error}"))?;

    let columns = frame
        .columns()
        .iter()
        .map(|column| DatasetColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
        })
        .collect();

    let rows = dataset_page(&frame, 0, PREVIEW_ROW_LIMIT)?.rows;

    let preview = DatasetPreview {
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
    };

    Ok((frame, preview))
}

#[tauri::command]
pub async fn pick_and_load_csv(
    app: AppHandle,
    state: State<'_, DatasetState>,
) -> Result<Option<DatasetPreview>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter("Archivo CSV", &["csv"])
        .blocking_pick_file();

    let Some(selection) = selection else {
        return Ok(None);
    };

    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    let (frame, preview) = load_csv(&path)?;

    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    *current = Some(LoadedDataset {
        _path: path,
        frame,
        profile: None,
    });

    Ok(Some(preview))
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
    let dataset = current
        .as_ref()
        .ok_or_else(|| "No hay un dataset activo. Selecciona primero un archivo CSV.".to_owned())?;

    dataset_page(&dataset.frame, offset, limit)
}

#[tauri::command]
pub async fn get_dataset_profile(app: AppHandle) -> Result<DatasetProfile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo CSV.".to_owned()
        })?;

        if let Some(profile) = &dataset.profile {
            return Ok(profile.clone());
        }

        let profile = profile_dataset(&dataset.frame)?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    })
    .await
    .map_err(|error| format!("El análisis de calidad se interrumpió: {error}"))?
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
    fn rejects_non_csv_files() {
        let path = std::env::temp_dir().join("columnia-invalid-dataset.txt");
        File::create(&path).expect("se debe poder crear el archivo temporal");

        let error = validate_csv(&path).expect_err("un archivo que no es CSV debe rechazarse");

        assert!(error.contains("solo admite archivos CSV"));
        fs::remove_file(path).expect("se debe limpiar el archivo temporal");
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
}
