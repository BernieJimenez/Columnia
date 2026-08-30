use std::{
    fs::File,
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

pub(crate) struct DuckDbQuerySpec {
    pub(crate) bounded_query: String,
    pub(crate) count_query: String,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
    pub(crate) dataset_view_query: Option<String>,
    pub(crate) current_order_column: Option<String>,
    pub(crate) compared_order_column: Option<String>,
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
    current: &DataFrame,
    compared: Option<&DataFrame>,
    spec: &DuckDbQuerySpec,
) -> Result<DatasetQueryResult, String> {
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el espacio temporal para DuckDB: {error}"))?;
    let current_path = directory.path().join("dataset.parquet");
    write_frame_snapshot(
        current,
        &current_path,
        "activo",
        spec.current_order_column.as_deref(),
    )?;
    if let Some(dataset_view_query) = &spec.dataset_view_query {
        let compared = compared.ok_or_else(|| {
            "La consulta DuckDB requiere un dataset comparado para construir el JOIN.".to_owned()
        })?;
        let compared_path = directory.path().join("compared.parquet");
        write_frame_snapshot(
            compared,
            &compared_path,
            "comparado",
            spec.compared_order_column.as_deref(),
        )?;
        register_parquet_view(connection, "__columnia_current", &current_path)?;
        register_parquet_view(connection, "__columnia_compared", &compared_path)?;
        connection
            .execute_batch(dataset_view_query)
            .map_err(|error| format!("DuckDB no pudo preparar la vista del JOIN: {error}"))?;
    } else {
        register_parquet_view(connection, "dataset", &current_path)?;
        if let Some(compared) = compared {
            let compared_path = directory.path().join("compared.parquet");
            write_frame_snapshot(compared, &compared_path, "comparado", None)?;
            register_parquet_view(connection, "compared", &compared_path)?;
        }
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
