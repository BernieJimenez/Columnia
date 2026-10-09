use std::hash::Hasher;

use polars::prelude::{AnyValue, Column, DataType, StringChunked};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use xxhash_rust::xxh3::Xxh3;

use crate::dataset::{normalize_text_value, preview_value};

pub(crate) type NormalizedRowFingerprint = u128;

pub(crate) enum NormalizedFingerprintColumn<'a> {
    String(&'a StringChunked),
    Other(&'a Column),
}

pub(crate) fn normalized_fingerprint_columns(
    columns: &[Column],
) -> Result<Vec<NormalizedFingerprintColumn<'_>>, String> {
    columns
        .iter()
        .map(|column| {
            if column.dtype() == &DataType::String {
                column
                    .str()
                    .map(NormalizedFingerprintColumn::String)
                    .map_err(|error| {
                        format!(
                            "No se pudieron normalizar las filas para detectar duplicados: {error}"
                        )
                    })
            } else {
                Ok(NormalizedFingerprintColumn::Other(column))
            }
        })
        .collect()
}

pub(crate) fn normalized_row_fingerprint(
    columns: &[NormalizedFingerprintColumn<'_>],
    row_index: usize,
) -> Result<NormalizedRowFingerprint, String> {
    let mut hasher = Xxh3::with_seed(0);

    for column in columns {
        match column {
            NormalizedFingerprintColumn::String(values) => {
                if let Some(value) = values.get(row_index) {
                    hasher.write_u8(1);
                    write_normalized_text_fingerprint(&mut hasher, value);
                } else {
                    hasher.write_u8(0);
                }
                hasher.write_u8(0xff);
            }
            NormalizedFingerprintColumn::Other(column) => {
                let value = column.get(row_index).map_err(|error| {
                    format!("No se pudieron normalizar las filas para detectar duplicados: {error}")
                })?;
                write_normalized_value_fingerprint(&mut hasher, value);
            }
        }
    }

    Ok(hasher.digest128())
}

/// Fingerprints rows `start..start + length` exactly like calling
/// [`normalized_row_fingerprint`] for each row, but walks every column once
/// with a sequential iterator and hashes each row from a reused buffer instead
/// of locating the chunk of every cell and feeding the hasher byte by byte.
pub(crate) fn normalized_row_fingerprints_range<C>(
    columns: &[Column],
    start: usize,
    length: usize,
    is_cancelled: &C,
) -> Result<Vec<NormalizedRowFingerprint>, String>
where
    C: Fn() -> bool,
{
    if length == 0 {
        return Ok(Vec::new());
    }
    let error = |error: polars::prelude::PolarsError| {
        format!("No se pudieron normalizar las filas para detectar duplicados: {error}")
    };
    let slices = columns
        .iter()
        .map(|column| column.slice(start as i64, length))
        .collect::<Vec<_>>();
    enum Cells<'a> {
        Text(Box<dyn Iterator<Item = Option<&'a str>> + 'a>),
        Other(Box<dyn Iterator<Item = AnyValue<'a>> + 'a>),
    }
    let mut cells = slices
        .iter()
        .map(|column| {
            if column.dtype() == &DataType::String {
                Ok(Cells::Text(Box::new(column.str().map_err(error)?.iter())))
            } else {
                Ok(Cells::Other(Box::new(
                    column.as_materialized_series().iter(),
                )))
            }
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut fingerprints = Vec::with_capacity(length);
    let mut row = Vec::<u8>::with_capacity(256);
    let mut display = String::new();
    for row_index in 0..length {
        if row_index % 4096 == 0 && is_cancelled() {
            return Err(crate::dataset::OPERATION_CANCELLED_MESSAGE.to_owned());
        }
        row.clear();
        for column in &mut cells {
            match column {
                Cells::Text(values) => {
                    match values.next().flatten() {
                        Some(value) => {
                            row.push(1);
                            push_normalized_text(&mut row, value);
                        }
                        None => row.push(0),
                    }
                    row.push(0xff);
                }
                Cells::Other(values) => {
                    match values.next().unwrap_or(AnyValue::Null) {
                        AnyValue::Null => row.push(0),
                        AnyValue::String(value) => {
                            row.push(1);
                            push_normalized_text(&mut row, value);
                        }
                        AnyValue::StringOwned(value) => {
                            row.push(1);
                            push_normalized_text(&mut row, value.as_str());
                        }
                        value => {
                            use std::fmt::Write as _;
                            row.push(1);
                            display.clear();
                            let _ = write!(display, "{value}");
                            push_normalized_text(&mut row, &display);
                        }
                    }
                    row.push(0xff);
                }
            }
        }
        fingerprints.push(xxhash_rust::xxh3::xxh3_128(&row));
    }
    Ok(fingerprints)
}

/// Appends the bytes [`write_normalized_text_fingerprint`] would hash.
fn push_normalized_text(output: &mut Vec<u8>, value: &str) {
    if value.is_ascii() {
        let mut emitted = false;
        let mut pending_space = false;
        for byte in value.bytes() {
            // FUN-70: the same predicate as the Unicode branch (`\x0b` included).
            if char::from(byte).is_whitespace() {
                if emitted {
                    pending_space = true;
                }
                continue;
            }
            if pending_space {
                output.push(b' ');
                pending_space = false;
            }
            output.push(byte.to_ascii_lowercase());
            emitted = true;
        }
        return;
    }

    let mut emitted = false;
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            if emitted {
                pending_space = true;
            }
            continue;
        }
        if pending_space {
            output.push(b' ');
            pending_space = false;
        }
        for lowered in character.to_lowercase() {
            for normalized in std::iter::once(lowered).nfd() {
                if is_combining_mark(normalized) {
                    continue;
                }
                let mut encoded = [0_u8; 4];
                output.extend_from_slice(normalized.encode_utf8(&mut encoded).as_bytes());
                emitted = true;
            }
        }
    }
}

fn write_normalized_text_fingerprint(hasher: &mut Xxh3, value: &str) {
    if value.is_ascii() {
        let mut emitted = false;
        let mut pending_space = false;
        for byte in value.bytes() {
            // FUN-70: the same predicate as the Unicode branch (`\x0b` included).
            if char::from(byte).is_whitespace() {
                if emitted {
                    pending_space = true;
                }
                continue;
            }
            if pending_space {
                hasher.write_u8(b' ');
                pending_space = false;
            }
            hasher.write_u8(byte.to_ascii_lowercase());
            emitted = true;
        }
        return;
    }

    let mut emitted = false;
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            if emitted {
                pending_space = true;
            }
            continue;
        }

        if pending_space {
            hasher.write_u8(b' ');
            pending_space = false;
        }
        for lowered in character.to_lowercase() {
            for normalized in std::iter::once(lowered).nfd() {
                if is_combining_mark(normalized) {
                    continue;
                }
                let mut encoded = [0_u8; 4];
                hasher.write(normalized.encode_utf8(&mut encoded).as_bytes());
                emitted = true;
            }
        }
    }
}

fn write_normalized_value_fingerprint(hasher: &mut Xxh3, value: AnyValue<'_>) {
    match value {
        AnyValue::Null => hasher.write_u8(0),
        AnyValue::String(value) => {
            hasher.write_u8(1);
            write_normalized_text_fingerprint(hasher, value);
        }
        AnyValue::StringOwned(value) => {
            hasher.write_u8(1);
            write_normalized_text_fingerprint(hasher, value.as_str());
        }
        value => {
            hasher.write_u8(1);
            let display = value.to_string();
            write_normalized_text_fingerprint(hasher, &display);
        }
    }
    // A delimiter makes adjacent columns unambiguous without allocating a
    // normalized String or computing its length for every cell.
    hasher.write_u8(0xff);
}

/// REN-01: the fingerprint of one row exactly as stored: text byte for byte
/// and other values by type and full value, so only identical rows share it
/// (a collision is about 1e-24 for 42 million rows).
pub(crate) fn exact_row_fingerprint(
    columns: &[NormalizedFingerprintColumn<'_>],
    row_index: usize,
) -> Result<NormalizedRowFingerprint, String> {
    let mut hasher = Xxh3::with_seed(0);
    for column in columns {
        match column {
            NormalizedFingerprintColumn::String(values) => match values.get(row_index) {
                Some(value) => {
                    hasher.write_u8(1);
                    hasher.write_u64(value.len() as u64);
                    hasher.write(value.as_bytes());
                }
                None => hasher.write_u8(0),
            },
            NormalizedFingerprintColumn::Other(column) => {
                match column.get(row_index).map_err(|error| {
                    format!("No se pudieron comparar las filas para detectar duplicados: {error}")
                })? {
                    AnyValue::Null => hasher.write_u8(0),
                    value => {
                        // Debug keeps the type and every digit of the value.
                        let exact = format!("{value:?}");
                        hasher.write_u8(1);
                        hasher.write_u64(exact.len() as u64);
                        hasher.write(exact.as_bytes());
                    }
                }
            }
        }
    }
    Ok(hasher.digest128())
}

pub(crate) fn row_fingerprint(
    columns: &[Column],
    row_index: usize,
    normalize_values: bool,
) -> Result<u128, String> {
    let mut hasher = Xxh3::with_seed(0);

    for column in columns {
        let value = column.get(row_index).map_err(|error| {
            format!("No se pudieron comparar las filas para detectar duplicados: {error}")
        })?;
        match value {
            AnyValue::Null => hasher.write_u8(0),
            value => {
                let display = preview_value(value).unwrap_or_default();
                let value = if normalize_values {
                    normalize_text_value(&display, true)
                } else {
                    display
                };
                hasher.write_u8(1);
                hasher.write_u64(value.len() as u64);
                hasher.write(value.as_bytes());
            }
        }
    }

    Ok(hasher.digest128())
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::*;

    #[test]
    fn a_vertical_tab_is_a_space_with_or_without_accents() {
        // FUN-70: the ASCII branch did not treat `\x0b` as a space.
        let frame = df!("texto" => ["a\u{b}b", "a b", "á\u{b}b", "á b"]).unwrap();
        let columns = normalized_fingerprint_columns(frame.columns()).unwrap();
        let fingerprints = (0..frame.height())
            .map(|row| normalized_row_fingerprint(&columns, row).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(fingerprints[0], fingerprints[1]);
        assert_eq!(fingerprints[2], fingerprints[3]);
        assert_eq!(
            normalized_row_fingerprints_range(frame.columns(), 0, 4, &|| false).unwrap(),
            fingerprints
        );
    }

    #[test]
    fn range_fingerprints_match_row_fingerprints_across_chunks() {
        let mut frame = df!(
            "texto" => [Some("  Ana  Pérez "), None, Some("ana pérez"), Some("JOSÉ"), Some("ǅ Ñandú")],
            "numero" => [Some(1_i64), Some(2), None, Some(1), Some(-7)],
            "decimal" => [Some(1.5_f64), None, Some(1.5), Some(0.0), Some(2.25)],
            "activo" => [Some(true), Some(false), None, Some(true), Some(false)],
        )
        .unwrap();
        let tail = frame.slice(2, 3);
        frame = frame.slice(0, 2);
        frame.vstack_mut(&tail).unwrap();
        assert!(
            frame.first_col_n_chunks() > 1,
            "la prueba necesita varios chunks"
        );

        let columns = normalized_fingerprint_columns(frame.columns()).unwrap();
        let expected = (0..frame.height())
            .map(|row| normalized_row_fingerprint(&columns, row).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            normalized_row_fingerprints_range(frame.columns(), 0, frame.height(), &|| false)
                .unwrap(),
            expected
        );
        assert_eq!(
            normalized_row_fingerprints_range(frame.columns(), 1, 3, &|| false).unwrap(),
            expected[1..4]
        );
    }

    #[test]
    fn cancellation_is_polled_once_every_4096_rows() {
        // QA-57: a mutant that polled on every row, or never, survived.
        let frame = df!("numero" => (0..8_193_i64).collect::<Vec<_>>()).unwrap();
        let polls = std::cell::Cell::new(0_usize);
        normalized_row_fingerprints_range(frame.columns(), 0, 8_193, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(polls.get(), 3, "filas 0, 4096 y 8192");
        let error =
            normalized_row_fingerprints_range(frame.columns(), 0, 10, &|| true).unwrap_err();
        assert_eq!(error, crate::dataset::OPERATION_CANCELLED_MESSAGE);
    }
}
