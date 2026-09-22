use super::*;

pub(super) fn source_scan(path: &Path, extension: &str) -> Result<LazyFrame, String> {
    source_scan_with_header(path, extension, true)
}

pub(super) fn source_scan_with_header(
    path: &Path,
    extension: &str,
    has_header: bool,
) -> Result<LazyFrame, String> {
    match extension {
        "csv" | "tsv" | "txt" => delimited_scan_with_header(path, extension, has_header),
        "parquet" => parquet_scan(path),
        _ => Err("El formato no admite una carga source-backed diferida.".to_owned()),
    }
}

pub(super) fn source_backed_join_source(
    path: &Path,
    extension: &str,
) -> Result<Option<(crate::duckdb_query::DuckDbFileFormat, DataFrame)>, String> {
    let source_format = match extension {
        "csv" | "tsv" | "txt" => crate::duckdb_query::DuckDbFileFormat::Delimited {
            delimiter: detect_delimiter(path, extension)?,
        },
        "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
        _ => return Ok(None),
    };
    let schema = source_scan(path, extension)?
        .collect_schema()
        .map_err(|error| format!("No se pudo leer el esquema comparado source-backed: {error}"))?;
    Ok(Some((source_format, DataFrame::empty_with_schema(&schema))))
}

pub(super) fn source_backed_load<C>(
    path: &Path,
    extension: &str,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview, usize), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    source_backed_load_with_header_mode(
        path,
        extension,
        SpreadsheetHeaderMode::FirstRow,
        is_cancelled,
    )
}

pub(super) fn source_backed_load_with_header_mode<C>(
    path: &Path,
    extension: &str,
    header_mode: SpreadsheetHeaderMode,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview, usize), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let has_header = header_mode == SpreadsheetHeaderMode::FirstRow;
    let mut schema_plan = source_scan_with_header(path, extension, has_header)?;
    let schema = schema_plan
        .collect_schema()
        .map_err(|error| format!("No se pudo leer el esquema source-backed: {error}"))?;
    ensure_not_cancelled(is_cancelled())?;
    let schema_frame = DataFrame::empty_with_schema(&schema);
    let source_format = match extension {
        "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
        "csv" | "tsv" | "txt" => {
            let delimiter = detect_delimiter(path, extension)?;
            if has_header {
                crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter }
            } else {
                crate::duckdb_query::DuckDbFileFormat::DelimitedWithoutHeader {
                    delimiter,
                    column_count: schema_frame.width(),
                }
            }
        }
        _ => return Err("El formato no admite un conteo source-backed.".to_owned()),
    };
    let row_count =
        crate::duckdb_query::count_file_rows(path, source_format, is_cancelled.clone())?;
    ensure_not_cancelled(is_cancelled())?;
    let page = collect_lazy_frame_streaming_with_cancel(
        source_scan_with_header(path, extension, has_header)?
            .slice(0, PREVIEW_ROW_LIMIT as IdxSize),
        "No se pudo leer la vista previa source-backed",
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let file_size_bytes = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del archivo: {error}"))?
        .len();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    let preview = dataset_preview_from_schema_and_page(
        &file_name,
        file_size_bytes,
        row_count,
        &schema_frame,
        &page,
    )?;
    Ok((schema_frame, preview, row_count))
}

pub(super) fn source_backed_json_load<C>(
    path: &Path,
    snapshot_path: &Path,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview, usize), String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    let source_size_before = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del JSON: {error}"))?
        .len();
    if let Err(error) = crate::duckdb_query::materialize_file_to_parquet_with_projection(
        path,
        crate::duckdb_query::DuckDbFileFormat::Json,
        snapshot_path,
        "*",
        is_cancelled.clone(),
    ) {
        let _ = fs::remove_file(snapshot_path);
        return Err(error);
    }
    ensure_not_cancelled(is_cancelled())?;
    let source_size_after = fs::metadata(path)
        .map_err(|error| format!("No se pudieron verificar los metadatos del JSON: {error}"))?
        .len();
    if source_size_before != source_size_after {
        let _ = fs::remove_file(snapshot_path);
        return Err("El archivo JSON cambió durante la creación del snapshot.".to_owned());
    }
    let schema = read_parquet_schema_frame(snapshot_path)?;
    let row_count = crate::duckdb_query::count_file_rows(
        snapshot_path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        is_cancelled.clone(),
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let page = if row_count == 0 {
        schema.slice(0, 0)
    } else {
        collect_lazy_frame_streaming_with_cancel(
            parquet_scan(snapshot_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa JSON source-backed",
            &is_cancelled,
        )?
    };
    ensure_not_cancelled(is_cancelled())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset.json")
        .to_owned();
    let preview = dataset_preview_from_schema_and_page(
        &file_name,
        source_size_after,
        row_count,
        &schema,
        &page,
    )?;
    Ok((schema, preview, row_count))
}

pub(super) fn source_backed_spreadsheet_load<C>(
    path: &Path,
    sheet_name: &str,
    header_mode: SpreadsheetHeaderMode,
    snapshot_path: &Path,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview, usize), String>
where
    C: Fn() -> bool + Clone + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let plan = spreadsheet_snapshot_plan_from_stream(path, sheet_name, header_mode, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    write_streamed_spreadsheet_snapshot(path, sheet_name, &plan, snapshot_path, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    let schema_frame = read_parquet_schema_frame(snapshot_path)?;
    let page = if plan.data_rows == 0 {
        schema_frame.slice(0, 0)
    } else {
        collect_lazy_frame_streaming_with_cancel(
            parquet_scan(snapshot_path)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa del libro source-backed",
            &is_cancelled,
        )?
    };
    ensure_not_cancelled(is_cancelled())?;
    let file_size_bytes = fs::metadata(path)
        .map_err(|error| format!("No se pudieron leer los metadatos del libro: {error}"))?
        .len();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset.xlsx")
        .to_owned();
    let preview = dataset_preview_from_schema_and_page(
        &file_name,
        file_size_bytes,
        plan.data_rows,
        &schema_frame,
        &page,
    )?;
    Ok((schema_frame, preview, plan.data_rows))
}

pub(super) fn should_defer_source_load(extension: &str, file_size_bytes: u64) -> bool {
    file_size_bytes >= SOURCE_BACKED_LOAD_THRESHOLD_BYTES
        && matches!(
            extension,
            "csv" | "tsv" | "txt" | "json" | "jsonl" | "ndjson" | "parquet" | "xlsx" | "xlsb"
        )
}

pub(super) fn dataset_resource_estimate(
    extension: &str,
    file_size_bytes: u64,
) -> DatasetResourceEstimate {
    let is_source_backed = should_defer_source_load(extension, file_size_bytes);
    let needs_temporary_snapshot =
        !is_source_backed || matches!(extension, "json" | "jsonl" | "ndjson" | "xlsx" | "xlsb");
    DatasetResourceEstimate {
        processing_path: if is_source_backed {
            DatasetLoadPath::SourceBacked
        } else {
            DatasetLoadPath::InMemory
        },
        estimated_materialization_ram_bytes: file_size_bytes
            .saturating_mul(MATERIALIZATION_ESTIMATE_MULTIPLIER)
            .saturating_add(MATERIALIZATION_RESERVE_BYTES),
        estimated_temporary_disk_bytes: needs_temporary_snapshot.then_some(file_size_bytes),
    }
}

pub(super) fn load_source_backed_dataset_for_automation(
    input: &Path,
    sheet_name: Option<&str>,
    header_mode: Option<SpreadsheetHeaderMode>,
) -> Result<(LoadedDataset, DatasetPreview), String> {
    let (canonical, file_size_bytes, extension) = validate_dataset_file(input)?;
    let mut deferred_history = None;
    let (frame, preview, row_count) = if spreadsheet_extensions(&extension) {
        let sheet_name = sheet_name.ok_or_else(|| "Selecciona una hoja del libro.".to_owned())?;
        let header_mode = header_mode
            .ok_or_else(|| "Elige cómo interpretar los encabezados del libro.".to_owned())?;
        if sheet_name.is_empty() {
            return Err("La hoja seleccionada no es válida.".to_owned());
        }
        if !matches!(extension.as_str(), "xlsx" | "xlsb") {
            return Err(
                "El libro no admite la ruta source-backed diferida para este formato.".to_owned(),
            );
        }
        let available_sheets = inspect_workbook(&canonical)?;
        if available_sheets
            .iter()
            .filter(|name| *name == sheet_name)
            .count()
            != 1
        {
            return Err("La hoja seleccionada no existe de forma única en el libro.".to_owned());
        }
        let mut history = HistoryManager::deferred()?;
        let snapshot_path = history.directory.path().join("source.parquet");
        let result = source_backed_spreadsheet_load(
            &canonical,
            sheet_name,
            header_mode,
            &snapshot_path,
            || false,
        )?;
        history.source_snapshot_path = Some(snapshot_path);
        deferred_history = Some(history);
        result
    } else {
        if sheet_name.is_some() || header_mode.is_some() {
            return Err("Este formato no utiliza selección de hoja ni encabezado.".to_owned());
        }
        if matches!(extension.as_str(), "json" | "jsonl" | "ndjson") {
            let mut history = HistoryManager::deferred()?;
            let snapshot_path = history.directory.path().join("source.parquet");
            let result = source_backed_json_load(&canonical, &snapshot_path, || false)?;
            history.source_snapshot_path = Some(snapshot_path);
            deferred_history = Some(history);
            result
        } else {
            source_backed_load(&canonical, &extension, || false)?
        }
    };
    let history = deferred_history.unwrap_or(HistoryManager::deferred()?);
    let file_name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    let dataset = LoadedDataset {
        source_path: Some(canonical),
        file_name,
        file_size_bytes,
        row_count,
        frame,
        source_backed: true,
        delimited_header_mode: None,
        profile: None,
        history,
    };
    Ok((dataset, preview))
}

pub(super) fn materialize_loaded_dataset(dataset: &mut LoadedDataset) -> Result<(), String> {
    materialize_loaded_dataset_with_cancel(dataset, || false)
}

pub(super) fn materialize_loaded_dataset_with_cancel<C>(
    dataset: &mut LoadedDataset,
    is_cancelled: C,
) -> Result<(), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if !dataset.source_backed {
        return Ok(());
    }
    let frame = materialized_dataset_frame_with_cancel(dataset, &is_cancelled)?;
    ensure_not_cancelled(is_cancelled())?;
    dataset.frame = frame;
    dataset.source_backed = false;
    Ok(())
}

pub(super) fn materialized_dataset_frame(dataset: &LoadedDataset) -> Result<DataFrame, String> {
    materialized_dataset_frame_with_cancel(dataset, || false)
}

pub(super) fn materialized_dataset_frame_with_cancel<C>(
    dataset: &LoadedDataset,
    is_cancelled: C,
) -> Result<DataFrame, String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    if !dataset.source_backed {
        return Ok(dataset.frame.clone());
    }
    ensure_materialization_budget(dataset.file_size_bytes)?;
    let path = dataset
        .source_path
        .as_deref()
        .ok_or_else(|| "La fuente source-backed ya no está disponible.".to_owned())?;
    let (canonical, file_size, extension) = validate_dataset_file(path)?;
    ensure_not_cancelled(is_cancelled())?;
    if file_size != dataset.file_size_bytes {
        return Err("El archivo source-backed cambió después de la carga.".to_owned());
    }
    let materialized_path = dataset
        .history
        .source_snapshot_path
        .as_deref()
        .unwrap_or(&canonical);
    let materialized_extension = dataset_extension(materialized_path)?;
    let frame = match materialized_extension.as_str() {
        "csv" | "tsv" | "txt" => read_delimited_frame_with_header_and_cancel(
            materialized_path,
            &extension,
            dataset
                .delimited_header_mode
                .unwrap_or(SpreadsheetHeaderMode::FirstRow),
            || is_cancelled(),
        )?,
        "json" | "jsonl" | "ndjson" => {
            load_json_records_with_cancel(materialized_path, || is_cancelled())?
        }
        "parquet" => read_parquet_frame_with_cancel(materialized_path, || is_cancelled())?,
        _ => return Err("El formato source-backed no se puede materializar.".to_owned()),
    };
    ensure_not_cancelled(is_cancelled())?;
    if frame.height() != dataset.row_count {
        return Err("El conteo del dataset source-backed cambió durante la lectura.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(frame)
}

pub(super) fn materialization_budget_error(
    file_size_bytes: u64,
    available_memory_bytes: u64,
) -> Option<String> {
    if file_size_bytes < MATERIALIZATION_GUARD_THRESHOLD_BYTES {
        return None;
    }
    let estimated_bytes = file_size_bytes
        .saturating_mul(MATERIALIZATION_ESTIMATE_MULTIPLIER)
        .saturating_add(MATERIALIZATION_RESERVE_BYTES);
    (available_memory_bytes < estimated_bytes).then(|| {
        format!(
            "La operación requiere materializar una fuente o snapshot grande (aprox. {} MiB), pero solo hay {} MiB de RAM disponible. Usa una operación source-backed compatible o libera memoria antes de continuar.",
            estimated_bytes / 1024 / 1024,
            available_memory_bytes / 1024 / 1024,
        )
    })
}

pub(super) fn ensure_materialization_budget(file_size_bytes: u64) -> Result<(), String> {
    let Some(available_memory_bytes) = crate::resource::available_memory_bytes() else {
        return Ok(());
    };
    if let Some(error) = materialization_budget_error(file_size_bytes, available_memory_bytes) {
        return Err(error);
    }
    Ok(())
}

pub(super) fn ensure_materialization_budget_for_path(path: &Path) -> Result<(), String> {
    let file_size_bytes = fs::metadata(path)
        .map_err(|_| "No se pudo verificar el archivo antes de materializarlo.".to_owned())?
        .len();
    ensure_materialization_budget(file_size_bytes)
}

pub(super) fn materialize_current_dataset(
    state: &DatasetState,
) -> Result<(DataFrame, String), String> {
    materialize_current_dataset_with_cancel(state, &|| false)
}

pub(super) fn materialize_current_dataset_with_cancel<C>(
    state: &DatasetState,
    is_cancelled: &C,
) -> Result<(DataFrame, String), String>
where
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    let mut current = state
        .current
        .lock()
        .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
    let dataset = current.as_mut().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    materialize_loaded_dataset_with_cancel(dataset, || is_cancelled())?;
    ensure_not_cancelled(is_cancelled())?;
    Ok((dataset.frame.clone(), dataset.file_name.clone()))
}

#[cfg(test)]
pub(super) fn load_csv_with_progress<F, C>(
    path: &Path,
    mut report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    report("Validando archivo", 10);
    let (_, _, extension) = validate_dataset_file(path)?;
    if extension != "csv" {
        return Err("El lector CSV recibió un formato diferente.".to_owned());
    }
    ensure_not_cancelled(is_cancelled())?;
    report("Leyendo y detectando columnas", 25);
    let frame = read_delimited_frame(path, &extension)?;

    ensure_not_cancelled(is_cancelled())?;
    report("Preparando vista previa", 85);
    let preview = dataset_preview(path, &frame)?;
    report("Preparando sesión", 95);

    Ok((frame, preview))
}

pub(super) fn load_dataset_with_progress<F, C>(
    path: &Path,
    report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    load_dataset_with_header_mode(path, SpreadsheetHeaderMode::FirstRow, report, is_cancelled)
}

pub(super) fn load_dataset_with_header_mode<F, C>(
    path: &Path,
    header_mode: SpreadsheetHeaderMode,
    mut report: F,
    is_cancelled: C,
) -> Result<(DataFrame, DatasetPreview), String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    ensure_not_cancelled(is_cancelled())?;
    report("Validando archivo", 10);
    let (_, file_size_bytes, extension) = validate_dataset_file(path)?;
    ensure_not_cancelled(is_cancelled())?;
    report("Leyendo y detectando columnas", 25);
    ensure_materialization_budget(file_size_bytes)?;

    let frame = match extension.as_str() {
        "csv" | "tsv" | "txt" => {
            read_delimited_frame_with_header_and_cancel(path, &extension, header_mode, || {
                is_cancelled()
            })?
        }
        "parquet" => read_parquet_frame_with_cancel(path, || is_cancelled())?,
        "json" | "jsonl" | "ndjson" => load_json_records_with_cancel(path, || is_cancelled())?,
        extension if spreadsheet_extensions(extension) => {
            return Err("Selecciona primero una hoja del libro.".to_owned());
        }
        _ => unreachable!("la extensión fue validada"),
    };

    ensure_not_cancelled(is_cancelled())?;
    report("Preparando vista previa", 85);
    let preview = dataset_preview(path, &frame)?;
    report("Preparando sesión", 95);
    Ok((frame, preview))
}

#[cfg(test)]
pub(super) fn load_csv(path: &Path) -> Result<(DataFrame, DatasetPreview), String> {
    load_csv_with_progress(path, |_, _| {}, || false)
}
