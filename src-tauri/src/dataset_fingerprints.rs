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

fn write_normalized_text_fingerprint(hasher: &mut Xxh3, value: &str) {
    if value.is_ascii() {
        let mut emitted = false;
        let mut pending_space = false;
        for byte in value.bytes() {
            if byte.is_ascii_whitespace() {
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
