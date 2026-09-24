use super::*;

#[cfg(test)]
pub(super) fn load_compare_frame(path: &Path, extension: &str) -> Result<DataFrame, String> {
    load_compare_frame_with_cancel(path, extension, || false)
}

pub(super) fn load_compare_frame_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    ensure_materialization_budget_for_path(path)?;
    let frame = if spreadsheet_extensions(extension) {
        let sheets = inspect_workbook(path)?;
        let sheet = sheets
            .first()
            .ok_or_else(|| "El libro no contiene hojas que se puedan comparar.".to_owned())?;
        load_spreadsheet_sheet_with_cancel(path, sheet, SpreadsheetHeaderMode::FirstRow, || {
            is_cancelled()
        })
    } else {
        load_dataset_with_progress(path, |_, _| {}, &is_cancelled).map(|(frame, _)| frame)
    }?;
    ensure_not_cancelled(is_cancelled())?;
    Ok(frame)
}

#[cfg(test)]
pub(super) fn persist_comparison_snapshot(
    frame: &DataFrame,
) -> Result<(tempfile::TempDir, PathBuf), String> {
    persist_comparison_snapshot_with_cancel(frame, &|| false)
}

#[cfg(test)]
pub(super) fn persist_comparison_source_file(
    path: &Path,
) -> Result<(tempfile::TempDir, PathBuf), String> {
    let source_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar la fuente comparada: {error}"))?
        .len();
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory.path())
        .map_err(|error| format!("No se pudo crear el snapshot comparado: {error}"))?;
    let mut source = File::open(path)
        .map_err(|error| format!("No se pudo abrir la fuente comparada: {error}"))?;
    let copied = std::io::copy(&mut source, temporary.as_file_mut())
        .map_err(|error| format!("No se pudo copiar la fuente comparada: {error}"))?;
    if copied != source_size {
        return Err("La fuente comparada cambió durante la copia al snapshot.".to_owned());
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot comparado: {error}"))?;
    let destination = directory.path().join("compared.parquet");
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {}", error.error))?;
    Ok((directory, destination))
}

#[cfg(test)]
pub(super) fn persist_delimited_comparison_source_file(
    path: &Path,
    extension: &str,
) -> Result<(tempfile::TempDir, PathBuf), String> {
    let delimiter = detect_delimiter(path, extension)?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    crate::duckdb_query::materialize_file_to_parquet_with_cancel(
        path,
        crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter },
        &temporary,
        None,
        || false,
    )?;
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    Ok((directory, destination))
}

#[cfg(test)]
pub(super) fn persist_json_comparison_source_file(
    path: &Path,
) -> Result<(tempfile::TempDir, PathBuf), String> {
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    crate::duckdb_query::materialize_file_to_parquet_with_cancel(
        path,
        crate::duckdb_query::DuckDbFileFormat::Json,
        &temporary,
        Some(&json_record_column_names(path)?),
        || false,
    )?;
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    Ok((directory, destination))
}

#[cfg(test)]
pub(super) fn persist_spreadsheet_comparison_source_file(
    path: &Path,
    extension: &str,
) -> Result<(tempfile::TempDir, PathBuf, usize), String> {
    let source_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar la fuente comparada: {error}"))?
        .len();
    let sheets = inspect_workbook(path)?;
    let sheet = sheets
        .first()
        .ok_or_else(|| "El libro no contiene hojas que se puedan comparar.".to_owned())?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    let row_count = if matches!(extension, "xlsx" | "xlsb") {
        let plan = spreadsheet_snapshot_plan_from_stream(
            path,
            sheet,
            SpreadsheetHeaderMode::FirstRow,
            || false,
        )?;
        write_streamed_spreadsheet_snapshot(path, sheet, &plan, &temporary, || false)?;
        plan.data_rows
    } else {
        let mut workbook = open_workbook_auto(path)
            .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
        let range = workbook
            .worksheet_range(sheet)
            .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
        write_spreadsheet_range_snapshot(&range, SpreadsheetHeaderMode::FirstRow, &temporary)?
    };
    let final_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo verificar la fuente comparada: {error}"))?
        .len();
    if final_size != source_size {
        return Err("La fuente comparada cambió durante la creación del snapshot.".to_owned());
    }
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    Ok((directory, destination, row_count))
}

pub(super) fn persist_comparison_source_file_with_cancel<C>(
    path: &Path,
    is_cancelled: &C,
) -> Result<(tempfile::TempDir, PathBuf), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let source_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar la fuente comparada: {error}"))?
        .len();
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory.path())
        .map_err(|error| format!("No se pudo crear el snapshot comparado: {error}"))?;
    let mut source = File::open(path)
        .map_err(|error| format!("No se pudo abrir la fuente comparada: {error}"))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut copied = 0_u64;
    loop {
        ensure_not_cancelled(is_cancelled())?;
        let bytes_read = source
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo copiar la fuente comparada: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        temporary
            .as_file_mut()
            .write_all(&buffer[..bytes_read])
            .map_err(|error| format!("No se pudo copiar la fuente comparada: {error}"))?;
        copied = copied.saturating_add(bytes_read as u64);
    }
    ensure_not_cancelled(is_cancelled())?;
    if copied != source_size {
        return Err("La fuente comparada cambió durante la copia al snapshot.".to_owned());
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    let destination = directory.path().join("compared.parquet");
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {}", error.error))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, destination))
}

pub(super) fn persist_delimited_comparison_source_file_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<(tempfile::TempDir, PathBuf), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let delimiter = detect_delimiter(path, extension)?;
    ensure_not_cancelled(is_cancelled())?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    crate::duckdb_query::materialize_file_to_parquet_with_cancel(
        path,
        crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter },
        &temporary,
        None,
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, destination))
}

pub(super) fn persist_json_comparison_source_file_with_cancel<C>(
    path: &Path,
    is_cancelled: C,
) -> Result<(tempfile::TempDir, PathBuf), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let column_names = json_record_column_names_with_cancel(path, is_cancelled.clone())?;
    ensure_not_cancelled(is_cancelled())?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    crate::duckdb_query::materialize_file_to_parquet_with_cancel(
        path,
        crate::duckdb_query::DuckDbFileFormat::Json,
        &temporary,
        Some(&column_names),
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, destination))
}

pub(super) fn persist_spreadsheet_comparison_source_file_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<(tempfile::TempDir, PathBuf, usize), String>
where
    C: Fn() -> bool + Clone + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let source_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar la fuente comparada: {error}"))?
        .len();
    let sheets = inspect_workbook(path)?;
    ensure_not_cancelled(is_cancelled())?;
    let sheet = sheets
        .first()
        .ok_or_else(|| "El libro no contiene hojas que se puedan comparar.".to_owned())?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let temporary = directory.path().join("compared.partial.parquet");
    let row_count = if matches!(extension, "xlsx" | "xlsb") {
        let plan = spreadsheet_snapshot_plan_from_stream(
            path,
            sheet,
            SpreadsheetHeaderMode::FirstRow,
            &is_cancelled,
        )?;
        write_streamed_spreadsheet_snapshot(path, sheet, &plan, &temporary, &is_cancelled)?;
        plan.data_rows
    } else {
        let mut workbook = open_workbook_auto(path)
            .map_err(|error| format!("No se pudo abrir el libro seleccionado: {error}"))?;
        let range = workbook
            .worksheet_range(sheet)
            .map_err(|error| format!("No se pudo leer la hoja seleccionada: {error}"))?;
        ensure_not_cancelled(is_cancelled())?;
        write_spreadsheet_range_snapshot_with_cancel(
            &range,
            SpreadsheetHeaderMode::FirstRow,
            &temporary,
            &is_cancelled,
        )?
    };
    ensure_not_cancelled(is_cancelled())?;
    let final_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo verificar la fuente comparada: {error}"))?
        .len();
    if final_size != source_size {
        return Err("La fuente comparada cambió durante la creación del snapshot.".to_owned());
    }
    let destination = directory.path().join("compared.parquet");
    fs::rename(&temporary, &destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, destination, row_count))
}

pub(super) fn persist_comparison_snapshot_with_cancel<C>(
    frame: &DataFrame,
    is_cancelled: &C,
) -> Result<(tempfile::TempDir, PathBuf), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let directory = tempfile::tempdir()
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory.path())
        .map_err(|error| format!("No se pudo crear el snapshot comparado: {error}"))?;
    let schema_frame = frame.slice(0, 0);
    let mut writer = ParquetWriter::new(temporary.as_file_mut())
        .set_parallel(false)
        .batched(schema_frame.schema())
        .map_err(|error| format!("No se pudo preparar el snapshot comparado: {error}"))?;
    let mut offset = 0usize;
    while offset < frame.height() {
        ensure_not_cancelled(is_cancelled())?;
        let row_count = LOCAL_QUERY_BLOCK_ROWS.min(frame.height() - offset);
        let slice_offset = i64::try_from(offset)
            .map_err(|_| "El snapshot comparado supera la capacidad del escritor.".to_owned())?;
        let mut batch = frame.slice(slice_offset, row_count);
        // The batched writer requires every column to share the chunk layout.
        batch.align_chunks_par();
        ensure_not_cancelled(is_cancelled())?;
        writer
            .write_batch(&batch)
            .map_err(|error| format!("No se pudo escribir el snapshot comparado: {error}"))?;
        offset += row_count;
        ensure_not_cancelled(is_cancelled())?;
    }
    ensure_not_cancelled(is_cancelled())?;
    writer
        .finish()
        .map_err(|error| format!("No se pudo cerrar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el snapshot comparado: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    let destination = directory.path().join("compared.parquet");
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar el snapshot comparado: {}", error.error))?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, destination))
}

pub(super) fn persist_comparison_file_with_cancel<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<(tempfile::TempDir, PathBuf, usize), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let (directory, snapshot_path, row_count) = match extension {
        "parquet" => {
            let (directory, snapshot_path) =
                persist_comparison_source_file_with_cancel(path, &is_cancelled)?;
            ensure_not_cancelled(is_cancelled())?;
            let row_count = crate::duckdb_query::count_file_rows(
                &snapshot_path,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                is_cancelled.clone(),
            )?;
            (directory, snapshot_path, row_count)
        }
        "json" => {
            let (directory, snapshot_path) =
                persist_json_comparison_source_file_with_cancel(path, is_cancelled.clone())?;
            ensure_not_cancelled(is_cancelled())?;
            let row_count = crate::duckdb_query::count_file_rows(
                &snapshot_path,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                is_cancelled.clone(),
            )?;
            (directory, snapshot_path, row_count)
        }
        "csv" | "tsv" | "txt" => {
            let (directory, snapshot_path) = persist_delimited_comparison_source_file_with_cancel(
                path,
                extension,
                is_cancelled.clone(),
            )?;
            ensure_not_cancelled(is_cancelled())?;
            let row_count = crate::duckdb_query::count_file_rows(
                &snapshot_path,
                crate::duckdb_query::DuckDbFileFormat::Parquet,
                is_cancelled.clone(),
            )?;
            (directory, snapshot_path, row_count)
        }
        extension if spreadsheet_extensions(extension) => {
            persist_spreadsheet_comparison_source_file_with_cancel(
                path,
                extension,
                is_cancelled.clone(),
            )?
        }
        _ => {
            let frame = load_compare_frame_with_cancel(path, extension, &is_cancelled)?;
            let row_count = frame.height();
            let (directory, snapshot_path) =
                persist_comparison_snapshot_with_cancel(&frame, &is_cancelled)?;
            (directory, snapshot_path, row_count)
        }
    };
    ensure_not_cancelled(is_cancelled())?;
    Ok((directory, snapshot_path, row_count))
}
