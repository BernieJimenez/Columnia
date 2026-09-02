use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufWriter, Write},
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

pub(crate) fn execute_duckdb_query_from_frame_and_parquet<C>(
    current: &DataFrame,
    compared_path: &Path,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_query_with_source(
        DatasetSource::Frame(current),
        Some(DatasetSource::Parquet(compared_path)),
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

pub(crate) fn materialize_file_to_parquet_with_projection<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    destination: &Path,
    projection: &str,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Send + 'static,
{
    if projection.trim().is_empty() {
        return Err("La proyección source-backed no puede estar vacía.".to_owned());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el espacio temporal para el snapshot protegido: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        let source = file_scan_expression(source_path, source_format);
        let destination = destination
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "''");
        let query = format!(
            "SET preserve_insertion_order = true; COPY (SELECT {projection} FROM {source}) TO '{destination}' (FORMAT PARQUET)"
        );
        connection
            .execute_batch(&query)
            .map_err(|error| format!("DuckDB no pudo crear el snapshot protegido: {error}"))?;
        Ok(())
    })
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

pub(crate) fn export_file_to_csv_with_cancel<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    destination: &Path,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el espacio temporal para la exportación CSV: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        let source = csv_export_scan_expression(source_path, source_format);
        let destination = destination
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "''");
        let projection = csv_export_projection(connection, &source)?;
        let query = format!(
            "SET preserve_insertion_order = true; COPY (SELECT {projection} FROM {source}) TO '{destination}' (FORMAT CSV, HEADER, DELIMITER ',')"
        );
        connection
            .execute_batch(&query)
            .map_err(|error| format!("DuckDB no pudo crear la exportación CSV: {error}"))?;
        Ok(())
    })
}

pub(crate) fn export_file_to_sql_with_cancel<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    destination: &Path,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation_with_cancel_state(is_cancelled, |connection, cancelled| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el espacio temporal para la exportación SQL: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        let source = csv_export_scan_expression(source_path, source_format);
        let source_columns = describe_source_columns(connection, &source)?;
        let projection = source_columns
            .iter()
            .map(|(name, data_type)| {
                let identifier = quote_identifier(name);
                if is_sql_text_projection(data_type) {
                    format!("CAST({identifier} AS VARCHAR) AS {identifier}")
                } else {
                    identifier
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let statement = format!("SELECT {projection} FROM {source}");
        let mut query = connection
            .prepare(&statement)
            .map_err(|error| format!("DuckDB no pudo preparar la exportación SQL: {error}"))?;
        let mut rows = query
            .query([])
            .map_err(|error| format!("DuckDB no pudo ejecutar la exportación SQL: {error}"))?;
        let result_statement = rows
            .as_ref()
            .ok_or_else(|| "DuckDB no devolvió el esquema de la exportación SQL.".to_owned())?;
        let columns = result_statement.column_names();
        let sql_types = source_columns
            .iter()
            .map(|(_, data_type)| duckdb_sql_type(data_type))
            .collect::<Vec<_>>();
        let mut output = BufWriter::new(
            File::create(destination)
                .map_err(|error| format!("No se pudo crear la exportación SQL: {error}"))?,
        );
        writeln!(
            output,
            "-- Exportado por Columnia como script SQL portable."
        )
        .map_err(|error| format!("No se pudo escribir el encabezado SQL: {error}"))?;
        writeln!(output, "BEGIN TRANSACTION;")
            .map_err(|error| format!("No se pudo escribir el inicio SQL: {error}"))?;
        writeln!(output, "DROP TABLE IF EXISTS \"dataset\";")
            .map_err(|error| format!("No se pudo escribir la limpieza SQL: {error}"))?;
        write!(output, "CREATE TABLE \"dataset\" (")
            .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")
                    .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
            }
            write!(output, "{} {}", quote_identifier(column), sql_types[index])
                .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
        }
        writeln!(output, ");")
            .map_err(|error| format!("No se pudo cerrar el esquema SQL: {error}"))?;

        while let Some(row) = rows
            .next()
            .map_err(|error| format!("DuckDB no pudo leer una fila SQL: {error}"))?
        {
            if cancelled.load(Ordering::Acquire) {
                return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            write!(output, "INSERT INTO \"dataset\" (")
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            for (index, column) in columns.iter().enumerate() {
                if index > 0 {
                    write!(output, ", ")
                        .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
                }
                write!(output, "{}", quote_identifier(column))
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            write!(output, ") VALUES (")
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            for index in 0..columns.len() {
                if index > 0 {
                    write!(output, ", ")
                        .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
                }
                let value = row
                    .get_ref(index)
                    .map_err(|error| format!("No se pudo leer una columna SQL: {error}"))?;
                write!(output, "{}", duckdb_sql_value(value)?)
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            writeln!(output, ");")
                .map_err(|error| format!("No se pudo cerrar una fila SQL: {error}"))?;
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        writeln!(output, "COMMIT;")
            .map_err(|error| format!("No se pudo escribir el cierre SQL: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("No se pudo sincronizar la exportación SQL: {error}"))?;
        Ok(())
    })
}

pub(crate) fn materialize_file_query_to_parquet(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    query: &str,
    destination: &Path,
) -> Result<(), String> {
    let connection = Connection::open_in_memory().map_err(|error| {
        format!("No se pudo iniciar DuckDB para la receta source-backed: {error}")
    })?;
    let resource_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el espacio temporal para la receta source-backed: {error}")
    })?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    register_file_view(&connection, "dataset", source_path, source_format, None)?;
    let destination = destination
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let statement = format!(
        "SET preserve_insertion_order = true; COPY ({query}) TO '{destination}' (FORMAT PARQUET)"
    );
    connection
        .execute_batch(&statement)
        .map_err(|error| format!("DuckDB no pudo publicar la receta source-backed: {error}"))
}

pub(crate) struct DuckDbFileSourcesQuery<'a> {
    pub(crate) current_path: &'a Path,
    pub(crate) current_format: DuckDbFileFormat,
    pub(crate) compared_path: &'a Path,
    pub(crate) compared_format: DuckDbFileFormat,
    pub(crate) dataset_view_query: &'a str,
    pub(crate) query: &'a str,
    pub(crate) destination: &'a Path,
    pub(crate) current_order_column: &'a str,
    pub(crate) compared_order_column: &'a str,
    pub(crate) max_rows: Option<usize>,
}

pub(crate) fn materialize_file_sources_query_to_parquet(
    request: DuckDbFileSourcesQuery<'_>,
) -> Result<usize, String> {
    let connection = Connection::open_in_memory().map_err(|error| {
        format!("No se pudo iniciar DuckDB para el resultado source-backed: {error}")
    })?;
    let resource_directory = tempfile::tempdir().map_err(|error| {
        format!("No se pudo preparar el espacio temporal para el resultado source-backed: {error}")
    })?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    register_file_view(
        &connection,
        "__columnia_current",
        request.current_path,
        request.current_format,
        Some(request.current_order_column),
    )?;
    register_file_view(
        &connection,
        "__columnia_compared",
        request.compared_path,
        request.compared_format,
        Some(request.compared_order_column),
    )?;
    connection
        .execute_batch(request.dataset_view_query)
        .map_err(|error| format!("DuckDB no pudo preparar el resultado source-backed: {error}"))?;

    let count_query = format!(
        "SELECT COUNT(*) FROM ({}) AS __columnia_join_count",
        request.query
    );
    let total_i64 = connection
        .query_row(&count_query, [], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("DuckDB no pudo contar el resultado source-backed: {error}"))?;
    let row_count = usize::try_from(total_i64)
        .map_err(|_| "DuckDB devolvió un conteo de resultado source-backed inválido.".to_owned())?;
    if let Some(limit) = request.max_rows {
        if row_count > limit {
            return Err(format!(
                "El resultado source-backed produciría {row_count} filas y supera el límite local de {limit}."
            ));
        }
    }

    let destination = request
        .destination
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let statement = format!(
        "SET preserve_insertion_order = true; COPY ({}) TO '{destination}' (FORMAT PARQUET)",
        request.query
    );
    connection
        .execute_batch(&statement)
        .map_err(|error| format!("DuckDB no pudo publicar el resultado source-backed: {error}"))?;
    Ok(row_count)
}

pub(crate) struct DuckDbFileSourcesScalarQuery<'a> {
    pub(crate) current_path: &'a Path,
    pub(crate) current_format: DuckDbFileFormat,
    pub(crate) compared_path: &'a Path,
    pub(crate) compared_format: DuckDbFileFormat,
    pub(crate) query: &'a str,
}

pub(crate) fn query_file_sources_scalar(
    request: DuckDbFileSourcesScalarQuery<'_>,
) -> Result<i64, String> {
    let connection = Connection::open_in_memory().map_err(|error| {
        format!("No se pudo iniciar DuckDB para validar la consolidación source-backed: {error}")
    })?;
    let resource_directory = tempfile::tempdir().map_err(|error| {
        format!(
            "No se pudo preparar el espacio temporal para validar la consolidación source-backed: {error}"
        )
    })?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    register_file_view(
        &connection,
        "__columnia_current",
        request.current_path,
        request.current_format,
        None,
    )?;
    register_file_view(
        &connection,
        "__columnia_compared",
        request.compared_path,
        request.compared_format,
        None,
    )?;
    connection
        .query_row(request.query, [], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("DuckDB no pudo validar la consolidación source-backed: {error}"))
}

pub(crate) fn query_file_scalar(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    query: &str,
) -> Result<i64, String> {
    query_file_scalar_with_cancel(source_path, source_format, query, || false)
}

fn query_file_scalar_with_cancel<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    query: &str,
    is_cancelled: C,
) -> Result<i64, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el espacio temporal para el conteo source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        connection
            .query_row(query, [], |row| row.get::<_, i64>(0))
            .map_err(|error| format!("DuckDB no pudo contar los cambios source-backed: {error}"))
    })
}

pub(crate) fn count_file_rows<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    is_cancelled: C,
) -> Result<usize, String>
where
    C: Fn() -> bool + Send + 'static,
{
    let count = query_file_scalar_with_cancel(
        source_path,
        source_format,
        "SELECT COUNT(*) FROM dataset",
        is_cancelled,
    )?;
    usize::try_from(count)
        .map_err(|_| "El conteo de filas source-backed excede la capacidad local.".to_owned())
}

pub(crate) fn count_file_nulls<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[String],
    is_cancelled: C,
) -> Result<Vec<usize>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!(
                "No se pudo preparar el espacio temporal para el diccionario source-backed: {error}"
            )
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        columns
            .iter()
            .map(|column| {
                let identifier = quote_identifier(column);
                let query = format!("SELECT COUNT(*) FROM dataset WHERE {identifier} IS NULL");
                let count: i64 =
                    connection
                        .query_row(&query, [], |row| row.get(0))
                        .map_err(|error| {
                            format!(
                                "DuckDB no pudo contar los nulos de la columna {column}: {error}"
                            )
                        })?;
                usize::try_from(count).map_err(|_| {
                    format!("El conteo de nulos de la columna {column} excede la capacidad local.")
                })
            })
            .collect()
    })
}

pub(crate) fn count_file_distinct_non_null<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[String],
    is_cancelled: C,
) -> Result<Vec<usize>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de valores source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        columns
            .iter()
            .map(|column| {
                let identifier = quote_identifier(column);
                let query = format!("SELECT COUNT(DISTINCT {identifier}) FROM dataset");
                let count: i64 = connection
                    .query_row(&query, [], |row| row.get(0))
                    .map_err(|error| {
                        format!(
                            "DuckDB no pudo contar los valores distintos de la columna {column}: {error}"
                        )
                    })?;
                usize::try_from(count).map_err(|_| {
                    format!(
                        "El conteo de valores distintos de la columna {column} excede la capacidad local."
                    )
                })
            })
            .collect()
    })
}

pub(crate) fn count_file_values_not_equal<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[String],
    excluded_value: &str,
    is_cancelled: C,
) -> Result<Vec<usize>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de valores source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let excluded_value = duckdb_sql_string_literal(excluded_value);
        columns
            .iter()
            .map(|column| {
                let identifier = quote_identifier(column);
                let query = format!(
                    "SELECT COUNT(*) FROM dataset WHERE {identifier} IS NOT NULL AND CAST({identifier} AS VARCHAR) <> {excluded_value}"
                );
                let count: i64 = connection
                    .query_row(&query, [], |row| row.get(0))
                    .map_err(|error| {
                        format!(
                            "DuckDB no pudo contar los valores protegibles de la columna {column}: {error}"
                        )
                    })?;
                usize::try_from(count).map_err(|_| {
                    format!(
                        "El conteo de valores protegibles de la columna {column} excede la capacidad local."
                    )
                })
            })
            .collect()
    })
}

pub(crate) fn count_file_expression_changes<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    expressions: &[(String, String)],
    is_cancelled: C,
) -> Result<(usize, Vec<usize>), String>
where
    C: Fn() -> bool + Send + 'static,
{
    if expressions.is_empty() {
        return Ok((0, Vec::new()));
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de cambios source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let predicates = expressions
            .iter()
            .map(|(column, expression)| {
                format!(
                    "{} IS DISTINCT FROM ({expression})",
                    quote_identifier(column)
                )
            })
            .collect::<Vec<_>>();
        let select = std::iter::once(format!(
            "COUNT(*) FILTER (WHERE {})",
            predicates.join(" OR ")
        ))
        .chain(
            predicates
                .iter()
                .map(|predicate| format!("COUNT(*) FILTER (WHERE {predicate})")),
        )
        .collect::<Vec<_>>()
        .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| format!("DuckDB no pudo preparar el conteo de cambios: {error}"))?;
        let counts = statement
            .query_row([], |row| {
                (0..=expressions.len())
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("DuckDB no pudo contar los cambios: {error}"))?;
        let counts = counts
            .into_iter()
            .map(|count| {
                usize::try_from(count)
                    .map_err(|_| "El conteo de cambios excede la capacidad local.".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (affected_row_count, changed_cell_counts) = counts
            .split_first()
            .ok_or_else(|| "DuckDB no devolvió el conteo de cambios.".to_owned())?;
        Ok((*affected_row_count, changed_cell_counts.to_vec()))
    })
}

pub(crate) fn count_file_predicate_matches<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    predicates: &[String],
    is_cancelled: C,
) -> Result<(usize, Vec<usize>), String>
where
    C: Fn() -> bool + Send + 'static,
{
    if predicates.is_empty() {
        return Ok((0, Vec::new()));
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de predicados source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let select = std::iter::once(format!(
            "COUNT(*) FILTER (WHERE {})",
            predicates.join(" OR ")
        ))
        .chain(
            predicates
                .iter()
                .map(|predicate| format!("COUNT(*) FILTER (WHERE {predicate})")),
        )
        .collect::<Vec<_>>()
        .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| format!("DuckDB no pudo preparar el conteo de predicados: {error}"))?;
        let counts = statement
            .query_row([], |row| {
                (0..=predicates.len())
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("DuckDB no pudo contar los predicados: {error}"))?;
        let counts = counts
            .into_iter()
            .map(|count| {
                usize::try_from(count)
                    .map_err(|_| "El conteo de predicados excede la capacidad local.".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (affected_row_count, matches) = counts
            .split_first()
            .ok_or_else(|| "DuckDB no devolvió el conteo de predicados.".to_owned())?;
        Ok((*affected_row_count, matches.to_vec()))
    })
}

pub(crate) struct FileNumericCandidateStats {
    pub(crate) non_null_count: usize,
    pub(crate) integer_count: usize,
    pub(crate) float_count: usize,
    pub(crate) leading_zero_count: usize,
    pub(crate) decimal_token_count: usize,
    pub(crate) precision_loss_count: usize,
}

pub(crate) fn count_file_numeric_candidate_stats<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[String],
    is_cancelled: C,
) -> Result<Vec<FileNumericCandidateStats>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario numérico source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let select = columns
            .iter()
            .flat_map(|column| {
                let identifier = quote_identifier(column);
                let trimmed = format!("TRIM(CAST({identifier} AS VARCHAR))");
                let integer = format!("TRY_CAST({trimmed} AS BIGINT)");
                let floating = format!("TRY_CAST({trimmed} AS DOUBLE)");
                let finite = format!("{floating} IS NOT NULL AND isfinite({floating})");
                let leading_zero = format!(
                    "regexp_matches({trimmed}, {})",
                    duckdb_sql_string_literal(r"^[+-]?0[0-9]")
                );
                let decimal_token = format!(
                    "strpos({trimmed}, '.') > 0 OR strpos(lower({trimmed}), 'e') > 0"
                );
                let precision_loss = format!(
                    "{finite} AND ABS({floating}) > 9007199254740992 AND TRUNC({floating}) = {floating}"
                );
                [
                    format!("COUNT(*) FILTER (WHERE {identifier} IS NOT NULL)"),
                    format!(
                        "COUNT(*) FILTER (WHERE {identifier} IS NOT NULL AND {integer} IS NOT NULL)"
                    ),
                    format!(
                        "COUNT(*) FILTER (WHERE {identifier} IS NOT NULL AND {finite})"
                    ),
                    format!(
                        "COUNT(*) FILTER (WHERE {identifier} IS NOT NULL AND {leading_zero})"
                    ),
                    format!(
                        "COUNT(*) FILTER (WHERE {identifier} IS NOT NULL AND ({decimal_token}))"
                    ),
                    format!(
                        "COUNT(*) FILTER (WHERE {identifier} IS NOT NULL AND {precision_loss})"
                    ),
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| format!("DuckDB no pudo preparar la inferencia numérica: {error}"))?;
        let counts = statement
            .query_row([], |row| {
                (0..columns.len() * 6)
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("DuckDB no pudo calcular la inferencia numérica: {error}"))?;
        counts
            .chunks_exact(6)
            .map(|counts| {
                let values = counts
                    .iter()
                    .map(|count| {
                        usize::try_from(*count)
                            .map_err(|_| "El conteo numérico excede la capacidad local.".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(FileNumericCandidateStats {
                    non_null_count: values[0],
                    integer_count: values[1],
                    float_count: values[2],
                    leading_zero_count: values[3],
                    decimal_token_count: values[4],
                    precision_loss_count: values[5],
                })
            })
            .collect()
    })
}

pub(crate) fn sample_file_column_values(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    column: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = Connection::open_in_memory().map_err(|error| {
        format!("No se pudo iniciar DuckDB para muestrear la columna source-backed: {error}")
    })?;
    let resource_directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el muestreo source-backed: {error}"))?;
    configure_duckdb_resources(&connection, resource_directory.path())?;
    register_file_view(&connection, "dataset", source_path, source_format, None)?;
    let identifier = quote_identifier(column);
    let query = format!(
        "SELECT CAST({identifier} AS VARCHAR) FROM dataset WHERE {identifier} IS NOT NULL LIMIT {limit}"
    );
    let mut statement = connection
        .prepare(&query)
        .map_err(|error| format!("DuckDB no pudo preparar el muestreo source-backed: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("DuckDB no pudo muestrear la columna source-backed: {error}"))?;
    let mut values = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("DuckDB no pudo leer la muestra source-backed: {error}"))?
    {
        values.push(
            row.get::<_, String>(0)
                .map_err(|error| format!("DuckDB no pudo leer un valor de la muestra: {error}"))?,
        );
    }
    Ok(values)
}

pub(crate) struct FileDateParseStats {
    pub(crate) non_null_count: usize,
    pub(crate) parsed_count: usize,
    pub(crate) in_range_count: usize,
}

pub(crate) fn count_file_date_parse_stats<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    parsed_expressions: &[(String, String)],
    is_cancelled: C,
) -> Result<Vec<FileDateParseStats>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if parsed_expressions.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de fechas source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let select = parsed_expressions
            .iter()
            .flat_map(|(column, parsed)| {
                let identifier = quote_identifier(column);
                [
                    format!("COUNT(*) FILTER (WHERE {identifier} IS NOT NULL)"),
                    format!("COUNT(*) FILTER (WHERE ({parsed}) IS NOT NULL)"),
                    format!(
                        "COUNT(*) FILTER (WHERE ({parsed}) IS NOT NULL AND EXTRACT(YEAR FROM ({parsed})) BETWEEN 1900 AND 2100)"
                    ),
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| format!("DuckDB no pudo preparar la inferencia de fechas: {error}"))?;
        let counts = statement
            .query_row([], |row| {
                (0..parsed_expressions.len() * 3)
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("DuckDB no pudo calcular la inferencia de fechas: {error}"))?;
        counts
            .chunks_exact(3)
            .map(|counts| {
                let values = counts
                    .iter()
                    .map(|count| {
                        usize::try_from(*count).map_err(|_| {
                            "El conteo de fechas excede la capacidad local.".to_owned()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(FileDateParseStats {
                    non_null_count: values[0],
                    parsed_count: values[1],
                    in_range_count: values[2],
                })
            })
            .collect()
    })
}

pub(crate) struct FileImputationStats {
    pub(crate) null_count: usize,
    pub(crate) replacement: Option<String>,
}

pub(crate) fn count_file_imputation_stats<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[(String, bool)],
    is_cancelled: C,
) -> Result<Vec<FileImputationStats>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de imputación source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let select = columns
            .iter()
            .flat_map(|(column, numeric)| {
                let identifier = quote_identifier(column);
                let null_count = format!("COUNT(*) FILTER (WHERE {identifier} IS NULL)");
                let replacement = if *numeric {
                    format!(
                        "(SELECT CAST(quantile_disc(CAST({identifier} AS DOUBLE), 0.5) AS VARCHAR) FROM dataset WHERE {identifier} IS NOT NULL AND isfinite(CAST({identifier} AS DOUBLE)))"
                    )
                } else {
                    format!(
                        "(SELECT value FROM (SELECT CAST({identifier} AS VARCHAR) AS value, COUNT(*) AS occurrences, MIN(__source_row) AS first_row FROM (SELECT {identifier}, ROW_NUMBER() OVER () AS __source_row FROM dataset) numbered WHERE {identifier} IS NOT NULL AND TRIM(CAST({identifier} AS VARCHAR)) <> '' GROUP BY {identifier} HAVING COUNT(*) >= 2 ORDER BY occurrences DESC, first_row LIMIT 1) mode)"
                    )
                };
                [null_count, replacement]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection.prepare(&query).map_err(|error| {
            format!("DuckDB no pudo preparar la imputación source-backed: {error}")
        })?;
        let values = statement
            .query_row([], |row| {
                (0..columns.len())
                    .map(|index| {
                        Ok((
                            row.get::<_, i64>(index * 2)?,
                            row.get::<_, Option<String>>(index * 2 + 1)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                format!("DuckDB no pudo calcular la imputación source-backed: {error}")
            })?;
        values
            .into_iter()
            .map(|(null_count, replacement)| {
                Ok(FileImputationStats {
                    null_count: usize::try_from(null_count)
                        .map_err(|_| "El conteo de nulos excede la capacidad local.".to_owned())?,
                    replacement,
                })
            })
            .collect()
    })
}

pub(crate) struct FileOutlierStats {
    pub(crate) valid_count: usize,
    pub(crate) non_finite_count: usize,
    pub(crate) precision_loss_count: usize,
    pub(crate) q1: Option<f64>,
    pub(crate) q3: Option<f64>,
    pub(crate) median: Option<f64>,
}

pub(crate) fn count_file_outlier_stats<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    columns: &[(String, bool)],
    is_cancelled: C,
) -> Result<Vec<FileOutlierStats>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario IQR source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let select = columns
            .iter()
            .flat_map(|(column, integer)| {
                let identifier = quote_identifier(column);
                let value = format!("CAST({identifier} AS DOUBLE)");
                let non_null = format!("{identifier} IS NOT NULL");
                let finite = format!("{non_null} AND isfinite({value})");
                let precision_loss = if *integer {
                    format!("{non_null} AND abs({value}) > 9007199254740992")
                } else {
                    "FALSE".to_owned()
                };
                let valid = if *integer {
                    format!("{finite} AND NOT ({precision_loss})")
                } else {
                    finite.clone()
                };
                let median = if *integer {
                    format!("quantile_disc(CASE WHEN {valid} THEN {value} ELSE NULL END, 0.5)")
                } else {
                    format!("quantile_cont(CASE WHEN {valid} THEN {value} ELSE NULL END, 0.5)")
                };
                [
                    format!("COUNT(*) FILTER (WHERE {valid})"),
                    format!("COUNT(*) FILTER (WHERE {non_null} AND NOT isfinite({value}))"),
                    format!("COUNT(*) FILTER (WHERE {precision_loss})"),
                    format!("quantile_cont(CASE WHEN {valid} THEN {value} ELSE NULL END, 0.25)"),
                    format!("quantile_cont(CASE WHEN {valid} THEN {value} ELSE NULL END, 0.75)"),
                    median,
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection.prepare(&query).map_err(|error| {
            format!("DuckDB no pudo preparar las estadísticas IQR source-backed: {error}")
        })?;
        let values = statement
            .query_row([], |row| {
                (0..columns.len())
                    .map(|index| {
                        Ok((
                            row.get::<_, i64>(index * 6)?,
                            row.get::<_, i64>(index * 6 + 1)?,
                            row.get::<_, i64>(index * 6 + 2)?,
                            row.get::<_, Option<f64>>(index * 6 + 3)?,
                            row.get::<_, Option<f64>>(index * 6 + 4)?,
                            row.get::<_, Option<f64>>(index * 6 + 5)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                format!("DuckDB no pudo calcular las estadísticas IQR source-backed: {error}")
            })?;
        values
            .into_iter()
            .map(
                |(valid_count, non_finite_count, precision_loss_count, q1, q3, median)| {
                    Ok(FileOutlierStats {
                        valid_count: usize::try_from(valid_count)
                            .map_err(|_| "El conteo IQR excede la capacidad local.".to_owned())?,
                        non_finite_count: usize::try_from(non_finite_count).map_err(|_| {
                            "El conteo de valores no finitos excede la capacidad local.".to_owned()
                        })?,
                        precision_loss_count: usize::try_from(precision_loss_count).map_err(
                            |_| {
                                "El conteo de pérdida de precisión excede la capacidad local."
                                    .to_owned()
                            },
                        )?,
                        q1,
                        q3,
                        median,
                    })
                },
            )
            .collect()
    })
}

pub(crate) fn count_file_boolean_candidates<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    normalized_expressions: &[(String, String)],
    is_cancelled: C,
) -> Result<Vec<(usize, usize)>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if normalized_expressions.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de booleanos source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let recognized = ["true", "yes", "si", "false", "no"]
            .into_iter()
            .map(duckdb_sql_string_literal)
            .collect::<Vec<_>>()
            .join(", ");
        let select = normalized_expressions
            .iter()
            .flat_map(|(_, expression)| {
                [
                    format!(
                        "COUNT(*) FILTER (WHERE {expression} IS NOT NULL AND {expression} <> '')"
                    ),
                    format!("COUNT(*) FILTER (WHERE {expression} IN ({recognized}))"),
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection.prepare(&query).map_err(|error| {
            format!("DuckDB no pudo preparar la detección de booleanos: {error}")
        })?;
        let counts = statement
            .query_row([], |row| {
                (0..normalized_expressions.len() * 2)
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("DuckDB no pudo contar candidatos booleanos: {error}"))?;
        counts
            .chunks_exact(2)
            .map(|counts| {
                let non_empty = usize::try_from(counts[0]).map_err(|_| {
                    "El conteo de valores booleanos excede la capacidad local.".to_owned()
                })?;
                let recognized = usize::try_from(counts[1]).map_err(|_| {
                    "El conteo de booleanos reconocidos excede la capacidad local.".to_owned()
                })?;
                Ok((non_empty, recognized))
            })
            .collect()
    })
}

pub(crate) struct FileTextTypeExpressions {
    pub(crate) column: String,
    pub(crate) normalized: String,
    pub(crate) integer: String,
    pub(crate) decimal: String,
    pub(crate) date: String,
}

pub(crate) struct FileTextTypeStats {
    pub(crate) non_empty_count: usize,
    pub(crate) boolean_count: usize,
    pub(crate) integer_count: usize,
    pub(crate) decimal_count: usize,
    pub(crate) date_count: usize,
}

pub(crate) fn count_file_text_type_stats<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    expressions: &[FileTextTypeExpressions],
    is_cancelled: C,
) -> Result<Vec<FileTextTypeStats>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if expressions.is_empty() {
        return Ok(Vec::new());
    }
    execute_duckdb_operation(is_cancelled, |connection| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!("No se pudo preparar el diccionario de tipos de texto source-backed: {error}")
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let recognized = ["true", "yes", "si", "false", "no"]
            .into_iter()
            .map(duckdb_sql_string_literal)
            .collect::<Vec<_>>()
            .join(", ");
        let select = expressions
            .iter()
            .flat_map(|expression| {
                [
                    format!(
                        "COUNT(*) FILTER (WHERE {} IS NOT NULL AND {} <> '')",
                        expression.normalized, expression.normalized
                    ),
                    format!(
                        "COUNT(*) FILTER (WHERE {} IN ({recognized}))",
                        expression.normalized
                    ),
                    format!("COUNT(*) FILTER (WHERE {})", expression.integer),
                    format!("COUNT(*) FILTER (WHERE {})", expression.decimal),
                    format!("COUNT(*) FILTER (WHERE {} IS NOT NULL)", expression.date),
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!("SELECT {select} FROM dataset");
        let mut statement = connection.prepare(&query).map_err(|error| {
            format!("DuckDB no pudo preparar la inferencia de tipos de texto: {error}")
        })?;
        let counts = statement
            .query_row([], |row| {
                (0..expressions.len() * 5)
                    .map(|index| row.get::<_, i64>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                format!("DuckDB no pudo calcular la inferencia de tipos de texto: {error}")
            })?;
        counts
            .chunks_exact(5)
            .map(|counts| {
                let values = counts
                    .iter()
                    .map(|count| {
                        usize::try_from(*count).map_err(|_| {
                            "El conteo de tipos de texto excede la capacidad local.".to_owned()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(FileTextTypeStats {
                    non_empty_count: values[0],
                    boolean_count: values[1],
                    integer_count: values[2],
                    decimal_count: values[3],
                    date_count: values[4],
                })
            })
            .collect()
    })
}

pub(crate) fn stream_file_rows<C, F>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    mut on_row: F,
    is_cancelled: C,
) -> Result<Vec<DatasetColumn>, String>
where
    C: Fn() -> bool + Send + 'static,
    F: FnMut(&[Option<String>]) -> Result<(), String>,
{
    execute_duckdb_operation_with_cancel_state(is_cancelled, |connection, cancelled| {
        let resource_directory = tempfile::tempdir().map_err(|error| {
            format!(
                "No se pudo preparar el espacio temporal para transmitir el dataset source-backed: {error}"
            )
        })?;
        configure_duckdb_resources(connection, resource_directory.path())?;
        register_file_view(connection, "dataset", source_path, source_format, None)?;
        let mut statement = connection
            .prepare("SELECT * FROM dataset")
            .map_err(|error| {
                format!("DuckDB no pudo preparar la transmisión source-backed: {error}")
            })?;
        let mut rows = statement.query([]).map_err(|error| {
            format!("DuckDB no pudo iniciar la transmisión source-backed: {error}")
        })?;
        let result_statement = rows
            .as_ref()
            .ok_or_else(|| "DuckDB no devolvió metadatos del dataset source-backed.".to_owned())?;
        let columns = result_statement
            .column_names()
            .into_iter()
            .enumerate()
            .map(|(index, name)| DatasetColumn {
                name,
                data_type: format!("{:?}", result_statement.column_type(index)),
            })
            .collect::<Vec<_>>();
        while let Some(row) = rows
            .next()
            .map_err(|error| format!("DuckDB no pudo transmitir una fila source-backed: {error}"))?
        {
            if cancelled.load(Ordering::Acquire) {
                return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
            }
            let values = (0..columns.len())
                .map(|index| {
                    row.get_ref(index)
                        .map_err(|error| {
                            format!("DuckDB no pudo leer una celda source-backed: {error}")
                        })
                        .and_then(value_to_preview)
                })
                .collect::<Result<Vec<_>, _>>()?;
            on_row(&values)?;
        }
        Ok(columns)
    })
}

fn execute_duckdb_operation<C, F, T>(is_cancelled: C, operation: F) -> Result<T, String>
where
    C: Fn() -> bool + Send + 'static,
    F: FnOnce(&Connection) -> Result<T, String>,
{
    execute_duckdb_operation_with_cancel_state(is_cancelled, |connection, _| operation(connection))
}

fn execute_duckdb_operation_with_cancel_state<C, F, T>(
    is_cancelled: C,
    operation: F,
) -> Result<T, String>
where
    C: Fn() -> bool + Send + 'static,
    F: FnOnce(&Connection, &AtomicBool) -> Result<T, String>,
{
    if is_cancelled() {
        return Err(OPERATION_CANCELLED_MESSAGE.to_owned());
    }
    let connection = Connection::open_in_memory()
        .map_err(|error| format!("No se pudo iniciar DuckDB para la operación: {error}"))?;
    let interrupt = connection.interrupt_handle();
    let cancelled = Arc::new(AtomicBool::new(false));
    let stop_watcher = Arc::new(AtomicBool::new(false));
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

    let result = operation(&connection, &cancelled);
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

fn execute_duckdb_query_with_source<C>(
    current: DatasetSource<'_>,
    compared: Option<DatasetSource<'_>>,
    spec: &DuckDbQuerySpec,
    is_cancelled: C,
) -> Result<DatasetQueryResult, String>
where
    C: Fn() -> bool + Send + 'static,
{
    execute_duckdb_operation(is_cancelled, |connection| {
        execute_query_with_connection(connection, current, compared, spec)
    })
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

fn csv_export_scan_expression(path: &Path, format: DuckDbFileFormat) -> String {
    let DuckDbFileFormat::Delimited { delimiter } = format else {
        return file_scan_expression(path, format);
    };
    let escaped_path = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let delimiter = char::from(delimiter);
    let escaped_delimiter = delimiter.to_string().replace('\'', "''");
    format!("read_csv_auto('{escaped_path}', header = true, delim = '{escaped_delimiter}')")
}

fn describe_source_columns(
    connection: &Connection,
    source: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut statement = connection
        .prepare(&format!("DESCRIBE SELECT * FROM {source}"))
        .map_err(|error| format!("DuckDB no pudo inspeccionar la fuente exportable: {error}"))?;
    let columns = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("DuckDB no pudo inspeccionar las columnas exportables: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("DuckDB no pudo leer las columnas exportables: {error}"))?;
    if columns.is_empty() {
        return Err("La fuente no contiene columnas exportables.".to_owned());
    }
    Ok(columns)
}

fn csv_export_projection(connection: &Connection, source: &str) -> Result<String, String> {
    let columns = describe_source_columns(connection, source)?;

    Ok(columns
        .into_iter()
        .map(|(name, data_type)| {
            let identifier = quote_identifier(&name);
            if data_type.to_ascii_uppercase().starts_with("VARCHAR") {
                format!(
                    "CASE WHEN left({identifier}, 1) IN ('=', '+', '-', '@', chr(9), chr(10), chr(13)) THEN chr(39) || {identifier} ELSE {identifier} END AS {identifier}"
                )
            } else {
                identifier
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
}

fn is_sql_text_projection(data_type: &str) -> bool {
    let data_type = data_type.to_ascii_uppercase();
    data_type.starts_with("DATE")
        || data_type.starts_with("TIME")
        || data_type.starts_with("TIMESTAMP")
        || data_type.contains("STRUCT")
        || data_type.contains("LIST")
        || data_type.contains("MAP")
        || data_type.contains("UNION")
        || data_type.starts_with("ENUM")
        || data_type.contains("[]")
}

fn duckdb_sql_type(data_type: &str) -> &'static str {
    let data_type = data_type.to_ascii_uppercase();
    if data_type.starts_with("BOOL") {
        "BOOLEAN"
    } else if data_type.contains("INT") {
        "BIGINT"
    } else if data_type.contains("FLOAT")
        || data_type.contains("DOUBLE")
        || data_type.contains("DECIMAL")
        || data_type == "REAL"
    {
        "DOUBLE"
    } else if data_type.starts_with("DATE") {
        "DATE"
    } else if data_type.starts_with("TIMESTAMP") {
        "TIMESTAMP"
    } else if data_type.starts_with("TIME") {
        "TIME"
    } else {
        "TEXT"
    }
}

fn duckdb_sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn duckdb_sql_value(value: ValueRef<'_>) -> Result<String, String> {
    match value {
        ValueRef::Null => Ok("NULL".to_owned()),
        ValueRef::Boolean(value) => Ok(if value { "TRUE" } else { "FALSE" }.to_owned()),
        ValueRef::TinyInt(value) => Ok(value.to_string()),
        ValueRef::SmallInt(value) => Ok(value.to_string()),
        ValueRef::Int(value) => Ok(value.to_string()),
        ValueRef::BigInt(value) => Ok(value.to_string()),
        ValueRef::HugeInt(value) => Ok(value.to_string()),
        ValueRef::UTinyInt(value) => Ok(value.to_string()),
        ValueRef::USmallInt(value) => Ok(value.to_string()),
        ValueRef::UInt(value) => Ok(value.to_string()),
        ValueRef::UBigInt(value) => Ok(value.to_string()),
        ValueRef::UHugeInt(value) => Ok(value.to_string()),
        ValueRef::Float(value) if value.is_finite() => Ok(value.to_string()),
        ValueRef::Double(value) if value.is_finite() => Ok(value.to_string()),
        ValueRef::Decimal(value) => Ok(value.to_string()),
        ValueRef::Text(value) => Ok(duckdb_sql_string_literal(&String::from_utf8_lossy(value))),
        ValueRef::Blob(value) | ValueRef::Geometry(value) => {
            let mut hex = String::with_capacity(value.len().saturating_mul(2));
            for byte in value {
                hex.push_str(&format!("{byte:02x}"));
            }
            Ok(format!("X'{hex}'"))
        }
        ValueRef::Float(_) | ValueRef::Double(_) => {
            Err("SQL no puede representar valores numéricos no finitos.".to_owned())
        }
        ValueRef::Date32(value) => Ok(duckdb_sql_string_literal(&value.to_string())),
        ValueRef::Time64(unit, value) | ValueRef::Timestamp(unit, value) => {
            Ok(duckdb_sql_string_literal(&format_timestamp(unit, value)))
        }
        ValueRef::Interval {
            months,
            days,
            nanos,
        } => Ok(duckdb_sql_string_literal(&format!(
            "{months} months {days} days {nanos} nanos"
        ))),
        ValueRef::Enum(..)
        | ValueRef::List(..)
        | ValueRef::Struct(..)
        | ValueRef::Array(..)
        | ValueRef::Map(..)
        | ValueRef::Union(..) => {
            Err("La exportación SQL no admite columnas anidadas en la fuente.".to_owned())
        }
        _ => Err("La exportación SQL encontró un tipo no compatible.".to_owned()),
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
    fn counts_a_delimited_source_without_creating_a_snapshot() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let path = directory.path().join("current.csv");
        fs::write(&path, "city,value\nSanto Domingo,10\nSantiago,20\n")
            .expect("se debe escribir el CSV");

        let count = count_file_rows(
            &path,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            || false,
        )
        .expect("DuckDB debe contar el CSV original");

        assert_eq!(count, 2);
        assert!(path.is_file());
        assert!(!directory.path().join("dataset.parquet").exists());
    }

    #[test]
    fn count_cancels_before_opening_the_source() {
        let missing_path = std::path::Path::new("missing-source.csv");

        let error = count_file_rows(
            missing_path,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            || true,
        )
        .expect_err("el conteo debe respetar la cancelación inicial");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
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
    fn exports_a_delimited_source_to_csv_without_creating_a_snapshot() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let source = directory.path().join("current.tsv");
        let destination = directory.path().join("exported.csv");
        fs::write(&source, "city\tvalue\nSanto Domingo\t10\n=1+1\t-20\n")
            .expect("se debe escribir la fuente delimitada");

        export_file_to_csv_with_cancel(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b'\t' },
            &destination,
            || false,
        )
        .expect("DuckDB debe exportar la fuente delimitada a CSV");

        let lines = fs::read_to_string(&destination)
            .expect("la salida CSV debe poder leerse")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                "city,value".to_owned(),
                "Santo Domingo,10".to_owned(),
                "'=1+1,-20".to_owned()
            ]
        );
        assert!(source.is_file());
        assert!(destination.is_file());
        assert!(!directory.path().join("dataset.parquet").exists());
    }

    #[test]
    fn csv_export_honors_initial_cancellation() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let source = directory.path().join("current.csv");
        let destination = directory.path().join("exported.csv");
        fs::write(&source, "city,value\nSanto Domingo,10\n")
            .expect("se debe escribir la fuente CSV");

        let error = export_file_to_csv_with_cancel(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            &destination,
            || true,
        )
        .expect_err("una exportación cancelada no debe iniciar DuckDB");

        assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
        assert!(!destination.exists());
    }

    #[test]
    fn exports_a_delimited_source_to_sql_without_creating_a_snapshot() {
        let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
        let source = directory.path().join("current.csv");
        let destination = directory.path().join("exported.sql");
        fs::write(&source, "name,amount\nO'Brien,10\n=1+1,-20\n")
            .expect("se debe escribir la fuente delimitada");

        export_file_to_sql_with_cancel(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            &destination,
            || false,
        )
        .expect("DuckDB debe exportar la fuente delimitada a SQL");

        let script = fs::read_to_string(&destination).expect("la salida SQL debe poder leerse");
        assert!(script.contains("CREATE TABLE \"dataset\""));
        assert!(script.contains("\"name\" TEXT"));
        assert!(script.contains("\"amount\" BIGINT"));
        assert!(script.contains("'O''Brien'"));
        assert!(script.contains("'=1+1'"));
        assert!(script.contains("-20"));
        assert!(script.ends_with("COMMIT;\n"));
        assert!(source.is_file());
        assert!(destination.is_file());
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
