use polars::prelude::{Column, DataFrame, DataType};

use super::{ensure_not_cancelled, LOCAL_QUERY_CANCEL_CHECK_ROWS};

fn starts_with_spreadsheet_formula_prefix(value: &str) -> bool {
    matches!(
        value.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r' | '\n')
    )
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    if starts_with_spreadsheet_formula_prefix(value) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
pub(super) fn csv_formula_safe_frame(frame: &DataFrame) -> Result<DataFrame, String> {
    csv_formula_safe_frame_with_cancel(frame, &|| false)
}

pub(super) fn csv_formula_safe_frame_with_cancel<C>(
    frame: &DataFrame,
    is_cancelled: &C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut safe = frame.clone();
    for column in frame
        .columns()
        .iter()
        .filter(|column| column.dtype() == &DataType::String)
    {
        ensure_not_cancelled(is_cancelled())?;
        let name = column.name().as_str().to_owned();
        let strings = column
            .str()
            .map_err(|_| "No se pudo preparar texto seguro para CSV.".to_owned())?;
        let mut values = Vec::with_capacity(strings.len());
        for (row_index, value) in strings.iter().enumerate() {
            if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                ensure_not_cancelled(is_cancelled())?;
            }
            values.push(value.map(neutralize_spreadsheet_formula));
        }
        ensure_not_cancelled(is_cancelled())?;
        safe.replace(&name, Column::new(name.clone().into(), values))
            .map_err(|_| "No se pudo proteger una columna de texto para CSV.".to_owned())?;
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(safe)
}
