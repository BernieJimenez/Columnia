//! REN-14: the part of the Preparar proposal that follows Rust's own rules
//! (numbers and dates typed only when every value converts, gaps filled with
//! the median or the most frequent value), applied to a large file block by
//! block after DuckDB wrote the rest of the corrections. The rules are those
//! of `cast_fully_numeric_columns`, `cast_fully_dated_columns` and
//! `impute_missing_values_in_columns`, in that order.

use super::date_inference::{parse_ordered_date, DateOrder};
use super::*;

/// What the stage converts (names as in the input): the text columns to type
/// as numbers, the date columns and the columns whose gaps it fills.
pub(super) struct SourceStagePlan {
    pub(super) numbers: Vec<String>,
    pub(super) dates: Vec<DateColumnPlan>,
    pub(super) impute: Option<Vec<String>>,
}

#[derive(Default)]
pub(super) struct SourceStageOutcome {
    pub(super) typed_column_count: usize,
    pub(super) dated_column_count: usize,
    pub(super) imputed_cell_count: usize,
}

/// The numeric kind of a text column, checked block by block with the rules
/// of `cast_fully_numeric_columns`.
fn numeric_target<C>(
    input: &Path,
    row_count: usize,
    name: &str,
    is_cancelled: &C,
) -> Result<Option<NumericCastKind>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut check = NumericCastCheck::new();
    for_each_parquet_column_block_with_size(
        input,
        row_count,
        name,
        SOURCE_STAGE_BLOCK_ROWS,
        |_, column| {
            ensure_not_cancelled(is_cancelled())?;
            let values = column
                .str()
                .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
            for value in values.iter().flatten() {
                check.observe(value);
            }
            Ok(())
        },
    )?;
    Ok(check.kind())
}

fn duckdb_cast_target(kind: NumericCastKind) -> crate::duckdb_query::NumericCastTarget {
    match kind {
        NumericCastKind::Integer => crate::duckdb_query::NumericCastTarget::Integer,
        NumericCastKind::IntegerFromDecimal => {
            crate::duckdb_query::NumericCastTarget::IntegerFromDecimal
        }
        NumericCastKind::Decimal => crate::duckdb_query::NumericCastTarget::Decimal,
    }
}

struct DateTarget {
    name: String,
    order: DateOrder,
    with_time: bool,
}

enum Fill {
    Text(String, String),
    Integer(String, i64),
    /// Any other number: filled as `f64` and cast back to the column type.
    Number(String, f64, DataType),
}

const SOURCE_STAGE_BLOCK_ROWS: usize = 4 * LOCAL_QUERY_BLOCK_ROWS;

/// Every non-null value of `column` read with `order`, and whether one has a
/// time; `None` when a value is not a date or the column has none.
fn date_target<C>(
    input: &Path,
    row_count: usize,
    name: &str,
    order: DateOrder,
    is_cancelled: &C,
) -> Result<Option<DateTarget>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut present = false;
    let mut with_time = false;
    let mut fits = true;
    for_each_parquet_column_block_with_size(
        input,
        row_count,
        name,
        SOURCE_STAGE_BLOCK_ROWS,
        |_, column| {
            ensure_not_cancelled(is_cancelled())?;
            if !fits {
                return Ok(());
            }
            let values = column
                .str()
                .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
            for value in values.iter().flatten() {
                present = true;
                with_time |= value.contains(':');
                if parse_ordered_date(value, order).is_none() {
                    fits = false;
                    break;
                }
            }
            Ok(())
        },
    )?;
    Ok((fits && present).then(|| DateTarget {
        name: name.to_owned(),
        order,
        with_time,
    }))
}

/// The most frequent non-blank text, the first seen on a tie, when it is
/// there at least twice and the column has a gap to fill.
fn text_fill<C>(
    input: &Path,
    row_count: usize,
    name: &str,
    is_cancelled: &C,
) -> Result<Option<String>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut counts = HashMap::<String, (usize, usize)>::new();
    let mut gaps = 0usize;
    let mut position = 0usize;
    for_each_parquet_column_block_with_size(
        input,
        row_count,
        name,
        SOURCE_STAGE_BLOCK_ROWS,
        |_, column| {
            ensure_not_cancelled(is_cancelled())?;
            let values = column
                .str()
                .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
            for value in values.iter() {
                match value {
                    None => gaps += 1,
                    Some(value) if !value.trim().is_empty() => {
                        let entry = counts.entry(value.to_owned()).or_insert((0, position));
                        entry.0 += 1;
                    }
                    Some(_) => {}
                }
                position += 1;
            }
            Ok(())
        },
    )?;
    if gaps == 0 {
        return Ok(None);
    }
    Ok(counts
        .into_iter()
        .filter(|(_, (count, _))| *count >= 2)
        .max_by(
            |(_, (left_count, left_first)), (_, (right_count, right_first))| {
                left_count
                    .cmp(right_count)
                    .then(right_first.cmp(left_first))
            },
        )
        .map(|(value, _)| value))
}

fn convert_block(
    block: &DataFrame,
    numbers: &[(String, NumericCastKind)],
    dates: &[DateTarget],
    fills: &[Fill],
) -> Result<(DataFrame, usize), String> {
    let mut converted = block.clone();
    for (name, kind) in numbers {
        let values = block
            .column(name)
            .and_then(|column| column.str().cloned())
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        converted
            .replace(name, numeric_cast_column(name, &values, *kind))
            .map_err(|error| format!("No se pudo convertir la columna '{name}': {error}"))?;
    }
    // Fills read the typed columns.
    let block = &converted.clone();
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha fija válida");
    for target in dates {
        let name = target.name.as_str();
        let values = block
            .column(name)
            .and_then(|column| column.str().cloned())
            .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
        let parsed = values
            .iter()
            .map(|value| value.and_then(|value| parse_ordered_date(value, target.order)));
        let column = if target.with_time {
            Series::new(
                name.into(),
                parsed
                    .map(|value| value.map(|value| value.and_utc().timestamp_millis()))
                    .collect::<Vec<_>>(),
            )
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
        } else {
            Series::new(
                name.into(),
                parsed
                    .map(|value| {
                        value
                            .and_then(|value| i32::try_from((value.date() - epoch).num_days()).ok())
                    })
                    .collect::<Vec<_>>(),
            )
            .cast(&DataType::Date)
        }
        .map_err(|error| format!("No se pudo convertir la columna '{name}' a fecha: {error}"))?
        .into_column();
        converted.replace(name, column).map_err(|error| {
            format!("No se pudo convertir la columna '{name}' a fecha: {error}")
        })?;
    }
    let mut filled = 0usize;
    for fill in fills {
        let (name, column) = match fill {
            Fill::Text(name, value) => {
                let column = block
                    .column(name)
                    .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
                filled += column.null_count();
                let values = column
                    .str()
                    .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
                let column = Column::new(
                    name.as_str().into(),
                    values
                        .iter()
                        .map(|cell| cell.or(Some(value.as_str())))
                        .collect::<Vec<_>>(),
                );
                (name, column)
            }
            Fill::Integer(name, value) => {
                let column = block
                    .column(name)
                    .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
                filled += column.null_count();
                let values = column.i64().map_err(|error| {
                    format!("No se pudo leer la columna numérica '{name}': {error}")
                })?;
                let column = Column::new(
                    name.as_str().into(),
                    values
                        .iter()
                        .map(|cell| cell.or(Some(*value)))
                        .collect::<Vec<_>>(),
                );
                (name, column)
            }
            Fill::Number(name, value, data_type) => {
                let column = block
                    .column(name)
                    .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
                filled += column.null_count();
                let numbers = column.cast(&DataType::Float64).map_err(|error| {
                    format!("No se pudo preparar la columna numérica '{name}': {error}")
                })?;
                let values = numbers.f64().map_err(|error| {
                    format!("No se pudo leer la columna numérica '{name}': {error}")
                })?;
                let column = Column::new(
                    name.as_str().into(),
                    values
                        .iter()
                        .map(|cell| cell.or(Some(*value)))
                        .collect::<Vec<_>>(),
                )
                .cast(data_type)
                .map_err(|error| format!("No se pudo conservar el tipo de '{name}': {error}"))?;
                (name, column)
            }
        };
        converted
            .replace(name, column)
            .map_err(|error| format!("No se pudo imputar la columna '{name}': {error}"))?;
    }
    Ok((converted, filled))
}

/// Writes `input` with the dates and fills of `plan` to `output`. Returns
/// `None`, writing nothing, when no column qualifies.
pub(super) fn source_date_and_impute_stage<C>(
    input: &Path,
    output: &Path,
    plan: &SourceStagePlan,
    is_cancelled: &C,
) -> Result<Option<SourceStageOutcome>, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let schema = read_parquet_schema_frame(input)?;
    let row_count = crate::duckdb_query::count_file_rows(
        input,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        || false,
    )?;
    let mut numbers = Vec::new();
    for name in &plan.numbers {
        if name == "_cambios"
            || privacy_signal(name) == Some("identifier")
            || !schema
                .column(name)
                .is_ok_and(|column| column.dtype() == &DataType::String)
        {
            continue;
        }
        if let Some(kind) = numeric_target(input, row_count, name, is_cancelled)? {
            numbers.push((name.clone(), kind));
        }
    }
    let mut dates = Vec::new();
    for date in &plan.dates {
        let order = DateOrder::from_label(&date.order)
            .ok_or_else(|| format!("Orden de fecha no válido: '{}'.", date.order))?;
        if schema
            .column(&date.column)
            .is_ok_and(|column| column.dtype() == &DataType::String)
        {
            if let Some(target) = date_target(input, row_count, &date.column, order, is_cancelled)?
            {
                dates.push(target);
            }
        }
    }
    let mut fills = Vec::new();
    for name in plan.impute.iter().flatten() {
        if name == "_cambios" || dates.iter().any(|target| &target.name == name) {
            continue;
        }
        let Ok(column) = schema.column(name) else {
            continue;
        };
        // A column this stage types fills with the median of its numbers.
        let typed = numbers
            .iter()
            .find(|(typed, _)| typed == name)
            .map(|(_, kind)| *kind);
        let data_type = match typed {
            Some(NumericCastKind::Decimal) => DataType::Float64,
            Some(_) => DataType::Int64,
            None => column.dtype().clone(),
        };
        let cast = typed.map(duckdb_cast_target);
        if data_type == DataType::String {
            if let Some(value) = text_fill(input, row_count, name, is_cancelled)? {
                fills.push(Fill::Text(name.clone(), value));
            }
        } else if data_type == DataType::Int64 {
            if let Some(value) =
                crate::duckdb_query::parquet_lower_median_integer(input, name, cast)?
            {
                fills.push(Fill::Integer(name.clone(), value));
            }
        } else if data_type.is_primitive_numeric() {
            if let Some(value) =
                crate::duckdb_query::parquet_lower_median_number(input, name, cast)?
            {
                fills.push(Fill::Number(name.clone(), value, data_type));
            }
        }
    }
    if numbers.is_empty() && dates.is_empty() && fills.is_empty() {
        return Ok(None);
    }

    let (empty, _) = convert_block(&schema, &numbers, &dates, &fills)?;
    let file = File::create(output)
        .map_err(|error| format!("No se pudo preparar la versión corregida: {error}"))?;
    let mut writer = ParquetWriter::new(file)
        .batched(empty.schema())
        .map_err(|error| format!("No se pudo preparar la versión corregida: {error}"))?;
    let mut imputed_cell_count = 0usize;
    for_each_parquet_block_prefetched(input, row_count, SOURCE_STAGE_BLOCK_ROWS, |_, block| {
        ensure_not_cancelled(is_cancelled())?;
        let (mut converted, filled) = convert_block(block, &numbers, &dates, &fills)?;
        imputed_cell_count += filled;
        converted.align_chunks_par();
        writer
            .write_batch(&converted)
            .map_err(|error| format!("No se pudo escribir la versión corregida: {error}"))
    })?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar la versión corregida: {error}"))?;
    Ok(Some(SourceStageOutcome {
        typed_column_count: numbers.len(),
        dated_column_count: dates.len(),
        imputed_cell_count,
    }))
}
