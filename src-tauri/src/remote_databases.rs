use std::{fmt::Debug, path::Path};

use odbc_api::{
    parameter::InputParameter, Bit, ConnectionOptions, Environment, IntoParameter, Nullable,
};
use polars::prelude::{AnyValue, DataFrame};
use serde::{Deserialize, Serialize};

use crate::duckdb_query::DuckDbFileFormat;

const MAX_CONNECTION_STRING_CHARS: usize = 16 * 1024;
const MAX_IDENTIFIER_CHARS: usize = 128;

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
    C: Fn() -> bool,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))?;
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
    C: Fn() -> bool + Clone + Send + 'static,
{
    validate_database_target(target)?;
    ensure_not_cancelled(&is_cancelled)?;
    if schema.width() == 0 {
        return Err("No se puede entregar un dataset sin columnas.".to_owned());
    }
    let environment = Environment::new()
        .map_err(|error| format_driver_error("No se pudo inicializar ODBC", error, target))?;
    let connection = environment
        .connect_with_connection_string(&target.connection_string, ConnectionOptions::default())
        .map_err(|error| format_driver_error("No se pudo abrir la conexión", error, target))?;
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
        _ => "TEXT",
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
        return Ok(match text.to_ascii_lowercase().as_str() {
            "true" => Box::new(Bit::from_bool(true)),
            "false" => Box::new(Bit::from_bool(false)),
            _ => {
                return Err(
                    "Se encontró un booleano incompatible al entregar la tabla remota.".to_owned(),
                )
            }
        });
    }
    if is_integer_dtype(&normalized_dtype) || is_decimal_dtype(&normalized_dtype) {
        if is_integer_dtype(&normalized_dtype) {
            if let Ok(value) = text.parse::<i64>() {
                return Ok(Box::new(value));
            }
            return Ok(Box::new(Nullable::<i64>::null()));
        }
        if let Ok(value) = text.parse::<f64>() {
            if value.is_finite() {
                return Ok(Box::new(value));
            }
        }
        return Ok(Box::new(Nullable::<f64>::null()));
    }
    if text.contains('\0') {
        return Err("Una celda contiene un carácter NUL no compatible con SQL/ODBC.".to_owned());
    }
    Ok(Box::new(text.to_owned().into_parameter()))
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
        prelude::{DataFrame, DataType},
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
        assert!(sql_parameter_text(Some("not-a-number"), &DataType::Float64).is_ok());
        assert!(sql_parameter_text(None, &DataType::String).is_ok());
        let columns = vec!["name".to_owned(), "active".to_owned()];
        assert_eq!(
            parameterized_insert_sql("`ventas`", &columns, 2),
            "INSERT INTO `ventas` (name, active) VALUES (?, ?)"
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
