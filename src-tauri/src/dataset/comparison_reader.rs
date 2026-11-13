use super::*;
use crate::crash_report::LockRecovering;

/// The active dataset as a comparison reads it: its name and rows, and the
/// history snapshot or source file the engine can read from disk, if any.
pub(super) struct CurrentComparisonSide {
    file_name: String,
    row_count: usize,
    snapshot: Option<PathBuf>,
    source: Option<(PathBuf, String)>,
}

pub(super) fn current_comparison_side(app: &AppHandle) -> Result<CurrentComparisonSide, String> {
    let state = app.state::<DatasetState>();
    let current = state.current.lock_recovering();
    let dataset = current.as_ref().ok_or_else(|| {
        "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
    })?;
    let snapshot = current_history_parquet_snapshot(dataset).map(|(path, _, _)| path);
    let source = current_duckdb_file_source(dataset).and_then(|(path, _)| {
        dataset_extension(&path)
            .ok()
            .map(|extension| (path, extension))
    });
    Ok(CurrentComparisonSide {
        file_name: dataset.file_name.clone(),
        row_count: dataset.row_count,
        snapshot,
        source,
    })
}

/// Compares the active dataset with a compared snapshot already on disk, by
/// the given key; from disk when it can, in memory otherwise (ARQ-08).
fn compare_with_current(
    app: &AppHandle,
    current: CurrentComparisonSide,
    compared_path: &Path,
    compared_file_name: &str,
    compared_row_count: usize,
    key_columns: &[String],
    cancellation: &DatasetComparisonCancellation,
) -> Result<DatasetComparison, String> {
    let is_cancelled = cancellation.callback();
    let state = app.state::<DatasetState>();
    let read_from_disk = current.snapshot.is_some() || current.source.is_some();
    let (_current_directory, current_path, current_row_count) = if let Some(current_path) =
        current.snapshot
    {
        (None, current_path, current.row_count)
    } else if let Some((current_path, current_extension)) = current.source {
        let (directory, snapshot_path, row_count) = match persist_comparison_file_with_cancel(
            &current_path,
            &current_extension,
            is_cancelled.clone(),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                cancellation.ensure()?;
                return Err(error);
            }
        };
        (Some(directory), snapshot_path, row_count)
    } else {
        let (current_frame, _) = materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
        let row_count = current_frame.height();
        let (directory, snapshot_path) =
            persist_comparison_snapshot_with_cancel(&current_frame, &is_cancelled)?;
        (Some(directory), snapshot_path, row_count)
    };

    match compare_parquet_sources_with_cancel(
        &current_path,
        &current.file_name,
        current_row_count,
        compared_path,
        compared_file_name,
        compared_row_count,
        key_columns,
        &is_cancelled,
    ) {
        Ok(comparison) => Ok(comparison),
        // ARQ-08: a wrong key or a cancellation is the answer; retrying in
        // memory only repeats the work and hides the cause.
        Err(error) if !read_from_disk || !comparison_may_succeed_in_memory(&error) => {
            cancellation.ensure()?;
            Err(error)
        }
        Err(disk_error) => {
            cancellation.ensure()?;
            let retry = || -> Result<DatasetComparison, String> {
                let (current_frame, fallback_file_name) =
                    materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
                let fallback_row_count = current_frame.height();
                let (_fallback_directory, fallback_path) =
                    persist_comparison_snapshot_with_cancel(&current_frame, &is_cancelled)?;
                compare_parquet_sources_with_cancel(
                    &fallback_path,
                    &fallback_file_name,
                    fallback_row_count,
                    compared_path,
                    compared_file_name,
                    compared_row_count,
                    key_columns,
                    &is_cancelled,
                )
            };
            match retry() {
                Ok(comparison) => Ok(comparison),
                Err(error) if error == OPERATION_CANCELLED_MESSAGE => Err(error),
                Err(memory_error) => Err(format!(
                    "{disk_error} Tampoco se pudo comparar en memoria: {memory_error}"
                )),
            }
        }
    }
}

pub(super) async fn compare_dataset_impl(
    app: AppHandle,
    key_columns: Option<Vec<String>>,
    options: ComparisonOptions,
) -> Result<Option<DatasetComparison>, String> {
    let key_columns = normalize_key_columns(key_columns)?;
    let cancellation = DatasetComparisonCancellation::begin(&app);
    cancellation.ensure()?;
    let current = current_comparison_side(&app)?;
    cancellation.ensure()?;
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
        cancellation.ensure()?;
        return Ok(None);
    };
    let path = selection
        .into_path()
        .map_err(|error| format!("No se pudo resolver la ruta seleccionada: {error}"))?;
    let (path, file_size_bytes, extension) = validate_dataset_file(&path)?;
    let compared_file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dataset")
        .to_owned();
    cancellation.ensure()?;
    let cancellation_for_work = cancellation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _options = ComparisonOptionsScope::enter(options);
        let is_cancelled = cancellation_for_work.callback();
        cancellation_for_work.ensure()?;
        let (directory, snapshot_path, compared_row_count) =
            match persist_comparison_file_with_cancel(&path, &extension, is_cancelled.clone()) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    cancellation_for_work.ensure()?;
                    return Err(error);
                }
            };

        let mut comparison = compare_with_current(
            &app,
            current,
            &snapshot_path,
            &compared_file_name,
            compared_row_count,
            &key_columns,
            &cancellation_for_work,
        )?;
        comparison.compared_source_note = compared_source_note(&path, &extension);
        cancellation_for_work.ensure()?;
        let state = app.state::<DatasetState>();
        cancellation_for_work.commit(|| {
            clear_conflict_index();
            *state.comparison.lock_recovering() = Some(PendingComparison {
                file_name: compared_file_name,
                file_size_bytes,
                row_count: compared_row_count,
                _directory: directory,
                snapshot_path,
                key_columns,
                options,
            });
            Ok(Some(comparison))
        })
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("La comparación se interrumpió", &error)
    })?
}

/// Preparar: repeats the active comparison against the dataset as it is now,
/// with the same compared file, key and options, after a correction or an
/// undo. `None` when there is no comparison to repeat, or another one replaced
/// it meanwhile. The compared file is not read again: its snapshot stays.
pub(super) async fn refresh_dataset_comparison_impl(
    app: AppHandle,
) -> Result<Option<DatasetComparison>, String> {
    let cancellation = DatasetComparisonCancellation::begin(&app);
    cancellation.ensure()?;
    let pending = {
        let state = app.state::<DatasetState>();
        let comparison = state.comparison.lock_recovering();
        comparison.as_ref().map(|pending| {
            (
                pending.snapshot_path.clone(),
                pending.file_name.clone(),
                pending.row_count,
                pending.key_columns.clone(),
                pending.options,
            )
        })
    };
    let Some((snapshot_path, compared_file_name, compared_row_count, key_columns, options)) =
        pending
    else {
        return Ok(None);
    };
    let current = current_comparison_side(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _options = ComparisonOptionsScope::enter(options);
        cancellation.ensure()?;
        let comparison = compare_with_current(
            &app,
            current,
            &snapshot_path,
            &compared_file_name,
            compared_row_count,
            &key_columns,
            &cancellation,
        )?;
        let state = app.state::<DatasetState>();
        cancellation.commit(|| {
            let still_active = state
                .comparison
                .lock_recovering()
                .as_ref()
                .is_some_and(|pending| {
                    pending.snapshot_path == snapshot_path && pending.key_columns == key_columns
                });
            if !still_active {
                return Ok(None);
            }
            // The conflict pages belong to the dataset compared before.
            clear_conflict_index();
            Ok(Some(comparison))
        })
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted(
            "La actualización de la comparación se interrumpió",
            &error,
        )
    })?
}

/// ARQ-08: errors a second, in-memory attempt cannot fix: the person must
/// change the keys, or asked to stop.
pub(super) fn comparison_may_succeed_in_memory(error: &str) -> bool {
    error != OPERATION_CANCELLED_MESSAGE
        && !error.starts_with("La columna clave")
        && !error.starts_with("Las columnas clave")
        && !error.starts_with("La comparación admite como máximo")
}

pub(super) async fn get_dataset_conflict_page_impl(
    app: AppHandle,
    offset: usize,
    limit: usize,
) -> Result<Option<DatasetConflictPage>, String> {
    if limit == 0 || limit > MAX_CONFLICT_PREVIEW {
        return Err(format!(
            "El tamaño de página de conflictos debe estar entre 1 y {MAX_CONFLICT_PREVIEW}."
        ));
    }
    let cancellation = DatasetComparisonCancellation::begin(&app);
    let is_cancelled = cancellation.callback();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_not_cancelled(is_cancelled())?;
        let state = app.state::<DatasetState>();
        let (compared_path, compared_row_count, key_columns, options) = {
            let comparison = state.comparison.lock_recovering();
            let pending = comparison
                .as_ref()
                .ok_or_else(|| "No hay una comparación activa para paginar.".to_owned())?;
            (
                pending.snapshot_path.clone(),
                pending.row_count,
                pending.key_columns.clone(),
                pending.options,
            )
        };
        let _options = ComparisonOptionsScope::enter(options);
        let page = if let Some(page) = disk_backed_conflict_page_with_cancel(
            &state,
            &compared_path,
            compared_row_count,
            &key_columns,
            offset,
            limit,
            is_cancelled.clone(),
        )? {
            page
        } else {
            let (current_frame, _) =
                materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
            ensure_not_cancelled(is_cancelled())?;
            let current_columns = current_frame
                .get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let compared_schema = read_parquet_schema_frame(&compared_path)?;
            ensure_not_cancelled(is_cancelled())?;
            let compared_columns = compared_schema
                .get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let shared_columns = current_columns
                .iter()
                .filter(|name| compared_columns.contains(name))
                .cloned()
                .collect::<Vec<_>>();
            let (conflicts, has_next) = collect_key_conflicts_page_from_parquet_with_cancel(
                &current_frame,
                &compared_path,
                compared_row_count,
                &key_columns,
                &shared_columns,
                offset,
                limit,
                &is_cancelled,
            )?;
            DatasetConflictPage {
                offset,
                conflicts: conflicts.into_iter().map(|item| item.conflict).collect(),
                has_next,
            }
        };
        ensure_not_cancelled(is_cancelled())?;
        let comparison_is_current = {
            let comparison = state.comparison.lock_recovering();
            comparison.as_ref().is_some_and(|pending| {
                pending.snapshot_path == compared_path
                    && pending.row_count == compared_row_count
                    && pending.key_columns == key_columns
            })
        };
        if !comparison_is_current {
            return Err("La comparación cambió durante la lectura de conflictos.".to_owned());
        }
        cancellation.commit(|| Ok(Some(page)))
    })
    .await
    .map_err(|error| {
        crate::crash_report::task_interrupted("La página de conflictos se interrumpió", &error)
    })?
}

/// How the compared file was read (FUN-41): workbooks use their first sheet
/// and every delimited or workbook file its first row as the header.
pub(super) fn compared_source_note(path: &Path, extension: &str) -> Option<String> {
    if spreadsheet_extensions(extension) {
        let sheets = inspect_workbook(path).ok()?;
        let first = sheets.first()?;
        return Some(if sheets.len() > 1 {
            format!(
                "Se comparó la hoja «{first}», la primera de {} del libro, con la primera fila como encabezado.",
                sheets.len()
            )
        } else {
            format!("Se comparó la hoja «{first}» con la primera fila como encabezado.")
        });
    }
    matches!(extension, "csv" | "tsv" | "txt").then(|| {
        "Se leyó con la primera fila como encabezado y sin convenciones de importación.".to_owned()
    })
}
