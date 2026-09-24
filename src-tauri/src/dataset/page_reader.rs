use super::*;
use crate::crash_report::LockRecovering;

fn validate_dataset_page_request(
    row_count: usize,
    offset: usize,
    limit: usize,
) -> Result<(), String> {
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(format!(
            "El tamaño de página debe estar entre 1 y {MAX_PAGE_SIZE} filas."
        ));
    }

    if offset > row_count {
        return Err("La página solicitada está fuera del dataset activo.".into());
    }
    Ok(())
}

pub(super) fn dataset_page(
    frame: &DataFrame,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    dataset_page_with_cancel(frame, offset, limit, || false)
}

pub(super) fn dataset_page_with_cancel<C>(
    frame: &DataFrame,
    offset: usize,
    limit: usize,
    is_cancelled: C,
) -> Result<DatasetPage, String>
where
    C: Fn() -> bool + Sync,
{
    validate_dataset_page_request(frame.height(), offset, limit)?;
    ensure_not_cancelled(is_cancelled())?;

    let end = offset.saturating_add(limit).min(frame.height());
    let mut rows = Vec::with_capacity(end.saturating_sub(offset));
    for row_index in offset..end {
        ensure_not_cancelled(is_cancelled())?;
        let mut row = Vec::with_capacity(frame.width());
        for (column_index, column) in frame.columns().iter().enumerate() {
            if column_index % 256 == 0 {
                ensure_not_cancelled(is_cancelled())?;
            }
            row.push(
                column
                    .get(row_index)
                    .map_err(|error| format!("No se pudo preparar la vista previa: {error}"))
                    .map(preview_value)?,
            );
        }
        rows.push(row);
    }

    Ok(DatasetPage { offset, rows })
}

pub(super) fn dataset_page_from_parquet(
    path: &Path,
    row_count: usize,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    dataset_page_from_parquet_with_cancel(path, row_count, offset, limit, || false)
}

pub(super) fn dataset_page_from_parquet_with_cancel<C>(
    path: &Path,
    row_count: usize,
    offset: usize,
    limit: usize,
    is_cancelled: C,
) -> Result<DatasetPage, String>
where
    C: Fn() -> bool + Sync,
{
    validate_dataset_page_request(row_count, offset, limit)?;
    ensure_not_cancelled(is_cancelled())?;
    let slice_offset = i64::try_from(offset)
        .map_err(|_| "La página solicitada excede la capacidad del lector Parquet.".to_owned())?;
    let plan = parquet_scan(path)?.slice(slice_offset, limit as IdxSize);
    let page_frame = collect_lazy_frame_streaming_with_cancel(
        plan,
        "No se pudo leer la página Parquet",
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let mut page = dataset_page_with_cancel(&page_frame, 0, limit, is_cancelled)?;
    page.offset = offset;
    Ok(page)
}

#[cfg(test)]
pub(super) fn dataset_page_from_source(
    path: &Path,
    extension: &str,
    row_count: usize,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    dataset_page_from_source_with_header_and_cancel(
        path,
        extension,
        row_count,
        offset,
        limit,
        SpreadsheetHeaderMode::FirstRow,
        || false,
    )
}

pub(super) fn dataset_page_from_source_with_header_and_cancel<C>(
    path: &Path,
    extension: &str,
    row_count: usize,
    offset: usize,
    limit: usize,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: C,
) -> Result<DatasetPage, String>
where
    C: Fn() -> bool + Sync,
{
    validate_dataset_page_request(row_count, offset, limit)?;
    ensure_not_cancelled(is_cancelled())?;
    let slice_offset = i64::try_from(offset)
        .map_err(|_| "La página solicitada excede la capacidad del lector.".to_owned())?;
    let plan =
        match extension {
            "parquet" => parquet_scan(path)?,
            "csv" | "tsv" | "txt" => delimited_scan_with_header(
                path,
                extension,
                header_mode == SpreadsheetHeaderMode::FirstRow,
            )?,
            _ => return Err(
                "La paginación directa solo está disponible para Parquet y archivos delimitados."
                    .to_owned(),
            ),
        }
        .slice(slice_offset, limit as IdxSize);
    let page_frame = collect_lazy_frame_streaming_with_cancel(
        plan,
        "No se pudo leer la página desde la fuente",
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let expected_rows = row_count.saturating_sub(offset).min(limit);
    if page_frame.height() != expected_rows {
        return Err(
            "La fuente cambió durante la lectura y ya no coincide con el dataset activo."
                .to_owned(),
        );
    }
    let mut page = dataset_page_with_cancel(&page_frame, 0, limit, is_cancelled)?;
    page.offset = offset;
    Ok(page)
}

pub(super) async fn get_dataset_page_impl(
    app: AppHandle,
    offset: usize,
    limit: usize,
) -> Result<DatasetPage, String> {
    let generation = app.state::<DatasetState>().begin_dataset_page();
    tauri::async_runtime::spawn_blocking(move || {
        let cancellation_app = app.clone();
        let is_cancelled = || {
            cancellation_app
                .state::<DatasetState>()
                .dataset_page_was_cancelled(generation)
        };
        ensure_not_cancelled(is_cancelled())?;

        let state = app.state::<DatasetState>();
        let mut current = state.current.lock_recovering();
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;

        if let Some((path, _, row_count)) = current_history_parquet_snapshot(dataset) {
            return dataset_page_from_parquet_with_cancel(
                &path,
                row_count,
                offset,
                limit,
                is_cancelled,
            );
        }

        // A degraded history still has a safe, immutable source reference while
        // the dataset has not been mutated. Read only the requested page from
        // disk instead of duplicating the whole active frame for the preview.
        if let Some((path, _)) = current_duckdb_file_source(dataset) {
            if let Ok(extension) = dataset_extension(&path) {
                match dataset_page_from_source_with_header_and_cancel(
                    &path,
                    &extension,
                    dataset.row_count,
                    offset,
                    limit,
                    dataset
                        .delimited_header_mode
                        .unwrap_or(SpreadsheetHeaderMode::FirstRow),
                    is_cancelled,
                ) {
                    Ok(page) => return Ok(page),
                    Err(error) if error == OPERATION_CANCELLED_MESSAGE => return Err(error),
                    Err(_) => {}
                }
            }
        }

        materialize_loaded_dataset_with_cancel(dataset, is_cancelled)?;
        ensure_not_cancelled(is_cancelled())?;
        dataset_page_with_cancel(&dataset.frame, offset, limit, is_cancelled)
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("La paginación local se interrumpió", &error)
    })?
}
