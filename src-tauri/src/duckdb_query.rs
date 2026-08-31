use std::{
    collections::HashSet,
    fs::{self, File},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use duckdb::{
    types::{TimeUnit, ValueRef},
    Connection,
};
use polars::prelude::{DataFrame, ParquetWriter};

use crate::dataset::{DatasetColumn, DatasetQueryResult, OPERATION_CANCELLED_MESSAGE};

const QUERY_POLL_INTERVAL: Duration = Duration::from_millis(10);
const DUCKDB_MEMORY_LIMIT: &str = "512MB";
const DUCKDB_MAX_TEMP_DIRECTORY_SIZE: &str = "8GB";

pub(crate) struct DuckDbQuerySpec {
    pub(crate) bounded_query: String,
    pub(crate) count_query: String,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
    pub(crate) dataset_view_query: Option<String>,
    pub(crate) current_order_column: Option<String>,
    pub(crate) compared_order_column: Option<String>,
}

#[derive(Clone, Copy)]
enum DatasetSource<'a> {
    Frame(&'a DataFrame),
    Parquet(&'a Path),
    File {
        path: &'a Path,
        format: DuckDbFileFormat,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum DuckDbFileFormat {
    Parquet,
    Delimited { delimiter: u8 },
    Json,
}

pub(crate) fn execute_duckdb_query<C>(
    current: &DataFrame,
    compared: Option<&DataFrame>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::Frame(current),
        compared.map(DatasetSource::Frame),
        spec,
        is_cancelled,
    )
}

pub(crate) fn execute_duckdb_query_from_parquet<C>(
    current_path: &Path,
    compared: Option<&DataFrame>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::Parquet(current_path),
        compared.map(DatasetSource::Frame),
        spec,
        is_cancelled,
    )
}

pub(crate) fn execute_duckdb_query_from_parquet_sources<C>(
    current_path: &Path,
    compared_path: Option<&Path>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::Parquet(current_path),
        compared_path.map(DatasetSource::Parquet),
        spec,
        is_cancelled,
    )
}

pub(crate) fn execute_duckdb_query_from_file<C>(
    current_path: &Path,
    current_format: DuckDbFileFormat,
    compared: Option<&DataFrame>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::File {
            path: current_path,
            format: current_format,
        },
        compared.map(DatasetSource::Frame),
        spec,
        is_cancelled,
    )
}

pub(crate) fn execute_duckdb_query_from_file_sources<C>(
    current_path: &Path,
    current_format: DuckDbFileFormat,
    compared_path: Option<&Path>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::File {
            path: current_path,
            format: current_format,
        },
        compared_path.map(DatasetSource::Parquet),
        spec,
        is_cancelled,
    )
}

pub(crate) fn materialize_file_to_parquet(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    destination: &Path,
    column_order: Option<&[String]>,
) -> Result<(), String> {
    let connection = Connection::open_in_memory()
        .map_err(|error| format!("No se pudo iniciar DuckDB para el snapshot: {error}"))?;
    let resource_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el espacio temporal para el snapshot DuckDB: {error}")
    })?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    let source = file_scan_expression(source_path, source_format);
    let destination = destination
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let projection = if matches!(source_format, DuckDbFileFormat::Json) {
        match column_order {
            Some(columns) => json_projection(&connection, &source, columns)?,
            None => "*".to_owned(),
        }
    } else {
        column_order
            .map(|columns| {
                if columns.is_empty() {
                    return Err("El JSON no contiene columnas utilizables.".to_owned());
                }
                Ok(columns
                    .iter()
                    .map(|column| quote_identifier(column))
                    .collect::<Vec<_>>()
                    .join(", "))
            })
            .transpose()?
            .unwrap_or_else(|| "*".to_owned())
    };
    let query = format!(
        "SET preserve_insertion_order = true; COPY (SELECT {projection} FROM {source}) TO '{destination}' (FORMAT PARQUET)"
    );
    connection
        .execute_batch(&query)
        .map_err(|error| format!("DuckDB no pudo crear el snapshot: {error}"))?;
    Ok(())
}

pub(crate) fn export_file_to_json(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    destination: &Path,
) -> Result<(), String> {
    let connection = Connection::open_in_memory()
        .map_err(|error| format!("No se pudo iniciar DuckDB para la exportación JSON: {error}"))?;
    let resource_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el espacio temporal para la exportación JSON: {error}")
    })?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    let source = file_scan_expression(source_path, source_format);
    let destination = destination
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let query = format!(
        "SET preserve_insertion_order = true; COPY (SELECT * FROM {source}) TO '{destination}' (FORMAT JSON, ARRAY true)"
    );
    connection
        .execute_batch(&query)
        .map_err(|error| format!("DuckDB no pudo crear la exportación JSON: {error}"))?;
    Ok(())
}

fn execute_duckdb_query_with_source<C>(
    current: DatasetSource<'_>,
    compared: Option<DatasetSource<'_>>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if is_cancelled() {
        return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let stop_watcher = Arc::new(AtomicBool::new(false));
    let connection = Connection::open_in_memory()
        .map_err(|error| format!("No se pudo iniciar DuckDB para la consulta: {error}"))?;
    let interrupt = connection.interrupt_handle();
    let watcher_cancelled = Arc::clone(&cancelled);
    let watcher_stop = Arc::clone(&stop_watcher);
    let watcher = thread::spawn(move || {
        while !watcher_stop.load(Ordering::Acquire) {
            if is_cancelled() {
                watcher_cancelled.store(true, Ordering::Release);
                interrupt.interrupt();
                return;
            }
            thread::sleep(QUERY_POLL_INTERVAL);
        }
    });

    let result = execute_query_with_connection(&connection, current, compared, spec);
    stop_watcher.store(true, Ordering::Release);
    let watcher_joined = watcher.join().is_ok();

    if cancelled.load(Ordering::Acquire) {
        return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
    }
    if !watcher_joined {
        return Err("El monitor de cancelación de DuckDB terminó inesperadamente.".to_owned());
    }
    result
}

fn execute_query_with_connection(
    connection: &Connection,
    current: DatasetSource<'_>,
    compared: Option<DatasetSource<'_>>,
    spec: &DuckDbQuerySpec,
) -> Result<DatasetQueryResult, String> {
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el espacio temporal para DuckDB: {error}"))?;
    configure_duckdb_resources(connection, directory.path())?;
    let current_path = directory.path().join("dataset.parquet");
    if let Some(dataset_view_query) = &spec.dataset_view_query {
        let compared = compared.ok_or_else(|| {
            "La consulta DuckDB requiere un dataset comparado para construir el JOIN.".to_owned()
        })?;
        let compared_path = directory.path().join("compared.parquet");
        register_dataset_view(
            connection,
            "__columnia_current",
            current,
            &current_path,
            "activo",
            spec.current_order_column.as_deref(),
        )?;
        register_dataset_view(
            connection,
            "__columnia_compared",
            compared,
            &compared_path,
            "comparado",
            spec.compared_order_column.as_deref(),
        )?;
        connection
            .execute_batch(dataset_view_query)
            .map_err(|error| format!("DuckDB no pudo preparar la vista del JOIN: {error}"))?;
    } else {
        register_dataset_view(
            connection,
            "dataset",
            current,
            &current_path,
            "activo",
            spec.current_order_column.as_deref(),
        )?;
    }

    let total_i64 = connection
        .query_row(&spec.count_query, [], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("DuckDB no pudo contar el resultado: {error}"))?;
    let row_count = usize::try_from(total_i64)
        .map_err(|_| "DuckDB devolvió un conteo de filas inválido.".to_owned())?;
    if spec.offset > row_count {
        return Err("La página solicitada está fuera del resultado filtrado.".to_owned());
    }

    let mut statement = connection
        .prepare(&spec.bounded_query)
        .map_err(|error| format!("DuckDB no pudo preparar la consulta: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("DuckDB no pudo ejecutar la consulta: {error}"))?;
    let result_statement = rows
        .as_ref()
        .ok_or_else(|| "DuckDB no devolvió metadatos de columnas.".to_owned())?;
    let columns = result_statement
        .column_names()
        .into_iter()
        .enumerate()
        .map(|(index, name)| DatasetColumn {
            name,
            data_type: format!("{:?}", result_statement.column_type(index)),
        })
        .collect::<Vec<_>>();
    let mut result_rows = Vec::with_capacity(spec.limit.min(row_count.saturating_sub(spec.offset)));
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("DuckDB no pudo leer una fila: {error}"))?
    {
        result_rows.push(
            (0..columns.len())
                .map(|index| {
                    row.get_ref(index)
                        .map_err(|error| format!("DuckDB no pudo leer la columna: {error}"))
                        .and_then(value_to_preview)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }

    Ok(DatasetQueryResult {
        columns,
        row_count,
        offset: spec.offset,
        rows: result_rows,
        truncated: spec.offset.saturating_add(spec.limit) < row_count,
    })
}

fn configure_duckdb_resources(connection: &Connection, directory: &Path) -> Result<(), String> {
    let spill_directory = directory.join("duckdb-spill");
    fs::create_dir_all(&spill_directory)
        .map_err(|error| format!("No se pudo preparar el derrame temporal de DuckDB: {error}"))?;
    let escaped_spill_directory = spill_directory
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let query = format!(
        "SET memory_limit = '{DUCKDB_MEMORY_LIMIT}'; SET max_temp_directory_size = '{DUCKDB_MAX_TEMP_DIRECTORY_SIZE}'; SET temp_directory = '{escaped_spill_directory}'; SET preserve_insertion_order = true;"
    );
    connection
        .execute_batch(&query)
        .map_err(|error| format!("DuckDB no pudo configurar sus límites de recursos: {error}"))
}

fn write_frame_snapshot(
    frame: &DataFrame,
    path: &std::path::Path,
    label: &str,
    order_column: Option<&str>,
) -> Result<(), String> {
    let mut file = File::create(path)
        .map_err(|error| format!("No se pudo preparar el snapshot {label} para DuckDB: {error}"))?;
    let mut snapshot = if let Some(order_column) = order_column {
        frame
            .with_row_index(order_column.into(), Some(0))
            .map_err(|error| {
                format!("No se pudo preparar el orden del snapshot {label}: {error}")
            })?
    } else {
        frame.clone()
    };
    ParquetWriter::new(&mut file)
        .finish(&mut snapshot)
        .map_err(|error| format!("No se pudo escribir el snapshot {label} para DuckDB: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot {label} para DuckDB: {error}"))
}

fn register_parquet_view(
    connection: &Connection,
    name: &str,
    path: &std::path::Path,
) -> Result<(), String> {
    let escaped_path = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let query = format!("CREATE VIEW {name} AS SELECT * FROM read_parquet('{escaped_path}')");
    connection
        .execute_batch(&query)
        .map_err(|error| format!("DuckDB no pudo registrar la tabla {name}: {error}"))?;
    Ok(())
}

fn register_dataset_view(
    connection: &Connection,
    name: &str,
    source: DatasetSource<'_>,
    frame_path: &Path,
    label: &str,
    order_column: Option<&str>,
) -> Result<(), String> {
    match source {
        DatasetSource::Frame(frame) => {
            write_frame_snapshot(frame, frame_path, label, order_column)?;
            register_parquet_view(connection, name, frame_path)
        }
        DatasetSource::Parquet(path) => register_file_view(
            connection,
            name,
            path,
            DuckDbFileFormat::Parquet,
            order_column,
        ),
        DatasetSource::File { path, format } => {
            register_file_view(connection, name, path, format, order_column)
        }
    }
}

fn register_file_view(
    connection: &Connection,
    name: &str,
    path: &Path,
    format: DuckDbFileFormat,
    order_column: Option<&str>,
) -> Result<(), String> {
    let source = file_scan_expression(path, format);
    let query = if let Some(order_column) = order_column {
        format!(
            "CREATE VIEW {name} AS SELECT *, row_number() OVER () - 1 AS {} FROM {source}",
            quote_identifier(order_column),
        )
    } else {
        format!("CREATE VIEW {name} AS SELECT * FROM {source}")
    };
    connection
        .execute_batch(&query)
        .map_err(|error| format!("DuckDB no pudo registrar la tabla {name}: {error}"))?;
    Ok(())
}

fn file_scan_expression(path: &Path, format: DuckDbFileFormat) -> String {
    let escaped_path = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    match format {
        DuckDbFileFormat::Parquet => format!("read_parquet('{escaped_path}')"),
        DuckDbFileFormat::Delimited { delimiter } => {
            let delimiter = char::from(delimiter);
            let escaped_delimiter = delimiter.to_string().replace('\'', "''");
            format!(
                "read_csv_auto('{escaped_path}', header = true, all_varchar = true, delim = '{escaped_delimiter}')"
            )
        }
        DuckDbFileFormat::Json => format!("read_json_auto('{escaped_path}')"),
    }
}

fn json_projection(
    connection: &Connection,
    source: &str,
    columns: &[String],
) -> Result<String, String> {
    if columns.is_empty() {
        return Err("El JSON no contiene columnas utilizables.".to_owned());
    }
    let mut statement = connection
        .prepare(&format!("DESCRIBE SELECT * FROM {source}"))
        .map_err(|error| format!("DuckDB no pudo inspeccionar el JSON: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("DuckDB no pudo inspeccionar el JSON: {error}"))?;
    let mut nested_columns = HashSet::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("DuckDB no pudo inspeccionar el JSON: {error}"))?
    {
        let name = row
            .get::<_, String>(0)
            .map_err(|error| format!("DuckDB no pudo inspeccionar el JSON: {error}"))?;
        let data_type = row
            .get::<_, String>(1)
            .map_err(|error| format!("DuckDB no pudo inspeccionar el JSON: {error}"))?;
        if data_type.contains("STRUCT")
            || data_type.contains("LIST")
            || data_type.contains("MAP")
            || data_type.contains("UNION")
            || data_type.contains("[]")
        {
            nested_columns.insert(name);
        }
    }
    Ok(columns
        .iter()
        .map(|column| {
            let identifier = quote_identifier(column);
            if nested_columns.contains(column) {
                format!("CAST(to_json({identifier}) AS VARCHAR) AS {identifier}")
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn value_to_preview(value: ValueRef<'_>) -> Result<Option<String>, String> {
    let preview = match value {
        ValueRef::Null => return Ok(None),
        ValueRef::Boolean(value) => value.to_string(),
        ValueRef::TinyInt(value) => value.to_string(),
        ValueRef::SmallInt(value) => value.to_string(),
        ValueRef::Int(value) => value.to_string(),
        ValueRef::BigInt(value) => value.to_string(),
        ValueRef::HugeInt(value) => value.to_string(),
        ValueRef::UHugeInt(value) => value.to_string(),
        ValueRef::UTinyInt(value) => value.to_string(),
        ValueRef::USmallInt(value) => value.to_string(),
        ValueRef::UInt(value) => value.to_string(),
        ValueRef::UBigInt(value) => value.to_string(),
        ValueRef::Float(value) => value.to_string(),
        ValueRef::Double(value) => value.to_string(),
        ValueRef::Decimal(value) => value.to_string(),
        ValueRef::Timestamp(unit, value) => format_timestamp(unit, value),
        ValueRef::Text(value) => String::from_utf8_lossy(value).into_owned(),
        ValueRef::Blob(value) | ValueRef::Geometry(value) => format_blob(value),
        ValueRef::Date32(value) => value.to_string(),
        ValueRef::Time64(unit, value) => format_timestamp(unit, value),
        ValueRef::Interval {
            months,
            days,
            nanos,
        } => {
            format!("{months} months {days} days {nanos} nanos")
        }
        ValueRef::Enum(..)
        | ValueRef::List(..)
        | ValueRef::Struct(..)
        | ValueRef::Array(..)
        | ValueRef::Map(..)
        | ValueRef::Union(..) => {
            return Err(
                "DuckDB devolvió un tipo anidado no compatible con la vista tabular.".to_owned(),
            )
        }
        _ => return Err("DuckDB devolvió un tipo no compatible con la vista tabular.".to_owned()),
    };
    Ok(Some(preview))
}

fn format_timestamp(unit: TimeUnit, value: i64) -> String {
    let unit = match unit {
        TimeUnit::Second => "s",
        TimeUnit::Millisecond => "ms",
        TimeUnit::Microsecond => "us",
        TimeUnit::Nanosecond => "ns",
    };
    format!("{value}{unit}")
}

fn format_blob(value: &[u8]) -> String {
    let mut output = String::with_capacity(value.len().saturating_mul(2) + 2);
    output.push_str("0x");
    for byte in value {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use polars::df;

    use super::*;

    #[test]
    fn executes_a_bounded_query_against_parquet_views() {
        let current = df![
            "city" => &["Santo Domingo", "Santiago", "La Romana"],
            "value" => &[10_i64, 20, 30]
        ]
        .expect("el dataset debe construirse");
        let spec = DuckDbQuerySpec {
            bounded_query: "SELECT city, value FROM dataset LIMIT 1 OFFSET 1".to_owned(),
            count_query: "SELECT COUNT(*) FROM (SELECT city, value FROM dataset) AS count_rows"
                .to_owned(),
            offset: 1,
            limit: 1,
            dataset_view_query: None,
            current_order_column: None,
            compared_order_column: None,
        };

        let result = execute_duckdb_query(&current, None, &spec, || false)
            .expect("DuckDB debe ejecutar la consulta");

        assert_eq!(result.row_count, 3);
        assert_eq!(result.offset, 1);
        assert_eq!(
            result.rows,
            vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
        );
        assert!(result.truncated);
    }

    #[test]
    fn reuses_an_existing_parquet_snapshot_without_writing_a_query_snapshot() {
        let current = df![
            "city" => &["Santo Domingo", "Santiago", "La Romana"],
            "value" => &[10_i64, 20, 30]
        ]
        .expect("el dataset debe construirse");
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let path = directory.path().join("current.parquet");
        write_frame_snapshot(&current, &path, "fixture", None)
            .expect("el snapshot debe escribirse");
        let spec = DuckDbQuerySpec {
            bounded_query:
                "SELECT city, value FROM dataset ORDER BY \"__columnia_order\" LIMIT 1 OFFSET 1"
                    .to_owned(),
            count_query: "SELECT COUNT(*) FROM (SELECT city, value FROM dataset) AS count_rows"
                .to_owned(),
            offset: 1,
            limit: 1,
            dataset_view_query: None,
            current_order_column: Some("__columnia_order".to_owned()),
            compared_order_column: None,
        };

        let result = execute_duckdb_query_from_parquet(&path, None, &spec, || false)
            .expect("DuckDB debe consultar el snapshot existente");

        assert_eq!(result.row_count, 3);
        assert_eq!(
            result.rows,
            vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
        );
        assert!(path.is_file());
    }

    #[test]
    fn queries_a_delimited_source_without_creating_a_snapshot() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let path = directory.path().join("current.csv");
        fs::write(&path, "city,value\nSanto Domingo,10\nSantiago,20\n")
            .expect("se debe escribir el CSV");
        let spec = DuckDbQuerySpec {
            bounded_query:
                "SELECT city, value FROM dataset ORDER BY \"__columnia_order\" LIMIT 1 OFFSET 1"
                    .to_owned(),
            count_query: "SELECT COUNT(*) FROM (SELECT city, value FROM dataset) AS count_rows"
                .to_owned(),
            offset: 1,
            limit: 1,
            dataset_view_query: None,
            current_order_column: Some("__columnia_order".to_owned()),
            compared_order_column: None,
        };

        let result = execute_duckdb_query_from_file(
            &path,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            None,
            &spec,
            || false,
        )
        .expect("DuckDB debe consultar el CSV original");

        assert_eq!(result.row_count, 2);
        assert_eq!(
            result.rows,
            vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
        );
        assert!(path.is_file());
        assert!(!directory.path().join("dataset.parquet").exists());
    }

    #[test]
    fn queries_a_json_source_without_creating_a_snapshot() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let path = directory.path().join("current.json");
        fs::write(
            &path,
            r#"[{"city":"Santo Domingo","value":10},{"city":"Santiago","value":20}]"#,
        )
        .expect("se debe escribir el JSON");
        let spec = DuckDbQuerySpec {
            bounded_query:
                "SELECT city, value FROM dataset ORDER BY \"__columnia_order\" LIMIT 1 OFFSET 1"
                    .to_owned(),
            count_query: "SELECT COUNT(*) FROM (SELECT city, value FROM dataset) AS count_rows"
                .to_owned(),
            offset: 1,
            limit: 1,
            dataset_view_query: None,
            current_order_column: Some("__columnia_order".to_owned()),
            compared_order_column: None,
        };

        let result =
            execute_duckdb_query_from_file(&path, DuckDbFileFormat::Json, None, &spec, || false)
                .expect("DuckDB debe consultar el JSON original");

        assert_eq!(result.row_count, 2);
        assert_eq!(
            result.rows,
            vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
        );
        assert!(path.is_file());
        assert!(!directory.path().join("dataset.parquet").exists());
    }

    #[test]
    fn joins_two_existing_parquet_snapshots_without_reencoding_them() {
        let current = df![
            "id" => &[1_i64, 2, 3],
            "name" => &["A", "B", "C"]
        ]
        .expect("el dataset activo debe construirse");
        let compared = df![
            "id" => &[2_i64, 3],
            "amount" => &[200_i64, 300]
        ]
        .expect("el dataset comparado debe construirse");
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let current_path = directory.path().join("current.parquet");
        let compared_path = directory.path().join("compared.parquet");
        write_frame_snapshot(&current, &current_path, "activo", None)
            .expect("el snapshot activo debe escribirse");
        write_frame_snapshot(&compared, &compared_path, "comparado", None)
            .expect("el snapshot comparado debe escribirse");
        let spec = DuckDbQuerySpec {
            bounded_query: "SELECT id, name, amount FROM joined ORDER BY \"__columnia_order\" LIMIT 10"
                .to_owned(),
            count_query: "SELECT COUNT(*) FROM joined".to_owned(),
            offset: 0,
            limit: 10,
            dataset_view_query: Some(
                "CREATE VIEW joined AS SELECT current.id, current.name, compared.amount, current.\"__columnia_order\" FROM __columnia_current AS current INNER JOIN __columnia_compared AS compared ON current.id = compared.id"
                    .to_owned(),
            ),
            current_order_column: Some("__columnia_order".to_owned()),
            compared_order_column: None,
        };

        let result = execute_duckdb_query_from_parquet_sources(
            &current_path,
            Some(&compared_path),
            &spec,
            || false,
        )
        .expect("DuckDB debe unir los snapshots existentes");

        assert_eq!(result.row_count, 2);
        assert_eq!(
            result.rows,
            vec![
                vec![
                    Some("2".to_owned()),
                    Some("B".to_owned()),
                    Some("200".to_owned())
                ],
                vec![
                    Some("3".to_owned()),
                    Some("C".to_owned()),
                    Some("300".to_owned())
                ]
            ]
        );
        assert!(current_path.is_file());
        assert!(compared_path.is_file());
    }

    #[test]
    fn configures_bounded_memory_and_a_private_spill_directory() {
        let connection = Connection::open_in_memory().expect("DuckDB debe iniciar");
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");

        configure_duckdb_resources(&connection, directory.path())
            .expect("DuckDB debe aceptar sus límites de recursos");

        let memory_limit: String = connection
            .query_row("SELECT current_setting('memory_limit')", [], |row| {
                row.get(0)
            })
            .expect("se debe consultar el límite de memoria");
        let temp_directory: String = connection
            .query_row("SELECT current_setting('temp_directory')", [], |row| {
                row.get(0)
            })
            .expect("se debe consultar el directorio de derrame");
        let max_temp_directory_size: String = connection
            .query_row(
                "SELECT current_setting('max_temp_directory_size')",
                [],
                |row| row.get(0),
            )
            .expect("se debe consultar el límite de disco temporal");

        assert!(
            memory_limit.contains("512") || memory_limit.contains("488"),
            "límite de memoria inesperado: {memory_limit}"
        );
        assert!(temp_directory.contains("duckdb-spill"));
        assert!(
            max_temp_directory_size.to_ascii_lowercase().contains("gib"),
            "límite de disco temporal inesperado: {max_temp_directory_size}"
        );
    }

    #[test]
    fn materializes_a_delimited_source_with_the_same_resource_boundary() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let source = directory.path().join("source.csv");
        let destination = directory.path().join("snapshot.parquet");
        fs::write(&source, "city,value\nSanto Domingo,10\nSantiago,20\n")
            .expect("se debe escribir la fuente delimitada");

        materialize_file_to_parquet(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            &destination,
            None,
        )
        .expect("DuckDB debe crear el snapshot delimitado");

        let connection = Connection::open_in_memory().expect("DuckDB debe iniciar");
        let escaped_path = destination
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "''");
        let row_count: i64 = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM read_parquet('{escaped_path}')"),
                [],
                |row| row.get(0),
            )
            .expect("el snapshot debe poder leerse");

        assert_eq!(row_count, 2);
        assert!(destination.is_file());
    }

    #[test]
    fn rejects_a_query_cancelled_before_start() {
        let current = df!["id" => &[1_i64]].expect("el dataset debe construirse");
        let spec = DuckDbQuerySpec {
            bounded_query: "SELECT id FROM dataset LIMIT 1".to_owned(),
            count_query: "SELECT COUNT(*) FROM dataset".to_owned(),
            offset: 0,
            limit: 1,
            dataset_view_query: None,
            current_order_column: None,
            compared_order_column: None,
        };

        let error = execute_duckdb_query(&current, None, &spec, || true)
            .expect_err("una consulta cancelada no debe iniciar DuckDB");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    }
}
