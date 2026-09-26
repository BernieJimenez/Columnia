use super::*;
use crate::crash_report::LockRecovering;

pub(super) async fn pick_dataset_source_impl(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    let selection = app
        .dialog()
        .file()
        .add_filter(
            "Datasets compatibles",
            &[
                "csv", "tsv", "txt", "json", "jsonl", "ndjson", "parquet", "xlsx", "xls", "xlsb",
                "ods",
            ],
        )
        .blocking_pick_file();

    let Some(selection) = selection else {
        return Ok(None);
    };

    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    inspect_dataset_path_impl(&app, path).await.map(Some)
}

pub(super) async fn inspect_dataset_path_impl(
    app: &AppHandle,
    path: PathBuf,
) -> Result<DatasetSourceInspection, String> {
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let state = app.state::<DatasetState>();
    let generation = state.begin_load()?;
    let selection_id = format!("selection-{generation}");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    let format = if spreadsheet_extensions(&extension) {
        "excel"
    } else if extension == "parquet" {
        "parquet"
    } else if matches!(extension.as_str(), "json" | "jsonl" | "ndjson") {
        "json"
    } else if extension == "tsv" {
        "tsv"
    } else {
        "csv"
    };
    state.commit_load(generation, || {
        *state.pending_selection.lock_recovering() = Some(PendingSelection {
            id: selection_id.clone(),
            generation,
            path,
            file_size_bytes,
            sheets: Vec::new(),
        });
        Ok(())
    })?;
    Ok(DatasetSourceInspection {
        selection_id,
        file_name,
        file_size_bytes,
        format,
        sheets: Vec::new(),
        default_sheet_id: if spreadsheet_extensions(&extension) {
            Some("0".to_owned())
        } else {
            None
        },
        is_compressed_container: matches!(extension.as_str(), "xlsx" | "xlsb" | "ods"),
        resource_estimate: dataset_resource_estimate(&extension, file_size_bytes),
    })
}

pub(super) async fn inspect_workbook_sheets_impl(
    app: AppHandle,
    selection_id: String,
) -> Result<Vec<WorkbookSheet>, String> {
    let pending = {
        let state = app.state::<DatasetState>();
        let selection = state.pending_selection.lock_recovering();
        let pending = selection
            .as_ref()
            .cloned()
            .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
        if pending.id != selection_id {
            return Err("La selección no coincide con el archivo pendiente.".to_owned());
        }
        ensure_not_cancelled(state.load_was_cancelled(pending.generation))?;
        pending
    };

    let (path, file_size_bytes, extension) = validate_dataset_file(&pending.path)?;
    if file_size_bytes != pending.file_size_bytes {
        return Err("El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned());
    }
    if !spreadsheet_extensions(&extension) {
        return Err("El archivo seleccionado no es un libro compatible.".to_owned());
    }

    let generation = pending.generation;
    let expected_size = pending.file_size_bytes;
    let cancellation_app = app.clone();
    let sheet_names = tauri::async_runtime::spawn_blocking(move || {
        let state = cancellation_app.state::<DatasetState>();
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let sheets = inspect_workbook(&path)?;
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let size_after_inspection = fs::metadata(&path)
            .map_err(|error| format!("No se pudieron verificar los metadatos del libro: {error}"))?
            .len();
        if size_after_inspection != expected_size {
            return Err(
                "El archivo cambió durante la inspección del libro; vuelve a elegirlo.".to_owned(),
            );
        }
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        Ok(sheets)
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("La inspección del libro se interrumpió", &error)
    })??;

    let workbook_sheets = sheet_names
        .iter()
        .enumerate()
        .map(|(index, name)| WorkbookSheet {
            id: index.to_string(),
            name: name.clone(),
        })
        .collect();
    {
        let state = app.state::<DatasetState>();
        state.commit_load(generation, || {
            let mut selection = state.pending_selection.lock_recovering();
            let pending = selection
                .as_mut()
                .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
            if pending.id != selection_id {
                return Err("La selección ya no corresponde al archivo pendiente.".to_owned());
            }
            pending.sheets = sheet_names;
            Ok(())
        })?;
    }

    Ok(workbook_sheets)
}

/// Consumes the path captured by Tauri's native drag/drop event. The path never
/// crosses the IPC boundary; React receives only the resulting inspection.
pub(super) async fn inspect_dropped_dataset_impl(
    app: AppHandle,
) -> Result<Option<DatasetSourceInspection>, String> {
    let Some(path) = app.state::<DatasetState>().take_dropped_path()? else {
        return Ok(None);
    };
    inspect_dataset_path_impl(&app, path).await.map(Some)
}

pub(super) async fn preview_delimited_header_review_impl(
    app: AppHandle,
    selection_id: String,
) -> Result<DelimitedHeaderReview, String> {
    let state = app.state::<DatasetState>();
    let pending = {
        let selection = state.pending_selection.lock_recovering();
        let pending = selection
            .as_ref()
            .cloned()
            .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
        if pending.id != selection_id {
            return Err("La selección no coincide con el archivo pendiente.".to_owned());
        }
        ensure_not_cancelled(state.load_was_cancelled(pending.generation))?;
        pending
    };

    let generation = pending.generation;
    let expected_size = pending.file_size_bytes;
    let cancellation_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = cancellation_app.state::<DatasetState>();
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let (path, file_size_bytes, extension) = validate_dataset_file(&pending.path)?;
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        if file_size_bytes != expected_size {
            return Err(
                "El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned(),
            );
        }
        if !matches!(extension.as_str(), "csv" | "tsv" | "txt") {
            return Err(
                "La revisión de encabezados solo está disponible para archivos delimitados."
                    .to_owned(),
            );
        }
        let review = delimited_header_review_with_cancel(&path, &extension, || {
            state.load_was_cancelled(generation)
        })?;
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let size_after_preview = fs::metadata(&path)
            .map_err(|error| {
                format!("No se pudieron verificar los metadatos del archivo: {error}")
            })?
            .len();
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        if size_after_preview != expected_size {
            return Err("El archivo cambió durante la vista previa; vuelve a elegirlo.".to_owned());
        }
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        Ok(review)
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted(
            "La vista previa de encabezados se interrumpió",
            &error,
        )
    })?
}

/// Replaces the pending selection with a UTF-8 copy of a Windows-1252 file,
/// only after the person approved it (RV20 / FUN-02). The copy keeps the
/// original file name, so the dataset still shows it, and the original file is
/// never modified.
pub(super) async fn convert_dataset_selection_encoding_impl(
    app: AppHandle,
    selection_id: String,
) -> Result<DatasetSourceInspection, String> {
    let pending = {
        let state = app.state::<DatasetState>();
        let selection = state.pending_selection.lock_recovering();
        let pending = selection
            .as_ref()
            .cloned()
            .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
        if pending.id != selection_id {
            return Err("La selección no coincide con el archivo pendiente.".to_owned());
        }
        ensure_not_cancelled(state.load_was_cancelled(pending.generation))?;
        pending
    };
    let (path, file_size_bytes, extension) = validate_dataset_file(&pending.path)?;
    if file_size_bytes != pending.file_size_bytes {
        return Err("El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned());
    }
    if !matches!(extension.as_str(), "csv" | "tsv" | "txt") {
        return Err("Solo los archivos delimitados se pueden convertir a UTF-8.".to_owned());
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset.csv")
        .to_owned();

    let generation = pending.generation;
    let conversion_app = app.clone();
    let conversion_name = file_name.clone();
    let (directory, converted, converted_size) = tauri::async_runtime::spawn_blocking(move || {
        let state = conversion_app.state::<DatasetState>();
        let directory = tempfile::Builder::new()
            .prefix("columnia-utf8-")
            .tempdir()
            .map_err(|error| format!("No se pudo preparar la copia convertida: {error}"))?;
        let converted = directory.path().join(&conversion_name);
        let bytes =
            convert_windows_1252_file(&path, &converted, &|| state.load_was_cancelled(generation))?;
        Ok::<_, String>((directory, converted, bytes))
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("La conversión a UTF-8 se interrumpió", &error)
    })??;

    let state = app.state::<DatasetState>();
    state.commit_load(generation, || {
        let current = state.current.lock_recovering();
        let mut selection = state.pending_selection.lock_recovering();
        let pending = selection
            .as_mut()
            .filter(|pending| pending.id == selection_id)
            .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
        pending.path = converted.clone();
        pending.file_size_bytes = converted_size;
        let active_source = current
            .as_ref()
            .and_then(|dataset| dataset.source_path.clone());
        let mut converted_sources = state.converted_sources.lock_recovering();
        converted_sources.retain(|kept| {
            active_source
                .as_deref()
                .is_some_and(|source| source.starts_with(kept.path()))
        });
        converted_sources.push(directory);
        Ok(())
    })?;

    Ok(DatasetSourceInspection {
        selection_id,
        file_name,
        file_size_bytes: converted_size,
        format: if extension == "tsv" { "tsv" } else { "csv" },
        sheets: Vec::new(),
        default_sheet_id: None,
        is_compressed_container: false,
        resource_estimate: dataset_resource_estimate(&extension, converted_size),
    })
}
