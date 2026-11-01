//! T-SQL for SQL Server (2026-10-31): the «Script T-SQL» export and the
//! loading script of the «Paquete BULK INSERT». Both export paths (Polars in
//! memory, DuckDB by blocks) feed their rows through this module.
//!
//! A script creates a new table, adds rows to an existing one or updates an
//! existing one by key. It never drops or empties the destination: it checks
//! the table first and stops before touching anything, and everything runs in
//! one transaction, so a failure leaves nothing half done.

use std::borrow::Cow;
use std::io::Write;

use polars::prelude::DataType;
use serde::Deserialize;

/// SQL Server accepts at most 1,000 rows in one `INSERT … VALUES`.
pub(crate) const ROWS_PER_INSERT: usize = 1_000;
const DEFAULT_SCHEMA: &str = "dbo";
const MAX_IDENTIFIER_CHARS: usize = 128;
/// Where the rows wait before an append from the package or any update by
/// key; created and dropped by the same script.
const STAGING_TABLE: &str = "#columnia_carga";
/// Fitted text lengths; beyond the last one SQL Server needs `NVARCHAR(MAX)`.
const FITTED_TEXT_SIZES: [usize; 7] = [50, 100, 255, 500, 1_000, 2_000, 4_000];

/// What the script does with the destination table.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SqlServerLoadMode {
    /// Creates the table; stops if it already exists.
    #[default]
    Create,
    /// Adds the rows to a table that exists and has every column.
    Append,
    /// Updates the rows whose key exists and adds the others.
    Upsert,
}

/// How long the text columns of a new table are.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SqlServerTextSize {
    /// `NVARCHAR(n)` sized from the longest value, with a margin.
    #[default]
    Fitted,
    /// Always `NVARCHAR(MAX)`.
    Max,
}

/// The options Entregar sends for «Script T-SQL» and «Paquete BULK INSERT».
/// Empty names take the defaults: `dbo` and the name of the saved file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SqlServerScriptOptions {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    table: String,
    #[serde(default)]
    mode: SqlServerLoadMode,
    #[serde(default)]
    key_columns: Vec<String>,
    #[serde(default)]
    text_size: SqlServerTextSize,
}

/// The destination of a script, checked and with its defaults applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TsqlTarget {
    schema: String,
    table: String,
    mode: SqlServerLoadMode,
    key_columns: Vec<String>,
    fitted_text: bool,
}

fn checked_identifier(value: &str, what: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_IDENTIFIER_CHARS {
        return Err(format!(
            "El nombre de {what} supera los 128 caracteres que admite SQL Server."
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!(
            "El nombre de {what} no puede tener saltos de línea ni caracteres de control."
        ));
    }
    if value.starts_with('#') {
        return Err(format!(
            "El nombre de {what} no puede empezar por #: SQL Server lo trataría como temporal."
        ));
    }
    Ok(Some(value.to_owned()))
}

impl TsqlTarget {
    /// Applies the defaults and checks the names; `file_stem` names the table
    /// when the person left it empty.
    pub(crate) fn resolve(
        options: Option<&SqlServerScriptOptions>,
        file_stem: &str,
    ) -> Result<Self, String> {
        let defaults = SqlServerScriptOptions::default();
        let options = options.unwrap_or(&defaults);
        let schema = checked_identifier(&options.schema, "el esquema")?
            .unwrap_or_else(|| DEFAULT_SCHEMA.to_owned());
        let table = checked_identifier(&options.table, "la tabla")?
            .unwrap_or_else(|| tsql_table_name(file_stem));
        let key_columns = if options.mode == SqlServerLoadMode::Upsert {
            if options.key_columns.is_empty() {
                return Err(
                    "Elige al menos una columna clave para actualizar la tabla por clave."
                        .to_owned(),
                );
            }
            let mut seen = std::collections::HashSet::new();
            if let Some(repeated) = options
                .key_columns
                .iter()
                .find(|column| !seen.insert(column.as_str()))
            {
                return Err(format!("La columna clave «{repeated}» está repetida."));
            }
            options.key_columns.clone()
        } else {
            Vec::new()
        };
        Ok(Self {
            schema,
            table,
            mode: options.mode,
            key_columns,
            fitted_text: options.text_size == SqlServerTextSize::Fitted,
        })
    }

    /// Whether text columns are sized from the data.
    pub(crate) fn fitted_text(&self) -> bool {
        self.fitted_text
    }

    fn qualified(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier(&self.table)
        )
    }

    fn uses_staging(&self, bulk: bool) -> bool {
        match self.mode {
            SqlServerLoadMode::Create => false,
            // BULK INSERT maps by position, so an append goes through a
            // staging table and then by column name.
            SqlServerLoadMode::Append => bulk,
            SqlServerLoadMode::Upsert => true,
        }
    }

    fn check_columns(&self, columns: &[TsqlColumn]) -> Result<(), String> {
        if columns.is_empty() {
            return Err("SQL Server requiere al menos una columna.".to_owned());
        }
        if let Some(missing) = self
            .key_columns
            .iter()
            .find(|key| !columns.iter().any(|column| &column.name == *key))
        {
            return Err(format!(
                "La columna clave «{missing}» no está en los datos que se exportan."
            ));
        }
        Ok(())
    }

    /// What the script does, in one sentence for its comments.
    fn summary(&self) -> String {
        let qualified = self.qualified();
        match self.mode {
            SqlServerLoadMode::Create => format!(
                "Crea la tabla {qualified} y carga sus filas. Si la tabla ya existe, se detiene sin tocarla."
            ),
            SqlServerLoadMode::Append => format!(
                "Añade las filas a la tabla {qualified}, que ya debe existir con todas estas columnas; si falta algo, se detiene sin tocarla."
            ),
            SqlServerLoadMode::Upsert => format!(
                "Actualiza las filas de {qualified} cuya clave ({}) ya existe y añade las demás. Si una clave falta o se repite en los datos, se detiene sin tocar la tabla.",
                self.key_list()
            ),
        }
    }

    fn key_list(&self) -> String {
        self.key_columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A column of the table the script fills.
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

/// `NVARCHAR(n)` for a text column whose longest value has
/// `longest_utf8_bytes`: a quarter more, rounded up to a usual size, or
/// `NVARCHAR(MAX)` past 4,000. UTF-8 bytes are never fewer than the UTF-16
/// units SQL Server counts, so every value fits. An empty or all-null column
/// gets 255.
pub(crate) fn fitted_nvarchar(longest_utf8_bytes: Option<usize>) -> String {
    let Some(longest) = longest_utf8_bytes else {
        return "NVARCHAR(255)".to_owned();
    };
    let needed = longest.saturating_add(longest / 4);
    FITTED_TEXT_SIZES
        .iter()
        .find(|&&size| size >= needed)
        .map_or_else(
            || "NVARCHAR(MAX)".to_owned(),
            |size| format!("NVARCHAR({size})"),
        )
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
    name.chars().take(MAX_IDENTIFIER_CHARS).collect()
}

pub(crate) fn quote_identifier(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
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

fn column_list(columns: &[TsqlColumn], prefix: &str) -> String {
    columns
        .iter()
        .map(|column| format!("{prefix}{}", quote_identifier(&column.name)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_table_definition<W: Write>(
    output: &mut W,
    name: &str,
    columns: &[TsqlColumn],
    staging: bool,
) -> Result<(), String> {
    writeln!(output, "CREATE TABLE {name} (").map_err(write_error)?;
    for (index, column) in columns.iter().enumerate() {
        let separator = if index + 1 < columns.len() { "," } else { "" };
        // A temporary table takes tempdb's collation; text compared with the
        // destination must take the database's instead.
        let collation = if staging && column.declaration.starts_with("NVARCHAR") {
            " COLLATE DATABASE_DEFAULT"
        } else {
            ""
        };
        writeln!(
            output,
            "    {} {}{collation} NULL{separator}",
            quote_identifier(&column.name),
            column.declaration
        )
        .map_err(write_error)?;
    }
    writeln!(output, ");").map_err(write_error)
}

/// The checks that stop the script before it touches anything, the
/// transaction, and the tables the rows go to. Shared by both scripts.
fn write_preamble<W: Write>(
    output: &mut W,
    target: &TsqlTarget,
    columns: &[TsqlColumn],
    staging: bool,
) -> Result<(), String> {
    target.check_columns(columns)?;
    let qualified = target.qualified();
    writeln!(output, "SET NOCOUNT ON;").map_err(write_error)?;
    writeln!(output, "SET XACT_ABORT ON;").map_err(write_error)?;
    if target.mode == SqlServerLoadMode::Create {
        let message = format!(
            "La tabla {qualified} ya existe: cámbiale el nombre en este script, bórrala antes de ejecutarlo o exporta para añadir filas."
        );
        writeln!(
            output,
            "IF OBJECT_ID({}, N'U') IS NOT NULL\n    THROW 50000, {}, 1;",
            unicode_literal(&qualified),
            unicode_literal(&message)
        )
        .map_err(write_error)?;
    } else {
        let message = format!(
            "La tabla {qualified} no existe en esta base de datos: revisa el nombre y la base, o exporta para crear una tabla nueva."
        );
        writeln!(
            output,
            "IF OBJECT_ID({}, N'U') IS NULL\n    THROW 50000, {}, 1;",
            unicode_literal(&qualified),
            unicode_literal(&message)
        )
        .map_err(write_error)?;
        for column in columns {
            let message = format!(
                "La tabla {qualified} no tiene la columna {}: añádela a la tabla o quítala de la exportación.",
                quote_identifier(&column.name)
            );
            writeln!(
                output,
                "IF COL_LENGTH({}, {}) IS NULL\n    THROW 50000, {}, 1;",
                unicode_literal(&qualified),
                unicode_literal(&column.name),
                unicode_literal(&message)
            )
            .map_err(write_error)?;
        }
    }
    writeln!(output, "BEGIN TRANSACTION;").map_err(write_error)?;
    if target.mode == SqlServerLoadMode::Create {
        write_table_definition(output, &qualified, columns, false)?;
    }
    if staging {
        write_table_definition(output, STAGING_TABLE, columns, true)?;
    }
    Ok(())
}

/// Moves the staged rows into the destination. Declares `@nuevas` and, for an
/// update by key, `@actualizadas`.
fn write_staged_merge<W: Write>(
    output: &mut W,
    target: &TsqlTarget,
    columns: &[TsqlColumn],
) -> Result<(), String> {
    let qualified = target.qualified();
    let all = column_list(columns, "");
    let staged = column_list(columns, "s.");
    if target.mode != SqlServerLoadMode::Upsert {
        writeln!(
            output,
            "INSERT INTO {qualified} ({all})\nSELECT {staged}\nFROM {STAGING_TABLE} AS s;\nDECLARE @nuevas BIGINT = @@ROWCOUNT;"
        )
        .map_err(write_error)?;
        return Ok(());
    }
    let keys = target.key_list();
    let null_key = target
        .key_columns
        .iter()
        .map(|key| format!("{} IS NULL", quote_identifier(key)))
        .collect::<Vec<_>>()
        .join(" OR ");
    writeln!(
        output,
        "IF EXISTS (SELECT 1 FROM {STAGING_TABLE} WHERE {null_key})\n    THROW 50000, {}, 1;",
        unicode_literal(&format!(
            "Hay filas sin valor en la clave ({keys}): no se pueden actualizar por clave."
        ))
    )
    .map_err(write_error)?;
    writeln!(
        output,
        "IF EXISTS (SELECT 1 FROM {STAGING_TABLE} GROUP BY {keys} HAVING COUNT(*) > 1)\n    THROW 50000, {}, 1;",
        unicode_literal(&format!(
            "La clave ({keys}) se repite en los datos: cada fila debe tener una clave distinta."
        ))
    )
    .map_err(write_error)?;
    let matches = target
        .key_columns
        .iter()
        .map(|key| {
            let key = quote_identifier(key);
            format!("t.{key} = s.{key}")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    writeln!(output, "DECLARE @actualizadas BIGINT = 0;").map_err(write_error)?;
    let updated = columns
        .iter()
        .filter(|column| !target.key_columns.contains(&column.name))
        .map(|column| {
            let name = quote_identifier(&column.name);
            format!("t.{name} = s.{name}")
        })
        .collect::<Vec<_>>();
    // With every column in the key there is nothing to update.
    if !updated.is_empty() {
        writeln!(
            output,
            "UPDATE t\nSET {}\nFROM {qualified} AS t WITH (UPDLOCK, SERIALIZABLE)\nINNER JOIN {STAGING_TABLE} AS s ON {matches};\nSET @actualizadas = @@ROWCOUNT;",
            updated.join(",\n    ")
        )
        .map_err(write_error)?;
    }
    writeln!(
        output,
        "INSERT INTO {qualified} ({all})\nSELECT {staged}\nFROM {STAGING_TABLE} AS s\nWHERE NOT EXISTS (SELECT 1 FROM {qualified} AS t WITH (UPDLOCK, SERIALIZABLE) WHERE {matches});\nDECLARE @nuevas BIGINT = @@ROWCOUNT;"
    )
    .map_err(write_error)
}

/// Commits, drops the staging table and reports what changed.
fn write_staged_ending<W: Write>(
    output: &mut W,
    target: &TsqlTarget,
    expected_rows: Option<usize>,
) -> Result<(), String> {
    writeln!(output, "COMMIT TRANSACTION;").map_err(write_error)?;
    writeln!(output, "DROP TABLE {STAGING_TABLE};").map_err(write_error)?;
    let expected = expected_rows
        .map(|rows| format!(", {rows} AS filas_exportadas"))
        .unwrap_or_default();
    let counts = if target.mode == SqlServerLoadMode::Upsert {
        "@actualizadas AS filas_actualizadas, @nuevas AS filas_nuevas"
    } else {
        "@nuevas AS filas_nuevas"
    };
    writeln!(output, "SELECT {counts}{expected};").map_err(write_error)
}

/// Writes a script that loads every row, 1,000 rows per `INSERT`, in a single
/// batch and transaction.
pub(crate) struct TsqlScriptWriter<W: Write> {
    output: W,
    target: TsqlTarget,
    columns: Vec<TsqlColumn>,
    insert_prefix: String,
    rows_in_statement: usize,
    rows: usize,
}

impl<W: Write> TsqlScriptWriter<W> {
    pub(crate) fn begin(
        mut output: W,
        target: &TsqlTarget,
        columns: &[TsqlColumn],
    ) -> Result<Self, String> {
        target.check_columns(columns)?;
        writeln!(
            output,
            "-- Exportado por Columnia como script T-SQL para SQL Server 2016 o posterior.\n\
             -- Ejecútalo en la base de destino con SSMS, Azure Data Studio o sqlcmd.\n\
             -- {}\n\
             -- Todo ocurre en una sola transacción: si algo falla, no queda nada a medias.\n\
             -- Para tablas grandes usa el paquete BULK INSERT.",
            target.summary()
        )
        .map_err(write_error)?;
        let staging = target.uses_staging(false);
        write_preamble(&mut output, target, columns, staging)?;
        let destination = if staging {
            STAGING_TABLE.to_owned()
        } else {
            target.qualified()
        };
        Ok(Self {
            output,
            target: target.clone(),
            columns: columns.to_vec(),
            insert_prefix: format!(
                "INSERT INTO {destination} ({}) VALUES",
                column_list(columns, "")
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
        if self.target.uses_staging(false) {
            write_staged_merge(&mut self.output, &self.target, &self.columns)?;
            write_staged_ending(&mut self.output, &self.target, Some(self.rows))?;
        } else {
            writeln!(self.output, "COMMIT TRANSACTION;").map_err(write_error)?;
        }
        writeln!(self.output, "-- {} filas.", self.rows).map_err(write_error)?;
        self.output.flush().map_err(write_error)?;
        Ok(self.output)
    }
}

/// The CSV file name inside the BULK INSERT package, safe as a file name.
pub(crate) fn bulk_csv_name(target: &TsqlTarget) -> String {
    format!("{}.csv", tsql_table_name(&target.table))
}

/// `cargar.sql` of the BULK INSERT package. `BULK INSERT … FORMAT = 'CSV'`
/// needs SQL Server 2017 or later and reads the file from the server's disk.
pub(crate) fn bulk_load_script(
    target: &TsqlTarget,
    columns: &[TsqlColumn],
    rows: Option<usize>,
) -> Result<String, String> {
    let mut script = Vec::new();
    let csv = bulk_csv_name(target);
    writeln!(
        script,
        "-- Paquete de Columnia para SQL Server 2017 o posterior.\n\
         -- 1. Copia {csv} a una carpeta que el servicio de SQL Server pueda leer:\n\
         --    en el propio servidor o en una ruta compartida (\\\\equipo\\carpeta).\n\
         -- 2. Cambia la ruta de BULK INSERT si no es C:\\Columnia\\{csv}.\n\
         -- 3. Ejecuta este script en la base de destino (SSMS, Azure Data Studio o sqlcmd).\n\
         -- {}\n\
         -- Todo ocurre en una sola transacción: si algo falla, no queda nada a medias.",
        target.summary()
    )
    .map_err(write_error)?;
    let staging = target.uses_staging(true);
    write_preamble(&mut script, target, columns, staging)?;
    let qualified = target.qualified();
    let destination = if staging { STAGING_TABLE } else { &qualified };
    writeln!(
        script,
        "BULK INSERT {destination}\nFROM N'C:\\Columnia\\{csv}'\nWITH (\n    FORMAT = 'CSV',\n    FIRSTROW = 2,\n    FIELDQUOTE = '\"',\n    FIELDTERMINATOR = ',',\n    ROWTERMINATOR = '0x0a',\n    CODEPAGE = '65001',\n    KEEPNULLS,\n    TABLOCK\n);"
    )
    .map_err(write_error)?;
    if staging {
        write_staged_merge(&mut script, target, columns)?;
        write_staged_ending(&mut script, target, rows)?;
    } else {
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
    }
    String::from_utf8(script).map_err(|_| "El script de carga no es UTF-8.".to_owned())
}

/// `LEEME.txt` of the BULK INSERT package.
pub(crate) fn bulk_readme(target: &TsqlTarget) -> String {
    let csv = bulk_csv_name(target);
    format!(
        "Paquete BULK INSERT de Columnia\r\n\
         ===============================\r\n\
         \r\n\
         Contiene:\r\n\
         - {csv}: los datos en CSV UTF-8, con encabezado. Una celda vacía se carga como NULL,\r\n\
           y BULK INSERT también carga como NULL un texto vacío; si necesitas distinguirlos,\r\n\
           exporta el script T-SQL.\r\n\
         - cargar.sql: {}\r\n\
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
         (ADMINISTER BULK OPERATIONS o el rol bulkadmin). Todo ocurre en una transacción:\r\n\
         si algo falla, no queda nada a medias.\r\n",
        target.summary()
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
                declaration: "NVARCHAR(60)".into(),
            },
            TsqlColumn {
                name: "activo".into(),
                declaration: "BIT".into(),
            },
        ]
    }

    fn target(table: &str) -> TsqlTarget {
        TsqlTarget::resolve(None, table).unwrap()
    }

    fn options(mode: SqlServerLoadMode, keys: &[&str]) -> SqlServerScriptOptions {
        SqlServerScriptOptions {
            schema: "ventas".into(),
            table: "Pedidos Diarios".into(),
            mode,
            key_columns: keys.iter().map(|key| (*key).to_owned()).collect(),
            text_size: SqlServerTextSize::Fitted,
        }
    }

    fn script_for(target: &TsqlTarget) -> String {
        let mut writer = TsqlScriptWriter::begin(Vec::new(), target, &columns()).unwrap();
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
        String::from_utf8(writer.finish().unwrap()).unwrap()
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
    fn fitted_text_leaves_a_margin_and_falls_back_to_max() {
        assert_eq!(fitted_nvarchar(Some(2)), "NVARCHAR(50)");
        assert_eq!(fitted_nvarchar(Some(40)), "NVARCHAR(50)");
        assert_eq!(fitted_nvarchar(Some(41)), "NVARCHAR(100)");
        assert_eq!(fitted_nvarchar(Some(200)), "NVARCHAR(255)");
        assert_eq!(fitted_nvarchar(Some(3_200)), "NVARCHAR(4000)");
        assert_eq!(fitted_nvarchar(Some(3_201)), "NVARCHAR(MAX)");
        assert_eq!(fitted_nvarchar(None), "NVARCHAR(255)");
    }

    #[test]
    fn names_take_defaults_and_reject_what_sql_server_would_misread() {
        let default = target("ventas limpias");
        assert_eq!(default.qualified(), "[dbo].[ventas_limpias]");
        assert!(default.fitted_text());
        let chosen =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Append, &[])), "x").unwrap();
        assert_eq!(chosen.qualified(), "[ventas].[Pedidos Diarios]");
        assert_eq!(bulk_csv_name(&chosen), "Pedidos_Diarios.csv");
        let mut bad = options(SqlServerLoadMode::Create, &[]);
        bad.table = "#temporal".into();
        assert!(TsqlTarget::resolve(Some(&bad), "x").is_err());
        bad.table = "a\nb".into();
        assert!(TsqlTarget::resolve(Some(&bad), "x").is_err());
        bad.table = "x".repeat(129);
        assert!(TsqlTarget::resolve(Some(&bad), "x").is_err());
        let upsert_without_key = options(SqlServerLoadMode::Upsert, &[]);
        assert!(TsqlTarget::resolve(Some(&upsert_without_key), "x")
            .unwrap_err()
            .contains("columna clave"));
        let repeated = options(SqlServerLoadMode::Upsert, &["id", "id"]);
        assert!(TsqlTarget::resolve(Some(&repeated), "x").is_err());
        let unknown =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Upsert, &["codigo"])), "x")
                .unwrap();
        assert!(TsqlScriptWriter::begin(Vec::new(), &unknown, &columns())
            .err()
            .unwrap()
            .contains("«codigo»"));
    }

    #[test]
    fn the_script_stops_on_an_existing_table_and_runs_in_one_transaction() {
        let script = script_for(&target("ventas"));
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
        assert!(script.contains("    [cliente] NVARCHAR(60) NULL,"));
        assert!(script.contains("INSERT INTO [dbo].[ventas] ([id], [cliente], [activo]) VALUES\n    (1, N'O''Brien Ñandú', 1),\n    (2, NULL, 0);"));
        assert!(script.ends_with("-- 2 filas.\n"));
    }

    #[test]
    fn appending_checks_the_table_and_every_column_before_inserting() {
        let target =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Append, &[])), "x").unwrap();
        let script = script_for(&target);
        assert!(script.contains(
            "IF OBJECT_ID(N'[ventas].[Pedidos Diarios]', N'U') IS NULL\n    THROW 50000,"
        ));
        assert!(script.contains("IF COL_LENGTH(N'[ventas].[Pedidos Diarios]', N'cliente') IS NULL\n    THROW 50000, N'La tabla [ventas].[Pedidos Diarios] no tiene la columna [cliente]"));
        assert!(!script.contains("CREATE TABLE"), "no crea nada al añadir");
        assert!(!script.contains("DROP TABLE"));
        let checks = script.rfind("IF COL_LENGTH").unwrap();
        let begin = script.find("BEGIN TRANSACTION;").unwrap();
        let insert = script
            .find("INSERT INTO [ventas].[Pedidos Diarios] ([id], [cliente], [activo]) VALUES")
            .unwrap();
        assert!(checks < begin && begin < insert);
        assert!(script.contains("COMMIT TRANSACTION;\n-- 2 filas."));
    }

    #[test]
    fn updating_by_key_stages_checks_the_key_and_reports_counts() {
        let target =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Upsert, &["id"])), "x").unwrap();
        let script = script_for(&target);
        let qualified = "[ventas].[Pedidos Diarios]";
        assert!(script.contains("CREATE TABLE #columnia_carga (\n    [id] BIGINT NULL,\n    [cliente] NVARCHAR(60) COLLATE DATABASE_DEFAULT NULL,"));
        assert!(script.contains("INSERT INTO #columnia_carga ([id], [cliente], [activo]) VALUES"));
        assert!(script.contains("IF EXISTS (SELECT 1 FROM #columnia_carga WHERE [id] IS NULL)"));
        assert!(script.contains(
            "IF EXISTS (SELECT 1 FROM #columnia_carga GROUP BY [id] HAVING COUNT(*) > 1)"
        ));
        assert!(script.contains(&format!("UPDATE t\nSET t.[cliente] = s.[cliente],\n    t.[activo] = s.[activo]\nFROM {qualified} AS t WITH (UPDLOCK, SERIALIZABLE)\nINNER JOIN #columnia_carga AS s ON t.[id] = s.[id];")));
        assert!(script.contains(&format!("INSERT INTO {qualified} ([id], [cliente], [activo])\nSELECT s.[id], s.[cliente], s.[activo]\nFROM #columnia_carga AS s\nWHERE NOT EXISTS (SELECT 1 FROM {qualified} AS t WITH (UPDLOCK, SERIALIZABLE) WHERE t.[id] = s.[id]);")));
        assert!(!script.contains(&format!("DROP TABLE {qualified}")));
        let commit = script.find("COMMIT TRANSACTION;").unwrap();
        let drop = script.find("DROP TABLE #columnia_carga;").unwrap();
        assert!(script.find("UPDATE t").unwrap() < commit && commit < drop);
        assert!(script.contains(
            "SELECT @actualizadas AS filas_actualizadas, @nuevas AS filas_nuevas, 2 AS filas_exportadas;"
        ));
    }

    #[test]
    fn a_key_made_of_every_column_only_inserts_the_new_rows() {
        let target = TsqlTarget::resolve(
            Some(&options(
                SqlServerLoadMode::Upsert,
                &["id", "cliente", "activo"],
            )),
            "x",
        )
        .unwrap();
        let script = script_for(&target);
        assert!(!script.contains("UPDATE t"));
        assert!(script.contains("WHERE NOT EXISTS"));
        assert!(script
            .contains("t.[id] = s.[id] AND t.[cliente] = s.[cliente] AND t.[activo] = s.[activo]"));
    }

    #[test]
    fn inserts_hold_at_most_one_thousand_rows() {
        let mut writer =
            TsqlScriptWriter::begin(Vec::new(), &target("t"), &columns()[..1]).unwrap();
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
        let script = bulk_load_script(&target("ventas"), &columns(), Some(42)).unwrap();
        assert!(script.contains("BULK INSERT [dbo].[ventas]\nFROM N'C:\\Columnia\\ventas.csv'"));
        assert!(
            script.contains("FORMAT = 'CSV'")
                && script.contains("CODEPAGE = '65001'")
                && script.contains("FIRSTROW = 2")
        );
        assert!(script.contains("KEEPNULLS"));
        assert!(!script.contains("DROP TABLE"));
        assert!(script.contains("SELECT COUNT(*) AS filas_cargadas, 42 AS filas_esperadas"));
        let readme = bulk_readme(&target("ventas"));
        assert!(readme.contains("ventas.csv") && readme.contains("disco del servidor"));
        assert!(readme.contains("Crea la tabla [dbo].[ventas]"));
    }

    #[test]
    fn the_bulk_package_appends_by_column_name_through_staging() {
        let target =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Append, &[])), "x").unwrap();
        let script = bulk_load_script(&target, &columns(), Some(42)).unwrap();
        assert!(script
            .contains("BULK INSERT #columnia_carga\nFROM N'C:\\Columnia\\Pedidos_Diarios.csv'"));
        assert!(script.contains("INSERT INTO [ventas].[Pedidos Diarios] ([id], [cliente], [activo])\nSELECT s.[id], s.[cliente], s.[activo]\nFROM #columnia_carga AS s;"));
        assert!(script.contains("SELECT @nuevas AS filas_nuevas, 42 AS filas_exportadas;"));
        assert!(!script.contains("CREATE TABLE [ventas]"));
        assert!(
            bulk_readme(&target).contains("Añade las filas a la tabla [ventas].[Pedidos Diarios]")
        );
    }

    #[test]
    fn the_bulk_package_updates_by_key() {
        let target =
            TsqlTarget::resolve(Some(&options(SqlServerLoadMode::Upsert, &["id"])), "x").unwrap();
        let script = bulk_load_script(&target, &columns(), None).unwrap();
        let bulk = script.find("BULK INSERT #columnia_carga").unwrap();
        let checks = script.find("GROUP BY [id] HAVING COUNT(*) > 1").unwrap();
        let update = script.find("UPDATE t").unwrap();
        assert!(bulk < checks && checks < update);
        assert!(script
            .ends_with("SELECT @actualizadas AS filas_actualizadas, @nuevas AS filas_nuevas;\n"));
    }

    #[test]
    fn options_arrive_from_the_interface_in_camel_case() {
        let parsed: SqlServerScriptOptions = serde_json::from_str(
            r#"{"schema":"ventas","table":"t","mode":"upsert","keyColumns":["id"],"textSize":"max"}"#,
        )
        .unwrap();
        let target = TsqlTarget::resolve(Some(&parsed), "x").unwrap();
        assert_eq!(target.mode, SqlServerLoadMode::Upsert);
        assert!(!target.fitted_text());
        assert!(serde_json::from_str::<SqlServerScriptOptions>(r#"{"other":1}"#).is_err());
        let empty: SqlServerScriptOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, SqlServerScriptOptions::default());
    }
}
