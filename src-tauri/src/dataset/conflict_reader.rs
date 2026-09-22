use super::*;

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
            let comparison = state
                .comparison
                .lock()
                .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?;
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
            let comparison = state
                .comparison
                .lock()
                .map_err(|_| "La comparación quedó bloqueada inesperadamente.".to_owned())?;
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
