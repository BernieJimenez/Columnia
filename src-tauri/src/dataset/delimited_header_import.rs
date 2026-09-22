use std::{io::Write, path::Path};

use polars::prelude::IdxSize;
use serde::Serialize;

use super::{
    collect_lazy_frame_streaming, dataset_page, delimited_scan_with_separator,
    detect_delimiter_with_cancel, ensure_not_cancelled, read_utf8_delimited_sample_with_cancel,
    DatasetColumn, SpreadsheetHeaderMode, HEADER_REVIEW_ROW_LIMIT,
};

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelimitedHeaderModePreview {
    pub(crate) header_mode: SpreadsheetHeaderMode,
    pub(crate) columns: Vec<DatasetColumn>,
    pub(crate) rows: Vec<Vec<Option<String>>>,
    pub(crate) includes_first_row: bool,
    pub(crate) sample_truncated: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DelimitedHeaderReview {
    pub(crate) delimiter: String,
    pub(crate) first_row: DelimitedHeaderModePreview,
    pub(crate) generated: DelimitedHeaderModePreview,
}

fn complete_delimited_sample_prefix(sample: &str, complete: bool) -> Result<&str, String> {
    if complete {
        return Ok(sample);
    }

    let bytes = sample.as_bytes();
    let mut in_quotes = false;
    let mut complete_record_end = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_quotes && bytes.get(index + 1) == Some(&b'"') => index += 1,
            b'"' => in_quotes = !in_quotes,
            b'\n' if !in_quotes => complete_record_end = index + 1,
            _ => {}
        }
        index += 1;
    }
    if complete_record_end == 0 {
        return Err(
            "La primera fila supera la muestra segura de 64 KiB; no se pudo previsualizar el encabezado."
                .to_owned(),
        );
    }
    Ok(&sample[..complete_record_end])
}

fn delimited_header_mode_preview(
    sample_path: &Path,
    separator: u8,
    header_mode: SpreadsheetHeaderMode,
    sample_truncated: bool,
) -> Result<DelimitedHeaderModePreview, String> {
    let limit = (HEADER_REVIEW_ROW_LIMIT + 1) as IdxSize;
    let frame = collect_lazy_frame_streaming(
        delimited_scan_with_separator(
            sample_path,
            separator,
            header_mode == SpreadsheetHeaderMode::FirstRow,
        )?
        .slice(0, limit),
        "No se pudo preparar la vista previa de encabezados",
    )?;
    let columns = frame
        .columns()
        .iter()
        .map(|column| DatasetColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
        })
        .collect();
    let rows = dataset_page(&frame, 0, HEADER_REVIEW_ROW_LIMIT)?.rows;
    Ok(DelimitedHeaderModePreview {
        header_mode,
        columns,
        rows,
        includes_first_row: header_mode == SpreadsheetHeaderMode::Generated,
        sample_truncated: sample_truncated || frame.height() > HEADER_REVIEW_ROW_LIMIT,
    })
}

#[cfg(test)]
pub(super) fn delimited_header_review(
    path: &Path,
    extension: &str,
) -> Result<DelimitedHeaderReview, String> {
    delimited_header_review_with_cancel(path, extension, || false)
}

pub(super) fn delimited_header_review_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<DelimitedHeaderReview, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let separator = detect_delimiter_with_cancel(path, extension, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    let (sample, complete) = read_utf8_delimited_sample_with_cancel(path, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    let sample = complete_delimited_sample_prefix(&sample, complete)?;
    let mut bounded_sample = tempfile::NamedTempFile::new()
        .map_err(|error| format!("No se pudo preparar la muestra de encabezados: {error}"))?;
    bounded_sample
        .write_all(sample.as_bytes())
        .map_err(|error| format!("No se pudo preparar la muestra de encabezados: {error}"))?;
    bounded_sample
        .flush()
        .map_err(|error| format!("No se pudo preparar la muestra de encabezados: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;

    let sample_truncated = !complete;
    let first_row = delimited_header_mode_preview(
        bounded_sample.path(),
        separator,
        SpreadsheetHeaderMode::FirstRow,
        sample_truncated,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let generated = delimited_header_mode_preview(
        bounded_sample.path(),
        separator,
        SpreadsheetHeaderMode::Generated,
        sample_truncated,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(DelimitedHeaderReview {
        delimiter: char::from(separator).to_string(),
        first_row,
        generated,
    })
}
