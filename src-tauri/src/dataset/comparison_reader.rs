use super::*;
use crate::crash_report::LockRecovering;

pub(super) async fn compare_dataset_impl(
    app: AppHandle,
    key_columns: Option<Vec<String>>,
) -> Result<Option<DatasetComparison>, String> {
    let key_columns = normalize_key_columns(key_columns)?;
    let cancellation = DatasetComparisonCancellation::begin(&app);
    cancellation.ensure()?;
    let (current_file_name, current_row_count, current_snapshot, current_source) = {
        let state = app.state::<DatasetState>();
        let current = state.current.lock_recovering();
        let dataset = current.as_ref().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;
        let current_snapshot = current_history_parquet_snapshot(dataset).map(|(path, _, _)| path);
        let current_source = current_duckdb_file_source(dataset).and_then(|(path, _)| {
            dataset_extension(&path)
                .ok()
                .map(|extension| (path, extension))
        });
        (
            dataset.file_name.clone(),
            dataset.row_count,
            current_snapshot,
            current_source,
        )
    };
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

        let state = app.state::<DatasetState>();
        let (current_directory, current_path, current_row_count) = if let Some(current_path) =
            current_snapshot
        {
            (None, current_path, current_row_count)
        } else if let Some((current_path, current_extension)) = current_source {
            let (directory, snapshot_path, row_count) = match persist_comparison_file_with_cancel(
                &current_path,
                &current_extension,
                is_cancelled.clone(),
            ) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    cancellation_for_work.ensure()?;
                    return Err(error);
                }
            };
            (Some(directory), snapshot_path, row_count)
        } else {
            let (current_frame, _) =
                materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
            let row_count = current_frame.height();
            let (directory, snapshot_path) =
                persist_comparison_snapshot_with_cancel(&current_frame, &is_cancelled)?;
            (Some(directory), snapshot_path, row_count)
        };

        let comparison = match compare_parquet_sources_with_cancel(
            &current_path,
            &current_file_name,
            current_row_count,
            &snapshot_path,
            &compared_file_name,
            compared_row_count,
            &key_columns,
            &is_cancelled,
        ) {
            Ok(comparison) => comparison,
            Err(_) => {
                cancellation_for_work.ensure()?;
                let (current_frame, fallback_file_name) =
                    materialize_current_dataset_with_cancel(&state, &is_cancelled)?;
                let fallback_row_count = current_frame.height();
                let (_fallback_directory, fallback_path) =
                    persist_comparison_snapshot_with_cancel(&current_frame, &is_cancelled)?;
                compare_parquet_sources_with_cancel(
                    &fallback_path,
                    &fallback_file_name,
                    fallback_row_count,
                    &snapshot_path,
                    &compared_file_name,
                    compared_row_count,
                    &key_columns,
                    &is_cancelled,
                )?
            }
        };
        let _ = &current_directory;
        cancellation_for_work.ensure()?;
        cancellation_for_work.commit(|| {
            *state.comparison.lock_recovering() = Some(PendingComparison {
                file_name: compared_file_name,
                file_size_bytes,
                row_count: compared_row_count,
                _directory: directory,
                snapshot_path,
                key_columns,
            });
            Ok(Some(comparison))
        })
    })
    .await
    .map_err(|error| format!("La comparación se interrumpió: {error}"))?
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
        let (compared_path, compared_row_count, key_columns) = {
            let comparison = state.comparison.lock_recovering();
            let pending = comparison
                .as_ref()
                .ok_or_else(|| "No hay una comparación activa para paginar.".to_owned())?;
            (
                pending.snapshot_path.clone(),
                pending.row_count,
                pending.key_columns.clone(),
            )
        };
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
    .map_err(|error| format!("La página de conflictos se interrumpió: {error}"))?
}
