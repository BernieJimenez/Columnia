use std::collections::{HashMap, HashSet};

use tauri::{ipc::Channel, AppHandle, Manager};

use super::{
    ensure_not_cancelled, evaluate_quality_rules_with_cancel, profile_dataset_with_progress,
    send_progress, validate_history_entry_id, validate_quality_rules_payload, DataFrame,
    DatasetProfile, DatasetState, OperationProgress, QualityRule, QualityRuleKind,
    SnapshotColumnComparison, SnapshotColumnSummary, SnapshotQualityComparison,
    SnapshotQualityRuleComparison, SnapshotRevisionComparison, SnapshotRevisionDeltas,
    SnapshotRevisionSummary, MAX_NUMERIC_CORRELATION_SAMPLE_ROWS, QUALITY_DATASET_COLUMN,
};

pub(super) async fn compare_history_snapshots_impl(
    app: AppHandle,
    before_snapshot_id: String,
    after_snapshot_id: String,
    quality_rules: Vec<QualityRule>,
    on_progress: Channel<OperationProgress>,
) -> Result<SnapshotRevisionComparison, String> {
    validate_quality_rules_payload(&quality_rules)?;
    validate_history_entry_id(&before_snapshot_id)?;
    validate_history_entry_id(&after_snapshot_id)?;
    if before_snapshot_id == after_snapshot_id {
        return Err("Selecciona dos revisiones distintas para compararlas.".to_owned());
    }

    let generation = app.state::<DatasetState>().begin_snapshot_comparison();
    tauri::async_runtime::spawn_blocking(move || {
        let (before_frame, before_label, after_frame, after_label) = {
            let state = app.state::<DatasetState>();
            let current = state
                .current
                .lock()
                .map_err(|_| "La sesión de datos no está disponible.".to_owned())?;
            let dataset = current
                .as_ref()
                .ok_or_else(|| "No hay un dataset activo para comparar revisiones.".to_owned())?;
            if !dataset.history.snapshots_enabled {
                return Err(dataset.history.degraded_reason.clone().unwrap_or_else(|| {
                    "La comparación no está disponible porque el historial está desactivado."
                        .to_owned()
                }));
            }
            let is_cancelled = || state.snapshot_comparison_was_cancelled(generation);
            let (before_frame, before_label) = dataset
                .history
                .restore_by_id_with_cancel(&before_snapshot_id, &is_cancelled)?;
            let (after_frame, after_label) = dataset
                .history
                .restore_by_id_with_cancel(&after_snapshot_id, &is_cancelled)?;
            (before_frame, before_label, after_frame, after_label)
        };

        let state = app.state::<DatasetState>();
        let is_cancelled = || state.snapshot_comparison_was_cancelled(generation);
        let comparison = compare_snapshot_frames(
            before_snapshot_id.clone(),
            after_snapshot_id.clone(),
            before_label,
            after_label,
            &before_frame,
            &after_frame,
            &quality_rules,
            |stage, percent| send_progress(&on_progress, "profile", stage, percent),
            &is_cancelled,
        )?;
        ensure_not_cancelled(state.snapshot_comparison_was_cancelled(generation))?;

        let current = state
            .current
            .lock()
            .map_err(|_| "La sesión de datos no está disponible.".to_owned())?;
        let dataset = current
            .as_ref()
            .ok_or_else(|| "El dataset cambió durante la comparación.".to_owned())?;
        if !dataset.history.contains_id(&before_snapshot_id)
            || !dataset.history.contains_id(&after_snapshot_id)
        {
            return Err(
                "La comparación quedó obsoleta porque una revisión salió del historial.".to_owned(),
            );
        }
        Ok(comparison)
    })
    .await
    .map_err(|error| format!("La comparación de revisiones se interrumpió: {error}"))?
}

fn snapshot_count_delta(before: usize, after: usize) -> Option<i64> {
    let before = i64::try_from(before).ok()?;
    let after = i64::try_from(after).ok()?;
    after.checked_sub(before)
}

fn snapshot_rule_columns(rule: &QualityRule) -> Vec<String> {
    let mut columns = Vec::new();
    if !matches!(
        rule.kind,
        QualityRuleKind::SchemaContract | QualityRuleKind::RowCount
    ) && !rule.column.is_empty()
        && rule.column != QUALITY_DATASET_COLUMN
    {
        columns.push(rule.column.clone());
    }
    if let Some(names) = rule.columns.as_deref() {
        columns.extend(names.iter().cloned());
    }
    if let Some(condition) = rule.when.as_ref() {
        columns.push(condition.column.clone());
    }
    if let Some(then) = rule.then.as_deref() {
        columns.extend(snapshot_rule_columns(then));
    }
    columns.sort();
    columns.dedup();
    columns
}

fn snapshot_rule_missing_columns(rule: &QualityRule, frame: &DataFrame) -> Vec<String> {
    let actual = frame
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .collect::<HashSet<_>>();
    snapshot_rule_columns(rule)
        .into_iter()
        .filter(|name| !actual.contains(name.as_str()))
        .collect()
}

fn snapshot_quality_comparison<C>(
    before: &DataFrame,
    after: &DataFrame,
    rules: &[QualityRule],
    is_cancelled: C,
) -> Result<SnapshotQualityComparison, String>
where
    C: Fn() -> bool + Sync,
{
    validate_quality_rules_payload(rules)?;
    let mut results = Vec::with_capacity(rules.len());
    for (index, rule) in rules.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        let before_missing = snapshot_rule_missing_columns(rule, before);
        let after_missing = snapshot_rule_missing_columns(rule, after);
        let missing = before_missing
            .iter()
            .chain(after_missing.iter())
            .cloned()
            .collect::<HashSet<_>>();
        if !missing.is_empty() {
            let mut missing = missing.into_iter().collect::<Vec<_>>();
            missing.sort();
            results.push(SnapshotQualityRuleComparison {
                rule_index: index + 1,
                kind: rule.kind,
                column: rule.column.clone(),
                comparable: false,
                reason: Some(format!(
                    "La regla requiere columnas que faltan en una revisión: {}.",
                    missing.join(", ")
                )),
                before_invalid_count: None,
                after_invalid_count: None,
                before_invalid_percentage: None,
                after_invalid_percentage: None,
                before_passed: None,
                after_passed: None,
            });
            continue;
        }
        let before_result =
            evaluate_quality_rules_with_cancel(before, std::slice::from_ref(rule), &is_cancelled)?;
        let after_result =
            evaluate_quality_rules_with_cancel(after, std::slice::from_ref(rule), &is_cancelled)?;
        let before_result = before_result
            .rules
            .first()
            .ok_or_else(|| "No se pudo calcular la regla sobre la primera revisión.".to_owned())?;
        let after_result = after_result
            .rules
            .first()
            .ok_or_else(|| "No se pudo calcular la regla sobre la segunda revisión.".to_owned())?;
        results.push(SnapshotQualityRuleComparison {
            rule_index: index + 1,
            kind: rule.kind,
            column: rule.column.clone(),
            comparable: true,
            reason: None,
            before_invalid_count: Some(before_result.invalid_count),
            after_invalid_count: Some(after_result.invalid_count),
            before_invalid_percentage: Some(before_result.invalid_pct),
            after_invalid_percentage: Some(after_result.invalid_pct),
            before_passed: Some(before_result.passed),
            after_passed: Some(after_result.passed),
        });
    }

    let comparable = results
        .iter()
        .filter(|rule| rule.comparable)
        .collect::<Vec<_>>();
    Ok(SnapshotQualityComparison {
        configured_rule_count: rules.len(),
        comparable_rule_count: comparable.len(),
        non_comparable_rule_count: rules.len().saturating_sub(comparable.len()),
        improved_rule_count: comparable
            .iter()
            .filter(|rule| {
                rule.before_invalid_percentage
                    .zip(rule.after_invalid_percentage)
                    .is_some_and(|(before, after)| after < before)
            })
            .count(),
        degraded_rule_count: comparable
            .iter()
            .filter(|rule| {
                rule.before_invalid_percentage
                    .zip(rule.after_invalid_percentage)
                    .is_some_and(|(before, after)| after > before)
            })
            .count(),
        before_passed_rule_count: comparable
            .iter()
            .filter(|rule| rule.before_passed == Some(true))
            .count(),
        after_passed_rule_count: comparable
            .iter()
            .filter(|rule| rule.after_passed == Some(true))
            .count(),
        rules: results,
    })
}

fn profile_snapshot_comparison_side<F, C>(
    frame: &DataFrame,
    mut report: F,
    is_cancelled: C,
) -> Result<DatasetProfile, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    // Recalcula sobre el Parquet inmutable del historial. No consulta ni modifica
    // el perfil cacheado del dataset activo.
    profile_dataset_with_progress(
        frame,
        |stage, percent| report(stage, percent),
        is_cancelled,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
}

pub(super) fn compare_snapshot_frames<F, C>(
    before_snapshot_id: String,
    after_snapshot_id: String,
    before_label: String,
    after_label: String,
    before_frame: &DataFrame,
    after_frame: &DataFrame,
    quality_rules: &[QualityRule],
    mut report: F,
    is_cancelled: C,
) -> Result<SnapshotRevisionComparison, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Sync,
{
    if before_snapshot_id == after_snapshot_id {
        return Err("Selecciona dos revisiones distintas para compararlas.".to_owned());
    }
    validate_quality_rules_payload(quality_rules)?;
    let before_profile = profile_snapshot_comparison_side(
        before_frame,
        |stage, percent| report(stage, percent / 2),
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let after_profile = profile_snapshot_comparison_side(
        after_frame,
        |stage, percent| report(stage, 50 + percent / 2),
        &is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled())?;
    let quality =
        snapshot_quality_comparison(before_frame, after_frame, quality_rules, &is_cancelled)?;

    let before_summary = SnapshotRevisionSummary {
        row_count: before_profile.row_count,
        column_count: before_profile.columns.len(),
        null_count: before_profile
            .columns
            .iter()
            .map(|column| column.null_count)
            .sum(),
        invalid_type_count: before_profile
            .columns
            .iter()
            .map(|column| column.invalid_type_count.unwrap_or_default())
            .sum(),
        duplicate_row_count: before_profile.duplicate_row_count,
    };
    let after_summary = SnapshotRevisionSummary {
        row_count: after_profile.row_count,
        column_count: after_profile.columns.len(),
        null_count: after_profile
            .columns
            .iter()
            .map(|column| column.null_count)
            .sum(),
        invalid_type_count: after_profile
            .columns
            .iter()
            .map(|column| column.invalid_type_count.unwrap_or_default())
            .sum(),
        duplicate_row_count: after_profile.duplicate_row_count,
    };

    let before_columns = before_profile
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();
    let after_columns = after_profile
        .columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();
    let mut names = before_profile
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    for column in &after_profile.columns {
        if !before_columns.contains_key(column.name.as_str()) {
            names.push(column.name.clone());
        }
    }
    let columns = names
        .into_iter()
        .map(|name| {
            let before = before_columns.get(name.as_str()).map(|column| {
                SnapshotColumnSummary {
                    data_type: column.data_type.clone(),
                    null_count: column.null_count,
                    invalid_type_count: column.invalid_type_count.unwrap_or_default(),
                }
            });
            let after = after_columns.get(name.as_str()).map(|column| {
                SnapshotColumnSummary {
                    data_type: column.data_type.clone(),
                    null_count: column.null_count,
                    invalid_type_count: column.invalid_type_count.unwrap_or_default(),
                }
            });
            let comparable = before.is_some() && after.is_some();
            SnapshotColumnComparison {
                name: name.clone(),
                comparable,
                reason: (!comparable).then(|| {
                    "La columna existe solo en una de las dos revisiones; no se comparan sus métricas.".to_owned()
                }),
                type_changed: before
                    .as_ref()
                    .zip(after.as_ref())
                    .map(|(before, after)| before.data_type != after.data_type),
                null_count_delta: before
                    .as_ref()
                    .zip(after.as_ref())
                    .and_then(|(before, after)| snapshot_count_delta(before.null_count, after.null_count)),
                invalid_type_count_delta: before
                    .as_ref()
                    .zip(after.as_ref())
                    .and_then(|(before, after)| snapshot_count_delta(before.invalid_type_count, after.invalid_type_count)),
                before,
                after,
            }
        })
        .collect();

    let deltas = SnapshotRevisionDeltas {
        row_count: snapshot_count_delta(before_summary.row_count, after_summary.row_count),
        column_count: snapshot_count_delta(before_summary.column_count, after_summary.column_count),
        null_count: snapshot_count_delta(before_summary.null_count, after_summary.null_count),
        invalid_type_count: snapshot_count_delta(
            before_summary.invalid_type_count,
            after_summary.invalid_type_count,
        ),
        duplicate_row_count: snapshot_count_delta(
            before_summary.duplicate_row_count,
            after_summary.duplicate_row_count,
        ),
    };
    report("Comparación agregada completada", 100);

    Ok(SnapshotRevisionComparison {
        before_snapshot_id,
        after_snapshot_id,
        before_label,
        after_label,
        before: before_summary,
        after: after_summary,
        deltas,
        columns,
        quality,
    })
}
