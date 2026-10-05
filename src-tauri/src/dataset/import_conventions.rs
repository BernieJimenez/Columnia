use chrono::NaiveDate;
use polars::prelude::*;

use super::date_inference::{parse_calendar_date, DateOrder, DATE_SEPARATORS};
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

/// FUN-80: a text column the chosen convention could not convert, and why:
/// one invalid value keeps the whole column as text, so the person is told.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnconvertedColumn {
    pub(crate) column: String,
    /// "date" or "number".
    pub(crate) convention: &'static str,
    /// Non-empty values that do not follow the convention.
    pub(crate) invalid_count: usize,
    /// The first of them, as written (`N/A`, `-`…).
    pub(crate) example: String,
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
    apply_import_conventions_with_report(frame, date_convention, number_convention, is_cancelled)
        .map(|(converted, _)| converted)
}

/// [`apply_import_conventions`] plus the columns that looked like the
/// convention (most values follow it) but kept their text (FUN-80).
pub(super) fn apply_import_conventions_with_report<C>(
    frame: &DataFrame,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
    is_cancelled: C,
) -> Result<(DataFrame, Vec<UnconvertedColumn>), String>
where
    C: Fn() -> bool,
{
    let date_convention =
        date_convention.filter(|value| *value != ImportDateConvention::Unresolved);
    let number_convention =
        number_convention.filter(|value| *value != ImportNumberConvention::Unresolved);
    if date_convention.is_none() && number_convention.is_none() {
        return Ok((frame.clone(), Vec::new()));
    }
    super::ensure_not_cancelled(is_cancelled())?;

    let mut converted = frame.clone();
    let mut unconverted = Vec::new();
    for column in frame.columns() {
        super::ensure_not_cancelled(is_cancelled())?;
        let Ok(values) = column.str() else {
            continue;
        };
        let lexical = values.iter().collect::<Vec<_>>();
        // The closest miss: the convention most values of the column follow.
        let mut closest: Option<(&'static str, Rejection)> = None;
        let mut keep_closest = |convention: &'static str, rejection: Rejection| {
            if rejection.looks_like_the_convention()
                && closest
                    .as_ref()
                    .is_none_or(|(_, current)| rejection.parsed > current.parsed)
            {
                closest = Some((convention, rejection));
            }
        };

        if let Some(convention) = date_convention {
            match parse_date_column(&lexical, convention, &is_cancelled)? {
                ColumnParse::Converted(days) => {
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
                ColumnParse::Rejected(rejection) => keep_closest("date", rejection),
                ColumnParse::Empty => {}
            }
        }

        if let Some(convention) = number_convention {
            let outcome = match convention {
                ImportNumberConvention::Integer => {
                    match parse_integer_column(&lexical, &is_cancelled)? {
                        ColumnParse::Converted(parsed) => Some(
                            converted
                                .with_column(Series::new(column.name().clone(), parsed).into())
                                .map(|_| ())
                                .map_err(|error| {
                                    format!("No se pudo actualizar una columna entera: {error}")
                                }),
                        ),
                        ColumnParse::Rejected(rejection) => {
                            keep_closest("number", rejection);
                            None
                        }
                        ColumnParse::Empty => None,
                    }
                }
                _ => match parse_decimal_column(&lexical, convention, &is_cancelled)? {
                    ColumnParse::Converted(parsed) => Some(
                        converted
                            .with_column(Series::new(column.name().clone(), parsed).into())
                            .map(|_| ())
                            .map_err(|error| {
                                format!("No se pudo actualizar una columna decimal: {error}")
                            }),
                    ),
                    ColumnParse::Rejected(rejection) => {
                        keep_closest("number", rejection);
                        None
                    }
                    ColumnParse::Empty => None,
                },
            };
            if let Some(result) = outcome {
                result?;
                continue;
            }
        }
        if let Some((convention, rejection)) = closest {
            unconverted.push(UnconvertedColumn {
                column: column.name().to_string(),
                convention,
                invalid_count: rejection.invalid,
                example: rejection.example,
            });
        }
    }
    Ok((converted, unconverted))
}

/// Values that did not follow a convention in a column that was not converted.
struct Rejection {
    parsed: usize,
    invalid: usize,
    example: String,
}

impl Rejection {
    /// Most values follow the convention: the person likely expected it here.
    fn looks_like_the_convention(&self) -> bool {
        self.parsed > 0 && self.parsed >= self.invalid
    }
}

enum ColumnParse<T> {
    Converted(Vec<Option<T>>),
    Rejected(Rejection),
    /// Only empty cells: nothing to convert.
    Empty,
}

/// Parses every value; the column converts only if all non-empty values do.
fn parse_column<T, C>(
    values: &[Option<&str>],
    is_cancelled: &C,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<ColumnParse<T>, String>
where
    C: Fn() -> bool,
{
    let mut parsed = Vec::with_capacity(values.len());
    let mut parsed_count = 0_usize;
    let mut invalid = 0_usize;
    let mut example = None;
    for (index, value) in values.iter().enumerate() {
        check_cancellation(index, is_cancelled)?;
        let Some(value) = value else {
            parsed.push(None);
            continue;
        };
        match parse(value) {
            Some(number) => {
                parsed_count += 1;
                if invalid == 0 {
                    parsed.push(Some(number));
                }
            }
            None => {
                invalid += 1;
                example.get_or_insert_with(|| value.chars().take(40).collect::<String>());
            }
        }
    }
    Ok(match (parsed_count, invalid) {
        (0, 0) => ColumnParse::Empty,
        (_, 0) => ColumnParse::Converted(parsed),
        _ => ColumnParse::Rejected(Rejection {
            parsed: parsed_count,
            invalid,
            example: example.unwrap_or_default(),
        }),
    })
}

fn parse_date_column<C>(
    values: &[Option<&str>],
    convention: ImportDateConvention,
    is_cancelled: &C,
) -> Result<ColumnParse<i32>, String>
where
    C: Fn() -> bool,
{
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
    parse_column(values, is_cancelled, |value| {
        let date = parse_date(value.trim(), convention)?;
        i32::try_from(date.signed_duration_since(epoch).num_days()).ok()
    })
}

/// Two-digit years follow Excel's rule, as in Preparar: with «día-mes-año»,
/// `01/02/25` is 2025-02-01, never the year 0025.
fn parse_date(value: &str, convention: ImportDateConvention) -> Option<NaiveDate> {
    let (order, separators): (DateOrder, &[char]) = match convention {
        ImportDateConvention::Unresolved => return None,
        ImportDateConvention::Iso8601 => (DateOrder::Iso, &['-']),
        ImportDateConvention::Ymd => (DateOrder::Iso, DATE_SEPARATORS),
        ImportDateConvention::Dmy => (DateOrder::Dmy, DATE_SEPARATORS),
        ImportDateConvention::Mdy => (DateOrder::Mdy, DATE_SEPARATORS),
    };
    parse_calendar_date(value, order, separators, true)
}

fn parse_integer_column<C>(
    values: &[Option<&str>],
    is_cancelled: &C,
) -> Result<ColumnParse<i64>, String>
where
    C: Fn() -> bool,
{
    parse_column(values, is_cancelled, |value| {
        let value = value.trim();
        if !is_plain_integer(value) {
            return None;
        }
        value.parse::<i64>().ok()
    })
}

fn parse_decimal_column<C>(
    values: &[Option<&str>],
    convention: ImportNumberConvention,
    is_cancelled: &C,
) -> Result<ColumnParse<f64>, String>
where
    C: Fn() -> bool,
{
    let (decimal_separator, grouping_separator) = match convention {
        ImportNumberConvention::DotDecimalCommaGrouping => ('.', Some(',')),
        ImportNumberConvention::CommaDecimalDotGrouping => (',', Some('.')),
        ImportNumberConvention::DotDecimalSpaceGrouping => ('.', Some(' ')),
        ImportNumberConvention::CommaDecimalSpaceGrouping => (',', Some(' ')),
        ImportNumberConvention::Unresolved | ImportNumberConvention::Integer => {
            return Ok(ColumnParse::Empty)
        }
    };
    parse_column(values, is_cancelled, |value| {
        // FUN-80: spreadsheets group thousands with a no-break space too.
        let value = if grouping_separator == Some(' ') {
            value.trim().replace(['\u{a0}', '\u{202f}'], " ")
        } else {
            value.trim().to_owned()
        };
        let canonical = normalize_decimal(&value, decimal_separator, grouping_separator)?;
        // FUN-40: codes with leading zeros («00123») and integers past 2^53
        // stay text, as with the integer convention.
        let unsigned = canonical.trim_start_matches('-');
        let integer_digits = unsigned.split('.').next().unwrap_or_default();
        if integer_digits.len() > 1 && integer_digits.starts_with('0') {
            return None;
        }
        if !unsigned.contains('.')
            && unsigned
                .parse::<u128>()
                .is_ok_and(|integer| integer > (1_u128 << 53))
        {
            return None;
        }
        canonical
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
    })
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
    fn the_report_names_columns_a_marker_kept_as_text() {
        // FUN-80: one `N/A` kept the whole column as text without a word.
        let frame = DataFrame::new(
            3,
            vec![
                Series::new("precio".into(), [Some("1.234,5"), Some("N/A"), Some("2,5")])
                    .into_column(),
                Series::new(
                    "alta".into(),
                    [Some("01/02/2024"), Some("-"), Some("03/02/2024")],
                )
                .into_column(),
                Series::new("nombre".into(), [Some("Ana"), Some("Luis"), None]).into_column(),
            ],
        )
        .unwrap();
        let (converted, report) = apply_import_conventions_with_report(
            &frame,
            Some(ImportDateConvention::Dmy),
            Some(ImportNumberConvention::CommaDecimalDotGrouping),
            || false,
        )
        .unwrap();
        assert_eq!(
            converted.column("precio").unwrap().dtype(),
            &DataType::String
        );
        assert_eq!(
            report,
            vec![
                UnconvertedColumn {
                    column: "precio".to_owned(),
                    convention: "number",
                    invalid_count: 1,
                    example: "N/A".to_owned(),
                },
                UnconvertedColumn {
                    column: "alta".to_owned(),
                    convention: "date",
                    invalid_count: 1,
                    example: "-".to_owned(),
                },
            ]
        );

        // A no-break space groups thousands like a space.
        let (converted, report) = apply_import_conventions_with_report(
            &text_frame("miles", &[Some("1\u{a0}234,5"), Some("2,5")]),
            None,
            Some(ImportNumberConvention::CommaDecimalSpaceGrouping),
            || false,
        )
        .unwrap();
        assert!(report.is_empty());
        assert_eq!(
            converted.column("miles").unwrap().f64().unwrap().get(0),
            Some(1234.5)
        );
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

    /// FUN-02: two-digit years follow Excel's rule, as in Preparar; years of
    /// one or three digits and impossible dates keep the column as text.
    #[test]
    fn two_digit_years_follow_the_rule_of_preparar_and_odd_years_keep_text() {
        let source = text_frame(
            "fecha",
            &[Some("01/02/25"), Some("28/11/99"), Some("5.6.2024")],
        );
        let parsed =
            apply_import_conventions(&source, Some(ImportDateConvention::Dmy), None, || false)
                .expect("las fechas DMY deben interpretarse");
        assert_eq!(parsed.column("fecha").unwrap().dtype(), &DataType::Date);
        assert_eq!(
            strings(&parsed, "fecha"),
            vec![
                Some("2025-02-01".into()),
                Some("1999-11-28".into()),
                Some("2024-06-05".into())
            ]
        );
        let preparar = super::super::date_inference::parse_ordered_date(
            "01/02/25",
            super::super::date_inference::DateOrder::Dmy,
        )
        .expect("Preparar interpreta la misma fecha");
        assert_eq!(preparar.date().to_string(), "2025-02-01");

        for odd in ["01/02/025", "01/02/5", "29/02/2023", "01/13/2024"] {
            let source = text_frame("fecha", &[Some("01/02/2024"), Some(odd)]);
            let unchanged =
                apply_import_conventions(&source, Some(ImportDateConvention::Dmy), None, || false)
                    .expect("un año raro no debe abortar la importación");
            assert_eq!(
                unchanged.column("fecha").unwrap().dtype(),
                &DataType::String,
                "{odd}"
            );
        }
        let iso = text_frame("fecha", &[Some("2024/01/02")]);
        let unchanged =
            apply_import_conventions(&iso, Some(ImportDateConvention::Iso8601), None, || false)
                .expect("ISO solo acepta guiones");
        assert_eq!(
            unchanged.column("fecha").unwrap().dtype(),
            &DataType::String
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
    fn decimal_conventions_preserve_leading_zeroes_and_overflow() {
        // FUN-40: a code column stays text whatever decimal convention is chosen.
        for convention in [
            ImportNumberConvention::CommaDecimalDotGrouping,
            ImportNumberConvention::DotDecimalCommaGrouping,
            ImportNumberConvention::CommaDecimalSpaceGrouping,
            ImportNumberConvention::DotDecimalSpaceGrouping,
        ] {
            for values in [
                vec![Some("00123"), Some("00456")],
                vec![Some("9007199254740993"), Some("1")],
            ] {
                let frame = text_frame("codigo", &values);
                let unchanged = apply_import_conventions(&frame, None, Some(convention), || false)
                    .expect("la conversión no debe fallar");
                assert_eq!(
                    unchanged.column("codigo").unwrap().dtype(),
                    &DataType::String,
                    "{convention:?} {values:?}"
                );
            }
        }
        let amounts = text_frame("importe", &[Some("1.234,56"), Some("0,5")]);
        let parsed = apply_import_conventions(
            &amounts,
            None,
            Some(ImportNumberConvention::CommaDecimalDotGrouping),
            || false,
        )
        .unwrap();
        assert_eq!(
            parsed.column("importe").unwrap().dtype(),
            &DataType::Float64
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
