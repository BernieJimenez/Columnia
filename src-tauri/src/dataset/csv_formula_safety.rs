use polars::prelude::{Column, DataFrame, DataType};

use super::{ensure_not_cancelled, LOCAL_QUERY_CANCEL_CHECK_ROWS};

fn starts_with_spreadsheet_formula_prefix(value: &str) -> bool {
    matches!(
        value.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r' | '\n')
    )
}

/// A complete signed number (`-1`, `+3`, `-0,5`, `-1.5e3`) is data, not a
/// formula: spreadsheets read it as a number. CSV loads keep columns as text,
/// so without this check every negative quantity gained an apostrophe.
fn is_signed_number(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(['+', '-']) else {
        return false;
    };
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(index) => (&rest[..index], Some(&rest[index + 1..])),
        None => (rest, None),
    };
    let mut digits = 0;
    let mut separators = 0;
    for character in mantissa.chars() {
        match character {
            '0'..='9' => digits += 1,
            '.' | ',' => separators += 1,
            _ => return false,
        }
    }
    let exponent_valid = exponent.is_none_or(|exponent| {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !exponent.is_empty() && exponent.chars().all(|character| character.is_ascii_digit())
    });
    digits > 0 && separators <= 1 && exponent_valid
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    if starts_with_spreadsheet_formula_prefix(value) && !is_signed_number(value) {
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
