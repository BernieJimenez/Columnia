//! The column constructions that the in-memory plan (`cast_fully_dated_columns`,
//! `impute_missing_values_in_columns`) and the block-by-block stage on disk
//! (`source_stage::convert_block`) share, so both write the same column.

use super::date_inference::{parse_ordered_date, DateOrder};
use super::*;
use std::borrow::Cow;

/// The value that fills the gaps of one column.
pub(super) enum Fill<'a> {
    Text(Cow<'a, str>),
    Integer(i64),
    /// Any other number: filled as `f64` and cast back to the column type.
    Number(f64),
}

/// The text column `values` read with `order` as dates (`with_time`: as
/// date-times); nulls and values that are not dates stay null.
pub(super) fn typed_date_column(
    name: &str,
    values: &StringChunked,
    order: DateOrder,
    with_time: bool,
) -> PolarsResult<Column> {
    let parsed = values
        .iter()
        .map(|value| value.and_then(|value| parse_ordered_date(value, order)));
    let series = if with_time {
        Series::new(
            name.into(),
            parsed
                .map(|value| value.map(|value| value.and_utc().timestamp_millis()))
                .collect::<Vec<_>>(),
        )
        .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
    } else {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha fija válida");
        Series::new(
            name.into(),
            parsed
                .map(|value| {
                    value.and_then(|value| i32::try_from((value.date() - epoch).num_days()).ok())
                })
                .collect::<Vec<_>>(),
        )
        .cast(&DataType::Date)
    }?;
    Ok(series.into_column())
}

/// `column` with its empty cells replaced by `fill`, keeping its type.
pub(super) fn fill_column(column: &Column, fill: &Fill) -> Result<Column, String> {
    let name = column.name().clone();
    match fill {
        Fill::Text(value) => {
            let values = column
                .str()
                .map_err(|error| format!("No se pudo leer la columna '{name}': {error}"))?;
            Ok(Column::new(
                name,
                values
                    .iter()
                    .map(|cell| cell.or(Some(value.as_ref())))
                    .collect::<Vec<_>>(),
            ))
        }
        Fill::Integer(value) => {
            let values = column.i64().map_err(|error| {
                format!("No se pudo leer la columna numérica '{name}': {error}")
            })?;
            Ok(Column::new(
                name,
                values
                    .iter()
                    .map(|cell| cell.or(Some(*value)))
                    .collect::<Vec<_>>(),
            ))
        }
        Fill::Number(value) => {
            let numbers = column.cast(&DataType::Float64).map_err(|error| {
                format!("No se pudo preparar la columna numérica '{name}': {error}")
            })?;
            let values = numbers.f64().map_err(|error| {
                format!("No se pudo leer la columna numérica '{name}': {error}")
            })?;
            Column::new(
                name.clone(),
                values
                    .iter()
                    .map(|cell| cell.or(Some(*value)))
                    .collect::<Vec<_>>(),
            )
            .cast(column.dtype())
            .map_err(|error| format!("No se pudo conservar el tipo de '{name}': {error}"))
        }
    }
}
