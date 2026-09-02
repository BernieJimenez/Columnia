use std::fmt::Debug;

use odbc_api::{ConnectionOptions, Environment};
use polars::prelude::{AnyValue, DataFrame};
use serde::{Deserialize, Serialize};

const MAX_CONNECTION_STRING_CHARS: usize = 16 * 1024;
const MAX_IDENTIFIER_CHARS: usize = 128;
const INSERT_BATCH_ROWS: usize = 100;
const INSERT_BATCH_BYTES: usize = 512 * 1024;

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
    validate_identifier(&target.table, "la tabla", false)
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
    let mut rows_written = 0usize;
    let mut batch = String::new();
    for row_index in 0..frame.height() {
        ensure_not_cancelled(&is_cancelled)?;
        let row = frame
            .columns()
            .iter()
            .map(|column| {
                let value = column
                    .get(row_index)
                    .map_err(|error| format!("No se pudo leer la fila {row_index}: {error}"))?;
                sql_literal(value, column.dtype(), target.kind)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let row_sql = format!("({})", row.join(", "));
        if batch.is_empty() {
            batch.push_str("INSERT INTO ");
            batch.push_str(&table);
            batch.push_str(" (");
            batch.push_str(&columns.join(", "));
            batch.push_str(") VALUES ");
        } else {
            batch.push_str(", ");
        }
        batch.push_str(&row_sql);
        rows_written += 1;
        let last_row = row_index + 1 == frame.height();
        if rows_written.is_multiple_of(INSERT_BATCH_ROWS)
            || batch.len() >= INSERT_BATCH_BYTES
            || last_row
        {
            execute_statement(
                &connection,
                &batch,
                "No se pudo insertar un lote en la tabla remota",
                target,
            )?;
            batch.clear();
        }
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

fn sql_literal(
    value: AnyValue<'_>,
    dtype: &polars::prelude::DataType,
    kind: DatabaseKind,
) -> Result<String, String> {
    if matches!(value, AnyValue::Null) {
        return Ok("NULL".to_owned());
    }
    let normalized_dtype = dtype.to_string().to_ascii_lowercase();
    let text = match value {
        AnyValue::String(value) => value.to_owned(),
        AnyValue::StringOwned(value) => value.to_string(),
        value => value.to_string(),
    };
    if normalized_dtype.contains("bool") {
        return Ok(match text.to_ascii_lowercase().as_str() {
            "true" => "1".to_owned(),
            "false" => "0".to_owned(),
            _ => {
                return Err(
                    "Se encontró un booleano incompatible al entregar la tabla remota.".to_owned(),
                )
            }
        });
    }
    if is_integer_dtype(&normalized_dtype) || is_decimal_dtype(&normalized_dtype) {
        if text.parse::<f64>().is_ok_and(f64::is_finite) {
            return Ok(text);
        }
        return Ok("NULL".to_owned());
    }
    if text.contains('\0') {
        return Err("Una celda contiene un carácter NUL no compatible con SQL/ODBC.".to_owned());
    }
    let escaped = text.replace('\'', "''");
    Ok(match kind {
        DatabaseKind::SqlServer => format!("N'{escaped}'"),
        DatabaseKind::Postgresql | DatabaseKind::Mysql => format!("'{escaped}'"),
    })
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
    use polars::df;

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
        assert_eq!(
            sql_literal(
                frame.column("name").expect("name").get(0).expect("value"),
                &polars::prelude::DataType::String,
                DatabaseKind::Postgresql
            )
            .expect("literal"),
            "'O''Brien'"
        );
        assert_eq!(
            qualified_table(&target(DatabaseKind::SqlServer)),
            "[public].[ventas]"
        );
    }

    #[test]
    fn maps_table_policies_to_explicit_ddl() {
        let table = target(DatabaseKind::Mysql);
        assert_eq!(
            drop_table_sql("`public`.`ventas`", table.kind),
            "DROP TABLE IF EXISTS `public`.`ventas`"
        );
        assert!(matches!(
            table.table_policy,
            DatabaseTablePolicy::CreateOnly
        ));
    }
}
