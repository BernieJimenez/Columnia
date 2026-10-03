use polars::prelude::{Column, DataFrame, DataType};

use super::{ensure_not_cancelled, LOCAL_QUERY_CANCEL_CHECK_ROWS};

fn starts_with_spreadsheet_formula_prefix(value: &str) -> bool {
    matches!(
        value.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r' | '\n')
    )
}

/// Text that starts like a formula but is data (FUN-07): a signed number
/// (`-1`, `+3`, `-0,5`, `-1.5e3`), one with thousands separators
/// (`-1.234,56`, `-1,234.56`, `-1 234`), a phone (`+34 600 000 000`) or a
/// lone `-` for «sin dato». The DuckDB export uses this same pattern with
/// `regexp_full_match`, so both paths protect the same values.
pub(crate) const SAFE_SIGNED_TEXT_PATTERN: &str = r"[+-]([0-9]+([.,][0-9]*)?|[.,][0-9]+)([eE][+-]?[0-9]+)?|[+-][0-9]{1,3}(\.[0-9]{3})+(,[0-9]+)?|[+-][0-9]{1,3}(,[0-9]{3})+(\.[0-9]+)?|[+-][0-9]{1,3}( [0-9]{3})+([.,][0-9]+)?|\+[0-9]+( [0-9]+)*|-";

fn is_signed_number(value: &str) -> bool {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!("^(?:{SAFE_SIGNED_TEXT_PATTERN})$"))
            .expect("el patrón de números con signo debe ser válido")
    });
    PATTERN.is_match(value)
}

/// The value with a leading `'` when a spreadsheet would read it as a formula.
pub(crate) fn neutralize_spreadsheet_formula(value: &str) -> String {
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

/// Column names get the same protection as the values (SEG-01): a header
/// `=1+1` would otherwise be a formula in the first row of the CSV.
pub(super) fn csv_formula_safe_column_names(frame: &DataFrame) -> Result<DataFrame, String> {
    let mut safe = frame.clone();
    let names = frame
        .get_column_names()
        .iter()
        .map(|name| neutralize_spreadsheet_formula(name.as_str()))
        .collect::<Vec<_>>();
    safe.set_column_names(&names)
        .map_err(|_| "No se pudieron proteger los encabezados del CSV.".to_owned())?;
    Ok(safe)
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
