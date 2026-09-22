use super::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum GroupKey {
    Missing,
    Value(String),
}

pub(super) fn categorical_group_key(value: AnyValue<'_>) -> Option<GroupKey> {
    match value {
        AnyValue::Null => Some(GroupKey::Missing),
        AnyValue::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                Some(GroupKey::Missing)
            } else if value.chars().count() <= MAX_GROUP_LABEL_CHARS {
                Some(GroupKey::Value(value.to_owned()))
            } else {
                None
            }
        }
        AnyValue::StringOwned(value) => {
            let value = value.as_str().trim();
            if value.is_empty() {
                Some(GroupKey::Missing)
            } else if value.chars().count() <= MAX_GROUP_LABEL_CHARS {
                Some(GroupKey::Value(value.to_owned()))
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(super) fn categorical_group_label(key: &GroupKey) -> String {
    match key {
        GroupKey::Missing => "Sin valor".to_owned(),
        GroupKey::Value(value) => value.clone(),
    }
}

pub(super) fn retain_group_candidate(counts: &mut HashMap<GroupKey, usize>, key: GroupKey) {
    if let Some(count) = counts.get_mut(&key) {
        *count = (*count).saturating_add(1);
        return;
    }
    if counts.len() < MAX_GROUP_CANDIDATES {
        counts.insert(key, 1);
        return;
    }

    let Some((least_key, least_count)) = counts
        .iter()
        .min_by_key(|(_, count)| **count)
        .map(|(key, count)| (key.clone(), *count))
    else {
        return;
    };
    counts.remove(&least_key);
    counts.insert(key, least_count.saturating_add(1));
}

fn finish_categorical_group_summary(
    column: &str,
    row_count: usize,
    distinct_count: usize,
    selected_counts: HashMap<GroupKey, usize>,
) -> Option<CategoricalGroupSummary> {
    let mut groups = selected_counts
        .into_iter()
        .filter(|(_, count)| *count >= MIN_GROUP_COUNT)
        .map(|(key, group_row_count)| CategoricalGroup {
            label: categorical_group_label(&key),
            row_count: group_row_count,
            percentage: if row_count == 0 {
                0.0
            } else {
                (group_row_count as f64 / row_count as f64) * 100.0
            },
            is_other: false,
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right
            .row_count
            .cmp(&left.row_count)
            .then_with(|| left.label.cmp(&right.label))
    });
    groups.truncate(MAX_CATEGORICAL_GROUPS);

    let displayed_count = groups.iter().map(|group| group.row_count).sum::<usize>();
    let other_count = row_count.saturating_sub(displayed_count);
    if other_count > 0 {
        groups.push(CategoricalGroup {
            label: "Resto".to_owned(),
            row_count: other_count,
            percentage: if row_count == 0 {
                0.0
            } else {
                (other_count as f64 / row_count as f64) * 100.0
            },
            is_other: true,
        });
    }

    if groups.is_empty() {
        return None;
    }
    Some(CategoricalGroupSummary {
        column: column.to_owned(),
        groups,
        distinct_count,
        truncated: other_count > 0,
    })
}

fn categorical_group_summary<C>(
    frame: &DataFrame,
    column: &Column,
    profile: &ColumnProfile,
    is_cancelled: &C,
) -> Result<Option<CategoricalGroupSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut candidates = HashMap::with_capacity(MAX_GROUP_CANDIDATES);
    for row_index in 0..frame.height() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column
            .get(row_index)
            .map_err(|error| format!("No se pudo resumir la columna {}: {error}", profile.name))?;
        if let Some(key) = categorical_group_key(value) {
            retain_group_candidate(&mut candidates, key);
        }
    }
    ensure_not_cancelled(is_cancelled())?;

    let mut selected_counts = HashMap::with_capacity(candidates.len());
    for row_index in 0..frame.height() {
        if row_index % 4096 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column
            .get(row_index)
            .map_err(|error| format!("No se pudo resumir la columna {}: {error}", profile.name))?;
        let Some(key) = categorical_group_key(value) else {
            continue;
        };
        if candidates.contains_key(&key) {
            let count = selected_counts.entry(key).or_insert(0usize);
            *count = (*count).saturating_add(1);
        }
    }

    Ok(finish_categorical_group_summary(
        &profile.name,
        frame.height(),
        profile.unique_count,
        selected_counts,
    ))
}

pub(super) fn categorical_group_summaries<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
) -> Result<Option<Vec<CategoricalGroupSummary>>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut summaries = Vec::new();
    for (column, profile) in frame.columns().iter().zip(profiles) {
        if summaries.len() >= MAX_CATEGORICAL_GROUP_COLUMNS {
            break;
        }
        // A category summary is useful for text dimensions, but raw identifiers,
        // contact fields and semantic numbers belong to other profile views.
        if column.dtype() != &DataType::String
            || profile.empty_count.is_none()
            || profile.suggested_type.is_some()
            || profile.privacy_signal.is_some()
            || profile.unique_count < 2
        {
            continue;
        }
        if let Some(summary) = categorical_group_summary(frame, column, profile, is_cancelled)? {
            summaries.push(summary);
        }
    }
    Ok((!summaries.is_empty()).then_some(summaries))
}

pub(super) fn source_categorical_group_summary<C>(
    path: &Path,
    row_count: usize,
    profile: &ColumnProfile,
    candidates: &HashMap<GroupKey, usize>,
    is_cancelled: &C,
) -> Result<Option<CategoricalGroupSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    if candidates.is_empty() {
        return Ok(None);
    }
    ensure_not_cancelled(is_cancelled())?;

    let mut selected_counts = HashMap::with_capacity(candidates.len());
    for_each_parquet_column_block_with_size(
        path,
        row_count,
        &profile.name,
        SOURCE_PROFILE_BLOCK_ROWS,
        |_, column| {
            for row_index in 0..column.len() {
                if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                    ensure_not_cancelled(is_cancelled())?;
                }
                let value = column.get(row_index).map_err(|error| {
                    format!("No se pudo resumir la columna {}: {error}", profile.name)
                })?;
                let Some(key) = categorical_group_key(value) else {
                    continue;
                };
                if candidates.contains_key(&key) {
                    let count = selected_counts.entry(key).or_insert(0usize);
                    *count = (*count).saturating_add(1);
                }
            }
            Ok(())
        },
    )?;

    Ok(finish_categorical_group_summary(
        &profile.name,
        row_count,
        profile.unique_count,
        selected_counts,
    ))
}
