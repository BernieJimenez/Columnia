use super::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum GroupKey {
    Missing,
    Value(String),
}

pub(super) fn categorical_group_label(key: &GroupKey) -> String {
    match key {
        GroupKey::Missing => "Sin valor".to_owned(),
        GroupKey::Value(value) => value.clone(),
    }
}

/// Exact group counts of a text column in one pass (REN-01). Values are
/// looked up by `&str`, so only a new distinct value allocates; after
/// `MAX_GROUP_CANDIDATES` distinct values, new ones are left out, so every
/// count kept is exact and the same in memory and on a large file.
#[derive(Default)]
pub(super) struct GroupTally {
    missing: usize,
    values: HashMap<String, usize>,
}

impl GroupTally {
    pub(super) fn observe(&mut self, value: Option<&str>) {
        let trimmed = value.map(str::trim).unwrap_or_default();
        if trimmed.is_empty() {
            self.missing = self.missing.saturating_add(1);
            return;
        }
        if trimmed.len() > MAX_GROUP_LABEL_CHARS && trimmed.chars().count() > MAX_GROUP_LABEL_CHARS
        {
            return;
        }
        if let Some(count) = self.values.get_mut(trimmed) {
            *count = count.saturating_add(1);
        } else if self.values.len() < MAX_GROUP_CANDIDATES {
            self.values.insert(trimmed.to_owned(), 1);
        }
    }

    pub(super) fn into_counts(self) -> HashMap<GroupKey, usize> {
        let mut counts = self
            .values
            .into_iter()
            .map(|(value, count)| (GroupKey::Value(value), count))
            .collect::<HashMap<_, _>>();
        if self.missing > 0 {
            counts.insert(GroupKey::Missing, self.missing);
        }
        counts
    }
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
    let values = column
        .str()
        .map_err(|error| format!("No se pudo resumir la columna {}: {error}", profile.name))?;
    let mut tally = GroupTally::default();
    for (row_index, value) in values.iter().enumerate() {
        if row_index % 65_536 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        tally.observe(value);
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(finish_categorical_group_summary(
        &profile.name,
        frame.height(),
        profile.unique_count,
        tally.into_counts(),
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

    // The candidates are exact counts (REN-01): no second pass over the file.
    let selected_counts = candidates.clone();

    Ok(finish_categorical_group_summary(
        &profile.name,
        row_count,
        profile.unique_count,
        selected_counts,
    ))
}
