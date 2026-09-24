use super::*;
use crate::crash_report::LockRecovering;

#[allow(clippy::too_many_arguments)]
pub(super) async fn load_dataset_selection_impl(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
    expected_profile: Option<ImportProfile>,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
    on_progress: Channel<OperationProgress>,
) -> Result<DatasetPreview, String> {
    let generation = app.state::<DatasetState>().begin_load()?;
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
        pending
    };
    tauri::async_runtime::spawn_blocking(move || {
        send_progress(&on_progress, "load", "Validando archivo", 10);
        let current_size = validate_dataset_file(&pending.path)?.1;
        if current_size != pending.file_size_bytes {
            return Err(
                "El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned(),
            );
        }
        let extension = dataset_extension(&pending.path)?;
        let is_delimited = matches!(extension.as_str(), "csv" | "tsv" | "txt");
        let selected_header_mode = if is_delimited {
            header_mode.unwrap_or(SpreadsheetHeaderMode::FirstRow)
        } else {
            SpreadsheetHeaderMode::FirstRow
        };
        if let Some(profile) = expected_profile.as_ref() {
            validate_import_profile(profile)?;
            if profile.format != import_format_for_extension(&extension) {
                return Err(
                    "El perfil guardado no corresponde al formato de este archivo.".to_owned(),
                );
            }
            if is_delimited {
                import_conventions::validate_profile_conventions(
                    profile,
                    date_convention,
                    number_convention,
                )?;
            }
            if is_delimited && profile.header_mode != Some(selected_header_mode) {
                return Err("Las opciones elegidas no coinciden con el perfil guardado.".to_owned());
            }
            if spreadsheet_extensions(&extension) {
                let index = sheet_id
                    .as_deref()
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| "El perfil requiere elegir la hoja guardada.".to_owned())?;
                let selected_sheet = pending
                    .sheets
                    .get(index)
                    .ok_or_else(|| "La hoja guardada ya no está disponible.".to_owned())?;
                if profile.sheet_name.as_deref() != Some(selected_sheet.as_str())
                    || profile.header_mode != header_mode
                {
                    return Err(
                        "Las opciones elegidas no coinciden con el perfil guardado.".to_owned()
                    );
                }
            }
        }
        let mut deferred_history = None;
        let (mut frame, mut preview, row_count, source_backed) = if spreadsheet_extensions(&extension) {
            let header_mode = header_mode
                .ok_or_else(|| "Elige cómo interpretar los encabezados del libro.".to_owned())?;
            let index = sheet_id
                .as_deref()
                .ok_or_else(|| "Selecciona una hoja del libro.".to_owned())?
                .parse::<usize>()
                .map_err(|_| "La hoja seleccionada no es válida.".to_owned())?;
            let sheet_name = pending
                .sheets
                .get(index)
                .ok_or_else(|| "La hoja seleccionada no existe en el libro.".to_owned())?;
            send_progress(&on_progress, "load", "Leyendo hoja", 25);
            if should_defer_source_load(&extension, pending.file_size_bytes)
                && matches!(extension.as_str(), "xlsx" | "xlsb")
            {
                let mut history = HistoryManager::deferred()?;
                let snapshot_path = history.directory.path().join("source.parquet");
                let cancellation_app = app.clone();
                send_progress(
                    &on_progress,
                    "load",
                    "Construyendo snapshot por bloques",
                    35,
                );
                let (frame, preview, row_count) = source_backed_spreadsheet_load(
                    &pending.path,
                    sheet_name,
                    header_mode,
                    &snapshot_path,
                    move || {
                        cancellation_app
                            .state::<DatasetState>()
                            .load_was_cancelled(generation)
                    },
                )?;
                history.source_snapshot_path = Some(snapshot_path);
                deferred_history = Some(history);
                send_progress(&on_progress, "load", "Preparando vista previa", 85);
                (frame, preview, row_count, true)
            } else {
                let cancellation_app = app.clone();
                let frame = load_spreadsheet_sheet_with_cancel(
                    &pending.path,
                    sheet_name,
                    header_mode,
                    move || {
                        cancellation_app
                            .state::<DatasetState>()
                            .load_was_cancelled(generation)
                    },
                )?;
                ensure_not_cancelled(app.state::<DatasetState>().load_was_cancelled(generation))?;
                send_progress(&on_progress, "load", "Preparando vista previa", 85);
                let preview = dataset_preview(&pending.path, &frame)?;
                let row_count = frame.height();
                (frame, preview, row_count, false)
            }
        } else if is_delimited {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
            }
            if should_defer_source_load(&extension, pending.file_size_bytes) {
                send_progress(
                    &on_progress,
                    "load",
                    "Inspeccionando estructura en disco",
                    25,
                );
                let cancellation_app = app.clone();
                let cancellation = move || {
                    cancellation_app
                        .state::<DatasetState>()
                        .load_was_cancelled(generation)
                };
                let (frame, preview, row_count) = source_backed_load_with_header_mode(
                    &pending.path,
                    &extension,
                    selected_header_mode,
                    cancellation,
                )?;
                send_progress(&on_progress, "load", "Preparando vista previa", 85);
                (frame, preview, row_count, true)
            } else {
                let cancellation_app = app.clone();
                let (frame, preview) = load_dataset_with_header_mode(
                    &pending.path,
                    selected_header_mode,
                    |stage, percent| send_progress(&on_progress, "load", stage, percent),
                    move || {
                        cancellation_app
                            .state::<DatasetState>()
                            .load_was_cancelled(generation)
                    },
                )?;
                let row_count = frame.height();
                (frame, preview, row_count, false)
            }
        } else {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
            }
            if header_mode.is_some() {
                return Err("Este formato no utiliza opciones de encabezado.".to_owned());
            }
            if should_defer_source_load(&extension, pending.file_size_bytes) {
                send_progress(
                    &on_progress,
                    "load",
                    "Inspeccionando estructura en disco",
                    25,
                );
                let cancellation_app = app.clone();
                let cancellation = move || {
                    cancellation_app
                        .state::<DatasetState>()
                        .load_was_cancelled(generation)
                };
                let result = if matches!(extension.as_str(), "json" | "jsonl" | "ndjson") {
                    let mut history = HistoryManager::deferred()?;
                    let snapshot_path = history.directory.path().join("source.parquet");
                    send_progress(
                        &on_progress,
                        "load",
                        "Construyendo snapshot JSON por bloques",
                        35,
                    );
                    let result =
                        source_backed_json_load(&pending.path, &snapshot_path, cancellation)?;
                    history.source_snapshot_path = Some(snapshot_path);
                    deferred_history = Some(history);
                    result
                } else {
                    source_backed_load(&pending.path, &extension, cancellation)?
                };
                let (frame, preview, row_count) = result;
                send_progress(&on_progress, "load", "Preparando vista previa", 85);
                (frame, preview, row_count, true)
            } else {
                let cancellation_app = app.clone();
                let (frame, preview) = load_dataset_with_progress(
                    &pending.path,
                    |stage, percent| send_progress(&on_progress, "load", stage, percent),
                    move || {
                        cancellation_app
                            .state::<DatasetState>()
                            .load_was_cancelled(generation)
                    },
                )?;
                let row_count = frame.height();
                (frame, preview, row_count, false)
            }
        };
        let state = app.state::<DatasetState>();
        ensure_not_cancelled(state.load_was_cancelled(generation))?;
        let conventions_selected = date_convention
            .is_some_and(|value| value != ImportDateConvention::Unresolved)
            || number_convention
                .is_some_and(|value| value != ImportNumberConvention::Unresolved);
        if is_delimited && conventions_selected {
            if source_backed {
                return Err(
                    "Las convenciones de fecha y número requieren una carga en memoria; este archivo supera el límite de materialización segura. Importa sin convenciones o usa un archivo más pequeño."
                        .to_owned(),
                );
            }
            let cancellation_app = app.clone();
            frame = import_conventions::apply_import_conventions(
                &frame,
                date_convention,
                number_convention,
                || {
                    cancellation_app
                        .state::<DatasetState>()
                        .load_was_cancelled(generation)
                },
            )?;
            preview = dataset_preview(&pending.path, &frame)?;
        }
        if let Some(profile) = expected_profile.as_ref() {
            if let Some(mismatch) = import_profile_schema_mismatch(profile, &frame) {
                let details = serde_json::to_string(&mismatch).map_err(|_| {
                    "No se pudo comparar el esquema del perfil de importación.".to_owned()
                })?;
                return Err(format!("{IMPORT_PROFILE_MISMATCH_PREFIX}{details}"));
            }
        }
        let history = if source_backed {
            deferred_history.unwrap_or(HistoryManager::deferred()?)
        } else {
            HistoryManager::new(&frame)?
        };
        let loaded = LoadedDataset {
            source_path: Some(pending.path.clone()),
            file_name: pending
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("dataset.csv")
                .to_owned(),
            file_size_bytes: pending.file_size_bytes,
            row_count,
            frame,
            source_backed,
            delimited_header_mode: is_delimited.then_some(selected_header_mode),
            profile: None,
            history,
        };
        state.commit_load(generation, || {
            let mut current = state
                .current
                .lock_recovering();
            let mut comparison = state
                .comparison
                .lock_recovering();
            let mut selection = state
                .pending_selection
                .lock_recovering();
            let pending_selection = selection
                .as_ref()
                .ok_or_else(|| "La selección caducó; vuelve a elegir el archivo.".to_owned())?;
            if pending_selection.id != selection_id {
                return Err("La selección ya no corresponde al archivo pendiente.".to_owned());
            }

            *current = Some(loaded);
            *comparison = None;
            *selection = None;
            Ok(())
        })?;
        send_progress(&on_progress, "load", "Dataset listo", 100);
        Ok(preview)
    })
    .await
    .map_err(|error| crate::crash_report::task_interrupted("La carga del dataset se interrumpió", &error))?
}

pub(super) fn discard_dataset_selection_impl(
    state: State<'_, DatasetState>,
    selection_id: String,
) -> Result<(), String> {
    let mut selection = state.pending_selection.lock_recovering();
    if selection
        .as_ref()
        .is_some_and(|pending| pending.id == selection_id)
    {
        *selection = None;
    }
    let _ = state.take_dropped_path();
    Ok(())
}
