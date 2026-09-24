use super::*;
use crate::crash_report::LockRecovering;

pub(super) async fn preview_dataset_selection_impl(
    app: AppHandle,
    selection_id: String,
    sheet_id: Option<String>,
    header_mode: Option<SpreadsheetHeaderMode>,
    expected_profile: Option<ImportProfile>,
    date_convention: Option<ImportDateConvention>,
    number_convention: Option<ImportNumberConvention>,
) -> Result<DatasetImportSchemaPreview, String> {
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
        let cancellation_app = app.clone();
        let is_cancelled = move || {
            cancellation_app
                .state::<DatasetState>()
                .load_was_cancelled(generation)
        };
        ensure_not_cancelled(is_cancelled())?;
        let (path, file_size_bytes, extension) = validate_dataset_file(&pending.path)?;
        if file_size_bytes != pending.file_size_bytes {
            return Err(
                "El archivo cambió después de seleccionarlo; vuelve a elegirlo.".to_owned(),
            );
        }

        let is_delimited = matches!(extension.as_str(), "csv" | "tsv" | "txt");
        let is_spreadsheet = spreadsheet_extensions(&extension);
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
                if profile.header_mode != Some(selected_header_mode) {
                    return Err("Las opciones elegidas no coinciden con el perfil guardado.".to_owned());
                }
            }
            if is_spreadsheet {
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
                        "Las opciones elegidas no coinciden con el perfil guardado.".to_owned(),
                    );
                }
            }
        }

        let (mut frame, row_count, source_backed) = if is_spreadsheet {
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
            if should_defer_source_load(&extension, pending.file_size_bytes)
                && matches!(extension.as_str(), "xlsx" | "xlsb")
            {
                let temporary = tempfile::tempdir().map_err(|error| {
                    format!("No se pudo preparar la revisión temporal del libro: {error}")
                })?;
                let snapshot_path = temporary.path().join("source.parquet");
                let (frame, _, row_count) = source_backed_spreadsheet_load(
                    &path,
                    sheet_name,
                    header_mode,
                    &snapshot_path,
                    is_cancelled.clone(),
                )?;
                (frame, row_count, true)
            } else {
                let frame = load_spreadsheet_sheet_with_cancel(
                    &path,
                    sheet_name,
                    header_mode,
                    is_cancelled.clone(),
                )?;
                let row_count = frame.height();
                (frame, row_count, false)
            }
        } else if is_delimited {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
            }
            if should_defer_source_load(&extension, pending.file_size_bytes) {
                let (frame, _, row_count) = source_backed_load_with_header_mode(
                    &path,
                    &extension,
                    selected_header_mode,
                    is_cancelled.clone(),
                )?;
                (frame, row_count, true)
            } else {
                let (frame, _) = load_dataset_with_header_mode(
                    &path,
                    selected_header_mode,
                    |_, _| {},
                    is_cancelled.clone(),
                )?;
                let row_count = frame.height();
                (frame, row_count, false)
            }
        } else {
            if sheet_id.is_some() {
                return Err("Este formato no utiliza hojas.".to_owned());
            }
            if header_mode.is_some() {
                return Err("Este formato no utiliza opciones de encabezado.".to_owned());
            }
            if should_defer_source_load(&extension, pending.file_size_bytes) {
                if matches!(extension.as_str(), "json" | "jsonl" | "ndjson") {
                    let temporary = tempfile::tempdir().map_err(|error| {
                        format!("No se pudo preparar la revisión temporal del JSON: {error}")
                    })?;
                    let snapshot_path = temporary.path().join("source.parquet");
                    let (frame, _, row_count) = source_backed_json_load(
                        &path,
                        &snapshot_path,
                        is_cancelled.clone(),
                    )?;
                    (frame, row_count, true)
                } else {
                    let (frame, _, row_count) =
                        source_backed_load(&path, &extension, is_cancelled.clone())?;
                    (frame, row_count, true)
                }
            } else {
                let (frame, _) = load_dataset_with_progress(
                    &path,
                    |_, _| {},
                    is_cancelled.clone(),
                )?;
                let row_count = frame.height();
                (frame, row_count, false)
            }
        };

        ensure_not_cancelled(is_cancelled())?;
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
            let conventions_app = app.clone();
            frame = import_conventions::apply_import_conventions(
                &frame,
                date_convention,
                number_convention,
                move || {
                    conventions_app
                        .state::<DatasetState>()
                        .load_was_cancelled(generation)
                },
            )?;
        }
        ensure_not_cancelled(is_cancelled())?;
        let size_after_preview = fs::metadata(&path)
            .map_err(|error| format!("No se pudieron verificar los metadatos del archivo: {error}"))?
            .len();
        if size_after_preview != pending.file_size_bytes {
            return Err("El archivo cambió durante la revisión del esquema; vuelve a elegirlo.".to_owned());
        }
        let schema_mismatch = expected_profile
            .as_ref()
            .and_then(|profile| import_profile_schema_mismatch(profile, &frame));
        let columns = frame
            .columns()
            .iter()
            .map(|column| DatasetColumn {
                name: column.name().to_string(),
                data_type: column.dtype().to_string(),
            })
            .collect();
        Ok(DatasetImportSchemaPreview {
            row_count,
            columns,
            schema_mismatch,
        })
    })
    .await
    .map_err(|error| crate::crash_report::task_interrupted("La revisión del esquema se interrumpió", &error))?
}
