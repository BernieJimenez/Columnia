use super::*;

pub(super) async fn get_dataset_profile_impl(
    app: AppHandle,
    on_progress: Channel<OperationProgress>,
    correlation_sample_rows: Option<usize>,
) -> Result<DatasetProfile, String> {
    let correlation_sample_rows = validate_numeric_correlation_sample_rows(
        correlation_sample_rows.unwrap_or(MAX_NUMERIC_CORRELATION_SAMPLE_ROWS),
    )?;
    let generation = app.state::<DatasetState>().begin_profile();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DatasetState>();
        let mut current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos quedó bloqueada inesperadamente.".to_owned())?;
        let dataset = current.as_mut().ok_or_else(|| {
            "No hay un dataset activo. Selecciona primero un archivo compatible.".to_owned()
        })?;

        let cached_source_is_valid = if dataset.source_backed {
            dataset.source_path.as_deref().is_some_and(|path| {
                validate_dataset_file(path)
                    .is_ok_and(|(_, source_size, _)| source_size == dataset.file_size_bytes)
            })
        } else {
            true
        };
        if cached_source_is_valid {
            if let Some(profile) = &dataset.profile {
                let has_enough_numeric_columns = profile
                    .columns
                    .iter()
                    .filter(|column| column.outlier_count.is_some())
                    .nth(1)
                    .is_some();
                let has_group_candidate = profile.columns.iter().any(|column| {
                    column.empty_count.is_some()
                        && column.suggested_type.is_none()
                        && column.privacy_signal.is_none()
                        && column.unique_count >= 2
                });
                let has_temporal_candidate = profile.columns.iter().any(|column| {
                    (column.data_type == "Date" || column.data_type.starts_with("Datetime"))
                        || column.suggested_type.as_deref() == Some("date")
                });
                let expected_sampled_row_count = dataset.row_count.min(correlation_sample_rows);
                let has_requested_numeric_correlations = profile
                    .numeric_correlations
                    .as_ref()
                    .is_some_and(|correlations| {
                        correlations.sampled_row_count == expected_sampled_row_count
                    });
                if (has_requested_numeric_correlations || !has_enough_numeric_columns)
                    && (profile.categorical_group_summaries.is_some() || !has_group_candidate)
                    && (profile.temporal_series.is_some() || !has_temporal_candidate)
                {
                    send_progress(&on_progress, "profile", "Perfil disponible", 100);
                    return Ok(profile.clone());
                }
            }
        }

        if dataset.source_backed {
            let (source_path, source_size, row_count) = current_source_backed_context(dataset)
                .ok_or_else(|| {
                    "La fuente source-backed cambió o ya no está disponible.".to_owned()
                })?;
            let extension = dataset_extension(&source_path)?;
            let profile = profile_source_backed_with_progress(
                &source_path,
                &extension,
                source_size,
                row_count,
                |stage, percent| send_progress(&on_progress, "profile", stage, percent),
                || {
                    app.state::<DatasetState>()
                        .profile_was_cancelled(generation)
                },
                correlation_sample_rows,
            )?;
            dataset.profile = Some(profile.clone());
            return Ok(profile);
        }

        if let Some((snapshot_path, snapshot_size, row_count)) =
            current_history_parquet_snapshot(dataset)
        {
            let profile = profile_source_backed_with_progress(
                &snapshot_path,
                "parquet",
                snapshot_size,
                row_count,
                |stage, percent| send_progress(&on_progress, "profile", stage, percent),
                || {
                    app.state::<DatasetState>()
                        .profile_was_cancelled(generation)
                },
                correlation_sample_rows,
            )?;
            dataset.profile = Some(profile.clone());
            return Ok(profile);
        }

        materialize_loaded_dataset_with_cancel(dataset, || {
            app.state::<DatasetState>()
                .profile_was_cancelled(generation)
        })?;

        let profile = profile_dataset_with_progress(
            &dataset.frame,
            |stage, percent| send_progress(&on_progress, "profile", stage, percent),
            || {
                app.state::<DatasetState>()
                    .profile_was_cancelled(generation)
            },
            correlation_sample_rows,
        )?;
        dataset.profile = Some(profile.clone());
        Ok(profile)
    })
    .await
    .map_err(|error| format!("El análisis de calidad se interrumpió: {error}"))?
}
