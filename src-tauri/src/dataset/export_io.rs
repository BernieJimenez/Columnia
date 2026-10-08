use super::*;

pub(super) fn path_with_extension(mut path: PathBuf, format: ExportFormat) -> PathBuf {
    let has_expected_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()));
    if !has_expected_extension {
        path.set_extension(format.extension());
    }
    path
}

const EAGER_EXPORT_BATCH_ROWS: usize = 8_192;

#[cfg(test)]
pub(super) fn frame_for_export(
    frame: &DataFrame,
    format: ExportFormat,
) -> Result<DataFrame, String> {
    match format {
        // CSV suele abrirse en hojas de cálculo: una comilla inicial fuerza texto y evita
        // ejecutar celdas controladas por datos. Parquet conserva los valores originales.
        ExportFormat::Csv | ExportFormat::CsvExcel => csv_formula_safe_frame(frame),
        ExportFormat::Json => Ok(frame.clone()),
        ExportFormat::Parquet => Ok(frame.clone()),
        ExportFormat::Sql => Ok(frame.clone()),
        ExportFormat::Excel => Ok(frame.clone()),
        ExportFormat::Sqlite => Ok(frame.clone()),
        ExportFormat::Bundle => csv_formula_safe_frame(frame),
    }
}

pub(super) fn for_each_export_batch<C, F>(
    frame: &DataFrame,
    is_cancelled: &C,
    mut write_batch: F,
) -> Result<(), String>
where
    C: Fn() -> bool + ?Sized,
    F: FnMut(&DataFrame) -> Result<(), String>,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut offset = 0_usize;
    while offset < frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        let batch_rows = EAGER_EXPORT_BATCH_ROWS.min(frame.height() - offset);
        let row_offset = i64::try_from(offset)
            .map_err(|_| "El dataset excede el rango de exportación por bloques.".to_owned())?;
        let batch = frame.slice(row_offset, batch_rows);
        write_batch(&batch)?;
        offset = offset.saturating_add(batch_rows);
        ensure_not_cancelled(is_cancelled())?;
    }
    Ok(())
}

/// The byte order mark that tells Excel the CSV is UTF-8 (UX-03).
pub(super) const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

pub(super) fn write_csv_frame_with_cancel<C>(
    frame: &DataFrame,
    output: &mut File,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + ?Sized,
{
    write_delimited_frame_with_cancel(frame, output, false, is_cancelled).map(|_| ())
}

/// CSV, or with `excel` the «CSV para Excel» variant: UTF-8 with BOM and
/// semicolons, which Excel opens in columns where the list separator is `;`
/// (UX-03).
pub(super) fn write_delimited_frame_with_cancel<C>(
    frame: &DataFrame,
    output: &mut File,
    excel: bool,
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    let frame = &csv_formula_safe_column_names(frame)?;
    if excel {
        output
            .write_all(UTF8_BOM)
            .map_err(|error| format!("No se pudo escribir el CSV: {error}"))?;
    }
    let batch_size = std::num::NonZeroUsize::new(EAGER_EXPORT_BATCH_ROWS)
        .expect("el tamaño de lote CSV debe ser mayor que cero");
    let mut writer = CsvWriter::new(&mut *output)
        .with_separator(if excel { b';' } else { b',' })
        .with_decimal_comma(excel)
        .with_batch_size(batch_size)
        .batched(frame.schema().as_ref())
        .map_err(|error| format!("No se pudo preparar el escritor CSV: {error}"))?;
    let mut protected_cells = 0_usize;
    for_each_export_batch(frame, is_cancelled, |batch| {
        let (mut safe_batch, batch_protected) =
            csv_formula_safe_frame_with_cancel(batch, is_cancelled)?;
        protected_cells += batch_protected;
        safe_batch.align_chunks_par();
        ensure_not_cancelled(is_cancelled())?;
        writer
            .write_batch(&safe_batch)
            .map_err(|error| format!("No se pudo escribir el CSV: {error}"))
    })?;
    ensure_not_cancelled(is_cancelled())?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar el CSV: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(protected_cells)
}

pub(super) fn write_json_frame_with_cancel<C>(
    frame: &DataFrame,
    output: &mut File,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    output
        .write_all(b"[")
        .map_err(|error| format!("No se pudo iniciar el JSON: {error}"))?;
    let mut wrote_row = false;
    for_each_export_batch(frame, is_cancelled, |batch| {
        let mut batch = batch.clone();
        let mut encoded = Vec::new();
        JsonWriter::new(&mut encoded)
            .with_json_format(JsonFormat::Json)
            .finish(&mut batch)
            .map_err(|error| format!("No se pudo escribir el JSON: {error}"))?;
        let rows = encoded
            .strip_prefix(b"[")
            .and_then(|value| value.strip_suffix(b"]"))
            .ok_or_else(|| "Polars no devolvió un bloque JSON válido.".to_owned())?;
        if !rows.is_empty() {
            ensure_not_cancelled(is_cancelled())?;
            if wrote_row {
                output
                    .write_all(b",")
                    .map_err(|error| format!("No se pudo separar el lote JSON: {error}"))?;
            }
            output
                .write_all(rows)
                .map_err(|error| format!("No se pudo escribir el lote JSON: {error}"))?;
            wrote_row = true;
        }
        Ok(())
    })?;
    ensure_not_cancelled(is_cancelled())?;
    output
        .write_all(b"]")
        .map_err(|error| format!("No se pudo cerrar el JSON: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(())
}

pub(super) fn write_parquet_frame_with_cancel<C>(
    frame: &DataFrame,
    output: &mut File,
    is_cancelled: &C,
) -> Result<(), String>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut writer = ParquetWriter::new(&mut *output)
        .batched(frame.schema().as_ref())
        .map_err(|error| format!("No se pudo preparar el escritor Parquet: {error}"))?;
    for_each_export_batch(frame, is_cancelled, |batch| {
        let mut aligned_batch = batch.clone();
        aligned_batch.align_chunks_par();
        writer
            .write_batch(&aligned_batch)
            .map_err(|error| format!("No se pudo escribir Parquet: {error}"))
    })?;
    ensure_not_cancelled(is_cancelled())?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar Parquet: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(())
}

pub(super) fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(super) fn sql_type(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "BOOLEAN",
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32 => "BIGINT",
        // A u64 can exceed BIGINT (DAT-11).
        DataType::UInt64 => "NUMERIC(20,0)",
        DataType::Float32 | DataType::Float64 => "DOUBLE",
        DataType::Date => "DATE",
        DataType::Datetime(_, _) => "TIMESTAMP",
        _ => "TEXT",
    }
}

pub(super) fn sql_value(value: AnyValue<'_>) -> Result<String, String> {
    match value {
        AnyValue::Null => Ok("NULL".to_owned()),
        AnyValue::Boolean(value) => Ok(if value { "TRUE" } else { "FALSE" }.to_owned()),
        AnyValue::Int8(value) => Ok(value.to_string()),
        AnyValue::Int16(value) => Ok(value.to_string()),
        AnyValue::Int32(value) => Ok(value.to_string()),
        AnyValue::Int64(value) => Ok(value.to_string()),
        AnyValue::UInt8(value) => Ok(value.to_string()),
        AnyValue::UInt16(value) => Ok(value.to_string()),
        AnyValue::UInt32(value) => Ok(value.to_string()),
        AnyValue::UInt64(value) => Ok(value.to_string()),
        AnyValue::Float32(value) if value.is_finite() => Ok(value.to_string()),
        AnyValue::Float64(value) if value.is_finite() => Ok(value.to_string()),
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            Err("SQL no puede representar valores numéricos no finitos.".to_owned())
        }
        AnyValue::String(value) => Ok(sql_string_literal(value)),
        AnyValue::StringOwned(value) => Ok(sql_string_literal(value.as_str())),
        value => Ok(sql_string_literal(&value.to_string())),
    }
}

pub(super) fn write_sql_script<F, C>(
    frame: &DataFrame,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    const TABLE: &str = "dataset";
    if frame.width() == 0 {
        return Err("SQL requiere al menos una columna para crear la tabla.".to_owned());
    }
    let table = sql_identifier(TABLE);
    // COD-15: every fragment went straight to the file (several system calls
    // per cell); a 64 KiB buffer took 1 M rows x 10 columns from 104 s to
    // 2.8 s, same bytes.
    let output = &mut std::io::BufWriter::with_capacity(64 * 1024, output);
    writeln!(output, "{}", crate::duckdb_query::SQL_SCRIPT_HEADER)
        .map_err(|error| format!("No se pudo escribir el encabezado SQL: {error}"))?;
    writeln!(output, "BEGIN TRANSACTION;")
        .map_err(|error| format!("No se pudo escribir el inicio SQL: {error}"))?;
    writeln!(output, "DROP TABLE IF EXISTS {table};")
        .map_err(|error| format!("No se pudo escribir la limpieza SQL: {error}"))?;
    write!(output, "CREATE TABLE {table} (")
        .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
    for (index, column) in frame.columns().iter().enumerate() {
        if index > 0 {
            write!(output, ", ")
                .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
        }
        write!(
            output,
            "{} {}",
            sql_identifier(column.name().as_str()),
            sql_type(column.dtype())
        )
        .map_err(|error| format!("No se pudo escribir el esquema SQL: {error}"))?;
    }
    writeln!(output, ");").map_err(|error| format!("No se pudo cerrar el esquema SQL: {error}"))?;

    let columns = frame.columns();
    for row_index in 0..frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        write!(output, "INSERT INTO {table} (")
            .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            write!(output, "{}", sql_identifier(column.name().as_str()))
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        }
        write!(output, ") VALUES (")
            .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")
                    .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
            }
            let value = column.get(row_index).map_err(|error| {
                format!("No se pudo leer la fila {row_index} para SQL: {error}")
            })?;
            write!(output, "{}", sql_value(value)?)
                .map_err(|error| format!("No se pudo escribir una fila SQL: {error}"))?;
        }
        writeln!(output, ");")
            .map_err(|error| format!("No se pudo cerrar una fila SQL: {error}"))?;
        let percent = if frame.height() == 0 {
            85
        } else {
            30 + (((row_index + 1) * 55) / frame.height()) as u8
        };
        report(percent);
    }
    ensure_not_cancelled(is_cancelled())?;
    writeln!(output, "COMMIT;")
        .map_err(|error| format!("No se pudo escribir el cierre SQL: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("No se pudo escribir el cierre SQL: {error}"))?;
    report(85);
    Ok(())
}

/// Excel limits (RV19 / FUN-05): a sheet beyond them does not open.
const EXCEL_MAX_DATA_ROWS: usize = 1_048_575;
const EXCEL_MAX_CELL_CHARS: usize = 32_767;
const EXCEL_MAX_COLUMNS: usize = 16_384;

/// XML 1.0 forbids most control characters; one of them made the whole sheet
/// unreadable. They become U+FFFD so the gap stays visible.
fn xml_forbidden(character: char) -> bool {
    (character < ' ' && !matches!(character, '\t' | '\n' | '\r'))
        || matches!(character, '\u{FFFE}' | '\u{FFFF}')
}

/// Whether Excel receives this text changed: the export reports how many
/// cells lost a control character (RV19 / FUN-05).
pub(super) fn excel_text_needs_replacement(value: &str) -> bool {
    value.chars().any(xml_forbidden)
}

fn excel_value_needs_replacement(value: &AnyValue<'_>) -> bool {
    match value {
        AnyValue::String(text) => excel_text_needs_replacement(text),
        AnyValue::StringOwned(text) => excel_text_needs_replacement(text.as_str()),
        _ => false,
    }
}

fn xml_safe_text(value: &str) -> std::borrow::Cow<'_, str> {
    let forbidden = xml_forbidden;
    if value.chars().any(forbidden) {
        std::borrow::Cow::Owned(
            value
                .chars()
                .map(|character| {
                    if forbidden(character) {
                        '\u{FFFD}'
                    } else {
                        character
                    }
                })
                .collect(),
        )
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

fn ensure_excel_dimensions(data_rows: usize, columns: usize) -> Result<(), String> {
    // FUN-68: past column XFD Excel declares the whole workbook damaged.
    if columns > EXCEL_MAX_COLUMNS {
        return Err(format!(
            "Excel admite como máximo 16.384 columnas por hoja y este dataset tiene {columns}. Exporta a CSV o Parquet, o quita columnas antes de exportar."
        ));
    }
    if data_rows > EXCEL_MAX_DATA_ROWS {
        return Err(format!(
            "Excel admite como máximo 1.048.575 filas de datos por hoja y este dataset tiene {data_rows}. Exporta a CSV o Parquet para conservar todas las filas."
        ));
    }
    Ok(())
}

fn excel_text_cell(reference: &str, value: &str) -> Result<String, String> {
    let length = value.chars().count();
    if length > EXCEL_MAX_CELL_CHARS {
        return Err(format!(
            "La celda {reference} tiene {length} caracteres y Excel admite como máximo 32.767 por celda. Exporta a CSV o Parquet para conservar el texto completo."
        ));
    }
    Ok(format!(
        "<c r=\"{reference}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
        xml_escape(value)
    ))
}

pub(super) fn xml_escape(value: &str) -> String {
    xml_safe_text(value)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Appends what [`xml_escape`] returns, in one pass and without allocating.
fn push_xml_escaped(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            character if xml_forbidden(character) => output.push('\u{FFFD}'),
            character => output.push(character),
        }
    }
}

pub(super) fn xlsx_column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = (index / 26) - 1;
    }
    name
}

pub(super) fn xlsx_cell(
    column_index: usize,
    row_index: usize,
    value: AnyValue<'_>,
) -> Result<String, String> {
    let mut cell = String::new();
    push_xlsx_cell(
        &mut cell,
        &xlsx_column_name(column_index),
        row_index + 1,
        value,
    )?;
    Ok(cell)
}

/// Appends the same XML as [`xlsx_cell`] to `output` without allocating per
/// cell; `column_name` is the precomputed column letter and `row_number` the
/// 1-based sheet row.
/// Largest integer an Excel number (an IEEE double) holds exactly: 2^53.
const EXCEL_EXACT_INTEGER: u64 = 1 << 53;

/// PROD-18: styles of `styles.xml` for dates (built-in short date, which Excel
/// shows in the regional format) and for dates with time.
const XLSX_DATE_STYLE: u8 = 1;
const XLSX_DATETIME_STYLE: u8 = 2;
/// Serial of 1970-01-01 in Excel's 1900 date system.
const EXCEL_UNIX_EPOCH_SERIAL: i64 = 25_569;
/// Excel counts a 1900-02-29 that never existed, so serials before
/// 1900-03-01 (61) are a day off; 9999-12-31 is the last date it accepts.
const EXCEL_FIRST_EXACT_SERIAL: i64 = 61;
const EXCEL_LAST_SERIAL: i64 = 2_958_465;

/// The Excel serial of a date given as days since 1970-01-01, when Excel shows
/// it as the same day.
fn excel_day_serial(days_since_epoch: i64) -> Option<i64> {
    let serial = days_since_epoch.checked_add(EXCEL_UNIX_EPOCH_SERIAL)?;
    (EXCEL_FIRST_EXACT_SERIAL..=EXCEL_LAST_SERIAL)
        .contains(&serial)
        .then_some(serial)
}

/// The Excel serial (days plus the fraction of the day) of a moment given in
/// `unit` since 1970-01-01 without a time zone.
fn excel_datetime_serial(value: i64, unit: TimeUnit) -> Option<f64> {
    let per_day: i64 = match unit {
        TimeUnit::Nanoseconds => 86_400_000_000_000,
        TimeUnit::Microseconds => 86_400_000_000,
        TimeUnit::Milliseconds => 86_400_000,
    };
    let days = value.div_euclid(per_day);
    let within_day = value.rem_euclid(per_day);
    let serial = excel_day_serial(days)?;
    Some(serial as f64 + within_day as f64 / per_day as f64)
}

fn push_excel_number_cell(output: &mut String, style: u8, serial: impl std::fmt::Display) {
    use std::fmt::Write as _;
    let _ = write!(output, " s=\"{style}\" t=\"n\"><v>{serial}</v></c>");
}

fn push_xlsx_cell(
    output: &mut String,
    column_name: &str,
    row_number: usize,
    value: AnyValue<'_>,
) -> Result<(), String> {
    use std::fmt::Write as _;
    let _ = write!(output, "<c r=\"{column_name}{row_number}\"");
    let _ = match value {
        AnyValue::Null => write!(output, "/>"),
        AnyValue::Boolean(value) => {
            write!(output, " t=\"b\"><v>{}</v></c>", if value { 1 } else { 0 })
        }
        AnyValue::Int8(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::Int16(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::Int32(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        // Excel keeps 15 significant digits: a larger integer goes as text,
        // as the reader does, so an ID keeps every digit (FUN-29).
        AnyValue::Int64(value) if value.unsigned_abs() > EXCEL_EXACT_INTEGER => {
            return push_excel_text_cell(output, column_name, row_number, &value.to_string());
        }
        AnyValue::UInt64(value) if value > EXCEL_EXACT_INTEGER => {
            return push_excel_text_cell(output, column_name, row_number, &value.to_string());
        }
        AnyValue::Int64(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt8(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt16(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt32(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::UInt64(value) => write!(output, " t=\"n\"><v>{value}</v></c>"),
        AnyValue::Float32(value) if value.is_finite() => {
            write!(output, " t=\"n\"><v>{value}</v></c>")
        }
        AnyValue::Float64(value) if value.is_finite() => {
            write!(output, " t=\"n\"><v>{value}</v></c>")
        }
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            return Err("Excel no puede representar valores numéricos no finitos.".to_owned());
        }
        AnyValue::Date(days) => match excel_day_serial(i64::from(days)) {
            Some(serial) => {
                push_excel_number_cell(output, XLSX_DATE_STYLE, serial);
                Ok(())
            }
            None => {
                return push_excel_text_cell(output, column_name, row_number, &value.to_string())
            }
        },
        AnyValue::Datetime(moment, unit, None) => match excel_datetime_serial(moment, unit) {
            Some(serial) => {
                push_excel_number_cell(output, XLSX_DATETIME_STYLE, serial);
                Ok(())
            }
            None => {
                return push_excel_text_cell(output, column_name, row_number, &value.to_string())
            }
        },
        AnyValue::String(value) => {
            return push_excel_text_cell(output, column_name, row_number, value);
        }
        AnyValue::StringOwned(value) => {
            return push_excel_text_cell(output, column_name, row_number, value.as_str());
        }
        value => {
            return push_excel_text_cell(output, column_name, row_number, &value.to_string());
        }
    };
    Ok(())
}

fn push_excel_text_cell(
    output: &mut String,
    column_name: &str,
    row_number: usize,
    value: &str,
) -> Result<(), String> {
    let length = value.chars().count();
    if length > EXCEL_MAX_CELL_CHARS {
        return Err(format!(
            "La celda {column_name}{row_number} tiene {length} caracteres y Excel admite como máximo 32.767 por celda. Exporta a CSV o Parquet para conservar el texto completo."
        ));
    }
    output.push_str(" t=\"inlineStr\"><is><t xml:space=\"preserve\">");
    push_xml_escaped(output, value);
    output.push_str("</t></is></c>");
    Ok(())
}

pub(super) fn xlsx_source_cell(
    column_index: usize,
    row_index: usize,
    data_type: &DataType,
    value: Option<&str>,
) -> Result<String, String> {
    let reference = format!("{}{}", xlsx_column_name(column_index), row_index + 1);
    let Some(value) = value else {
        return Ok(format!("<c r=\"{reference}\"/>"));
    };
    let numeric = matches!(
        data_type,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
    );
    if numeric {
        if value
            .parse::<i128>()
            .is_ok_and(|number| number.unsigned_abs() <= u128::from(EXCEL_EXACT_INTEGER))
        {
            return Ok(format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"));
        }
    } else if matches!(data_type, DataType::Float32 | DataType::Float64) {
        let number = value.parse::<f64>().map_err(|_| {
            "Excel no puede representar un valor de punto flotante source-backed inválido."
                .to_owned()
        })?;
        if !number.is_finite() {
            return Err("Excel no puede representar valores numéricos no finitos.".to_owned());
        }
        return Ok(format!("<c r=\"{reference}\" t=\"n\"><v>{value}</v></c>"));
    } else if matches!(data_type, DataType::Date) {
        use chrono::Datelike as _;
        let serial = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .and_then(|date| excel_day_serial(i64::from(date.num_days_from_ce() - 719_163)));
        if let Some(serial) = serial {
            let mut cell = format!("<c r=\"{reference}\"");
            push_excel_number_cell(&mut cell, XLSX_DATE_STYLE, serial);
            return Ok(cell);
        }
    } else if matches!(data_type, DataType::Datetime(_, None)) {
        let serial = ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"]
            .iter()
            .find_map(|format| chrono::NaiveDateTime::parse_from_str(value, format).ok())
            .and_then(|moment| {
                excel_datetime_serial(moment.and_utc().timestamp_micros(), TimeUnit::Microseconds)
            });
        if let Some(serial) = serial {
            let mut cell = format!("<c r=\"{reference}\"");
            push_excel_number_cell(&mut cell, XLSX_DATETIME_STYLE, serial);
            return Ok(cell);
        }
    } else if matches!(data_type, DataType::Boolean) {
        if ["true", "1"]
            .iter()
            .any(|candidate| value.eq_ignore_ascii_case(candidate))
        {
            return Ok(format!("<c r=\"{reference}\" t=\"b\"><v>1</v></c>"));
        }
        if ["false", "0"]
            .iter()
            .any(|candidate| value.eq_ignore_ascii_case(candidate))
        {
            return Ok(format!("<c r=\"{reference}\" t=\"b\"><v>0</v></c>"));
        }
    }
    excel_text_cell(&reference, value)
}

/// ZIP options for an entry of about `bytes`. Entries that can pass 4 GiB
/// need ZIP64, or the crate refuses them at the end of a long export (ARQ-05).
pub(super) fn zip_entry_options(bytes: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(bytes >= ZIP64_THRESHOLD_BYTES)
}

/// Below `u32::MAX` with room for an estimate that falls short.
const ZIP64_THRESHOLD_BYTES: u64 = 3 * 1024 * 1024 * 1024;

/// Generous size of the sheet XML: about 64 bytes per cell.
fn estimated_sheet_bytes(rows: usize, columns: usize) -> u64 {
    (rows as u64)
        .saturating_add(1)
        .saturating_mul(columns as u64)
        .saturating_mul(64)
}

pub(super) fn write_source_backed_xlsx<F, C>(
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    schema: &DataFrame,
    row_count: usize,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<usize, String>
where
    F: FnMut(u8),
    C: Fn() -> bool + Send + 'static,
{
    if schema.width() == 0 {
        return Err("Excel requiere al menos una columna.".to_owned());
    }
    ensure_excel_dimensions(row_count, schema.width())?;
    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
    const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="dataset" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/><xf numFmtId="22" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/></cellXfs></styleSheet>"#;

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut archive = ZipWriter::new(output);
    for (name, contents) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("xl/workbook.xml", WORKBOOK),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS),
        ("xl/styles.xml", STYLES),
    ] {
        archive
            .start_file(name, options)
            .map_err(|error| format!("No se pudo preparar el libro Excel: {error}"))?;
        archive
            .write_all(contents.as_bytes())
            .map_err(|error| format!("No se pudo escribir el libro Excel: {error}"))?;
    }
    archive
        .start_file(
            "xl/worksheets/sheet1.xml",
            zip_entry_options(estimated_sheet_bytes(row_count, schema.width())),
        )
        .map_err(|error| format!("No se pudo preparar la hoja Excel: {error}"))?;
    archive
        .write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#)
        .map_err(|error| format!("No se pudo escribir la hoja Excel: {error}"))?;
    let last_cell = format!(
        "<dimension ref=\"A1:{}{}\"/><sheetData>",
        xlsx_column_name(schema.width() - 1),
        row_count + 1
    );
    archive
        .write_all(last_cell.as_bytes())
        .map_err(|error| format!("No se pudo escribir las dimensiones Excel: {error}"))?;
    archive
        .write_all(b"<row r=\"1\">")
        .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    for (column_index, column) in schema.columns().iter().enumerate() {
        let header = xlsx_cell(column_index, 0, AnyValue::String(column.name().as_str()))?;
        archive
            .write_all(header.as_bytes())
            .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    }
    archive
        .write_all(b"</row>")
        .map_err(|error| format!("No se pudo cerrar el encabezado Excel: {error}"))?;

    let mut streamed_rows = 0_usize;
    let mut replaced_control_cell_count = 0_usize;
    let streamed_columns = crate::duckdb_query::stream_file_rows(
        source_path,
        source_format,
        |values| {
            if values.len() != schema.width() {
                return Err("La transmisión source-backed devolvió un ancho inesperado.".to_owned());
            }
            let row_index = streamed_rows;
            archive
                .write_all(format!("<row r=\"{}\">", row_index + 2).as_bytes())
                .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
            for (column_index, (column, value)) in
                schema.columns().iter().zip(values.iter()).enumerate()
            {
                replaced_control_cell_count +=
                    usize::from(value.as_deref().is_some_and(excel_text_needs_replacement));
                let cell = xlsx_source_cell(
                    column_index,
                    row_index + 1,
                    column.dtype(),
                    value.as_deref(),
                )?;
                archive
                    .write_all(cell.as_bytes())
                    .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
            }
            archive
                .write_all(b"</row>")
                .map_err(|error| format!("No se pudo cerrar una fila Excel: {error}"))?;
            streamed_rows = streamed_rows.saturating_add(1);
            let percent = if row_count == 0 {
                85
            } else {
                30 + (streamed_rows
                    .saturating_mul(55)
                    .checked_div(row_count)
                    .unwrap_or_default()) as u8
            };
            report(percent.min(85));
            Ok(())
        },
        is_cancelled,
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
            "La transmisión source-backed devolvió columnas distintas al esquema.".to_owned(),
        );
    }
    if streamed_rows != row_count {
        return Err(
            "El conteo del dataset source-backed cambió durante la exportación.".to_owned(),
        );
    }
    archive
        .write_all(b"</sheetData></worksheet>")
        .map_err(|error| format!("No se pudo cerrar la hoja Excel: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("No se pudo finalizar el libro Excel: {error}"))?;
    report(85);
    Ok(replaced_control_cell_count)
}

/// The source of a source-backed export, already validated.
pub(super) struct SourceBackedExportSource<'a> {
    pub(super) path: &'a Path,
    pub(super) extension: &'a str,
    pub(super) format: crate::duckdb_query::DuckDbFileFormat,
}

/// The steps every source-backed export shares (LIM-02): validate the source,
/// refuse to write over it, write into a temporary file next to the
/// destination, check that the source did not change and publish atomically.
/// `write` fills the temporary file and returns the replaced control cells.
pub(super) fn publish_source_backed_export<F, C, W>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    format: ExportFormat,
    mut report: F,
    is_cancelled: C,
    write: W,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
    W: FnOnce(
        &SourceBackedExportSource<'_>,
        &mut tempfile::NamedTempFile,
        &mut F,
        C,
    ) -> Result<usize, String>,
{
    ensure_not_cancelled(is_cancelled())?;
    let destination = canonicalize_write_destination(destination, "la exportación")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?;
    let (source_path, source_size, extension) = validate_dataset_file(source_path)?;
    if source_size != expected_file_size {
        return Err("El archivo source-backed cambió después de la validación.".to_owned());
    }
    ensure_destination_is_not_source(&destination, &[source_path.as_path()])?;
    let source_format = match extension.as_str() {
        "csv" | "tsv" | "txt" => crate::duckdb_query::DuckDbFileFormat::Delimited {
            delimiter: detect_delimiter(&source_path, &extension)?,
        },
        "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
        _ => {
            return Err(format!(
                "El formato source-backed no se puede exportar a {}.",
                format.label()
            ))
        }
    };
    report("Preparando archivo temporal", 10);
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo preparar la publicación temporal: {error}"))?;
    report("Escribiendo dataset", 25);
    let source = SourceBackedExportSource {
        path: &source_path,
        extension: &extension,
        format: source_format,
    };
    let replaced_control_cell_count =
        write(&source, &mut temporary, &mut report, is_cancelled.clone())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la exportación: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    let final_source_size = fs::metadata(&source_path)
        .map_err(|error| format!("No se pudieron verificar los metadatos source-backed: {error}"))?
        .len();
    if final_source_size != expected_file_size {
        return Err("El archivo source-backed cambió durante la exportación.".to_owned());
    }
    report("Publicando archivo completo", 90);
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar la exportación: {}", error.error))?;
    let file_size_bytes = fs::metadata(&destination)
        .map_err(|error| format!("No se pudo verificar la exportación: {error}"))?
        .len();
    report("Exportación lista", 100);
    Ok(ExportResult {
        file_name: destination
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("dataset.{}", format.extension())),
        file_size_bytes,
        format: format.label(),
        protected_column_count: 0,
        protected_columns: Vec::new(),
        replaced_control_cell_count,
        row_count: None,
        folder_name: export_folder_name(&destination),
        formula_protected_cell_count: None,
    })
}

/// Lets DuckDB write the CSV, JSON, SQL or Parquet export and makes that file
/// the publication file. COD-15: it used to write a scratch file and copy it,
/// twice the disk space and twice the writing for a large export; only a file
/// that needs a prefix (the Excel CSV BOM) is still copied.
fn write_through_scratch<C>(
    temporary: &mut tempfile::NamedTempFile,
    extension: &str,
    is_cancelled: C,
    write: impl FnOnce(&Path, C) -> Result<(), String>,
) -> Result<usize, String>
where
    C: Fn() -> bool + Clone + Send + 'static,
{
    let parent = temporary
        .path()
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?
        .to_path_buf();
    // DuckDB picks the format by the path, so the file keeps the extension;
    // the guard still deletes it if the export fails or is cancelled.
    let staged = tempfile::Builder::new()
        .prefix(".columnia-export-")
        .suffix(&format!(".{extension}"))
        .tempfile_in(&parent)
        .map_err(|error| format!("No se pudo preparar el archivo temporal: {error}"))?
        .into_temp_path();
    write(&staged, is_cancelled.clone())?;
    ensure_not_cancelled(is_cancelled())?;
    let has_prefix = temporary
        .as_file()
        .metadata()
        .map_err(|error| format!("No se pudo preparar la exportación temporal: {error}"))?
        .len()
        > 0;
    if has_prefix {
        // The Excel CSV starts with a BOM DuckDB cannot write: only then the
        // DuckDB output is appended after it.
        copy_file_with_cancel(&staged, temporary.as_file_mut(), is_cancelled)?;
        return Ok(0);
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&staged)
        .map_err(|error| format!("No se pudo abrir la exportación temporal: {error}"))?;
    *temporary = tempfile::NamedTempFile::from_parts(file, staged);
    Ok(0)
}

pub(super) fn export_source_backed_xlsx_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    row_count: usize,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Excel,
        report,
        is_cancelled,
        |source, temporary, report, cancel| {
            let schema = source_scan(source.path, source.extension)?
                .collect_schema()
                .map_err(|error| format!("No se pudo leer el esquema source-backed: {error}"))?;
            let schema_frame = DataFrame::empty_with_schema(&schema);
            let replaced = write_source_backed_xlsx(
                source.path,
                source.format,
                &schema_frame,
                row_count,
                temporary.as_file_mut(),
                |percent| report("Escribiendo Excel", percent),
                cancel,
            )?;
            Ok(replaced)
        },
    )
}

pub(super) fn write_xlsx<F, C>(
    frame: &DataFrame,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<usize, String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    if frame.width() == 0 {
        return Err("Excel requiere al menos una columna.".to_owned());
    }
    ensure_excel_dimensions(frame.height(), frame.width())?;
    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
    const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="dataset" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/><xf numFmtId="22" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/></cellXfs></styleSheet>"#;

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut archive = ZipWriter::new(output);
    for (name, contents) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("xl/workbook.xml", WORKBOOK),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS),
        ("xl/styles.xml", STYLES),
    ] {
        archive
            .start_file(name, options)
            .map_err(|error| format!("No se pudo preparar el libro Excel: {error}"))?;
        archive
            .write_all(contents.as_bytes())
            .map_err(|error| format!("No se pudo escribir el libro Excel: {error}"))?;
    }

    archive
        .start_file(
            "xl/worksheets/sheet1.xml",
            zip_entry_options(estimated_sheet_bytes(frame.height(), frame.width())),
        )
        .map_err(|error| format!("No se pudo preparar la hoja Excel: {error}"))?;
    archive
        .write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#)
        .map_err(|error| format!("No se pudo escribir la hoja Excel: {error}"))?;
    let last_cell = format!(
        "<dimension ref=\"A1:{}{}\"/><sheetData>",
        xlsx_column_name(frame.width() - 1),
        frame.height() + 1
    );
    archive
        .write_all(last_cell.as_bytes())
        .map_err(|error| format!("No se pudo escribir las dimensiones Excel: {error}"))?;

    archive
        .write_all(b"<row r=\"1\">")
        .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    for (column_index, column) in frame.columns().iter().enumerate() {
        let header = xlsx_cell(column_index, 0, AnyValue::String(column.name().as_str()))?;
        archive
            .write_all(header.as_bytes())
            .map_err(|error| format!("No se pudo escribir el encabezado Excel: {error}"))?;
    }
    archive
        .write_all(b"</row>")
        .map_err(|error| format!("No se pudo cerrar el encabezado Excel: {error}"))?;

    // Rows are read block by block with sequential iterators (instead of
    // locating each cell's chunk) and written through one reused buffer.
    const XLSX_BLOCK_ROWS: usize = 8_192;
    let column_names = (0..frame.width()).map(xlsx_column_name).collect::<Vec<_>>();
    let mut replaced_control_cell_count = 0_usize;
    let mut buffer = String::with_capacity(128 * 1024);
    let mut block_start = 0;
    while block_start < frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        let block_length = XLSX_BLOCK_ROWS.min(frame.height() - block_start);
        let block = frame
            .columns()
            .iter()
            .map(|column| column.slice(block_start as i64, block_length))
            .collect::<Vec<_>>();
        let mut cells = block
            .iter()
            .map(|column| column.as_materialized_series().iter())
            .collect::<Vec<_>>();
        for row_offset in 0..block_length {
            let row_number = block_start + row_offset + 2;
            {
                use std::fmt::Write as _;
                let _ = write!(buffer, "<row r=\"{row_number}\">");
            }
            for (column_index, values) in cells.iter_mut().enumerate() {
                let value = values.next().unwrap_or(AnyValue::Null);
                replaced_control_cell_count += usize::from(excel_value_needs_replacement(&value));
                push_xlsx_cell(&mut buffer, &column_names[column_index], row_number, value)?;
            }
            buffer.push_str("</row>");
            if buffer.len() >= 64 * 1024 {
                archive
                    .write_all(buffer.as_bytes())
                    .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
                buffer.clear();
            }
            if row_offset % 1024 == 1023 {
                ensure_not_cancelled(is_cancelled())?;
            }
        }
        block_start += block_length;
        report(30 + ((block_start * 55) / frame.height()) as u8);
    }
    archive
        .write_all(buffer.as_bytes())
        .map_err(|error| format!("No se pudo escribir una fila Excel: {error}"))?;
    archive
        .write_all(b"</sheetData></worksheet>")
        .map_err(|error| format!("No se pudo cerrar la hoja Excel: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("No se pudo finalizar el libro Excel: {error}"))?;
    report(85);
    Ok(replaced_control_cell_count)
}

pub(super) fn sqlite_type(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => "INTEGER",
        DataType::Float32 | DataType::Float64 => "REAL",
        _ => "TEXT",
    }
}

pub(super) fn sqlite_value(value: AnyValue<'_>) -> Result<SqlValue, String> {
    match value {
        AnyValue::Null => Ok(SqlValue::Null),
        AnyValue::Boolean(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int8(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int16(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int32(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::Int64(value) => Ok(SqlValue::Integer(value)),
        AnyValue::UInt8(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt16(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt32(value) => Ok(SqlValue::Integer(i64::from(value))),
        AnyValue::UInt64(value) => i64::try_from(value)
            .map(SqlValue::Integer)
            .or_else(|_| Ok(SqlValue::Text(value.to_string()))),
        AnyValue::Float32(value) if value.is_finite() => Ok(SqlValue::Real(value as f64)),
        AnyValue::Float64(value) if value.is_finite() => Ok(SqlValue::Real(value)),
        AnyValue::Float32(_) | AnyValue::Float64(_) => {
            Err("SQLite no puede representar valores numéricos no finitos.".to_owned())
        }
        AnyValue::String(value) => Ok(SqlValue::Text(value.to_owned())),
        AnyValue::StringOwned(value) => Ok(SqlValue::Text(value.to_string())),
        value => Ok(SqlValue::Text(value.to_string())),
    }
}

pub(super) fn write_sqlite_database<F, C>(
    frame: &DataFrame,
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool,
{
    if frame.width() == 0 {
        return Err("SQLite requiere al menos una columna.".to_owned());
    }
    let mut connection = Connection::open(path)
        .map_err(|error| format!("No se pudo crear la base SQLite: {error}"))?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("No se pudo iniciar la transacción SQLite: {error}"))?;
    transaction
        .execute_batch("DROP TABLE IF EXISTS \"dataset\";")
        .map_err(|error| format!("No se pudo preparar la tabla SQLite: {error}"))?;
    let definition = frame
        .columns()
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                sql_identifier(column.name().as_str()),
                sqlite_type(column.dtype())
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    transaction
        .execute_batch(&format!("CREATE TABLE \"dataset\" ({definition});"))
        .map_err(|error| format!("No se pudo crear la tabla SQLite: {error}"))?;
    let placeholders = (0..frame.width())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let columns = frame
        .columns()
        .iter()
        .map(|column| sql_identifier(column.name().as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = transaction
        .prepare(&format!(
            "INSERT INTO \"dataset\" ({columns}) VALUES ({placeholders});"
        ))
        .map_err(|error| format!("No se pudo preparar la inserción SQLite: {error}"))?;
    for row_index in 0..frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        let values = frame
            .columns()
            .iter()
            .map(|column| {
                column
                    .get(row_index)
                    .map_err(|error| {
                        format!("No se pudo leer la fila {row_index} para SQLite: {error}")
                    })
                    .and_then(sqlite_value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        statement
            .execute(params_from_iter(values))
            .map_err(|error| {
                format!("No se pudo insertar la fila {row_index} en SQLite: {error}")
            })?;
        let percent = if frame.height() == 0 {
            85
        } else {
            30 + (((row_index + 1) * 55) / frame.height()) as u8
        };
        report(percent);
    }
    drop(statement);
    transaction
        .commit()
        .map_err(|error| format!("No se pudo confirmar la base SQLite: {error}"))?;
    connection
        .execute_batch("PRAGMA user_version = 1;")
        .map_err(|error| format!("No se pudo versionar la base SQLite: {error}"))?;
    report(85);
    Ok(())
}

pub(super) fn sqlite_source_value(
    data_type: &DataType,
    value: Option<&str>,
) -> Result<SqlValue, String> {
    let Some(value) = value else {
        return Ok(SqlValue::Null);
    };
    match data_type {
        DataType::Boolean => {
            if value.eq_ignore_ascii_case("true") || value == "1" {
                Ok(SqlValue::Integer(1))
            } else if value.eq_ignore_ascii_case("false") || value == "0" {
                Ok(SqlValue::Integer(0))
            } else {
                Err("SQLite recibió un booleano source-backed inválido.".to_owned())
            }
        }
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => value
            .parse::<i64>()
            .map(SqlValue::Integer)
            .map_err(|_| "SQLite recibió un entero source-backed inválido.".to_owned()),
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
            let number = value
                .parse::<u64>()
                .map_err(|_| "SQLite recibió un entero source-backed inválido.".to_owned())?;
            Ok(i64::try_from(number)
                .map(SqlValue::Integer)
                .unwrap_or_else(|_| SqlValue::Text(value.to_owned())))
        }
        DataType::Float32 | DataType::Float64 => {
            let number = value
                .parse::<f64>()
                .map_err(|_| "SQLite recibió un decimal source-backed inválido.".to_owned())?;
            if !number.is_finite() {
                return Err("SQLite no puede representar valores numéricos no finitos.".to_owned());
            }
            Ok(SqlValue::Real(number))
        }
        _ => Ok(SqlValue::Text(value.to_owned())),
    }
}

pub(super) fn write_source_backed_sqlite<F, C>(
    source_path: &Path,
    source_format: crate::duckdb_query::DuckDbFileFormat,
    schema: &DataFrame,
    expected_row_count: usize,
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    if schema.width() == 0 {
        return Err("SQLite requiere al menos una columna.".to_owned());
    }
    let mut connection = Connection::open(path)
        .map_err(|error| format!("No se pudo crear la base SQLite: {error}"))?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("No se pudo iniciar la transacción SQLite: {error}"))?;
    transaction
        .execute_batch("DROP TABLE IF EXISTS \"dataset\";")
        .map_err(|error| format!("No se pudo preparar la tabla SQLite: {error}"))?;
    let definition = schema
        .columns()
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                sql_identifier(column.name().as_str()),
                sqlite_type(column.dtype())
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    transaction
        .execute_batch(&format!("CREATE TABLE \"dataset\" ({definition});"))
        .map_err(|error| format!("No se pudo crear la tabla SQLite: {error}"))?;
    let placeholders = (0..schema.width())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let columns = schema
        .columns()
        .iter()
        .map(|column| sql_identifier(column.name().as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = transaction
        .prepare(&format!(
            "INSERT INTO \"dataset\" ({columns}) VALUES ({placeholders});"
        ))
        .map_err(|error| format!("No se pudo preparar la inserción SQLite: {error}"))?;
    let mut streamed_rows = 0_usize;
    let row_cancellation = is_cancelled.clone();
    let streamed_columns = crate::duckdb_query::stream_file_rows(
        source_path,
        source_format,
        |values| {
            ensure_not_cancelled(row_cancellation())?;
            if values.len() != schema.width() {
                return Err("La transmisión source-backed devolvió un ancho inesperado.".to_owned());
            }
            let sqlite_values = schema
                .columns()
                .iter()
                .zip(values.iter())
                .map(|(column, value)| sqlite_source_value(column.dtype(), value.as_deref()))
                .collect::<Result<Vec<_>, _>>()?;
            statement
                .execute(params_from_iter(sqlite_values))
                .map_err(|error| {
                    format!("No se pudo insertar la fila {streamed_rows} en SQLite: {error}")
                })?;
            streamed_rows = streamed_rows.saturating_add(1);
            let percent = if expected_row_count == 0 {
                85
            } else {
                30 + (streamed_rows
                    .saturating_mul(55)
                    .checked_div(expected_row_count)
                    .unwrap_or_default()) as u8
            };
            report(percent.min(85));
            Ok(())
        },
        is_cancelled,
    )?;
    drop(statement);
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
            "La transmisión source-backed devolvió columnas distintas al esquema.".to_owned(),
        );
    }
    if streamed_rows != expected_row_count {
        return Err(
            "El conteo del dataset source-backed cambió durante la exportación.".to_owned(),
        );
    }
    transaction
        .commit()
        .map_err(|error| format!("No se pudo confirmar la base SQLite: {error}"))?;
    connection
        .execute_batch("PRAGMA user_version = 1;")
        .map_err(|error| format!("No se pudo versionar la base SQLite: {error}"))?;
    report(85);
    Ok(())
}

pub(super) fn export_source_backed_sqlite_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    row_count: usize,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Sqlite,
        report,
        is_cancelled,
        |source, temporary, report, cancel| {
            let schema = source_scan(source.path, source.extension)?
                .collect_schema()
                .map_err(|error| format!("No se pudo leer el esquema source-backed: {error}"))?;
            let schema_frame = DataFrame::empty_with_schema(&schema);
            write_source_backed_sqlite(
                source.path,
                source.format,
                &schema_frame,
                row_count,
                temporary.path(),
                |percent| report("Escribiendo SQLite", percent),
                cancel,
            )?;
            Ok(0)
        },
    )
}

#[cfg(test)]
thread_local! {
    /// Lets a test fix the salt to compare both export paths.
    pub(super) static TEST_PRIVACY_SALT: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// A random salt for one export (SEG-02): the hash of a phone or an e-mail
/// cannot be reversed by hashing candidate values, and two exports do not
/// share hashes.
pub(super) fn fresh_privacy_salt() -> Result<String, String> {
    #[cfg(test)]
    if let Some(salt) = TEST_PRIVACY_SALT.with(|salt| salt.borrow().clone()) {
        return Ok(salt);
    }
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("No se pudo generar la sal de la protección: {error}"))?;
    Ok(hex::encode(bytes))
}

pub(super) fn salted_privacy_hash(salt: &str, value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(salt.as_bytes());
    digest.update(value.as_bytes());
    hex::encode(digest.finalize())
}

pub(super) fn privacy_safe_frame(
    frame: &DataFrame,
    mode: PrivacyMode,
) -> Result<(DataFrame, Vec<String>), String> {
    privacy_safe_frame_with_cancel(frame, mode, &|| false)
}

pub(super) fn privacy_safe_frame_with_cancel<C>(
    frame: &DataFrame,
    mode: PrivacyMode,
    is_cancelled: &C,
) -> Result<(DataFrame, Vec<String>), String>
where
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    if mode == PrivacyMode::None {
        return Ok((frame.clone(), Vec::new()));
    }
    let mut safe = frame.clone();
    let salt = if mode == PrivacyMode::Hash {
        fresh_privacy_salt()?
    } else {
        String::new()
    };
    let protected_columns = frame
        .columns()
        .iter()
        .filter(|column| privacy_signal(column.name()).is_some())
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    for column in frame
        .columns()
        .iter()
        .filter(|column| privacy_signal(column.name()).is_some())
    {
        ensure_not_cancelled(is_cancelled())?;
        let mut values = Vec::with_capacity(column.len());
        for row_index in 0..column.len() {
            if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            let value = column.get(row_index).map_err(|_| {
                "No se pudo preparar una columna para protección de privacidad.".to_owned()
            })?;
            values.push(match value {
                AnyValue::Null => None,
                value => {
                    let value = match value {
                        AnyValue::String(value) => value.to_owned(),
                        AnyValue::StringOwned(value) => value.as_str().to_owned(),
                        value => value.to_string(),
                    };
                    Some(match mode {
                        PrivacyMode::None => value,
                        PrivacyMode::Mask => REDACTED_VALUE.to_owned(),
                        PrivacyMode::Hash => salted_privacy_hash(&salt, &value),
                    })
                }
            });
        }
        ensure_not_cancelled(is_cancelled())?;
        safe.replace(
            column.name().as_str(),
            Column::new(column.name().clone(), values),
        )
        .map_err(|_| "No se pudo proteger una columna de datos personales.".to_owned())?;
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok((safe, protected_columns))
}

pub(super) fn source_backed_privacy_snapshot<C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    mode: PrivacyMode,
    is_cancelled: C,
) -> Result<Vec<String>, String>
where
    C: Fn() -> bool + Send + 'static,
{
    if mode == PrivacyMode::None {
        return Err("La protección source-backed requiere un modo explícito.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    let (source_path, source_size, extension) = validate_dataset_file(source_path)?;
    if source_size != expected_file_size {
        return Err("El archivo source-backed cambió antes de protegerse.".to_owned());
    }
    let schema = source_scan(&source_path, &extension)?
        .collect_schema()
        .map_err(|error| format!("No se pudo leer el esquema para proteger la fuente: {error}"))?;
    let schema_frame = DataFrame::empty_with_schema(&schema);
    let protected_columns = schema_frame
        .columns()
        .iter()
        .filter(|column| privacy_signal(column.name()).is_some())
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if protected_columns.is_empty() {
        return Ok(protected_columns);
    }
    let salt = if mode == PrivacyMode::Hash {
        fresh_privacy_salt()?
    } else {
        String::new()
    };
    let source_format = match extension.as_str() {
        "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
        "csv" | "tsv" | "txt" => crate::duckdb_query::DuckDbFileFormat::Delimited {
            delimiter: detect_delimiter(&source_path, &extension)?,
        },
        _ => return Err("El formato source-backed no admite protección incremental.".to_owned()),
    };
    let projection = schema_frame
        .columns()
        .iter()
        .map(|column| {
            let identifier = duckdb_identifier(column.name().as_str());
            if !protected_columns.iter().any(|name| name == column.name().as_str()) {
                return identifier;
            }
            match mode {
                PrivacyMode::Mask => format!(
                    "CASE WHEN {identifier} IS NULL THEN NULL ELSE {} END AS {identifier}",
                    sql_string_literal(REDACTED_VALUE)
                ),
                PrivacyMode::Hash => format!(
                    "CASE WHEN {identifier} IS NULL THEN NULL ELSE sha256({} || CAST({identifier} AS VARCHAR)) END AS {identifier}",
                    sql_string_literal(&salt)
                ),
                PrivacyMode::None => unreachable!("se validó un modo de privacidad explícito"),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    crate::duckdb_query::materialize_file_to_parquet_with_projection(
        &source_path,
        source_format,
        destination,
        &projection,
        is_cancelled,
    )?;
    let snapshot_size = fs::metadata(destination)
        .map_err(|error| format!("No se pudo verificar el snapshot protegido: {error}"))?
        .len();
    if snapshot_size == 0 {
        return Err("El snapshot protegido quedó vacío.".to_owned());
    }
    let final_source_size = fs::metadata(&source_path)
        .map_err(|error| format!("No se pudieron verificar los metadatos source-backed: {error}"))?
        .len();
    if final_source_size != expected_file_size {
        return Err("El archivo source-backed cambió durante la protección.".to_owned());
    }
    Ok(protected_columns)
}

pub(super) fn export_frame_atomic<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy(
        frame,
        destination,
        format,
        PrivacyMode::None,
        report,
        is_cancelled,
    )
}

pub(crate) fn copy_file_with_cancel<C, W>(
    source_path: &Path,
    destination: &mut W,
    is_cancelled: C,
) -> Result<u64, String>
where
    C: Fn() -> bool,
    W: Write,
{
    let mut source = File::open(source_path)
        .map_err(|error| format!("No se pudo leer el archivo temporal: {error}"))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut copied = 0_u64;
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo leer el archivo temporal: {error}"))?;
        if read == 0 {
            return Ok(copied);
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|error| format!("No se pudo copiar el archivo temporal: {error}"))?;
        copied = copied
            .checked_add(read as u64)
            .ok_or_else(|| "El tamaño de la exportación excede la capacidad local.".to_owned())?;
    }
}

pub(super) fn export_source_backed_parquet_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Parquet,
        report,
        is_cancelled,
        |source, temporary, _, cancel| {
            write_through_scratch(temporary, "parquet", cancel, |partial, cancel| {
                crate::duckdb_query::materialize_file_to_parquet_with_cancel(
                    source.path,
                    source.format,
                    partial,
                    None,
                    cancel,
                )
            })
        },
    )
}

#[cfg(test)]
pub(super) fn export_source_backed_csv_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    export_source_backed_delimited_atomic(
        source_path,
        expected_file_size,
        destination,
        false,
        report,
        is_cancelled,
    )
}

/// CSV of a large file, or with `excel` the «CSV para Excel» variant (UX-03).
pub(super) fn export_source_backed_delimited_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    excel: bool,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        if excel {
            ExportFormat::CsvExcel
        } else {
            ExportFormat::Csv
        },
        report,
        is_cancelled,
        move |source, temporary, _, cancel| {
            if excel {
                temporary
                    .as_file_mut()
                    .write_all(UTF8_BOM)
                    .map_err(|error| format!("No se pudo escribir el CSV: {error}"))?;
            }
            write_through_scratch(temporary, "csv", cancel, |partial, cancel| {
                crate::duckdb_query::export_file_to_delimited_with_cancel(
                    source.path,
                    source.format,
                    partial,
                    if excel { ';' } else { ',' },
                    cancel,
                )
            })
        },
    )
}

pub(super) fn export_source_backed_sql_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Sql,
        report,
        is_cancelled,
        |source, temporary, _, cancel| {
            write_through_scratch(temporary, "sql", cancel, |partial, cancel| {
                crate::duckdb_query::export_file_to_sql_with_cancel(
                    source.path,
                    source.format,
                    partial,
                    cancel,
                )
            })
        },
    )
}

pub(super) fn export_source_backed_json_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Json,
        report,
        is_cancelled,
        |source, temporary, _, cancel| {
            write_through_scratch(temporary, "json", cancel, |partial, cancel| {
                crate::duckdb_query::export_file_to_json_with_cancel(
                    source.path,
                    source.format,
                    partial,
                    cancel,
                )
            })
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn export_source_backed_bundle_atomic<F, C>(
    source_path: &Path,
    expected_file_size: u64,
    row_count: usize,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    destination: &Path,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    if let Some(recipe) = recipe {
        validate_stored_recipe(recipe)?;
    }
    publish_source_backed_export(
        source_path,
        expected_file_size,
        destination,
        ExportFormat::Bundle,
        report,
        is_cancelled,
        |source, temporary, report, cancel| {
            let schema = source_scan(source.path, source.extension)?
                .collect_schema()
                .map_err(|error| format!("No se pudo leer el esquema source-backed: {error}"))?;
            let schema_frame = DataFrame::empty_with_schema(&schema);
            let column_names = schema_frame
                .columns()
                .iter()
                .map(|column| column.name().to_string())
                .collect::<Vec<_>>();
            let parent = temporary
                .path()
                .parent()
                .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?
                .to_path_buf();
            let scratch = tempfile::tempdir_in(&parent)
                .map_err(|error| format!("No se pudo preparar el archivo temporal: {error}"))?;
            let dataset_path = scratch.path().join("dataset.partial.csv");
            crate::duckdb_query::export_file_to_csv_with_cancel(
                source.path,
                source.format,
                &dataset_path,
                cancel.clone(),
            )?;
            ensure_not_cancelled(cancel())?;
            report("Preparando diccionario", 35);
            let null_counts = crate::duckdb_query::count_file_nulls(
                source.path,
                source.format,
                &column_names,
                cancel.clone(),
            )?;
            if null_counts.len() != column_names.len() {
                return Err(
                    "DuckDB no devolvió todos los conteos del diccionario source-backed."
                        .to_owned(),
                );
            }
            let dictionary = BundleDictionary {
                format: "columnia-dictionary".to_owned(),
                version: BUNDLE_DICTIONARY_VERSION,
                columns: schema_frame
                    .columns()
                    .iter()
                    .zip(null_counts)
                    .map(|(column, null_count)| BundleDictionaryColumn {
                        name: column.name().to_string(),
                        data_type: column.dtype().to_string(),
                        null_count,
                    })
                    .collect(),
            };
            let mut dataset_file = File::open(&dataset_path).map_err(|error| {
                format!("No se pudo leer el dataset temporal del paquete: {error}")
            })?;
            let (dataset_bytes, dataset_sha256) =
                hash_and_rewind_with_cancel(&mut dataset_file, &cancel)?;
            assemble_bundle(
                temporary.as_file_mut(),
                &mut dataset_file,
                dataset_bytes,
                dataset_sha256,
                dictionary,
                row_count,
                schema_frame.width(),
                quality_validation,
                recipe,
                report,
                &cancel,
            )?;
            Ok(0)
        },
    )
}

pub(super) fn export_frame_atomic_with_privacy<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy_and_quality(
        frame,
        destination,
        format,
        privacy_mode,
        None,
        report,
        is_cancelled,
    )
}

pub(super) fn export_frame_atomic_with_privacy_and_quality<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    quality_validation: Option<&QualityValidationResult>,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    export_frame_atomic_with_privacy_and_quality_and_recipe(
        frame,
        destination,
        format,
        privacy_mode,
        quality_validation,
        None,
        report,
        is_cancelled,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn export_frame_atomic_with_privacy_and_quality_and_recipe<F, C>(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    privacy_mode: PrivacyMode,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    mut report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    let destination = canonicalize_write_destination(destination, "la exportación")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de exportación.".to_owned())?;
    report("Preparando archivo temporal", 10);
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo crear el archivo temporal: {error}"))?;
    let (protected_frame, protected_columns) =
        privacy_safe_frame_with_cancel(frame, privacy_mode, &is_cancelled)?;

    report("Escribiendo dataset", 25);
    let mut replaced_control_cell_count = 0;
    let mut formula_protected_cell_count = None;
    match format {
        ExportFormat::Csv | ExportFormat::CsvExcel => {
            formula_protected_cell_count = Some(write_delimited_frame_with_cancel(
                &protected_frame,
                temporary.as_file_mut(),
                format == ExportFormat::CsvExcel,
                &is_cancelled,
            )?)
        }
        ExportFormat::Json => {
            write_json_frame_with_cancel(&protected_frame, temporary.as_file_mut(), &is_cancelled)?
        }
        ExportFormat::Parquet => write_parquet_frame_with_cancel(
            &protected_frame,
            temporary.as_file_mut(),
            &is_cancelled,
        )?,
        ExportFormat::Sql => write_sql_script(
            &protected_frame,
            temporary.as_file_mut(),
            |percent| report("Escribiendo SQL", percent),
            &is_cancelled,
        )?,
        ExportFormat::Excel => {
            replaced_control_cell_count = write_xlsx(
                &protected_frame,
                temporary.as_file_mut(),
                |percent| report("Escribiendo Excel", percent),
                &is_cancelled,
            )?
        }
        ExportFormat::Sqlite => write_sqlite_database(
            &protected_frame,
            temporary.path(),
            |percent| report("Escribiendo SQLite", percent),
            &is_cancelled,
        )?,
        ExportFormat::Bundle => write_bundle(
            &protected_frame,
            quality_validation,
            recipe,
            temporary.as_file_mut(),
            &mut report,
            &is_cancelled,
        )?,
    }

    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la exportación: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    report("Publicando archivo completo", 90);
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar la exportación: {}", error.error))?;

    let file_size_bytes = fs::metadata(&destination)
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
        protected_column_count: protected_columns.len(),
        protected_columns,
        replaced_control_cell_count,
        row_count: None,
        folder_name: export_folder_name(&destination),
        formula_protected_cell_count,
    })
}

pub(super) fn bundle_json_bytes<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| format!("No se pudo preparar {label} del paquete: {error}"))
}

pub(super) fn bundle_delivery_summary(
    row_count: usize,
    column_count: usize,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    files: &[BundleFileManifest],
) -> String {
    let mut lines = vec![
        "# Resumen de entrega Columnia".to_owned(),
        String::new(),
        format!("Este documento usa el formato de resumen v{BUNDLE_DELIVERY_SUMMARY_VERSION}."),
        "No incluye muestras, valores de configuración, rutas locales, nombres de columnas ni credenciales."
            .to_owned(),
        String::new(),
        "## Dataset".to_owned(),
        format!("- Filas: {row_count}"),
        format!("- Columnas: {column_count}"),
        "- Formato de entrega: CSV".to_owned(),
        String::new(),
        "## Transformaciones".to_owned(),
    ];

    if let Some(recipe) = recipe {
        let configured_operations = [
            ("Renombres de columnas", recipe.recipe.renames.len()),
            ("Conversiones de tipo", recipe.recipe.casts.len()),
            ("Parseos de fecha", recipe.recipe.date_parses.len()),
            ("Filtros de filas", recipe.recipe.filters.len()),
            (
                "Columnas calculadas",
                recipe.recipe.calculated_column.is_some() as usize,
            ),
            (
                "Búsquedas y reemplazos",
                recipe.recipe.find_replace.is_some() as usize,
            ),
            (
                "Selecciones de columnas",
                recipe.recipe.keep_columns.is_some() as usize,
            ),
            (
                "Divisiones de columnas",
                recipe.recipe.split_column.is_some() as usize,
            ),
            (
                "Uniones de columnas",
                recipe.recipe.merge_columns.is_some() as usize,
            ),
            (
                "Tratamientos de atípicos",
                recipe.recipe.outlier_treatments.len(),
            ),
            (
                "Resúmenes agrupados",
                recipe.recipe.group_summary.is_some() as usize,
            ),
            (
                "Normalizaciones de contacto",
                recipe.recipe.contact_normalizations.len(),
            ),
            (
                "Extracciones de texto",
                recipe.recipe.text_extractions.len(),
            ),
        ];
        let total_operations = configured_operations
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>();
        lines.push(format!(
            "- Operaciones agregadas en la receta adjunta: {total_operations}"
        ));
        lines.push(
            "- Los conteos describen la receta adjunta; no cuantifican filas o celdas efectivamente modificadas."
                .to_owned(),
        );
        lines.push("- Los parámetros y valores de la receta no se reproducen aquí.".to_owned());
        for (label, count) in configured_operations {
            if count > 0 {
                lines.push(format!("- {label}: {count}"));
            }
        }
    } else {
        lines.push("- No se incluyó una receta de transformación.".to_owned());
    }

    lines.push(String::new());
    lines.push("## Contrato y cobertura de calidad".to_owned());
    if let Some(validation) = quality_validation {
        let status = if validation.total_rules == 0 {
            "sin reglas"
        } else if validation.passed {
            "aprobado"
        } else {
            "no aprobado"
        };
        let passed_rules = validation
            .total_rules
            .saturating_sub(validation.failed_rules);
        lines.push(format!("- Estado del contrato validado: {status}"));
        lines.push(format!(
            "- Reglas con resultado incluido: {} de {}",
            validation.rules.len(),
            validation.total_rules
        ));
        lines.push(format!("- Reglas aprobadas: {passed_rules}"));
        lines.push(format!("- Reglas fallidas: {}", validation.failed_rules));
        lines.push(format!(
            "- Filas consideradas por la validación: {}",
            validation.row_count
        ));
        lines.push(
            "- La calidad se validó antes de aplicar la política de privacidad de exportación; esa política puede alterar los resultados de reglas.".to_owned(),
        );
    } else {
        lines.push("- No se adjuntó un reporte de validación del contrato.".to_owned());
    }

    lines.extend([
        String::new(),
        "## Contratos y versiones".to_owned(),
        format!("- Paquete: `columnia-bundle` v{BUNDLE_MANIFEST_VERSION}"),
        format!("- Diccionario: `columnia-dictionary` v{BUNDLE_DICTIONARY_VERSION}"),
        format!(
            "- Reporte de calidad: {}",
            if quality_validation.is_some() {
                format!("`columnia-quality-report` v{BUNDLE_QUALITY_REPORT_VERSION}")
            } else {
                "no incluido".to_owned()
            }
        ),
        format!(
            "- Receta: {}",
            recipe.map_or_else(
                || "no incluida (no había una receta activa)".to_owned(),
                |recipe| format!("`recipe.json` v{}", recipe.version)
            )
        ),
        String::new(),
        "## Integridad".to_owned(),
        "SHA-256 de los archivos indicados, sobre los bytes exactos incluidos en el paquete:"
            .to_owned(),
    ]);

    for (path, label) in [
        ("dataset.csv", "Dataset"),
        ("dictionary.json", "Diccionario"),
        ("quality-report.json", "Reporte de calidad"),
        ("recipe.json", "Receta"),
    ] {
        if let Some(file) = files.iter().find(|file| file.path == path) {
            lines.push(format!("- {label}: `{}`", file.sha256));
        }
    }
    lines.push(format!(
        "- El hash de `{BUNDLE_DELIVERY_SUMMARY_FILE}` está registrado en `manifest.json`; el manifiesto no contiene un hash de sí mismo."
    ));
    lines.push(String::new());
    lines.join("\n") + "\n"
}

pub(super) fn hash_and_rewind_with_cancel<C>(
    file: &mut File,
    is_cancelled: &C,
) -> Result<(u64, String), String>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo leer el dataset temporal del paquete: {error}"))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo calcular el hash del paquete: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes = bytes.checked_add(read as u64).ok_or_else(|| {
            "El tamaño del dataset del paquete excede la capacidad local.".to_owned()
        })?;
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("No se pudo rebobinar el dataset temporal: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((bytes, hex::encode(hasher.finalize())))
}

/// Writes a Columnia bundle (LIM-02): the dataset CSV, already written and
/// hashed, plus the dictionary, quality report, recipe, delivery summary and
/// manifest. Both export paths build their bundle here.
#[allow(clippy::too_many_arguments)]
fn assemble_bundle<F, C>(
    output: &mut File,
    dataset_file: &mut File,
    dataset_bytes: u64,
    dataset_sha256: String,
    dictionary: BundleDictionary,
    row_count: usize,
    column_count: usize,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    report: &mut F,
    is_cancelled: &C,
) -> Result<(), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + ?Sized,
{
    let dictionary_bytes = bundle_json_bytes(&dictionary, "el diccionario")?;
    let dictionary_sha256 = hex::encode(Sha256::digest(&dictionary_bytes));
    let quality_bytes = quality_validation
        .map(|validation| {
            let report = BundleQualityReport {
                format: "columnia-quality-report".to_owned(),
                version: BUNDLE_QUALITY_REPORT_VERSION,
                passed: validation.passed,
                row_count: validation.row_count,
                total_rules: validation.total_rules,
                failed_rules: validation.failed_rules,
                rules: validation
                    .rules
                    .iter()
                    .map(|rule| BundleQualityRuleReport {
                        column: rule.column.clone(),
                        kind: rule.kind,
                        checked_count: rule.checked_count,
                        invalid_count: rule.invalid_count,
                        invalid_pct: rule.invalid_pct,
                        passed: rule.passed,
                    })
                    .collect(),
            };
            bundle_json_bytes(&report, "el reporte de calidad")
        })
        .transpose()?;
    let quality_manifest = quality_bytes.as_ref().map(|bytes| BundleFileManifest {
        path: "quality-report.json".to_owned(),
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    });
    let recipe_bytes = recipe
        .map(|recipe| bundle_json_bytes(recipe, "la receta"))
        .transpose()?;
    let recipe_manifest = recipe_bytes.as_ref().map(|bytes| BundleFileManifest {
        path: "recipe.json".to_owned(),
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    });
    let mut files = vec![
        BundleFileManifest {
            path: "dataset.csv".to_owned(),
            bytes: dataset_bytes,
            sha256: dataset_sha256,
        },
        BundleFileManifest {
            path: "dictionary.json".to_owned(),
            bytes: dictionary_bytes.len() as u64,
            sha256: dictionary_sha256,
        },
    ];
    files.extend(quality_manifest);
    files.extend(recipe_manifest);
    let summary_bytes =
        bundle_delivery_summary(row_count, column_count, quality_validation, recipe, &files)
            .into_bytes();
    files.push(BundleFileManifest {
        path: BUNDLE_DELIVERY_SUMMARY_FILE.to_owned(),
        bytes: summary_bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&summary_bytes)),
    });
    let manifest = BundleManifest {
        format: "columnia-bundle".to_owned(),
        version: BUNDLE_MANIFEST_VERSION,
        dataset_file: "dataset.csv".to_owned(),
        dataset_format: "csv".to_owned(),
        row_count,
        column_count,
        dictionary_file: "dictionary.json".to_owned(),
        quality_report_file: quality_bytes
            .as_ref()
            .map(|_| "quality-report.json".to_owned()),
        recipe_file: recipe_bytes.as_ref().map(|_| "recipe.json".to_owned()),
        delivery_summary_file: BUNDLE_DELIVERY_SUMMARY_FILE.to_owned(),
        files,
    };
    let manifest_bytes = bundle_json_bytes(&manifest, "el manifest")?;
    ensure_not_cancelled(is_cancelled())?;
    report("Empaquetando archivos", 50);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut archive = ZipWriter::new(output);
    archive
        .start_file("dataset.csv", zip_entry_options(dataset_bytes))
        .map_err(|error| format!("No se pudo preparar el dataset del paquete: {error}"))?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let read = dataset_file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo leer el dataset del paquete: {error}"))?;
        if read == 0 {
            break;
        }
        archive
            .write_all(&buffer[..read])
            .map_err(|error| format!("No se pudo empaquetar el dataset: {error}"))?;
        copied = copied.saturating_add(read as u64);
        let percent = if dataset_bytes == 0 {
            65
        } else {
            50 + (copied
                .saturating_mul(15)
                .checked_div(dataset_bytes)
                .unwrap_or(0) as u8)
                .min(15)
        };
        report("Empaquetando dataset", percent);
    }
    ensure_not_cancelled(is_cancelled())?;
    archive
        .start_file("dictionary.json", options)
        .map_err(|error| format!("No se pudo preparar el diccionario del paquete: {error}"))?;
    archive
        .write_all(&dictionary_bytes)
        .map_err(|error| format!("No se pudo empaquetar el diccionario del paquete: {error}"))?;
    if let Some(quality_bytes) = quality_bytes {
        ensure_not_cancelled(is_cancelled())?;
        archive
            .start_file("quality-report.json", options)
            .map_err(|error| format!("No se pudo preparar el reporte de calidad: {error}"))?;
        archive
            .write_all(&quality_bytes)
            .map_err(|error| format!("No se pudo empaquetar el reporte de calidad: {error}"))?;
    }
    if let Some(recipe_bytes) = recipe_bytes {
        ensure_not_cancelled(is_cancelled())?;
        archive
            .start_file("recipe.json", options)
            .map_err(|error| format!("No se pudo preparar la receta del paquete: {error}"))?;
        archive
            .write_all(&recipe_bytes)
            .map_err(|error| format!("No se pudo empaquetar la receta del paquete: {error}"))?;
    }
    ensure_not_cancelled(is_cancelled())?;
    archive
        .start_file(BUNDLE_DELIVERY_SUMMARY_FILE, options)
        .map_err(|error| format!("No se pudo preparar el resumen de entrega: {error}"))?;
    archive
        .write_all(&summary_bytes)
        .map_err(|error| format!("No se pudo empaquetar el resumen de entrega: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    archive
        .start_file("manifest.json", options)
        .map_err(|error| format!("No se pudo preparar el manifest del paquete: {error}"))?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|error| format!("No se pudo empaquetar el manifest: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("No se pudo cerrar el paquete: {error}"))?;
    Ok(())
}

pub(super) fn write_bundle<F, C>(
    frame: &DataFrame,
    quality_validation: Option<&QualityValidationResult>,
    recipe: Option<&StoredTransformRecipe>,
    output: &mut File,
    mut report: F,
    is_cancelled: C,
) -> Result<(), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    if let Some(recipe) = recipe {
        validate_stored_recipe(recipe)?;
    }
    report("Preparando paquete", 10);

    // El dataset se materializa en disco antes de abrir el ZIP para poder
    // calcular su hash sin duplicar datasets grandes en memoria.
    let mut dataset_file = tempfile::tempfile()
        .map_err(|error| format!("No se pudo preparar el dataset del paquete: {error}"))?;
    write_csv_frame_with_cancel(frame, &mut dataset_file, &is_cancelled)
        .map_err(|error| format!("No se pudo escribir el dataset del paquete: {error}"))?;
    dataset_file
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el dataset del paquete: {error}"))?;
    let (dataset_bytes, dataset_sha256) =
        hash_and_rewind_with_cancel(&mut dataset_file, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Preparando diccionario", 35);

    let dictionary = BundleDictionary {
        format: "columnia-dictionary".to_owned(),
        version: BUNDLE_DICTIONARY_VERSION,
        columns: frame
            .columns()
            .iter()
            .map(|column| BundleDictionaryColumn {
                name: column.name().to_string(),
                data_type: column.dtype().to_string(),
                null_count: column.null_count(),
            })
            .collect(),
    };
    assemble_bundle(
        output,
        &mut dataset_file,
        dataset_bytes,
        dataset_sha256,
        dictionary,
        frame.height(),
        frame.width(),
        quality_validation,
        recipe,
        &mut report,
        &is_cancelled,
    )?;
    report("Paquete listo", 88);
    Ok(())
}

#[cfg(test)]
mod xlsx_writer_tests {
    use super::*;
    use calamine::{open_workbook_auto, Data, Reader};

    #[test]
    fn single_pass_escape_matches_xml_escape() {
        for value in [
            "",
            "plain",
            "a & b < c > d \" e ' f",
            "&amp; ya escapado",
            "LÍNEA\u{0007}CONTROL\u{FFFE}\u{FFFF}",
            "tab\tnueva\nlínea\rretorno",
            "ñandú 😀",
        ] {
            let mut pushed = String::new();
            push_xml_escaped(&mut pushed, value);
            assert_eq!(pushed, xml_escape(value), "{value:?}");
        }
    }

    #[test]
    #[ignore = "medición: cargo test --lib sql_script_benchmark -- --ignored --nocapture"]
    fn sql_script_benchmark() {
        // COD-15: 1 M rows x 10 columns to a real file.
        const ROWS: usize = 1_000_000;
        let columns = (0..10)
            .map(|index| {
                if index % 2 == 0 {
                    Column::new(
                        format!("n{index}").into(),
                        (0..ROWS as i64).collect::<Vec<_>>(),
                    )
                } else {
                    Column::new(
                        format!("t{index}").into(),
                        (0..ROWS)
                            .map(|row| format!("valor {row}"))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();
        let frame = DataFrame::new(ROWS, columns).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut file = File::create(directory.path().join("bench.sql")).unwrap();
        let started = std::time::Instant::now();
        write_sql_script(&frame, &mut file, |_| {}, || false).unwrap();
        file.sync_all().unwrap();
        println!(
            "COD-15 sql 1M x 10: {:.2} s, {} bytes",
            started.elapsed().as_secs_f64(),
            file.metadata().unwrap().len()
        );
    }

    #[test]
    fn xlsx_rejects_more_columns_than_excel_opens_before_writing() {
        // FUN-68
        let wide = |width: usize| {
            DataFrame::new(
                1,
                (0..width)
                    .map(|index| Column::new(format!("c{index}").into(), [index as i64]))
                    .collect(),
            )
            .unwrap()
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ancho.xlsx");
        let mut file = File::create(&path).unwrap();
        let error = write_xlsx(&wide(EXCEL_MAX_COLUMNS + 1), &mut file, |_| {}, || false)
            .expect_err("más de 16.384 columnas no caben en una hoja");
        assert!(error.contains("16.384 columnas"), "{error}");
        assert_eq!(file.metadata().unwrap().len(), 0);
        assert!(ensure_excel_dimensions(1, EXCEL_MAX_COLUMNS).is_ok());
    }

    #[test]
    fn xlsx_rows_survive_chunk_and_block_boundaries() {
        const ROWS: usize = 8_192 * 2 + 5;
        let texts = (0..ROWS)
            .map(|row| (row % 7 != 0).then(|| format!("fila {row} <&>")))
            .collect::<Vec<_>>();
        let numbers = (0..ROWS)
            .map(|row| (row % 5 != 0).then_some(row as i64 - 3))
            .collect::<Vec<_>>();
        let mut frame = df!("texto" => texts, "numero" => numbers).unwrap();
        let tail = frame.slice(9_000, ROWS - 9_000);
        frame = frame.slice(0, 9_000);
        frame.vstack_mut(&tail).unwrap();
        assert!(frame.first_col_n_chunks() > 1);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bloques.xlsx");
        let mut file = File::create(&path).unwrap();
        write_xlsx(&frame, &mut file, |_| {}, || false).unwrap();
        drop(file);

        let mut workbook = open_workbook_auto(&path).unwrap();
        let range = workbook.worksheet_range("dataset").unwrap();
        assert_eq!(range.height(), ROWS + 1);
        for row in [0, 6, 7, 8_191, 8_192, 8_999, 9_000, ROWS - 1] {
            let expected_text = (row % 7 != 0).then(|| format!("fila {row} <&>"));
            let text = match range.get((row + 1, 0)) {
                Some(Data::String(value)) => Some(value.clone()),
                Some(Data::Empty) | None => None,
                other => panic!("celda inesperada {other:?}"),
            };
            assert_eq!(text, expected_text, "texto de la fila {row}");
            let number = match range.get((row + 1, 1)) {
                Some(Data::Float(value)) => Some(*value as i64),
                Some(Data::Int(value)) => Some(*value),
                Some(Data::Empty) | None => None,
                other => panic!("celda inesperada {other:?}"),
            };
            assert_eq!(
                number,
                (row % 5 != 0).then_some(row as i64 - 3),
                "número {row}"
            );
        }
    }
}
