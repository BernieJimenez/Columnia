//! Explorar (EX01): the automatic panel over the prepared dataset. Charts are
//! chosen from the quality profile and every figure is computed over all
//! rows; each chart applies every filter except its own column (crossfilter).

use super::*;
use chrono::Datelike;
use std::collections::BTreeMap;

const MAX_CATEGORY_CHARTS: usize = 6;
const MAX_CATEGORY_VALUES: usize = 60;
const MAX_CATEGORY_TEXT_LENGTH: f64 = 40.0;
/// A number is a category (bedrooms, a 0/1 flag) only with few repeated values.
const MAX_NUMERIC_CATEGORY_VALUES: usize = 12;
const MAX_BARS: usize = 12;
/// Bars of a chart the person expanded with «Ver todos».
const MAX_EXPANDED_BARS: usize = 60;
/// Values one filter may hold: every bar of an expanded chart plus «Sin dato».
const MAX_FILTER_VALUES: usize = MAX_EXPANDED_BARS + 1;
/// Most distinct values a column may have to be offered as a bar chart.
const MAX_CUSTOM_CATEGORY_VALUES: usize = 1_000;
const HISTOGRAM_BINS: usize = 20;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExploreFilter {
    column: String,
    /// Keep rows whose value (as text) is one of these; `null` keeps gaps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    values: Option<Vec<Option<String>>>,
    /// Keep rows whose number falls in `[min, max]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    range: Option<ExploreRange>,
    /// Keep rows whose date falls in this trend period: `2025`, `2025-03`
    /// or `2025-03-09`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    period: Option<String>,
}

/// Filters a request may send, and a project may keep, at once.
pub(crate) const MAX_EXPLORE_FILTERS: usize = 32;
/// Longest column name or value a saved filter may keep.
const MAX_FILTER_TEXT_CHARS: usize = 4 * 1024;

/// UX-09: the shape every filter needs, checked both before computing the
/// panel and before a project keeps the filters.
pub(crate) fn validate_explore_filters(filters: &[ExploreFilter]) -> Result<(), String> {
    if filters.len() > MAX_EXPLORE_FILTERS {
        return Err("Demasiados filtros a la vez.".to_owned());
    }
    for filter in filters {
        let kinds = usize::from(filter.values.is_some())
            + usize::from(filter.range.is_some())
            + usize::from(filter.period.is_some());
        if kinds != 1 {
            return Err("Cada filtro necesita valores o un rango, no ambos.".to_owned());
        }
        // FUN-77: an empty list used to drop the filter («no match» became
        // «no filter»); a huge one built a deep OR tree.
        match filter.values.as_ref().map(Vec::len) {
            Some(0) => return Err("Un filtro de valores necesita al menos un valor.".to_owned()),
            Some(count) if count > MAX_FILTER_VALUES => {
                return Err(format!(
                    "Un filtro admite como máximo {MAX_FILTER_VALUES} valores."
                ))
            }
            _ => {}
        }
        if filter
            .period
            .as_deref()
            .is_some_and(|period| period_days(period).is_none())
        {
            return Err("El periodo del filtro no es una fecha válida.".to_owned());
        }
        if filter
            .range
            .is_some_and(|range| !range.min.is_finite() || !range.max.is_finite())
        {
            return Err("El rango del filtro no es válido.".to_owned());
        }
        let too_long = std::iter::once(filter.column.as_str())
            .chain(filter.values.iter().flatten().flatten().map(String::as_str))
            .any(|text| text.chars().count() > MAX_FILTER_TEXT_CHARS);
        if too_long {
            return Err("Un filtro contiene un texto demasiado largo.".to_owned());
        }
    }
    Ok(())
}

/// What the person chose in «Personalizar»; anything left out stays automatic.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExploreLayout {
    #[serde(default)]
    categories: Option<Vec<String>>,
    #[serde(default)]
    measure: Option<String>,
    #[serde(default)]
    date: Option<String>,
    /// Bar charts that show every value instead of the most frequent ones.
    #[serde(default)]
    expanded: Vec<String>,
}

/// The columns each kind of chart accepts, for «Personalizar».
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreOptions {
    categories: Vec<String>,
    measures: Vec<String>,
    dates: Vec<String>,
    /// UX-09: text columns that look like dates; once interpreted in
    /// Preparar they can draw the trend.
    text_dates: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExploreRange {
    min: f64,
    max: f64,
    /// `[min, max)`, as the histogram counts every bin but the last (FUN-06).
    #[serde(default)]
    exclusive_max: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExplorePanel {
    row_count: usize,
    total_row_count: usize,
    kpis: Vec<ExploreKpi>,
    categories: Vec<ExploreCategoryChart>,
    histogram: Option<ExploreHistogram>,
    trend: Option<ExploreTrend>,
    options: ExploreOptions,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreKpi {
    /// "count", "median" or "mean".
    kind: String,
    column: Option<String>,
    value: Option<f64>,
    /// FUN-78: rows of the median or mean without a usable number (text,
    /// empty or not finite), left out of the figure.
    #[serde(skip_serializing_if = "Option::is_none")]
    ignored_count: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreCategoryChart {
    column: String,
    bars: Vec<ExploreBar>,
    /// Rows in values beyond the bars shown.
    other_count: usize,
    distinct_count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreBar {
    value: Option<String>,
    count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreHistogram {
    column: String,
    bins: Vec<ExploreBin>,
    /// FUN-78: filtered rows without a usable number, in no bin.
    ignored_count: usize,
    /// TXT-06: every value is a whole number (a year, a count), so the bins
    /// have whole-number edges and every bin but the last excludes `upper`.
    integer: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreBin {
    lower: f64,
    upper: f64,
    count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploreTrend {
    column: String,
    /// "day", "month" or "year".
    granularity: String,
    points: Vec<ExplorePoint>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExplorePoint {
    period: String,
    count: usize,
}

/// The columns each chart uses, chosen once from the profile.
struct PanelPlan {
    /// Automatic bar charts in order of preference, with their distinct counts.
    categories: Vec<(String, usize)>,
    measures: Vec<String>,
    date: Option<String>,
    options: ExploreOptions,
}

fn is_numeric_type(data_type: &str) -> bool {
    let lower = data_type.to_ascii_lowercase();
    [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
    ]
    .contains(&lower.as_str())
        || lower.starts_with("int")
        || lower.starts_with("uint")
        || lower.starts_with("float")
}

fn is_text_type(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_lowercase().as_str(),
        "str" | "string" | "utf8"
    )
}

fn is_date_type(data_type: &str) -> bool {
    let lower = data_type.to_ascii_lowercase();
    lower == "date" || lower.starts_with("datetime")
}

/// A row number or key: every value distinct and, when numeric, a run of
/// consecutive integers (1..n). Unique prices are a measure, not a key.
fn is_row_identifier(column: &ColumnProfile, row_count: usize) -> bool {
    if row_count == 0 || column.unique_count < row_count {
        return false;
    }
    let bound =
        |value: &Option<String>| value.as_deref().and_then(|value| value.parse::<f64>().ok());
    match (bound(&column.minimum), bound(&column.maximum)) {
        (Some(minimum), Some(maximum)) => (maximum - minimum + 1.0 - row_count as f64).abs() < 0.5,
        _ => true,
    }
}

fn plan_panel(profile: &DatasetProfile) -> PanelPlan {
    let columns = profile
        .columns
        .iter()
        .filter(|column| column.name != "_cambios" && column.privacy_signal.is_none());
    let mut text_categories = Vec::new();
    let mut numeric_categories = Vec::new();
    let mut binary_categories = Vec::new();
    let mut measures = Vec::new();
    let mut date = None;
    let mut options = ExploreOptions::default();
    for column in columns {
        let numeric = is_numeric_type(&column.data_type)
            || (is_text_type(&column.data_type)
                && matches!(
                    column.suggested_type.as_deref(),
                    Some("integer" | "decimal")
                ));
        let identifier = is_row_identifier(column, profile.row_count);
        let dated = is_date_type(&column.data_type);
        let short_text = column.average_length.unwrap_or(0.0) <= MAX_CATEGORY_TEXT_LENGTH;
        if is_text_type(&column.data_type) && column.suggested_type.as_deref() == Some("date") {
            options.text_dates.push(column.name.clone());
        }
        if dated {
            options.dates.push(column.name.clone());
        } else {
            if (2..=MAX_CUSTOM_CATEGORY_VALUES).contains(&column.unique_count)
                && (numeric || short_text)
                && !identifier
            {
                options.categories.push(column.name.clone());
            }
            if numeric && !identifier && column.unique_count >= 2 {
                options.measures.push(column.name.clone());
            }
        }
        let entry = (column.name.clone(), column.unique_count);
        if dated {
            date.get_or_insert_with(|| column.name.clone());
        } else if numeric {
            let repeats = column.unique_count.saturating_mul(2) <= profile.row_count;
            if column.unique_count == 2 {
                binary_categories.push(entry);
            } else if column.unique_count > 2
                && column.unique_count <= MAX_NUMERIC_CATEGORY_VALUES
                && repeats
            {
                numeric_categories.push(entry);
            } else if !identifier {
                measures.push(column.name.clone());
            }
        } else if column.unique_count >= 2
            && column.unique_count <= MAX_CATEGORY_VALUES
            && short_text
        {
            text_categories.push(entry);
        }
    }
    PanelPlan {
        categories: text_categories
            .into_iter()
            .chain(numeric_categories)
            .chain(binary_categories)
            .collect(),
        measures,
        date,
        options,
    }
}

fn value_expression(column: &str) -> Expr {
    col(column).cast(DataType::String)
}

fn number_expression(column: &str) -> Expr {
    col(column).cast(DataType::Float64)
}

/// Days since 1970-01-01, the unit Polars stores dates in.
fn day_expression(column: &str) -> Expr {
    col(column).cast(DataType::Date).cast(DataType::Int32)
}

/// First day of a trend period and first day after it, as days since 1970.
fn period_days(period: &str) -> Option<(i32, i32)> {
    let parts = period
        .split('-')
        .map(|part| part.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    let year = i32::try_from(*parts.first()?).ok()?;
    let (first, next) = match parts.as_slice() {
        [_] => (
            NaiveDate::from_ymd_opt(year, 1, 1)?,
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?,
        ),
        [_, month] => {
            let first = NaiveDate::from_ymd_opt(year, *month, 1)?;
            (first, first.checked_add_months(chrono::Months::new(1))?)
        }
        [_, month, day] => {
            let first = NaiveDate::from_ymd_opt(year, *month, *day)?;
            (first, first.succ_opt()?)
        }
        _ => return None,
    };
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    let days = |date: NaiveDate| i32::try_from((date - epoch).num_days()).ok();
    Some((days(first)?, days(next)?))
}

/// Whether two columns say the same thing (a state and its full name): every
/// value of one goes with exactly one value of the other.
fn mirrors(plan: &LazyFrame, left: &str, right: &str) -> Result<bool, String> {
    let text = |column: &str| value_expression(column).fill_null(lit("\u{0}"));
    let frame = collect(
        plan.clone().select([
            text(left).n_unique().alias("left"),
            text(right).n_unique().alias("right"),
            concat_str([text(left), text(right)], "\u{1f}", false)
                .n_unique()
                .alias("pair"),
        ]),
    )?;
    let count = |name: &str| {
        frame
            .column(name)
            .ok()
            .and_then(|column| column.get(0).ok())
            .and_then(|value| value.extract::<u64>())
    };
    Ok(count("pair") == count("left") && count("pair") == count("right"))
}

/// Every filter except the ones on `skip`, combined with AND.
fn filter_expression(filters: &[ExploreFilter], skip: Option<&str>) -> Option<Expr> {
    filters
        .iter()
        .filter(|filter| Some(filter.column.as_str()) != skip)
        .filter_map(|filter| {
            if let Some(values) = filter.values.as_ref() {
                values
                    .iter()
                    .map(|value| match value {
                        Some(value) => value_expression(&filter.column).eq(lit(value.clone())),
                        None => col(filter.column.as_str()).is_null(),
                    })
                    .reduce(|left, right| left.or(right))
            } else if let Some(period) = filter.period.as_deref() {
                period_days(period).map(|(first, next)| {
                    day_expression(&filter.column)
                        .gt_eq(lit(first))
                        .and(day_expression(&filter.column).lt(lit(next)))
                })
            } else {
                filter.range.map(|range| {
                    let upper = if range.exclusive_max.unwrap_or(false) {
                        number_expression(&filter.column).lt(lit(range.max))
                    } else {
                        number_expression(&filter.column).lt_eq(lit(range.max))
                    };
                    number_expression(&filter.column)
                        .gt_eq(lit(range.min))
                        .and(upper)
                })
            }
        })
        .reduce(|left, right| left.and(right))
}

fn filtered(plan: &LazyFrame, filters: &[ExploreFilter], skip: Option<&str>) -> LazyFrame {
    match filter_expression(filters, skip) {
        Some(expression) => plan.clone().filter(expression),
        None => plan.clone(),
    }
}

fn collect(plan: LazyFrame) -> Result<DataFrame, String> {
    plan.collect()
        .map_err(|error| format!("No se pudo calcular el panel: {error}"))
}

/// The finite numbers of `column` and how many rows had none (FUN-78).
fn numbers(frame: &DataFrame, column: &str) -> Result<(Vec<f64>, usize), String> {
    let values = frame
        .column(column)
        .and_then(|column| column.f64())
        .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?;
    let numbers = values
        .iter()
        .flatten()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let ignored = frame.height().saturating_sub(numbers.len());
    Ok((numbers, ignored))
}

fn row_count(plan: LazyFrame) -> Result<usize, String> {
    let frame = collect(plan.select([len().alias("n")]))?;
    let value = frame
        .column("n")
        .map_err(|error| format!("No se pudo contar las filas: {error}"))?
        .get(0)
        .map_err(|error| format!("No se pudo contar las filas: {error}"))?;
    Ok(match value {
        AnyValue::UInt32(value) => value as usize,
        AnyValue::UInt64(value) => value as usize,
        AnyValue::Int64(value) => value.max(0) as usize,
        AnyValue::Int32(value) => value.max(0) as usize,
        _ => 0,
    })
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

fn category_chart(
    plan: &LazyFrame,
    filters: &[ExploreFilter],
    column: &str,
    max_bars: usize,
) -> Result<ExploreCategoryChart, String> {
    let counts = collect(
        filtered(plan, filters, Some(column))
            .group_by([value_expression(column).alias("value")])
            .agg([len().alias("count")]),
    )?;
    let values = counts
        .column("value")
        .and_then(|column| column.str())
        .map_err(|error| format!("No se pudo agrupar '{column}': {error}"))?;
    let totals = counts
        .column("count")
        .map_err(|error| format!("No se pudo agrupar '{column}': {error}"))?
        .cast(&DataType::UInt64)
        .map_err(|error| format!("No se pudo agrupar '{column}': {error}"))?;
    let totals = totals
        .u64()
        .map_err(|error| format!("No se pudo agrupar '{column}': {error}"))?;
    let mut bars = values
        .iter()
        .zip(totals.iter())
        .map(|(value, count)| ExploreBar {
            value: value.map(str::to_owned),
            count: count.unwrap_or(0) as usize,
        })
        .collect::<Vec<_>>();
    // Most frequent first; ties keep a stable, readable order.
    bars.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    let distinct_count = bars.len();
    let other_count = bars.iter().skip(max_bars).map(|bar| bar.count).sum();
    bars.truncate(max_bars);
    Ok(ExploreCategoryChart {
        column: column.to_owned(),
        bars,
        other_count,
        distinct_count,
    })
}

fn histogram(
    plan: &LazyFrame,
    filters: &[ExploreFilter],
    column: &str,
) -> Result<Option<ExploreHistogram>, String> {
    // Bins span the whole column so they stay put while filters change.
    let (all, _) = numbers(
        &collect(
            plan.clone()
                .select([number_expression(column).alias(column)]),
        )?,
        column,
    )?;
    let (Some(minimum), Some(maximum)) = (
        all.iter().copied().reduce(f64::min),
        all.iter().copied().reduce(f64::max),
    ) else {
        return Ok(None);
    };
    // TXT-06: whole numbers get whole-number bins (1925–1929, not
    // 1925–1929.8); the span counts both ends, as a year range does.
    let integer = all.iter().all(|value| value.fract() == 0.0) && maximum - minimum < 1e15;
    let (width, bin_count) = if maximum <= minimum {
        (1.0, 1)
    } else if integer {
        let span = maximum - minimum + 1.0;
        let width = (span / HISTOGRAM_BINS as f64).ceil().max(1.0);
        (
            width,
            ((span / width).ceil() as usize).clamp(1, HISTOGRAM_BINS),
        )
    } else {
        ((maximum - minimum) / HISTOGRAM_BINS as f64, HISTOGRAM_BINS)
    };
    let mut counts = vec![0usize; bin_count];
    let (selected, ignored_count) = numbers(
        &collect(
            filtered(plan, filters, Some(column)).select([number_expression(column).alias(column)]),
        )?,
        column,
    )?;
    for value in selected {
        let index = (((value - minimum) / width) as usize).min(bin_count - 1);
        counts[index] += 1;
    }
    Ok(Some(ExploreHistogram {
        column: column.to_owned(),
        bins: counts
            .into_iter()
            .enumerate()
            .map(|(index, count)| ExploreBin {
                lower: minimum + width * index as f64,
                upper: if index + 1 == bin_count {
                    maximum
                } else {
                    minimum + width * (index + 1) as f64
                },
                count,
            })
            .collect(),
        ignored_count,
        integer,
    }))
}

fn trend(
    plan: &LazyFrame,
    filters: &[ExploreFilter],
    column: &str,
) -> Result<Option<ExploreTrend>, String> {
    let frame = collect(
        filtered(plan, filters, Some(column))
            .select([col(column).cast(DataType::Date).alias(column)]),
    )?;
    let days = frame
        .column(column)
        .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
        .cast(&DataType::Int32)
        .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha base válida");
    let dates = days
        .i32()
        .map_err(|error| format!("No se pudo leer la columna '{column}': {error}"))?
        .iter()
        .flatten()
        .filter_map(|day| epoch.checked_add_signed(chrono::Duration::days(day.into())))
        .collect::<Vec<_>>();
    let (Some(first), Some(last)) = (dates.iter().min(), dates.iter().max()) else {
        return Ok(None);
    };
    let span = (*last - *first).num_days();
    let granularity = if span <= 92 {
        "day"
    } else if span <= 366 * 5 {
        "month"
    } else {
        "year"
    };
    let mut points = BTreeMap::<String, usize>::new();
    for date in &dates {
        let period = match granularity {
            "day" => date.format("%Y-%m-%d").to_string(),
            "month" => format!("{:04}-{:02}", date.year(), date.month()),
            _ => format!("{:04}", date.year()),
        };
        *points.entry(period).or_default() += 1;
    }
    Ok(Some(ExploreTrend {
        column: column.to_owned(),
        granularity: granularity.to_owned(),
        points: points
            .into_iter()
            .map(|(period, count)| ExplorePoint { period, count })
            .collect(),
    }))
}

/// The panel for `plan` (the prepared dataset) with the given filters.
pub(super) fn explore_panel(
    plan: LazyFrame,
    profile: &DatasetProfile,
    filters: &[ExploreFilter],
    custom: &ExploreLayout,
) -> Result<ExplorePanel, String> {
    validate_explore_filters(filters)?;
    for filter in filters {
        if !profile
            .columns
            .iter()
            .any(|column| column.name == filter.column)
        {
            return Err(format!(
                "La columna '{}' no existe en el dataset.",
                filter.column
            ));
        }
    }
    let mut layout = plan_panel(profile);
    let offered = |options: &[String], column: &String| {
        if options.contains(column) {
            Ok(())
        } else {
            Err(format!(
                "La columna '{column}' no se puede usar en este gráfico."
            ))
        }
    };
    let categories = match custom.categories.as_ref() {
        Some(chosen) => {
            if chosen.len() > MAX_CATEGORY_CHARTS {
                return Err(format!(
                    "Elige como máximo {MAX_CATEGORY_CHARTS} gráficos de barras."
                ));
            }
            for column in chosen {
                offered(&layout.options.categories, column)?;
            }
            chosen.clone()
        }
        None => {
            // Two columns that say the same thing get one chart, not two.
            let mut kept = Vec::<(String, usize)>::new();
            for (column, distinct) in std::mem::take(&mut layout.categories) {
                if kept.len() == MAX_CATEGORY_CHARTS {
                    break;
                }
                let mut repeated = false;
                for (other, other_distinct) in &kept {
                    if *other_distinct == distinct && mirrors(&plan, other, &column)? {
                        repeated = true;
                        break;
                    }
                }
                if !repeated {
                    kept.push((column, distinct));
                }
            }
            kept.into_iter().map(|(column, _)| column).collect()
        }
    };
    if let Some(measure) = custom.measure.as_ref() {
        offered(&layout.options.measures, measure)?;
        layout.measures.retain(|column| column != measure);
        layout.measures.insert(0, measure.clone());
    }
    if let Some(date) = custom.date.as_ref() {
        offered(&layout.options.dates, date)?;
        layout.date = Some(date.clone());
    }
    let total_row_count = profile.row_count;
    let everything = filtered(&plan, filters, None);
    let row_count = row_count(everything.clone())?;

    let mut kpis = vec![ExploreKpi {
        kind: "count".to_owned(),
        column: None,
        value: Some(row_count as f64),
        ignored_count: None,
    }];
    for (index, measure) in layout.measures.iter().take(2).enumerate() {
        let (mut values, ignored_count) = numbers(
            &collect(
                everything
                    .clone()
                    .select([number_expression(measure).alias(measure.as_str())]),
            )?,
            measure,
        )?;
        let (kind, value) = if index == 0 {
            ("median", median(&mut values))
        } else {
            (
                "mean",
                (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
            )
        };
        kpis.push(ExploreKpi {
            kind: kind.to_owned(),
            column: Some(measure.clone()),
            value,
            ignored_count: Some(ignored_count),
        });
    }

    let categories = categories
        .iter()
        .map(|column| {
            let max_bars = if custom.expanded.contains(column) {
                MAX_EXPANDED_BARS
            } else {
                MAX_BARS
            };
            category_chart(&plan, filters, column, max_bars)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let histogram = match layout.measures.first() {
        Some(measure) => histogram(&plan, filters, measure)?,
        None => None,
    };
    let trend = match layout.date.as_deref() {
        Some(date) => trend(&plan, filters, date)?,
        None => None,
    };
    Ok(ExplorePanel {
        row_count,
        total_row_count,
        kpis,
        categories,
        histogram,
        trend,
        options: layout.options,
    })
}
