use super::*;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub(super) struct TemporalPeriodKey {
    pub(super) year: i32,
    pub(super) month: u32,
    pub(super) day: u32,
}

pub(super) fn temporal_period_key(value: NaiveDateTime) -> TemporalPeriodKey {
    TemporalPeriodKey {
        year: value.year(),
        month: value.month(),
        day: value.day(),
    }
}

pub(super) fn temporal_period_label(key: TemporalPeriodKey, granularity: &str) -> String {
    match granularity {
        "year" => format!("{:04}", key.year),
        "day" => format!("{:04}-{:02}-{:02}", key.year, key.month, key.day),
        _ => format!("{:04}-{:02}", key.year, key.month),
    }
}

pub(super) fn temporal_periods_between(
    first: TemporalPeriodKey,
    last: TemporalPeriodKey,
    granularity: &str,
    counts: &HashMap<TemporalPeriodKey, usize>,
) -> Vec<(String, usize)> {
    if granularity == "day" {
        let Some(first_date) = NaiveDate::from_ymd_opt(first.year, first.month, first.day) else {
            return Vec::new();
        };
        let Some(last_date) = NaiveDate::from_ymd_opt(last.year, last.month, last.day) else {
            return Vec::new();
        };
        let span = (last_date - first_date).num_days();
        return (0..=span.max(0))
            .map(|offset| {
                let date = first_date + chrono::Duration::days(offset);
                let key = TemporalPeriodKey {
                    year: date.year(),
                    month: date.month(),
                    day: date.day(),
                };
                (
                    temporal_period_label(key, granularity),
                    counts.get(&key).copied().unwrap_or(0),
                )
            })
            .collect();
    }

    if granularity == "month" {
        let span = (i64::from(last.year) - i64::from(first.year)) * 12 + i64::from(last.month)
            - i64::from(first.month);
        return (0..=span.max(0))
            .map(|offset| {
                let absolute_month =
                    i64::from(first.year) * 12 + i64::from(first.month.saturating_sub(1)) + offset;
                let year = absolute_month.div_euclid(12) as i32;
                let month = absolute_month.rem_euclid(12) as u32 + 1;
                let key = TemporalPeriodKey {
                    year,
                    month,
                    day: 1,
                };
                let count = counts
                    .iter()
                    .filter(|(period, _)| period.year == year && period.month == month)
                    .map(|(_, count)| *count)
                    .sum();
                (temporal_period_label(key, granularity), count)
            })
            .collect();
    }

    let year_span = i64::from(last.year) - i64::from(first.year);
    if (0..=MAX_TEMPORAL_PERIODS as i64).contains(&year_span) {
        return (0..=year_span)
            .map(|offset| {
                let key = TemporalPeriodKey {
                    year: first.year.saturating_add(offset as i32),
                    month: 1,
                    day: 1,
                };
                let count = counts
                    .iter()
                    .filter(|(period, _)| period.year == key.year)
                    .map(|(_, count)| *count)
                    .sum();
                (temporal_period_label(key, granularity), count)
            })
            .collect();
    }

    let mut years = counts.keys().map(|key| key.year).collect::<Vec<_>>();
    years.sort_unstable();
    years.dedup();
    years
        .into_iter()
        .map(|year| {
            let key = TemporalPeriodKey {
                year,
                month: 1,
                day: 1,
            };
            let count = counts
                .iter()
                .filter(|(period, _)| period.year == year)
                .map(|(_, count)| *count)
                .sum();
            (temporal_period_label(key, granularity), count)
        })
        .collect()
}

fn finish_temporal_series_summary(
    column: &str,
    row_count: usize,
    parsed_row_count: usize,
    first: Option<TemporalPeriodKey>,
    last: Option<TemporalPeriodKey>,
    counts: HashMap<TemporalPeriodKey, usize>,
) -> Result<Option<TemporalSeriesSummary>, String> {
    let (Some(first), Some(last)) = (first, last) else {
        return Ok(None);
    };
    let first_date = NaiveDate::from_ymd_opt(first.year, first.month, first.day)
        .ok_or_else(|| "No se pudo interpretar el inicio de la tendencia temporal.".to_owned())?;
    let last_date = NaiveDate::from_ymd_opt(last.year, last.month, last.day)
        .ok_or_else(|| "No se pudo interpretar el fin de la tendencia temporal.".to_owned())?;
    let day_span = (last_date - first_date).num_days();
    let month_span = (i64::from(last.year) - i64::from(first.year)) * 12 + i64::from(last.month)
        - i64::from(first.month);
    let granularity = if day_span <= MAX_TEMPORAL_DAY_SPAN {
        "day"
    } else if month_span <= MAX_TEMPORAL_MONTH_SPAN {
        "month"
    } else {
        "year"
    };
    let mut raw_periods = temporal_periods_between(first, last, granularity, &counts);
    raw_periods.retain(|(_, count)| *count > 0 || matches!(granularity, "month" | "day"));

    let truncated = raw_periods.len() > MAX_TEMPORAL_PERIODS;
    if truncated {
        let split = raw_periods.len() - (MAX_TEMPORAL_PERIODS - 1);
        let previous_count = raw_periods[..split]
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>();
        let mut retained = vec![("Periodos anteriores".to_owned(), previous_count)];
        retained.extend(raw_periods.into_iter().skip(split));
        raw_periods = retained;
    }
    let denominator = parsed_row_count.max(1) as f64;
    let periods = raw_periods
        .into_iter()
        .map(|(period, row_count)| TemporalPeriod {
            period,
            row_count,
            percentage: (row_count as f64 / denominator) * 100.0,
        })
        .collect::<Vec<_>>();

    Ok(Some(TemporalSeriesSummary {
        column: column.to_owned(),
        granularity: granularity.to_owned(),
        periods,
        parsed_row_count,
        unparsed_row_count: row_count.saturating_sub(parsed_row_count),
        truncated,
    }))
}

fn temporal_series_summary<C>(
    frame: &DataFrame,
    column: &Column,
    profile: &ColumnProfile,
    is_cancelled: &C,
) -> Result<Option<TemporalSeriesSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    let month_first = profile.date_order.as_deref() == Some("mdy");
    // REN-01: reading dates is the slow part, so row ranges run in parallel
    // and their counts are added up; the result does not depend on the split.
    let height = frame.height();
    let workers = std::thread::available_parallelism()
        .map_or(1, |parallelism| parallelism.get().min(8))
        .min(height / 65_536 + 1);
    let chunk = height.div_ceil(workers.max(1)).max(1);
    let partials = std::thread::scope(|scope| {
        let handles = (0..height)
            .step_by(chunk)
            .map(|start| {
                let end = (start + chunk).min(height);
                scope.spawn(move || {
                    let mut counts = HashMap::<TemporalPeriodKey, usize>::new();
                    let mut first: Option<TemporalPeriodKey> = None;
                    let mut last: Option<TemporalPeriodKey> = None;
                    let mut parsed = 0usize;
                    for row_index in start..end {
                        if row_index % 4096 == 0 {
                            ensure_not_cancelled(is_cancelled())?;
                        }
                        let value = column.get(row_index).map_err(|error| {
                            format!(
                                "No se pudo resumir la tendencia temporal de {}: {error}",
                                profile.name
                            )
                        })?;
                        let Some(datetime) = ordered_datetime_value(value, month_first) else {
                            continue;
                        };
                        let key = temporal_period_key(datetime);
                        first = Some(first.map_or(key, |current| current.min(key)));
                        last = Some(last.map_or(key, |current| current.max(key)));
                        *counts.entry(key).or_insert(0) += 1;
                        parsed += 1;
                    }
                    Ok::<_, String>((counts, first, last, parsed))
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "El resumen temporal se interrumpió.".to_owned())?
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let mut counts = HashMap::<TemporalPeriodKey, usize>::new();
    let mut first: Option<TemporalPeriodKey> = None;
    let mut last: Option<TemporalPeriodKey> = None;
    let mut parsed_row_count = 0usize;
    for (partial_counts, partial_first, partial_last, parsed) in partials {
        for (key, count) in partial_counts {
            *counts.entry(key).or_insert(0) += count;
        }
        first = match (first, partial_first) {
            (Some(current), Some(partial)) => Some(current.min(partial)),
            (current, partial) => current.or(partial),
        };
        last = last.max(partial_last);
        parsed_row_count += parsed;
    }
    ensure_not_cancelled(is_cancelled())?;

    finish_temporal_series_summary(
        &profile.name,
        frame.height(),
        parsed_row_count,
        first,
        last,
        counts,
    )
}

pub(super) fn temporal_series_summaries<C>(
    frame: &DataFrame,
    profiles: &[ColumnProfile],
    is_cancelled: &C,
) -> Result<Option<Vec<TemporalSeriesSummary>>, String>
where
    C: Fn() -> bool + Sync,
{
    let mut summaries = Vec::new();
    for (column, profile) in frame.columns().iter().zip(profiles) {
        if summaries.len() >= MAX_TEMPORAL_COLUMNS
            || profile.privacy_signal.is_some()
            || (!matches!(column.dtype(), DataType::Date | DataType::Datetime(_, _))
                && profile.suggested_type.as_deref() != Some("date"))
        {
            continue;
        }
        if let Some(summary) = temporal_series_summary(frame, column, profile, is_cancelled)? {
            summaries.push(summary);
        }
    }
    Ok((!summaries.is_empty()).then_some(summaries))
}

/// Periods counted over part of a column; parts merge in any order.
#[derive(Default)]
struct TemporalTally {
    counts: HashMap<TemporalPeriodKey, usize>,
    first: Option<TemporalPeriodKey>,
    last: Option<TemporalPeriodKey>,
    parsed_row_count: usize,
}

impl TemporalTally {
    fn observe(&mut self, key: TemporalPeriodKey) {
        self.first = Some(self.first.map_or(key, |current| current.min(key)));
        self.last = Some(self.last.map_or(key, |current| current.max(key)));
        *self.counts.entry(key).or_insert(0) += 1;
        self.parsed_row_count = self.parsed_row_count.saturating_add(1);
    }

    fn merge(mut self, other: TemporalTally) -> TemporalTally {
        for (key, count) in other.counts {
            *self.counts.entry(key).or_insert(0) += count;
        }
        self.first = match (self.first, other.first) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        };
        self.last = match (self.last, other.last) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
        self.parsed_row_count = self.parsed_row_count.saturating_add(other.parsed_row_count);
        self
    }
}

/// REN-01: each block's dates are read in parallel parts, and `rows_read`
/// counts the rows done so the profile can show how far it has gone.
pub(super) fn source_temporal_series_summary<C>(
    path: &Path,
    row_count: usize,
    profile: &ColumnProfile,
    is_cancelled: &C,
    rows_read: &std::sync::atomic::AtomicUsize,
) -> Result<Option<TemporalSeriesSummary>, String>
where
    C: Fn() -> bool + Sync,
{
    let month_first = profile.date_order.as_deref() == Some("mdy");
    let mut tally = TemporalTally::default();
    for_each_parquet_column_block_with_size(
        path,
        row_count,
        &profile.name,
        SOURCE_PROFILE_BLOCK_ROWS,
        |_, column| {
            let block = (0..column.len())
                .into_par_iter()
                .with_min_len(4096)
                .try_fold(TemporalTally::default, |mut part, row_index| {
                    if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
                        ensure_not_cancelled(is_cancelled())?;
                    }
                    let value = column.get(row_index).map_err(|error| {
                        format!(
                            "No se pudo resumir la tendencia temporal de {}: {error}",
                            profile.name
                        )
                    })?;
                    if let Some(datetime) = ordered_datetime_value(value, month_first) {
                        part.observe(temporal_period_key(datetime));
                    }
                    Ok::<_, String>(part)
                })
                .try_reduce(TemporalTally::default, |left, right| Ok(left.merge(right)))?;
            tally = std::mem::take(&mut tally).merge(block);
            rows_read.fetch_add(column.len(), std::sync::atomic::Ordering::Relaxed);
            Ok(())
        },
    )?;

    finish_temporal_series_summary(
        &profile.name,
        row_count,
        tally.parsed_row_count,
        tally.first,
        tally.last,
        tally.counts,
    )
}
