use chrono::NaiveDate;
use polars::prelude::*;

use super::{ImportDateConvention, ImportNumberConvention, ImportProfile};

pub(super) fn validate_profile_conventions(
    profile: &ImportProfile,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
) -> Result<(), String> {
    let profile_date = profile
        .date_convention
        .unwrap_or(ImportDateConvention::Unresolved);
    let selected_date = date_convention.unwrap_or(ImportDateConvention::Unresolved);
    let profile_number = profile
        .number_convention
        .unwrap_or(ImportNumberConvention::Unresolved);
    let selected_number = number_convention.unwrap_or(ImportNumberConvention::Unresolved);
    if profile_date != selected_date || profile_number != selected_number {
        return Err("Las convenciones elegidas no coinciden con el perfil guardado.".to_owned());
    }
    Ok(())
}

/// Applies only explicitly selected conventions to delimited text columns.
/// Each column is converted atomically: one invalid non-null value leaves the
/// entire original column untouched, avoiding silent nulls or partial casts.
pub(super) fn apply_import_conventions<C>(
    frame: &DataFrame,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool,
{
    let date_convention =
        date_convention.filter(|value| *value != ImportDateConvention::Unresolved);
    let number_convention =
        number_convention.filter(|value| *value != ImportNumberConvention::Unresolved);
    if date_convention.is_none() && number_convention.is_none() {
        return Ok(frame.clone());
    }
    super::ensure_not_cancelled(is_cancelled())?;

    let mut converted = frame.clone();
    for column in frame.columns() {
        super::ensure_not_cancelled(is_cancelled())?;
        let Ok(values) = column.str() else {
            continue;
        };
        let lexical = values.iter().collect::<Vec<_>>();

        if let Some(convention) = date_convention {
            if let Some(days) = parse_date_column(&lexical, convention, &is_cancelled)? {
                let series = Series::new(column.name().clone(), days)
                    .cast(&DataType::Date)
                    .map_err(|error| {
                        format!("No se pudo convertir una columna de fecha: {error}")
                    })?;
                converted.with_column(series.into()).map_err(|error| {
                    format!("No se pudo actualizar una columna de fecha: {error}")
                })?;
                continue;
            }
        }

        if let Some(convention) = number_convention {
            match convention {
                ImportNumberConvention::Integer => {
                    if let Some(parsed) = parse_integer_column(&lexical, &is_cancelled)? {
                        converted
                            .with_column(Series::new(column.name().clone(), parsed).into())
                            .map_err(|error| {
                                format!("No se pudo actualizar una columna entera: {error}")
                            })?;
                    }
                }
                _ => {
                    if let Some(parsed) = parse_decimal_column(&lexical, convention, &is_cancelled)?
                    {
                        converted
                            .with_column(Series::new(column.name().clone(), parsed).into())
                            .map_err(|error| {
                                format!("No se pudo actualizar una columna decimal: {error}")
                            })?;
                    }
                }
            }
        }
    }
    Ok(converted)
}

fn parse_date_column<C>(
    values: &[Option<&str>],
    convention: ImportDateConvention,
    is_cancelled: &C,
) -> Result<Option<Vec<Option<i32>>>, String>
where
    C: Fn() -> bool,
{
    let mut parsed = Vec::with_capacity(values.len());
    let mut found_value = false;
    for (index, value) in values.iter().enumerate() {
        check_cancellation(index, is_cancelled)?;
        let Some(value) = value else {
            parsed.push(None);
            continue;
        };
        found_value = true;
        let Some(date) = parse_date(value.trim(), convention) else {
            return Ok(None);
        };
        let Some(epoch) = NaiveDate::from_ymd_opt(1970, 1, 1) else {
            return Ok(None);
        };
        let days = date.signed_duration_since(epoch).num_days();
        let Ok(days) = i32::try_from(days) else {
            return Ok(None);
        };
        parsed.push(Some(days));
    }
    Ok(found_value.then_some(parsed))
}

fn parse_date(value: &str, convention: ImportDateConvention) -> Option<NaiveDate> {
    let formats: &[&str] = match convention {
        ImportDateConvention::Unresolved => return None,
        ImportDateConvention::Iso8601 => &["%Y-%m-%d"],
        ImportDateConvention::Ymd => &["%Y/%m/%d", "%Y.%m.%d", "%Y-%m-%d"],
        ImportDateConvention::Dmy => &["%d/%m/%Y", "%d-%m-%Y", "%d.%m.%Y"],
        ImportDateConvention::Mdy => &["%m/%d/%Y", "%m-%d-%Y", "%m.%d.%Y"],
    };
    formats
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(value, format).ok())
}

fn parse_integer_column<C>(
    values: &[Option<&str>],
    is_cancelled: &C,
) -> Result<Option<Vec<Option<i64>>>, String>
where
    C: Fn() -> bool,
{
    let mut parsed = Vec::with_capacity(values.len());
    let mut found_value = false;
    for (index, value) in values.iter().enumerate() {
        check_cancellation(index, is_cancelled)?;
        let Some(value) = value else {
            parsed.push(None);
            continue;
        };
        found_value = true;
        let value = value.trim();
        if !is_plain_integer(value) {
            return Ok(None);
        }
        let Ok(value) = value.parse::<i64>() else {
            return Ok(None);
        };
        parsed.push(Some(value));
    }
    Ok(found_value.then_some(parsed))
}

fn parse_decimal_column<C>(
    values: &[Option<&str>],
    convention: ImportNumberConvention,
    is_cancelled: &C,
) -> Result<Option<Vec<Option<f64>>>, String>
where
    C: Fn() -> bool,
{
    let (decimal_separator, grouping_separator) = match convention {
        ImportNumberConvention::DotDecimalCommaGrouping => ('.', Some(',')),
        ImportNumberConvention::CommaDecimalDotGrouping => (',', Some('.')),
        ImportNumberConvention::DotDecimalSpaceGrouping => ('.', Some(' ')),
        ImportNumberConvention::CommaDecimalSpaceGrouping => (',', Some(' ')),
        ImportNumberConvention::Unresolved | ImportNumberConvention::Integer => return Ok(None),
    };

    let mut parsed = Vec::with_capacity(values.len());
    let mut found_value = false;
    for (index, value) in values.iter().enumerate() {
        check_cancellation(index, is_cancelled)?;
        let Some(value) = value else {
            parsed.push(None);
            continue;
        };
        found_value = true;
        let Some(canonical) =
            normalize_decimal(value.trim(), decimal_separator, grouping_separator)
        else {
            return Ok(None);
        };
        let Ok(number) = canonical.parse::<f64>() else {
            return Ok(None);
        };
        if !number.is_finite() {
            return Ok(None);
        }
        parsed.push(Some(number));
    }
    Ok(found_value.then_some(parsed))
}

fn check_cancellation<C>(index: usize, is_cancelled: &C) -> Result<(), String>
where
    C: Fn() -> bool,
{
    if index.is_multiple_of(8_192) {
        super::ensure_not_cancelled(is_cancelled())?;
    }
    Ok(())
}

fn is_signed_ascii_digits(value: &str) -> bool {
    let unsigned = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    !unsigned.is_empty() && unsigned.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_plain_integer(value: &str) -> bool {
    if !is_signed_ascii_digits(value) {
        return false;
    }
    let digits = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    digits == "0" || !digits.starts_with('0')
}

fn normalize_decimal(
    value: &str,
    decimal_separator: char,
    grouping_separator: Option<char>,
) -> Option<String> {
    let (sign, unsigned) = if let Some(value) = value.strip_prefix('-') {
        ("-", value)
    } else if let Some(value) = value.strip_prefix('+') {
        ("", value)
    } else {
        ("", value)
    };
    if unsigned.is_empty() {
        return None;
    }

    let mut decimal_parts = unsigned.split(decimal_separator);
    let integer = decimal_parts.next()?;
    let fraction = decimal_parts.next();
    if decimal_parts.next().is_some() {
        return None;
    }
    if fraction
        .is_some_and(|value| value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }

    let normalized_integer = match grouping_separator {
        Some(separator) if integer.contains(separator) => {
            let groups = integer.split(separator).collect::<Vec<_>>();
            let first = *groups.first()?;
            if first.is_empty()
                || first.len() > 3
                || !first.bytes().all(|byte| byte.is_ascii_digit())
                || groups.iter().skip(1).any(|group| {
                    group.len() != 3 || !group.bytes().all(|byte| byte.is_ascii_digit())
                })
            {
                return None;
            }
            groups.concat()
        }
        _ => {
            if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            integer.to_owned()
        }
    };

    Some(match fraction {
        Some(fraction) => format!("{sign}{normalized_integer}.{fraction}"),
        None => format!("{sign}{normalized_integer}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::{IntoColumn, NamedFrom};

    fn profile() -> ImportProfile {
        ImportProfile {
            version: 1,
            format: "csv".to_owned(),
            sheet_name: None,
            header_mode: Some(super::super::SpreadsheetHeaderMode::FirstRow),
            date_convention: Some(ImportDateConvention::Dmy),
            number_convention: Some(ImportNumberConvention::CommaDecimalDotGrouping),
            schema: Vec::new(),
        }
    }

    fn text_frame(name: &str, values: &[Option<&str>]) -> DataFrame {
        DataFrame::new(
            values.len(),
            vec![Series::new(name.into(), values).into_column()],
        )
        .expect("el frame de prueba debe ser válido")
    }

    #[test]
    fn saved_profile_and_applied_conventions_must_match() {
        let profile = profile();
        assert!(validate_profile_conventions(
            &profile,
            Some(ImportDateConvention::Dmy),
            Some(ImportNumberConvention::CommaDecimalDotGrouping),
        )
        .is_ok());
        assert!(validate_profile_conventions(
            &profile,
            Some(ImportDateConvention::Mdy),
            Some(ImportNumberConvention::CommaDecimalDotGrouping),
        )
        .is_err());
        assert!(validate_profile_conventions(&profile, None, None).is_err());
    }

    fn strings(frame: &DataFrame, column: &str) -> Vec<Option<String>> {
        let as_text = frame
            .column(column)
            .expect("la columna debe existir")
            .cast(&DataType::String)
            .expect("la columna se debe poder presentar como texto");
        as_text
            .str()
            .expect("la columna presentada debe ser textual")
            .iter()
            .map(|value| value.map(str::to_owned))
            .collect()
    }

    #[test]
    fn explicit_local_date_convention_resolves_ambiguous_values() {
        let source = text_frame("fecha", &[Some("01/02/2025")]);
        let dmy =
            apply_import_conventions(&source, Some(ImportDateConvention::Dmy), None, || false)
                .expect("la fecha DMY debe interpretarse");
        let mdy =
            apply_import_conventions(&source, Some(ImportDateConvention::Mdy), None, || false)
                .expect("la fecha MDY debe interpretarse");

        assert_eq!(dmy.column("fecha").unwrap().dtype(), &DataType::Date);
        assert_eq!(strings(&dmy, "fecha"), vec![Some("2025-02-01".into())]);
        assert_eq!(mdy.column("fecha").unwrap().dtype(), &DataType::Date);
        assert_eq!(strings(&mdy, "fecha"), vec![Some("2025-01-02".into())]);
    }

    #[test]
    fn unresolved_date_convention_does_not_resolve_ambiguous_dates() {
        let source = text_frame("fecha", &[Some("01/02/2025"), Some("03/04/2025")]);
        let unchanged = apply_import_conventions(
            &source,
            Some(ImportDateConvention::Unresolved),
            None,
            || false,
        )
        .expect("sin convención la importación debe conservar el texto");

        assert_eq!(
            unchanged.column("fecha").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(
            strings(&unchanged, "fecha"),
            vec![Some("01/02/2025".into()), Some("03/04/2025".into())]
        );
    }

    #[test]
    fn invalid_value_preserves_the_entire_date_column() {
        let source = text_frame("fecha", &[Some("31/12/2025"), Some("32/12/2025")]);
        let unchanged =
            apply_import_conventions(&source, Some(ImportDateConvention::Dmy), None, || false)
                .expect("una fecha inválida no debe abortar ni vaciar valores");

        assert_eq!(
            unchanged.column("fecha").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(
            strings(&unchanged, "fecha"),
            vec![Some("31/12/2025".into()), Some("32/12/2025".into())]
        );
    }

    #[test]
    fn cancellation_during_column_conversion_keeps_the_source_unchanged() {
        use std::cell::Cell;

        let source = text_frame("fecha", &[Some("31/12/2025")]);
        let checks = Cell::new(0);
        let result =
            apply_import_conventions(&source, Some(ImportDateConvention::Dmy), None, || {
                let count = checks.get() + 1;
                checks.set(count);
                count >= 3
            });

        assert!(result.is_err(), "la cancelación debe interrumpir el parseo");
        assert_eq!(source.column("fecha").unwrap().dtype(), &DataType::String);
        assert_eq!(strings(&source, "fecha"), vec![Some("31/12/2025".into())]);
    }

    #[test]
    fn explicit_number_conventions_parse_locale_grouping_without_guessing() {
        let dot_decimal = text_frame("importe", &[Some("1,234.56"), Some("-2.50"), None]);
        let parsed = apply_import_conventions(
            &dot_decimal,
            None,
            Some(ImportNumberConvention::DotDecimalCommaGrouping),
            || false,
        )
        .expect("el formato con decimal punto debe interpretarse");
        assert_eq!(
            parsed.column("importe").unwrap().dtype(),
            &DataType::Float64
        );
        let values = parsed.column("importe").unwrap().f64().unwrap();
        assert_eq!(values.get(0), Some(1234.56));
        assert_eq!(values.get(1), Some(-2.5));
        assert_eq!(values.get(2), None);

        let comma_decimal = text_frame("importe", &[Some("1.234,56"), Some("2.345,00")]);
        let parsed = apply_import_conventions(
            &comma_decimal,
            None,
            Some(ImportNumberConvention::CommaDecimalDotGrouping),
            || false,
        )
        .expect("el formato con decimal coma debe interpretarse");
        let values = parsed.column("importe").unwrap().f64().unwrap();
        assert_eq!(values.get(0), Some(1234.56));
        assert_eq!(values.get(1), Some(2345.0));

        let space_grouping = text_frame("importe", &[Some("1 234,5")]);
        let parsed = apply_import_conventions(
            &space_grouping,
            None,
            Some(ImportNumberConvention::CommaDecimalSpaceGrouping),
            || false,
        )
        .expect("el espacio de agrupación debe respetarse");
        assert_eq!(
            parsed.column("importe").unwrap().f64().unwrap().get(0),
            Some(1234.5)
        );
    }

    #[test]
    fn invalid_or_ambiguous_number_values_keep_the_whole_column_lexical() {
        let source = text_frame("importe", &[Some("12,34.56"), Some("1,234.56")]);
        let unchanged = apply_import_conventions(
            &source,
            None,
            Some(ImportNumberConvention::DotDecimalCommaGrouping),
            || false,
        )
        .expect("un grupo inválido no debe convertir parcialmente la columna");

        assert_eq!(
            unchanged.column("importe").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(
            strings(&unchanged, "importe"),
            vec![Some("12,34.56".into()), Some("1,234.56".into())]
        );
    }

    #[test]
    fn integer_convention_preserves_leading_zeroes_and_overflow() {
        let identifiers = text_frame("id", &[Some("00123"), Some("00456")]);
        let unchanged = apply_import_conventions(
            &identifiers,
            None,
            Some(ImportNumberConvention::Integer),
            || false,
        )
        .expect("los identificadores con ceros iniciales deben conservarse");
        assert_eq!(unchanged.column("id").unwrap().dtype(), &DataType::String);
        assert_eq!(
            strings(&unchanged, "id"),
            vec![Some("00123".into()), Some("00456".into())]
        );

        let integers = text_frame("id", &[Some("123"), Some("-456")]);
        let parsed = apply_import_conventions(
            &integers,
            None,
            Some(ImportNumberConvention::Integer),
            || false,
        )
        .expect("los enteros explícitos deben parsearse");
        assert_eq!(parsed.column("id").unwrap().dtype(), &DataType::Int64);
        assert_eq!(
            parsed.column("id").unwrap().i64().unwrap().get(0),
            Some(123)
        );

        let overflow = text_frame("id", &[Some("999999999999999999999999999")]);
        let unchanged = apply_import_conventions(
            &overflow,
            None,
            Some(ImportNumberConvention::Integer),
            || false,
        )
        .expect("el desbordamiento debe conservar el texto");
        assert_eq!(unchanged.column("id").unwrap().dtype(), &DataType::String);
        assert_eq!(
            strings(&unchanged, "id"),
            vec![Some("999999999999999999999999999".into())]
        );
    }
}
