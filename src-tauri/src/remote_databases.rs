use std::{fmt::Debug, path::Path};

use odbc_api::{
    parameter::InputParameter, Bit, ColumnsRow, ConnectionOptions, Environment, IntoParameter,
    Nullable, TablesRow,
};
use polars::prelude::{AnyValue, DataFrame};
use serde::{Deserialize, Serialize};

use crate::duckdb_query::DuckDbFileFormat;

const MAX_CONNECTION_STRING_CHARS: usize = 16 * 1024;
const MAX_IDENTIFIER_CHARS: usize = 128;
const PREFLIGHT_CANCEL_CHECK_ROWS: usize = 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseKind {
    Postgresql,
    Mysql,
    SqlServer,
}

impl DatabaseKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Postgresql => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::SqlServer => "SQL Server",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseTablePolicy {
    Append,
    CreateOnly,
    Replace,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseTarget {
    pub kind: DatabaseKind,
    /// The complete ODBC connection string. It is intentionally not persisted.
    pub connection_string: String,
    pub schema: String,
    pub table: String,
    pub table_policy: DatabaseTablePolicy,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionResult {
    pub kind: DatabaseKind,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteInputColumn {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) null_count: usize,
    pub(crate) maximum_length: Option<usize>,
    pub(crate) contains_nul: bool,
    pub(crate) unrepresentable_integer_count: usize,
    pub(crate) unrepresentable_decimal_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteExportPreflight {
    pub kind: DatabaseKind,
    pub schema: String,
    pub table: String,
    pub table_policy: DatabaseTablePolicy,
    pub table_exists: bool,
    pub ready: bool,
    pub issues: Vec<RemotePreflightIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemotePreflightIssue {
    pub severity: &'static str,
    pub category: &'static str,
    pub column: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExistingColumn {
    name: String,
    data_type: String,
    maximum_length: Option<usize>,
    nullable: bool,
    has_default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeFamily {
    Text,
    Boolean,
    Integer,
    Decimal,
    Temporal,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RemoteExportResult {
    pub(crate) rows_written: usize,
    pub(crate) table_name: String,
    pub(crate) format: &'static str,
}

#[tauri::command]
pub fn test_database_connection(
    target: DatabaseTarget,
) -> Result<DatabaseConnectionResult, String> {
    validate_database_target(&target)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, &target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, &target))?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, &target))?;
    Ok(DatabaseConnectionResult {
        kind: target.kind,
        message: format!("Conexión ODBC verificada para {}.", target.kind.label()),
    })
}

pub(crate) fn preflight_export_with_cancel<C>(
    target: &DatabaseTarget,
    input_columns: &[RemoteInputColumn],
    is_cancelled: &C,
) -> Result<RemoteExportPreflight, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled)?;
    validate_database_target(target)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))?;
    ensure_not_cancelled(is_cancelled)?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    ensure_not_cancelled(is_cancelled)?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    ensure_not_cancelled(is_cancelled)?;
    Ok(assess_remote_export(
        target,
        input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    ))
}

pub(crate) fn preflight_source_backed<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    row_count: usize,
    target: &DatabaseTarget,
    is_cancelled: C,
) -> Result<RemoteExportPreflight, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(&is_cancelled)?;
    let input_columns = input_shape_from_source(
        source_path,
        source_format,
        schema,
        row_count,
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(&is_cancelled)?;
    preflight_export_with_cancel(target, &input_columns, &is_cancelled)
}

#[cfg(test)]
pub(crate) fn input_shape_from_frame(frame: &DataFrame) -> Result<Vec<RemoteInputColumn>, String> {
    input_shape_from_frame_with_cancel(frame, &|| false)
}

pub(crate) fn input_shape_from_frame_with_cancel<C>(
    frame: &DataFrame,
    is_cancelled: &C,
) -> Result<Vec<RemoteInputColumn>, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled)?;
    let mut input_columns = Vec::with_capacity(frame.width());
    for column in frame.columns() {
        ensure_not_cancelled(is_cancelled)?;
        let data_type = column.dtype().to_string();
        let is_text = type_family(&data_type) == TypeFamily::Text;
        let normalized_dtype = data_type.to_ascii_lowercase();
        let is_integer = is_integer_dtype(&normalized_dtype);
        let is_decimal = is_decimal_dtype(&normalized_dtype);
        let mut null_count = 0usize;
        let mut maximum_length = 0usize;
        let mut contains_nul = false;
        let mut unrepresentable_integer_count = 0usize;
        let mut unrepresentable_decimal_count = 0usize;
        if is_text || is_integer || is_decimal {
            for row_index in 0..column.len() {
                if row_index.is_multiple_of(PREFLIGHT_CANCEL_CHECK_ROWS) {
                    ensure_not_cancelled(is_cancelled)?;
                }
                let value = column.get(row_index).map_err(|error| {
                    format!(
                        "No se pudo revisar la columna '{}' para la entrega: {error}",
                        column.name()
                    )
                })?;
                let text = match value {
                    AnyValue::Null => {
                        null_count += 1;
                        continue;
                    }
                    AnyValue::String(value) => value.to_owned(),
                    AnyValue::StringOwned(value) => value.as_str().to_owned(),
                    value => value.to_string(),
                };
                if is_text {
                    maximum_length = maximum_length.max(text.chars().count());
                    contains_nul |= text.contains('\0');
                }
                if is_integer && text.parse::<i64>().is_err() {
                    unrepresentable_integer_count = unrepresentable_integer_count.saturating_add(1);
                }
                if is_decimal && !matches!(text.parse::<f64>(), Ok(value) if value.is_finite()) {
                    unrepresentable_decimal_count = unrepresentable_decimal_count.saturating_add(1);
                }
            }
        } else {
            null_count = column.null_count() as usize;
        }
        ensure_not_cancelled(is_cancelled)?;
        input_columns.push(RemoteInputColumn {
            name: column.name().to_string(),
            data_type,
            null_count,
            maximum_length: is_text.then_some(maximum_length),
            contains_nul,
            unrepresentable_integer_count,
            unrepresentable_decimal_count,
        });
    }
    ensure_not_cancelled(is_cancelled)?;
    Ok(input_columns)
}

fn input_shape_from_source<C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    expected_row_count: usize,
    is_cancelled: C,
) -> Result<Vec<RemoteInputColumn>, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(&is_cancelled)?;
    let mut columns = schema
        .columns()
        .iter()
        .map(|column| RemoteInputColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
            null_count: 0,
            maximum_length: (type_family(&column.dtype().to_string()) == TypeFamily::Text)
                .then_some(0),
            contains_nul: false,
            unrepresentable_integer_count: 0,
            unrepresentable_decimal_count: 0,
        })
        .collect::<Vec<_>>();
    let integer_columns = columns
        .iter()
        .map(|column| is_integer_dtype(&column.data_type.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let decimal_columns = columns
        .iter()
        .map(|column| is_decimal_dtype(&column.data_type.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let mut row_count = 0usize;
    let streamed_columns = crate::duckdb_query::stream_file_rows(
        source_path,
        source_format,
        |values| {
            if values.len() != columns.len() {
                return Err("El ancho de la fuente cambió durante el preflight remoto.".to_owned());
            }
            for (index, (column, value)) in columns.iter_mut().zip(values.iter()).enumerate() {
                match value {
                    None => column.null_count = column.null_count.saturating_add(1),
                    Some(value) => {
                        if let Some(maximum_length) = column.maximum_length.as_mut() {
                            *maximum_length = (*maximum_length).max(value.chars().count());
                            column.contains_nul |= value.contains('\0');
                        }
                        if integer_columns[index] && value.parse::<i64>().is_err() {
                            column.unrepresentable_integer_count =
                                column.unrepresentable_integer_count.saturating_add(1);
                        }
                        if decimal_columns[index]
                            && !matches!(value.parse::<f64>(), Ok(number) if number.is_finite())
                        {
                            column.unrepresentable_decimal_count =
                                column.unrepresentable_decimal_count.saturating_add(1);
                        }
                    }
                }
            }
            row_count = row_count.saturating_add(1);
            Ok(())
        },
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(&is_cancelled)?;
    if row_count != expected_row_count {
        return Err(
            "El conteo de filas cambió durante el preflight remoto; vuelve a revisar el dataset."
                .to_owned(),
        );
    }
    let expected_names = columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    let actual_names = streamed_columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    if actual_names != expected_names {
        return Err("El esquema source-backed cambió durante el preflight remoto.".to_owned());
    }
    Ok(columns)
}

fn inspect_destination(
    connection: &odbc_api::Connection<'_>,
    target: &DatabaseTarget,
) -> Result<(bool, bool, Vec<ExistingColumn>), String> {
    let table_rows = connection
        .tables("", &target.schema, &target.table, "")
        .map_err(|error| {
            format_driver_error("No se pudo revisar la política del destino", error, target)
        })?
        .collect::<Result<Vec<TablesRow>, _>>()
        .map_err(|error| {
            format_driver_error("No se pudo leer la política del destino", error, target)
        })?;
    let matching_objects = table_rows
        .iter()
        .filter(|row| {
            let table_name = row.table.as_str().ok().flatten().unwrap_or_default();
            let schema_name = row.schema.as_str().ok().flatten().unwrap_or_default();
            table_name.eq_ignore_ascii_case(&target.table)
                && (target.schema.trim().is_empty()
                    || schema_name.eq_ignore_ascii_case(&target.schema))
        })
        .collect::<Vec<_>>();
    let table_exists = !matching_objects.is_empty();
    let is_base_table = matching_objects.iter().any(|row| {
        row.table_type
            .as_str()
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("table")
    });
    if !table_exists || !is_base_table {
        return Ok((table_exists, is_base_table, Vec::new()));
    }

    let columns = connection
        .columns("", &target.schema, &target.table, "")
        .map_err(|error| {
            format_driver_error("No se pudo revisar el esquema remoto", error, target)
        })?
        .collect::<Result<Vec<ColumnsRow>, _>>()
        .map_err(|error| format_driver_error("No se pudo leer el esquema remoto", error, target))?
        .into_iter()
        .map(|column| {
            let maximum_length = column
                .column_size
                .into_opt()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0);
            let nullable = column
                .is_nullable
                .as_str()
                .ok()
                .flatten()
                .is_none_or(|value| !value.eq_ignore_ascii_case("NO"));
            let has_default = column
                .column_default
                .as_str()
                .ok()
                .flatten()
                .is_some_and(|value| !value.trim().is_empty());
            ExistingColumn {
                name: column
                    .column_name
                    .as_str()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_owned(),
                data_type: column
                    .type_name
                    .as_str()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_owned(),
                maximum_length,
                nullable,
                has_default,
            }
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(
            "El controlador no devolvió el esquema de la tabla. La entrega se detuvo antes de escribir."
                .to_owned(),
        );
    }
    Ok((table_exists, is_base_table, columns))
}

fn assess_remote_export(
    target: &DatabaseTarget,
    input_columns: &[RemoteInputColumn],
    table_exists: bool,
    is_base_table: bool,
    existing_columns: &[ExistingColumn],
) -> RemoteExportPreflight {
    let mut issues = Vec::new();
    if input_columns.is_empty() {
        push_issue(
            &mut issues,
            "blocking",
            "type",
            None,
            "El dataset no contiene columnas para entregar.".to_owned(),
        );
    }
    for column in input_columns {
        if column.unrepresentable_integer_count > 0 {
            push_issue(
                &mut issues,
                "blocking",
                "value",
                Some(column.name.clone()),
                format!(
                    "La columna '{}' tiene {} entero(s) que no caben en el parámetro Int64 usado por ODBC; no se escribirán como NULL.",
                    column.name, column.unrepresentable_integer_count
                ),
            );
        }
        if column.unrepresentable_decimal_count > 0 {
            push_issue(
                &mut issues,
                "blocking",
                "value",
                Some(column.name.clone()),
                format!(
                    "La columna '{}' tiene {} valor(es) numérico(s) no finito(s) o no representable(s) por ODBC; no se escribirán como NULL.",
                    column.name, column.unrepresentable_decimal_count
                ),
            );
        }
    }
    if table_exists && !is_base_table {
        push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "El destino existe, pero no se identificó como una tabla escribible.".to_owned(),
        );
    }
    match (target.table_policy, table_exists) {
        (DatabaseTablePolicy::Append, false) => push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "Añadir a tabla existente requiere que el destino ya exista.".to_owned(),
        ),
        (DatabaseTablePolicy::CreateOnly, true) => push_issue(
            &mut issues,
            "blocking",
            "policy",
            None,
            "Crear; fallar si existe está seleccionado y la tabla ya existe.".to_owned(),
        ),
        (DatabaseTablePolicy::Replace, true) => push_issue(
            &mut issues,
            "warning",
            "policy",
            None,
            "La tabla existente se eliminará y se volverá a crear. La política Reemplazar fue seleccionada explícitamente.".to_owned(),
        ),
        (DatabaseTablePolicy::CreateOnly | DatabaseTablePolicy::Replace, false) => push_issue(
            &mut issues,
            "info",
            "policy",
            None,
            "El destino no existe; se creará una tabla nueva.".to_owned(),
        ),
        (DatabaseTablePolicy::Append, true) => {}
    }

    if table_exists && is_base_table && target.table_policy == DatabaseTablePolicy::Append {
        for source in input_columns {
            let destination = existing_columns
                .iter()
                .find(|column| column.name == source.name);
            let Some(destination) = destination else {
                push_issue(
                    &mut issues,
                    "blocking",
                    "type",
                    Some(source.name.clone()),
                    format!(
                        "La tabla remota no contiene la columna '{}' con ese nombre exacto.",
                        source.name
                    ),
                );
                continue;
            };
            if !remote_types_compatible(&source.data_type, &destination.data_type) {
                push_issue(
                    &mut issues,
                    "blocking",
                    "type",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' llega como {} y el destino usa {}; no se garantiza una conversión segura.",
                        source.name, source.data_type, destination.data_type
                    ),
                );
            }
            if source.null_count > 0 && !destination.nullable {
                push_issue(
                    &mut issues,
                    "blocking",
                    "nullability",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' contiene {} nulos y el destino los rechaza.",
                        source.name, source.null_count
                    ),
                );
            }
            if source.contains_nul {
                push_issue(
                    &mut issues,
                    "blocking",
                    "value",
                    Some(source.name.clone()),
                    format!(
                        "La columna '{}' contiene caracteres NUL no compatibles con ODBC.",
                        source.name
                    ),
                );
            }
            if type_family(&source.data_type) == TypeFamily::Text {
                match (source.maximum_length, destination.maximum_length) {
                    (Some(actual), Some(limit)) if actual > limit => push_issue(
                        &mut issues,
                        "blocking",
                        "length",
                        Some(source.name.clone()),
                        format!(
                            "La columna '{}' alcanza {} caracteres y el destino admite {}.",
                            source.name, actual, limit
                        ),
                    ),
                    (None, Some(_)) => push_issue(
                        &mut issues,
                        "blocking",
                        "length",
                        Some(source.name.clone()),
                        format!(
                            "No se pudo verificar la longitud máxima de '{}'; se requiere un destino sin límite o revisar la columna.",
                            source.name
                        ),
                    ),
                    _ => {}
                }
            }
        }
        for destination in existing_columns {
            if input_columns
                .iter()
                .any(|source| source.name == destination.name)
            {
                continue;
            }
            if !destination.nullable && !destination.has_default {
                push_issue(
                    &mut issues,
                    "blocking",
                    "nullability",
                    Some(destination.name.clone()),
                    format!(
                        "La columna adicional '{}' es obligatoria y no tiene valor predeterminado; la inserción no podría completarla.",
                        destination.name
                    ),
                );
            }
        }
    }

    let ready = !issues.iter().any(|issue| issue.severity == "blocking");
    RemoteExportPreflight {
        kind: target.kind,
        schema: target.schema.clone(),
        table: target.table.clone(),
        table_policy: target.table_policy,
        table_exists,
        ready,
        issues,
    }
}

fn push_issue(
    issues: &mut Vec<RemotePreflightIssue>,
    severity: &'static str,
    category: &'static str,
    column: Option<String>,
    message: String,
) {
    issues.push(RemotePreflightIssue {
        severity,
        category,
        column,
        message,
    });
}

fn remote_types_compatible(source_type: &str, destination_type: &str) -> bool {
    let source = type_family(source_type);
    let destination = type_family(destination_type);
    match source {
        TypeFamily::Integer => matches!(destination, TypeFamily::Integer | TypeFamily::Decimal),
        TypeFamily::Decimal => matches!(destination, TypeFamily::Decimal),
        TypeFamily::Boolean => matches!(destination, TypeFamily::Boolean),
        TypeFamily::Temporal => matches!(destination, TypeFamily::Temporal | TypeFamily::Text),
        TypeFamily::Text => matches!(destination, TypeFamily::Text),
        TypeFamily::Unknown => false,
    }
}

fn type_family(data_type: &str) -> TypeFamily {
    let normalized = data_type.to_ascii_lowercase();
    if normalized.contains("bool") || normalized == "bit" {
        TypeFamily::Boolean
    } else if normalized.contains("int")
        || normalized.starts_with('u')
        || normalized.starts_with('i')
    {
        TypeFamily::Integer
    } else if ["float", "double", "decimal", "numeric", "real", "money"]
        .iter()
        .any(|kind| normalized.contains(kind))
    {
        TypeFamily::Decimal
    } else if ["date", "time", "timestamp", "datetime"]
        .iter()
        .any(|kind| normalized.contains(kind))
    {
        TypeFamily::Temporal
    } else if [
        "string", "text", "char", "clob", "varchar", "nvarchar", "longtext",
    ]
    .iter()
    .any(|kind| normalized.contains(kind))
    {
        TypeFamily::Text
    } else {
        TypeFamily::Unknown
    }
}

pub(crate) fn validate_database_target(target: &DatabaseTarget) -> Result<(), String> {
    let connection_length = target.connection_string.chars().count();
    if target.connection_string.trim().is_empty() {
        return Err("Indica una cadena de conexión ODBC antes de continuar.".to_owned());
    }
    if connection_length > MAX_CONNECTION_STRING_CHARS {
        return Err(format!(
            "La cadena de conexión ODBC no puede superar {MAX_CONNECTION_STRING_CHARS} caracteres."
        ));
    }
    if target.connection_string.chars().any(char::is_control) {
        return Err(
            "La cadena de conexión ODBC contiene caracteres de control no permitidos.".to_owned(),
        );
    }
    validate_identifier(&target.schema, "el esquema", true)?;
    validate_identifier(&target.table, "la tabla", false)?;
    if target.kind == DatabaseKind::Mysql && target.table_policy == DatabaseTablePolicy::Replace {
        return Err(
            "La política Reemplazar está deshabilitada para MySQL hasta validar una sustitución atómica segura."
                .to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn export_frame<F, C>(
    frame: &DataFrame,
    target: &DatabaseTarget,
    mut report: F,
    is_cancelled: C,
) -> Result<RemoteExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let input_columns = input_shape_from_frame_with_cancel(frame, &is_cancelled)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))?;
    ensure_not_cancelled(&is_cancelled)?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    ensure_not_cancelled(&is_cancelled)?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let preflight = assess_remote_export(
        target,
        &input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    );
    ensure_preflight_ready(&preflight)?;
    connection
        .set_autocommit(false)
        .map_err(|error| format_driver_error("No se pudo iniciar la transacción", error, target))?;

    let table = qualified_table(target);
    let columns = frame
        .get_column_names()
        .iter()
        .map(|name| quote_identifier(name, target.kind))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err("No se puede entregar un dataset sin columnas.".to_owned());
    }

    report("Preparando tabla remota", 10);
    if matches!(target.table_policy, DatabaseTablePolicy::Replace) {
        execute_statement(
            &connection,
            &drop_table_sql(&table, target.kind),
            "No se pudo reemplazar la tabla remota",
            target,
        )?;
    }
    if !matches!(target.table_policy, DatabaseTablePolicy::Append) {
        execute_statement(
            &connection,
            &create_table_sql(frame, &table, target.kind),
            "No se pudo crear la tabla remota",
            target,
        )?;
    }

    report("Escribiendo filas remotas", 25);
    let statement_sql = parameterized_insert_sql(&table, &columns, columns.len());
    let mut statement = connection.prepare(&statement_sql).map_err(|error| {
        format_driver_error("No se pudo preparar la inserción remota", error, target)
    })?;
    let mut rows_written = 0usize;
    for row_index in 0..frame.height() {
        ensure_not_cancelled(&is_cancelled)?;
        let params = frame
            .columns()
            .iter()
            .map(|column| {
                let value = column
                    .get(row_index)
                    .map_err(|error| format!("No se pudo leer la fila {row_index}: {error}"))?;
                sql_parameter(value, column.dtype())
            })
            .collect::<Result<Vec<_>, String>>()?;
        statement
            .execute(params.as_slice())
            .map(|_| ())
            .map_err(|error| {
                format_driver_error(
                    "No se pudo insertar una fila en la tabla remota",
                    error,
                    target,
                )
            })?;
        rows_written += 1;
        if frame.height() > 0 {
            let percent = 25 + ((row_index + 1) * 60 / frame.height()).min(60) as u8;
            report("Escribiendo filas remotas", percent);
        }
    }

    ensure_not_cancelled(&is_cancelled)?;
    connection.commit().map_err(|error| {
        format_driver_error("No se pudo confirmar la tabla remota", error, target)
    })?;
    report("Entrega remota lista", 100);
    Ok(RemoteExportResult {
        rows_written,
        table_name: table,
        format: target.kind.label(),
    })
}

pub(crate) fn export_source_backed<F, C>(
    source_path: &Path,
    source_format: DuckDbFileFormat,
    schema: &DataFrame,
    row_count: usize,
    target: &DatabaseTarget,
    mut report: F,
    is_cancelled: C,
) -> Result<RemoteExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    if schema.width() == 0 {
        return Err("No se puede entregar un dataset sin columnas.".to_owned());
    }
    let input_columns = input_shape_from_source(
        source_path,
        source_format,
        schema,
        row_count,
        is_cancelled.clone(),
    )?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))?;
    connection
        .execute("SELECT 1", (), Some(10))
        .map_err(|error| format_driver_error("La prueba SELECT 1 falló", error, target))?;
    let (table_exists, is_base_table, existing_columns) = inspect_destination(&connection, target)?;
    let preflight = assess_remote_export(
        target,
        &input_columns,
        table_exists,
        is_base_table,
        &existing_columns,
    );
    ensure_preflight_ready(&preflight)?;
    connection
        .set_autocommit(false)
        .map_err(|error| format_driver_error("No se pudo iniciar la transacción", error, target))?;

    let table = qualified_table(target);
    let columns = schema
        .columns()
        .iter()
        .map(|column| quote_identifier(column.name(), target.kind))
        .collect::<Vec<_>>();
    report("Preparando tabla remota", 10);
    if matches!(target.table_policy, DatabaseTablePolicy::Replace) {
        execute_statement(
            &connection,
            &drop_table_sql(&table, target.kind),
            "No se pudo reemplazar la tabla remota",
            target,
        )?;
    }
    if !matches!(target.table_policy, DatabaseTablePolicy::Append) {
        execute_statement(
            &connection,
            &create_table_sql(schema, &table, target.kind),
            "No se pudo crear la tabla remota",
            target,
        )?;
    }

    report("Escribiendo filas remotas", 25);
    let statement_sql = parameterized_insert_sql(&table, &columns, columns.len());
    let mut statement = connection.prepare(&statement_sql).map_err(|error| {
        format_driver_error("No se pudo preparar la inserción remota", error, target)
    })?;
    let mut rows_written = 0usize;
    let streamed_columns = crate::duckdb_query::stream_file_rows(
        source_path,
        source_format,
        |values| {
            ensure_not_cancelled(&is_cancelled)?;
            if values.len() != schema.width() {
                return Err(
                    "La transmisión source-backed devolvió un ancho inesperado para ODBC."
                        .to_owned(),
                );
            }
            let params = schema
                .columns()
                .iter()
                .zip(values.iter())
                .map(|(column, value)| sql_parameter_text(value.as_deref(), column.dtype()))
                .collect::<Result<Vec<_>, String>>()?;
            statement
                .execute(params.as_slice())
                .map(|_| ())
                .map_err(|error| {
                    format_driver_error(
                        "No se pudo insertar una fila en la tabla remota",
                        error,
                        target,
                    )
                })?;
            rows_written = rows_written.saturating_add(1);
            let percent = if row_count == 0 {
                85
            } else {
                25 + (rows_written
                    .saturating_mul(60)
                    .checked_div(row_count)
                    .unwrap_or_default())
                .min(60) as u8
            };
            report("Escribiendo filas remotas", percent);
            Ok(())
        },
        is_cancelled.clone(),
    )?;
    let expected_columns = schema
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let actual_columns = streamed_columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    if actual_columns != expected_columns {
        return Err(
            "La transmisión source-backed devolvió columnas distintas al esquema ODBC.".to_owned(),
        );
    }
    if rows_written != row_count {
        return Err(
            "El conteo del dataset source-backed cambió durante la entrega ODBC.".to_owned(),
        );
    }
    ensure_not_cancelled(&is_cancelled)?;
    connection.commit().map_err(|error| {
        format_driver_error("No se pudo confirmar la tabla remota", error, target)
    })?;
    report("Entrega remota lista", 100);
    Ok(RemoteExportResult {
        rows_written,
        table_name: table,
        format: target.kind.label(),
    })
}

fn ensure_preflight_ready(preflight: &RemoteExportPreflight) -> Result<(), String> {
    if preflight.ready {
        return Ok(());
    }
    let blocking = preflight
        .issues
        .iter()
        .filter(|issue| issue.severity == "blocking")
        .map(|issue| issue.message.as_str())
        .collect::<Vec<_>>();
    Err(format!(
        "Preflight remoto bloqueó la entrega antes de escribir: {}",
        blocking.join(" ")
    ))
}

fn execute_statement(
    connection: &odbc_api::Connection<'_>,
    statement: &str,
    context: &str,
    target: &DatabaseTarget,
) -> Result<(), String> {
    connection
        .execute(statement, (), None)
        .map(|_| ())
        .map_err(|error| format_driver_error(context, error, target))
}

fn parameterized_insert_sql(table: &str, columns: &[String], parameter_count: usize) -> String {
    let placeholders = vec!["?"; parameter_count].join(", ");
    format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders})",
        columns.join(", ")
    )
}

fn ensure_not_cancelled<C>(is_cancelled: &C) -> Result<(), String>
where
    C: Fn() -> bool,
{
    if is_cancelled() {
        Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, label: &str, optional: bool) -> Result<(), String> {
    if optional && value.trim().is_empty() {
        return Ok(());
    }
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_IDENTIFIER_CHARS
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || value
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphabetic() && character != '_')
    {
        return Err(format!(
            "{label} solo puede contener letras ASCII, números y guiones bajos, y debe comenzar por letra o guion bajo."
        ));
    }
    Ok(())
}

fn quote_identifier(value: &str, kind: DatabaseKind) -> String {
    match kind {
        DatabaseKind::Postgresql => format!("\"{}\"", value.replace('"', "\"\"")),
        DatabaseKind::Mysql => format!("`{}`", value.replace('`', "``")),
        DatabaseKind::SqlServer => format!("[{}]", value.replace(']', "]]")),
    }
}

fn qualified_table(target: &DatabaseTarget) -> String {
    if target.schema.trim().is_empty() {
        quote_identifier(&target.table, target.kind)
    } else {
        format!(
            "{}.{}",
            quote_identifier(&target.schema, target.kind),
            quote_identifier(&target.table, target.kind)
        )
    }
}

fn drop_table_sql(table: &str, kind: DatabaseKind) -> String {
    match kind {
        DatabaseKind::Postgresql | DatabaseKind::Mysql => {
            format!("DROP TABLE IF EXISTS {table}")
        }
        DatabaseKind::SqlServer => format!(
            "IF OBJECT_ID(N'{escaped}', N'U') IS NOT NULL DROP TABLE {table}",
            escaped = table.replace('\'', "''")
        ),
    }
}

fn create_table_sql(frame: &DataFrame, table: &str, kind: DatabaseKind) -> String {
    let columns = frame
        .columns()
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                quote_identifier(column.name(), kind),
                database_type(column.dtype(), kind)
            )
        })
        .collect::<Vec<_>>();
    format!("CREATE TABLE {table} ({})", columns.join(", "))
}

fn database_type(dtype: &polars::prelude::DataType, kind: DatabaseKind) -> &'static str {
    let dtype = dtype.to_string().to_ascii_lowercase();
    if dtype.contains("bool") {
        return match kind {
            DatabaseKind::SqlServer => "BIT",
            _ => "BOOLEAN",
        };
    }
    if is_integer_dtype(&dtype) {
        return "BIGINT";
    }
    if is_decimal_dtype(&dtype) {
        return match kind {
            DatabaseKind::SqlServer => "FLOAT",
            _ => "DOUBLE PRECISION",
        };
    }
    if dtype.contains("date") || dtype.contains("datetime") || dtype.contains("time") {
        return match kind {
            DatabaseKind::SqlServer => "DATETIME2",
            _ => "TIMESTAMP",
        };
    }
    match kind {
        DatabaseKind::SqlServer => "NVARCHAR(MAX)",
        DatabaseKind::Postgresql => "TEXT",
        DatabaseKind::Mysql => "LONGTEXT",
    }
}

fn sql_parameter(
    value: AnyValue<'_>,
    dtype: &polars::prelude::DataType,
) -> Result<Box<dyn InputParameter>, String> {
    if matches!(value, AnyValue::Null) {
        return sql_parameter_text(None, dtype);
    }
    let text = match value {
        AnyValue::String(value) => value.to_owned(),
        AnyValue::StringOwned(value) => value.to_string(),
        value => value.to_string(),
    };
    sql_parameter_text(Some(&text), dtype)
}

fn sql_parameter_text(
    value: Option<&str>,
    dtype: &polars::prelude::DataType,
) -> Result<Box<dyn InputParameter>, String> {
    let normalized_dtype = dtype.to_string().to_ascii_lowercase();
    let Some(text) = value else {
        return Ok(if normalized_dtype.contains("bool") {
            Box::new(Nullable::<Bit>::null())
        } else if is_integer_dtype(&normalized_dtype) {
            Box::new(Nullable::<i64>::null())
        } else if is_decimal_dtype(&normalized_dtype) {
            Box::new(Nullable::<f64>::null())
        } else {
            Box::new(None::<String>.into_parameter())
        });
    };
    if normalized_dtype.contains("bool") {
        return Ok(Box::new(Bit::from_bool(parse_boolean_parameter(text)?)));
    }
    if is_integer_dtype(&normalized_dtype) || is_decimal_dtype(&normalized_dtype) {
        if is_integer_dtype(&normalized_dtype) {
            let value = text.parse::<i64>().map_err(|_| {
                "Una celda entera no cabe en el parámetro Int64 usado por ODBC.".to_owned()
            })?;
            return Ok(Box::new(value));
        }
        let value = text
            .parse::<f64>()
            .map_err(|_| "Una celda decimal no puede representarse como número ODBC.".to_owned())?;
        if !value.is_finite() {
            return Err("Una celda decimal no finita no se puede escribir con ODBC.".to_owned());
        }
        return Ok(Box::new(value));
    }
    if text.contains('\0') {
        return Err("Una celda contiene un carácter NUL no compatible con SQL/ODBC.".to_owned());
    }
    Ok(Box::new(text.to_owned().into_parameter()))
}

fn parse_boolean_parameter(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Se encontró un booleano incompatible al entregar la tabla remota.".to_owned()),
    }
}

fn is_integer_dtype(dtype: &str) -> bool {
    dtype.contains("int")
        || dtype.contains("uint")
        || matches!(
            dtype,
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
        )
}

fn is_decimal_dtype(dtype: &str) -> bool {
    dtype.contains("float")
        || dtype.contains("decimal")
        || dtype.contains("double")
        || matches!(dtype, "f32" | "f64")
}

fn format_driver_error<E: Debug>(context: &str, error: E, target: &DatabaseTarget) -> String {
    let mut detail = format!("{error:?}");
    for part in target.connection_string.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if matches!(
            key.trim().to_ascii_lowercase().as_str(),
            "pwd" | "password" | "pass" | "secret" | "token"
        ) && !value.trim().is_empty()
        {
            detail = detail.replace(value.trim(), "[REDACTED]");
        }
    }
    format!("{context} para {}: {detail}", target.kind.label())
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use odbc_api::{buffers::RowVec, parameter::VarWCharArray, Cursor};
    use polars::{
        df,
        prelude::{DataFrame, DataType, ParquetWriter},
    };
    use tempfile::tempdir;

    use super::*;

    fn target(kind: DatabaseKind) -> DatabaseTarget {
        DatabaseTarget {
            kind,
            connection_string: "Driver={test};Server=localhost;Pwd=secret".to_owned(),
            schema: "public".to_owned(),
            table: "ventas".to_owned(),
            table_policy: DatabaseTablePolicy::CreateOnly,
        }
    }

    #[test]
    fn validates_identifiers_and_redacts_secrets() {
        let mut value = target(DatabaseKind::Postgresql);
        assert!(validate_database_target(&value).is_ok());
        value.table = "ventas; DROP TABLE users".to_owned();
        assert!(validate_database_target(&value).is_err());
        let error = format_driver_error("falló", "Pwd=secret", &target(DatabaseKind::Postgresql));
        assert!(!error.contains("secret"));
        assert!(error.contains("[REDACTED]"));
    }

    #[test]
    fn generates_dialect_specific_schema_and_escaped_literals() {
        let frame = df!(
            "name" => &["O'Brien"],
            "amount" => &[10i64],
            "active" => &[true]
        )
        .expect("frame válido");
        assert_eq!(
            create_table_sql(&frame, "\"ventas\"", DatabaseKind::Postgresql),
            "CREATE TABLE \"ventas\" (\"name\" TEXT, \"amount\" BIGINT, \"active\" BOOLEAN)"
        );
        assert!(sql_parameter(
            frame.column("name").expect("name").get(0).expect("value"),
            &polars::prelude::DataType::String,
        )
        .is_ok());
        assert_eq!(
            qualified_table(&target(DatabaseKind::SqlServer)),
            "[public].[ventas]"
        );
    }

    #[test]
    fn streamed_parameters_preserve_adversarial_text_and_typed_nulls() {
        assert!(sql_parameter_text(Some("O'Brien; DROP TABLE users"), &DataType::String).is_ok());
        assert!(sql_parameter_text(Some("42"), &DataType::Int64).is_ok());
        assert!(sql_parameter_text(Some("not-a-number"), &DataType::Float64).is_err());
        assert!(sql_parameter_text(None, &DataType::String).is_ok());
        let columns = vec!["name".to_owned(), "active".to_owned()];
        assert_eq!(
            parameterized_insert_sql("`ventas`", &columns, 2),
            "INSERT INTO `ventas` (name, active) VALUES (?, ?)"
        );
    }

    #[test]
    fn materialized_preflight_blocks_unsigned_overflow_and_non_finite_numbers() {
        let frame = df!(
            "unsigned" => &[u64::MAX, 42_u64],
            "measurement" => &[f64::NAN, f64::INFINITY]
        )
        .expect("frame numérico válido");
        let input = input_shape_from_frame(&frame).expect("se debe revisar cada valor numérico");

        assert_eq!(input[0].unrepresentable_integer_count, 1);
        assert_eq!(input[0].null_count, 0);
        assert_eq!(input[1].unrepresentable_decimal_count, 2);
        assert_eq!(input[1].null_count, 0);

        let report =
            assess_remote_export(&target(DatabaseKind::Postgresql), &input, false, false, &[]);
        assert!(
            !report.ready,
            "CreateOnly no debe aceptar valores convertidos silenciosamente en NULL"
        );
        assert!(report.issues.iter().any(|issue| {
            issue.column.as_deref() == Some("unsigned")
                && issue.message.contains("no caben en el parámetro Int64")
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.column.as_deref() == Some("measurement") && issue.message.contains("no finito(s)")
        }));
    }

    #[test]
    fn source_backed_preflight_blocks_unsigned_overflow_and_non_finite_numbers() {
        let directory = tempdir().expect("se debe crear la carpeta temporal");
        let path = directory.path().join("unrepresentable.parquet");
        let mut frame = df!(
            "unsigned" => &[u64::MAX, 42_u64],
            "measurement" => &[f64::NAN, f64::INFINITY]
        )
        .expect("frame Parquet válido");
        let mut file = fs::File::create(&path).expect("se debe crear Parquet");
        ParquetWriter::new(&mut file)
            .finish(&mut frame)
            .expect("se debe escribir Parquet");

        let schema = frame.slice(0, 0);
        let input = input_shape_from_source(&path, DuckDbFileFormat::Parquet, &schema, 2, || false)
            .expect("el preflight debe inspeccionar todos los valores source-backed");

        assert_eq!(input[0].unrepresentable_integer_count, 1);
        assert_eq!(input[0].null_count, 0);
        assert_eq!(input[1].unrepresentable_decimal_count, 2);
        assert_eq!(input[1].null_count, 0);
        let report =
            assess_remote_export(&target(DatabaseKind::Postgresql), &input, false, false, &[]);
        assert!(!report.ready);
        assert_eq!(
            report
                .issues
                .iter()
                .filter(|issue| issue.severity == "blocking")
                .count(),
            2
        );
    }

    #[test]
    fn numeric_binding_rejects_overflow_and_non_finite_values_but_preserves_real_nulls() {
        assert!(sql_parameter(AnyValue::UInt64(u64::MAX), &DataType::UInt64).is_err());
        assert!(sql_parameter(AnyValue::Float64(f64::NAN), &DataType::Float64).is_err());
        assert!(sql_parameter(AnyValue::Float64(f64::INFINITY), &DataType::Float64).is_err());
        assert!(sql_parameter(AnyValue::UInt64(42), &DataType::UInt64).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::UInt64).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::Float64).is_ok());
        assert!(sql_parameter_text(Some("18446744073709551615"), &DataType::UInt64).is_err());
        assert!(sql_parameter_text(Some("NaN"), &DataType::Float64).is_err());
        assert!(sql_parameter_text(Some("Infinity"), &DataType::Float64).is_err());
    }

    #[test]
    fn boolean_parameters_parse_values_reject_invalid_text_and_map_dialect_types() {
        assert_eq!(parse_boolean_parameter("true"), Ok(true));
        assert_eq!(parse_boolean_parameter("FALSE"), Ok(false));
        assert!(parse_boolean_parameter("not-a-boolean").is_err());

        assert!(sql_parameter(AnyValue::Boolean(true), &DataType::Boolean).is_ok());
        assert!(sql_parameter(AnyValue::Boolean(false), &DataType::Boolean).is_ok());
        assert!(sql_parameter(AnyValue::Null, &DataType::Boolean).is_ok());
        assert!(sql_parameter_text(Some("not-a-boolean"), &DataType::Boolean).is_err());
        assert!(sql_parameter_text(None, &DataType::Boolean).is_ok());

        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::Postgresql),
            "BOOLEAN"
        );
        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::Mysql),
            "BOOLEAN"
        );
        assert_eq!(
            database_type(&DataType::Boolean, DatabaseKind::SqlServer),
            "BIT"
        );
    }

    #[test]
    fn maps_table_policies_to_explicit_ddl() {
        let mut table = target(DatabaseKind::Mysql);
        assert_eq!(
            drop_table_sql("`public`.`ventas`", table.kind),
            "DROP TABLE IF EXISTS `public`.`ventas`"
        );
        assert!(matches!(
            table.table_policy,
            DatabaseTablePolicy::CreateOnly
        ));
        table.table_policy = DatabaseTablePolicy::Replace;
        assert!(validate_database_target(&table).is_err());
    }

    #[test]
    fn preflight_explains_safe_new_table_and_blocks_conflicting_policies() {
        let input = vec![RemoteInputColumn {
            name: "name".to_owned(),
            data_type: "String".to_owned(),
            null_count: 0,
            maximum_length: Some(12),
            contains_nul: false,
            unrepresentable_integer_count: 0,
            unrepresentable_decimal_count: 0,
        }];
        let mut destination = target(DatabaseKind::Postgresql);
        let create = assess_remote_export(&destination, &input, false, false, &[]);
        assert!(create.ready);
        assert!(create.issues.iter().any(|issue| issue.category == "policy"));

        let exists = assess_remote_export(&destination, &input, true, true, &[]);
        assert!(!exists.ready);
        assert!(exists
            .issues
            .iter()
            .any(|issue| { issue.severity == "blocking" && issue.message.contains("ya existe") }));

        destination.table_policy = DatabaseTablePolicy::Append;
        let missing_append = assess_remote_export(&destination, &input, false, false, &[]);
        assert!(!missing_append.ready);
    }

    #[test]
    fn append_preflight_checks_types_nulls_lengths_nul_and_required_extra_columns() {
        let mut destination = target(DatabaseKind::SqlServer);
        destination.table_policy = DatabaseTablePolicy::Append;
        let input = vec![
            RemoteInputColumn {
                name: "name".to_owned(),
                data_type: "String".to_owned(),
                null_count: 2,
                maximum_length: Some(14),
                contains_nul: true,
                unrepresentable_integer_count: 0,
                unrepresentable_decimal_count: 0,
            },
            RemoteInputColumn {
                name: "amount".to_owned(),
                data_type: "Boolean".to_owned(),
                null_count: 0,
                maximum_length: None,
                contains_nul: false,
                unrepresentable_integer_count: 0,
                unrepresentable_decimal_count: 0,
            },
        ];
        let existing = vec![
            ExistingColumn {
                name: "name".to_owned(),
                data_type: "NVARCHAR(10)".to_owned(),
                maximum_length: Some(10),
                nullable: false,
                has_default: false,
            },
            ExistingColumn {
                name: "amount".to_owned(),
                data_type: "BIGINT".to_owned(),
                maximum_length: None,
                nullable: true,
                has_default: false,
            },
            ExistingColumn {
                name: "required_at_destination".to_owned(),
                data_type: "INT".to_owned(),
                maximum_length: None,
                nullable: false,
                has_default: false,
            },
        ];
        let report = assess_remote_export(&destination, &input, true, true, &existing);
        assert!(!report.ready);
        assert!(report.issues.iter().any(|issue| issue.category == "type"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.category == "nullability"));
        assert!(report.issues.iter().any(|issue| issue.category == "length"));
        assert!(report.issues.iter().any(|issue| issue.category == "value"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("required_at_destination")));
    }

    #[test]
    fn replace_policy_is_reported_as_explicit_warning() {
        let mut destination = target(DatabaseKind::Postgresql);
        destination.table_policy = DatabaseTablePolicy::Replace;
        let report = assess_remote_export(&destination, &[], true, true, &[]);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.severity == "warning" && issue.category == "policy"));
    }

    fn external_target(kind: DatabaseKind, variable: &str) -> DatabaseTarget {
        let connection_string = env::var(variable).unwrap_or_else(|_| {
            panic!(
                "La prueba ODBC externa requiere la variable {variable} con una cadena de conexión de sesión."
            )
        });
        DatabaseTarget {
            kind,
            connection_string,
            schema: match kind {
                DatabaseKind::Postgresql => "public".to_owned(),
                DatabaseKind::Mysql => String::new(),
                DatabaseKind::SqlServer => "dbo".to_owned(),
            },
            table: "columnia_t6_external".to_owned(),
            table_policy: DatabaseTablePolicy::CreateOnly,
        }
    }

    fn adversarial_frame() -> DataFrame {
        df!(
            "ordinal" => &[1i64, 2, 3],
            "text" => &[
                "O'Brien; DROP TABLE users \\ barra",
                "línea 1\nlínea 2 — Unicode",
                "comillas \"dobles\" y dos puntos: sí"
            ],
            "active" => &[Some(true), Some(false), None]
        )
        .expect("frame externo válido")
    }

    fn source_csv(path: &Path) {
        fs::write(
            path,
            "ordinal,text,active\n1,\"O'Brien; DROP TABLE users \\ barra\",true\n2,\"línea 1\nlínea 2 — Unicode\",false\n3,\"comillas \"\"dobles\"\" y dos puntos: sí\",\n",
        )
        .expect("CSV externo válido");
    }

    fn query_round_trip(
        target: &DatabaseTarget,
        expected: &[(i64, Option<&str>, &str)],
    ) -> Result<(), String> {
        let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
        let connection = environment
            .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
            .map_err(|error| format!("conexión ODBC: {error:?}"))?;
        let ordinal = quote_identifier("ordinal", target.kind);
        let text = quote_identifier("text", target.kind);
        let active = quote_identifier("active", target.kind);
        let table = qualified_table(target);
        let text_projection = match target.kind {
            DatabaseKind::Postgresql => format!("encode(convert_to({text}, 'UTF8'), 'hex')"),
            DatabaseKind::Mysql => format!("HEX({text})"),
            DatabaseKind::SqlServer => text.clone(),
        };
        let active_true = match target.kind {
            DatabaseKind::Postgresql => active.clone(),
            DatabaseKind::Mysql | DatabaseKind::SqlServer => format!("{active} = 1"),
        };
        let query = format!(
            "SELECT {ordinal}, {text_projection}, CASE WHEN {active} IS NULL THEN 'null' WHEN {active_true} THEN 'true' ELSE 'false' END FROM {table} ORDER BY {ordinal}"
        );
        let cursor = connection
            .execute(&query, (), None)
            .map_err(|error| format!("consulta ODBC: {error:?}"))?
            .ok_or_else(|| "La consulta ODBC no devolvió filas.".to_owned())?;
        let mut cursor = cursor
            .bind_buffer(RowVec::<(i64, VarWCharArray<512>, VarWCharArray<8>)>::new(
                16,
            ))
            .map_err(|error| format!("buffer ODBC: {error:?}"))?;
        let mut actual = Vec::new();
        while let Some(batch) = cursor
            .fetch()
            .map_err(|error| format!("lectura ODBC: {error:?}"))?
        {
            for (ordinal, text, active) in batch.iter() {
                let text = text
                    .as_utf16()
                    .map(|value| value.to_string().to_ascii_lowercase());
                let active = active
                    .as_utf16()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                actual.push((*ordinal, text, active));
            }
        }
        let expected = expected
            .iter()
            .map(|(ordinal, text, active)| {
                let text = text.map(|text| match target.kind {
                    DatabaseKind::Postgresql | DatabaseKind::Mysql => text
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    DatabaseKind::SqlServer => text.to_owned(),
                });
                (*ordinal, text, (*active).to_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "round-trip ODBC alteró los valores");
        Ok(())
    }

    fn execute_external_sql(target: &DatabaseTarget, sql: &str) -> Result<(), String> {
        let environment = Environment::new().map_err(|error| format!("ODBC: {error:?}"))?;
        let connection = environment
            .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
            .map_err(|error| format!("conexión ODBC: {error:?}"))?;
        connection
            .execute(sql, (), None)
            .map(|_| ())
            .map_err(|error| format!("SQL externo: {error:?}"))
    }

    fn run_external_round_trip(target: &DatabaseTarget, suffix: &str) -> Result<(), String> {
        let expected = [
            (1, Some("O'Brien; DROP TABLE users \\ barra"), "true"),
            (2, Some("línea 1\nlínea 2 — Unicode"), "false"),
            (3, Some("comillas \"dobles\" y dos puntos: sí"), "null"),
        ];
        let mut frame_target = target.clone();
        frame_target.table = format!("columnia_t6_{suffix}_frame");
        export_frame(&adversarial_frame(), &frame_target, |_, _| {}, || false)?;
        query_round_trip(&frame_target, &expected)?;

        let directory = tempdir().map_err(|error| format!("temp externo: {error}"))?;
        let source = directory.path().join("adversarial.csv");
        source_csv(&source);
        let schema = adversarial_frame();
        let mut source_target = target.clone();
        source_target.table = format!("columnia_t6_{suffix}_source");
        export_source_backed(
            &source,
            DuckDbFileFormat::Delimited { delimiter: b',' },
            &schema,
            schema.height(),
            &source_target,
            |_, _| {},
            || false,
        )?;
        query_round_trip(&source_target, &expected)?;

        execute_external_sql(
            &frame_target,
            &drop_table_sql(&qualified_table(&frame_target), frame_target.kind),
        )?;
        execute_external_sql(
            &source_target,
            &drop_table_sql(&qualified_table(&source_target), source_target.kind),
        )?;
        Ok(())
    }

    fn unique_suffix(prefix: &str) -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("reloj del sistema válido")
            .as_millis();
        format!("{prefix}_{millis}")
    }

    #[test]
    #[ignore = "requiere servidores y controladores ODBC reales configurados por variables de sesión"]
    fn external_odbc_round_trip_postgresql() {
        let target = external_target(DatabaseKind::Postgresql, "COLUMNIA_ODBC_POSTGRESQL");
        run_external_round_trip(&target, &unique_suffix("pg"))
            .expect("round-trip PostgreSQL completo");
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC MySQL reales configurados por variables de sesión"]
    fn external_odbc_round_trip_mysql_with_and_without_no_backslash_escapes() {
        let target = external_target(DatabaseKind::Mysql, "COLUMNIA_ODBC_MYSQL");
        run_external_round_trip(&target, &unique_suffix("mysql_default"))
            .expect("round-trip MySQL por defecto completo");
        execute_external_sql(&target, "SET GLOBAL sql_mode = 'NO_BACKSLASH_ESCAPES'")
            .expect("activar NO_BACKSLASH_ESCAPES");
        let result = run_external_round_trip(&target, &unique_suffix("mysql_no_backslash"));
        let _ = execute_external_sql(&target, "SET GLOBAL sql_mode = 'STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_AUTO_CREATE_USER,NO_ENGINE_SUBSTITUTION'");
        result.expect("round-trip MySQL con NO_BACKSLASH_ESCAPES completo");
    }

    #[test]
    #[ignore = "requiere servidor y controlador ODBC SQL Server reales configurados por variables de sesión"]
    fn external_odbc_round_trip_sql_server() {
        let target = external_target(DatabaseKind::SqlServer, "COLUMNIA_ODBC_SQLSERVER");
        run_external_round_trip(&target, &unique_suffix("sqlserver"))
            .expect("round-trip SQL Server completo");
    }
}
