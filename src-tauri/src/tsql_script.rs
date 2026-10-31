//! T-SQL for SQL Server (2026-10-31): the «Script T-SQL» export and the
//! loading script of the «Paquete BULK INSERT». Both export paths (Polars in
//! memory, DuckDB by blocks) feed their rows through this module.
//!
//! The scripts never drop anything: if the table already exists they stop
//! before touching it, and everything runs in one transaction, so a failure
//! leaves no half-loaded table.

use std::borrow::Cow;
use std::io::Write;

use polars::prelude::DataType;

/// SQL Server accepts at most 1,000 rows in one `INSERT … VALUES`.
pub(crate) const ROWS_PER_INSERT: usize = 1_000;
const SCHEMA: &str = "dbo";
const MAX_TABLE_NAME_CHARS: usize = 128;

/// A column of the table the script creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TsqlColumn {
    pub(crate) name: String,
    pub(crate) declaration: String,
}

/// One value of a row, already decided by the caller's type.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TsqlValue<'a> {
    Null,
    Bit(bool),
    /// An integer, decimal or finite float, written as is.
    Number(String),
    Text(Cow<'a, str>),
    Binary(&'a [u8]),
}

/// The SQL Server type of a Polars column. Text and anything without an exact
/// SQL Server equivalent become `NVARCHAR(MAX)`, which keeps every character.
pub(crate) fn tsql_type_for_polars(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "BIT",
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32 => "BIGINT",
        // A u64 can exceed BIGINT.
        DataType::UInt64 => "DECIMAL(20,0)",
        DataType::Float32 | DataType::Float64 => "FLOAT",
        DataType::Date => "DATE",
        // With a time zone the text carries an offset DATETIME2 rejects.
        DataType::Datetime(_, None) => "DATETIME2",
        DataType::Time => "TIME",
        _ => "NVARCHAR(MAX)",
    }
}

/// The SQL Server type of a DuckDB column (its `DESCRIBE` name).
pub(crate) fn tsql_type_for_duckdb(data_type: &str) -> String {
    let data_type = data_type.trim().to_ascii_uppercase();
    if data_type.starts_with("BOOL") {
        "BIT".to_owned()
    } else if data_type.starts_with("DECIMAL") || data_type.starts_with("NUMERIC") {
        data_type
            .split_once('(')
            .map(|(_, precision)| format!("DECIMAL({precision}"))
            .unwrap_or_else(|| "DECIMAL(18,3)".to_owned())
    } else if data_type == "UBIGINT" {
        "DECIMAL(20,0)".to_owned()
    } else if matches!(data_type.as_str(), "HUGEINT" | "UHUGEINT") {
        "DECIMAL(38,0)".to_owned()
    } else if matches!(
        data_type.as_str(),
        "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "UTINYINT" | "USMALLINT" | "UINTEGER"
    ) {
        "BIGINT".to_owned()
    } else if data_type.contains("FLOAT") || data_type.contains("DOUBLE") || data_type == "REAL" {
        "FLOAT".to_owned()
    } else if data_type.starts_with("TIMESTAMP") {
        "DATETIME2".to_owned()
    } else if data_type.starts_with("DATE") {
        "DATE".to_owned()
    } else if data_type.starts_with("TIME") {
        "TIME".to_owned()
    } else if data_type == "BLOB" {
        "VARBINARY(MAX)".to_owned()
    } else {
        "NVARCHAR(MAX)".to_owned()
    }
}

/// A table name SQL Server accepts without surprises, taken from the file
/// name the person chose: letters, digits and underscores.
pub(crate) fn tsql_table_name(file_stem: &str) -> String {
    let mut name = file_stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned();
    if name.is_empty() {
        name = "dataset".to_owned();
    }
    if name.starts_with(|character: char| character.is_ascii_digit()) {
        name = format!("t_{name}");
    }
    name.chars().take(MAX_TABLE_NAME_CHARS).collect()
}

pub(crate) fn quote_identifier(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn qualified_table(table: &str) -> String {
    format!("{}.{}", quote_identifier(SCHEMA), quote_identifier(table))
}

fn unicode_literal(value: &str) -> String {
    format!("N'{}'", value.replace('\'', "''"))
}

fn render_value(value: &TsqlValue<'_>) -> String {
    match value {
        TsqlValue::Null => "NULL".to_owned(),
        TsqlValue::Bit(value) => if *value { "1" } else { "0" }.to_owned(),
        TsqlValue::Number(value) => value.clone(),
        TsqlValue::Text(value) => unicode_literal(value),
        TsqlValue::Binary(value) => {
            let mut hex = String::with_capacity(2 + value.len() * 2);
            hex.push_str("0x");
            for byte in value.iter() {
                hex.push_str(&format!("{byte:02X}"));
            }
            hex
        }
    }
}

fn write_error(error: std::io::Error) -> String {
    format!("No se pudo escribir el script T-SQL: {error}")
}

/// Stops the script when the table exists, then opens the transaction and
/// creates the table. Shared by both scripts.
fn write_table_creation<W: Write>(
    output: &mut W,
    table: &str,
    columns: &[TsqlColumn],
) -> Result<(), String> {
    if columns.is_empty() {
        return Err("SQL Server requiere al menos una columna para crear la tabla.".to_owned());
    }
    let qualified = qualified_table(table);
    let message = format!(
        "La tabla {qualified} ya existe: cámbiale el nombre en este script o bórrala antes de ejecutarlo."
    );
    writeln!(output, "SET NOCOUNT ON;").map_err(write_error)?;
    writeln!(output, "SET XACT_ABORT ON;").map_err(write_error)?;
    writeln!(
        output,
        "IF OBJECT_ID({}, N'U') IS NOT NULL\n    THROW 50000, {}, 1;",
        unicode_literal(&qualified),
        unicode_literal(&message)
    )
    .map_err(write_error)?;
    writeln!(output, "BEGIN TRANSACTION;").map_err(write_error)?;
    writeln!(output, "CREATE TABLE {qualified} (").map_err(write_error)?;
    for (index, column) in columns.iter().enumerate() {
        let separator = if index + 1 < columns.len() { "," } else { "" };
        writeln!(
            output,
            "    {} {} NULL{separator}",
            quote_identifier(&column.name),
            column.declaration
        )
        .map_err(write_error)?;
    }
    writeln!(output, ");").map_err(write_error)
}

/// Writes a script that creates the table and inserts every row, 1,000 rows
/// per `INSERT`, in a single batch and transaction.
pub(crate) struct TsqlScriptWriter<W: Write> {
    output: W,
    insert_prefix: String,
    rows_in_statement: usize,
    rows: usize,
}

impl<W: Write> TsqlScriptWriter<W> {
    pub(crate) fn begin(
        mut output: W,
        table: &str,
        columns: &[TsqlColumn],
    ) -> Result<Self, String> {
        writeln!(
            output,
            "-- Exportado por Columnia como script T-SQL para SQL Server 2016 o posterior.\n\
             -- Ejecútalo en la base de destino con SSMS, Azure Data Studio o sqlcmd.\n\
             -- Crea la tabla {} y carga sus filas en una sola transacción: si algo falla, no queda nada a medias.\n\
             -- Si la tabla ya existe, el script se detiene sin tocarla. Para tablas grandes usa el paquete BULK INSERT.",
            qualified_table(table)
        )
        .map_err(write_error)?;
        write_table_creation(&mut output, table, columns)?;
        let column_list = columns
            .iter()
            .map(|column| quote_identifier(&column.name))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Self {
            output,
            insert_prefix: format!(
                "INSERT INTO {} ({column_list}) VALUES",
                qualified_table(table)
            ),
            rows_in_statement: 0,
            rows: 0,
        })
    }

    pub(crate) fn push_row<'a>(
        &mut self,
        values: impl IntoIterator<Item = TsqlValue<'a>>,
    ) -> Result<(), String> {
        let rendered = values
            .into_iter()
            .map(|value| render_value(&value))
            .collect::<Vec<_>>()
            .join(", ");
        if self.rows_in_statement == 0 {
            writeln!(self.output, "{}", self.insert_prefix).map_err(write_error)?;
            write!(self.output, "    ({rendered})").map_err(write_error)?;
        } else {
            write!(self.output, ",\n    ({rendered})").map_err(write_error)?;
        }
        self.rows_in_statement += 1;
        self.rows += 1;
        if self.rows_in_statement == ROWS_PER_INSERT {
            writeln!(self.output, ";").map_err(write_error)?;
            self.rows_in_statement = 0;
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<W, String> {
        if self.rows_in_statement > 0 {
            writeln!(self.output, ";").map_err(write_error)?;
        }
        writeln!(self.output, "COMMIT TRANSACTION;").map_err(write_error)?;
        writeln!(self.output, "-- {} filas.", self.rows).map_err(write_error)?;
        self.output.flush().map_err(write_error)?;
        Ok(self.output)
    }
}

/// The CSV file name inside the BULK INSERT package.
pub(crate) fn bulk_csv_name(table: &str) -> String {
    format!("{table}.csv")
}

/// `cargar.sql` of the BULK INSERT package. `BULK INSERT … FORMAT = 'CSV'`
/// needs SQL Server 2017 or later and reads the file from the server's disk.
pub(crate) fn bulk_load_script(
    table: &str,
    columns: &[TsqlColumn],
    rows: Option<usize>,
) -> Result<String, String> {
    let mut script = Vec::new();
    let csv = bulk_csv_name(table);
    let qualified = qualified_table(table);
    writeln!(
        script,
        "-- Paquete de Columnia para SQL Server 2017 o posterior.\n\
         -- 1. Copia {csv} a una carpeta que el servicio de SQL Server pueda leer:\n\
         --    en el propio servidor o en una ruta compartida (\\\\equipo\\carpeta).\n\
         -- 2. Cambia la ruta de BULK INSERT si no es C:\\Columnia\\{csv}.\n\
         -- 3. Ejecuta este script en la base de destino (SSMS, Azure Data Studio o sqlcmd).\n\
         -- Crea la tabla {qualified} y la carga en una sola transacción. Si la tabla ya existe, se detiene sin tocarla."
    )
    .map_err(write_error)?;
    write_table_creation(&mut script, table, columns)?;
    writeln!(
        script,
        "BULK INSERT {qualified}\nFROM N'C:\\Columnia\\{csv}'\nWITH (\n    FORMAT = 'CSV',\n    FIRSTROW = 2,\n    FIELDQUOTE = '\"',\n    FIELDTERMINATOR = ',',\n    ROWTERMINATOR = '0x0a',\n    CODEPAGE = '65001',\n    KEEPNULLS,\n    TABLOCK\n);"
    )
    .map_err(write_error)?;
    writeln!(script, "COMMIT TRANSACTION;").map_err(write_error)?;
    match rows {
        Some(rows) => writeln!(
            script,
            "SELECT COUNT(*) AS filas_cargadas, {rows} AS filas_esperadas FROM {qualified};"
        ),
        None => writeln!(
            script,
            "SELECT COUNT(*) AS filas_cargadas FROM {qualified};"
        ),
    }
    .map_err(write_error)?;
    String::from_utf8(script).map_err(|_| "El script de carga no es UTF-8.".to_owned())
}

/// `LEEME.txt` of the BULK INSERT package.
pub(crate) fn bulk_readme(table: &str) -> String {
    let csv = bulk_csv_name(table);
    format!(
        "Paquete BULK INSERT de Columnia\r\n\
         ===============================\r\n\
         \r\n\
         Contiene:\r\n\
         - {csv}: los datos en CSV UTF-8, con encabezado. Una celda vacía se carga como NULL,\r\n\
           y BULK INSERT también carga como NULL un texto vacío; si necesitas distinguirlos,\r\n\
           exporta el script T-SQL.\r\n\
         - cargar.sql: crea la tabla [dbo].[{table}] y la carga con BULK INSERT.\r\n\
         \r\n\
         Pasos:\r\n\
         1. Copia {csv} a una carpeta que SQL Server pueda leer. BULK INSERT lee el archivo\r\n\
            desde el disco del servidor, no desde tu equipo: si el servidor es otro equipo,\r\n\
            usa una carpeta de ese servidor o una ruta compartida (\\\\equipo\\carpeta).\r\n\
         2. Abre cargar.sql y cambia la ruta C:\\Columnia\\{csv} si hace falta.\r\n\
         3. Ejecuta cargar.sql en la base de destino. Con sqlcmd, por ejemplo:\r\n\
            sqlcmd -S servidor -d base -E -i cargar.sql\r\n\
         \r\n\
         Necesitas SQL Server 2017 o posterior y permiso para BULK INSERT\r\n\
         (ADMINISTER BULK OPERATIONS o el rol bulkadmin). Si la tabla ya existe, el script\r\n\
         se detiene sin tocarla. Todo ocurre en una transacción: si algo falla, no queda nada a medias.\r\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<TsqlColumn> {
        vec![
            TsqlColumn {
                name: "id".into(),
                declaration: "BIGINT".into(),
            },
            TsqlColumn {
                name: "cliente".into(),
                declaration: "NVARCHAR(MAX)".into(),
            },
            TsqlColumn {
                name: "activo".into(),
                declaration: "BIT".into(),
            },
        ]
    }

    #[test]
    fn table_names_are_safe_identifiers_from_the_file_name() {
        assert_eq!(tsql_table_name("ventas limpias"), "ventas_limpias");
        assert_eq!(tsql_table_name("2026-ventas"), "t_2026_ventas");
        assert_eq!(tsql_table_name("año.ñ"), "a_o");
        assert_eq!(tsql_table_name("..."), "dataset");
        assert_eq!(tsql_table_name(&"x".repeat(300)).len(), 128);
    }

    #[test]
    fn types_follow_sql_server_without_losing_values() {
        assert_eq!(tsql_type_for_polars(&DataType::Boolean), "BIT");
        assert_eq!(tsql_type_for_polars(&DataType::Int64), "BIGINT");
        assert_eq!(tsql_type_for_polars(&DataType::UInt64), "DECIMAL(20,0)");
        assert_eq!(tsql_type_for_polars(&DataType::Float64), "FLOAT");
        assert_eq!(tsql_type_for_polars(&DataType::Date), "DATE");
        assert_eq!(tsql_type_for_polars(&DataType::String), "NVARCHAR(MAX)");
        assert_eq!(tsql_type_for_duckdb("DECIMAL(10,2)"), "DECIMAL(10,2)");
        assert_eq!(
            tsql_type_for_duckdb("TIMESTAMP WITH TIME ZONE"),
            "DATETIME2"
        );
        assert_eq!(tsql_type_for_duckdb("DATE"), "DATE");
        assert_eq!(tsql_type_for_duckdb("UBIGINT"), "DECIMAL(20,0)");
        assert_eq!(tsql_type_for_duckdb("VARCHAR"), "NVARCHAR(MAX)");
    }

    #[test]
    fn the_script_stops_on_an_existing_table_and_runs_in_one_transaction() {
        let mut writer = TsqlScriptWriter::begin(Vec::new(), "ventas", &columns()).unwrap();
        writer
            .push_row([
                TsqlValue::Number("1".into()),
                TsqlValue::Text("O'Brien Ñandú".into()),
                TsqlValue::Bit(true),
            ])
            .unwrap();
        writer
            .push_row([
                TsqlValue::Number("2".into()),
                TsqlValue::Null,
                TsqlValue::Bit(false),
            ])
            .unwrap();
        let script = String::from_utf8(writer.finish().unwrap()).unwrap();
        assert!(
            script.contains("IF OBJECT_ID(N'[dbo].[ventas]', N'U') IS NOT NULL\n    THROW 50000,")
        );
        assert!(!script.contains("DROP TABLE"), "nunca borra una tabla");
        assert!(
            !script.contains("\nGO"),
            "un solo lote: un error detiene todo"
        );
        let begin = script.find("BEGIN TRANSACTION;").unwrap();
        let create = script.find("CREATE TABLE [dbo].[ventas]").unwrap();
        let commit = script.find("COMMIT TRANSACTION;").unwrap();
        assert!(script.find("THROW").unwrap() < begin && begin < create && create < commit);
        assert!(script.contains("INSERT INTO [dbo].[ventas] ([id], [cliente], [activo]) VALUES\n    (1, N'O''Brien Ñandú', 1),\n    (2, NULL, 0);"));
        assert!(script.ends_with("-- 2 filas.\n"));
    }

    #[test]
    fn inserts_hold_at_most_one_thousand_rows() {
        let mut writer = TsqlScriptWriter::begin(Vec::new(), "t", &columns()[..1]).unwrap();
        for value in 0..2_500 {
            writer
                .push_row([TsqlValue::Number(value.to_string())])
                .unwrap();
        }
        let script = String::from_utf8(writer.finish().unwrap()).unwrap();
        assert_eq!(script.matches("INSERT INTO").count(), 3);
        assert!(script.contains("-- 2500 filas."));
    }

    #[test]
    fn identifiers_and_binary_values_are_escaped() {
        assert_eq!(quote_identifier("a]b"), "[a]]b]");
        assert_eq!(render_value(&TsqlValue::Binary(&[0x0f, 0xa0])), "0x0FA0");
        assert_eq!(render_value(&TsqlValue::Text("x'".into())), "N'x'''");
    }

    #[test]
    fn the_bulk_package_loads_utf8_csv_from_the_server_and_counts_rows() {
        let script = bulk_load_script("ventas", &columns(), Some(42)).unwrap();
        assert!(script.contains("BULK INSERT [dbo].[ventas]\nFROM N'C:\\Columnia\\ventas.csv'"));
        assert!(
            script.contains("FORMAT = 'CSV'")
                && script.contains("CODEPAGE = '65001'")
                && script.contains("FIRSTROW = 2")
        );
        assert!(script.contains("KEEPNULLS"));
        assert!(!script.contains("DROP TABLE"));
        assert!(script.contains("SELECT COUNT(*) AS filas_cargadas, 42 AS filas_esperadas"));
        let readme = bulk_readme("ventas");
        assert!(readme.contains("ventas.csv") && readme.contains("disco del servidor"));
    }
}
