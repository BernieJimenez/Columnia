use super::{
    validate_quality_rule_definition, validate_quality_rules_payload, validate_stored_recipe,
    DataFrame, DatasetProfile, QualityRule, StoredTransformRecipe, MAX_CATEGORICAL_GROUPS,
    MAX_CATEGORICAL_GROUP_COLUMNS, MAX_GROUP_LABEL_CHARS, MAX_NUMERIC_CORRELATION_COLUMNS,
    MAX_NUMERIC_CORRELATION_SAMPLE_ROWS, MAX_TEMPORAL_COLUMNS, MAX_TEMPORAL_PERIODS,
};
pub(crate) fn validate_project_workspace(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    recipe_draft: Option<&StoredTransformRecipe>,
) -> Result<(), String> {
    validate_quality_rules_payload(quality_rules)?;
    for rule in quality_rules {
        validate_quality_rule_definition(frame, rule)?;
    }
    if let Some(recipe) = recipe_draft {
        validate_stored_recipe(recipe)?;
    }
    Ok(())
}

pub(crate) fn validate_project_profile(
    frame: &DataFrame,
    profile: &DatasetProfile,
) -> Result<(), String> {
    validate_project_profile_with_row_count(frame, frame.height(), profile)
}

pub(crate) fn validate_project_profile_with_row_count(
    frame: &DataFrame,
    row_count: usize,
    profile: &DatasetProfile,
) -> Result<(), String> {
    let finite = |value: Option<f64>| value.is_none_or(f64::is_finite);
    if !profile.duplicate_percentage.is_finite()
        || !(0.0..=100.0).contains(&profile.duplicate_percentage)
        || profile.columns.iter().any(|column| {
            !column.completeness_percentage.is_finite()
                || !(0.0..=100.0).contains(&column.completeness_percentage)
                || !finite(column.mean)
                || !finite(column.average_length)
                || !finite(column.type_match_percentage)
                || !finite(column.standard_deviation)
                || !finite(column.first_quartile)
                || !finite(column.median)
                || !finite(column.third_quartile)
        })
    {
        return Err("El perfil guardado contiene métricas no válidas.".to_owned());
    }
    if let Some(correlations) = &profile.numeric_correlations {
        if correlations.columns.len() < 2
            || correlations.columns.len() > MAX_NUMERIC_CORRELATION_COLUMNS
            || correlations.sampled_row_count > MAX_NUMERIC_CORRELATION_SAMPLE_ROWS
            || correlations.pairs.iter().any(|pair| {
                pair.first_column == pair.second_column
                    || !correlations.columns.contains(&pair.first_column)
                    || !correlations.columns.contains(&pair.second_column)
                    || pair.coefficient.is_some_and(|coefficient| {
                        !coefficient.is_finite() || !(-1.0..=1.0).contains(&coefficient)
                    })
            })
        {
            return Err("El perfil guardado contiene correlaciones no válidas.".to_owned());
        }
    }
    if let Some(summaries) = &profile.categorical_group_summaries {
        if summaries.len() > MAX_CATEGORICAL_GROUP_COLUMNS
            || summaries.iter().any(|summary| {
                summary.groups.is_empty()
                    || summary.groups.len() > MAX_CATEGORICAL_GROUPS + 1
                    || summary.distinct_count < 2
                    || summary.groups.iter().filter(|group| group.is_other).count() > 1
                    || summary.groups.iter().any(|group| {
                        group.label.is_empty()
                            || group.row_count == 0
                            || !group.percentage.is_finite()
                            || !(0.0..=100.0).contains(&group.percentage)
                            || group.label.chars().count() > MAX_GROUP_LABEL_CHARS
                    })
                    || summary
                        .groups
                        .iter()
                        .map(|group| group.row_count)
                        .sum::<usize>()
                        != profile.row_count
            })
        {
            return Err(
                "El perfil guardado contiene resúmenes de categorías no válidos.".to_owned(),
            );
        }
    }
    if let Some(summaries) = &profile.temporal_series {
        if summaries.len() > MAX_TEMPORAL_COLUMNS
            || summaries.iter().any(|summary| {
                !matches!(summary.granularity.as_str(), "day" | "month" | "year")
                    || summary.periods.is_empty()
                    || summary.periods.len() > MAX_TEMPORAL_PERIODS
                    || summary.parsed_row_count == 0
                    || summary.parsed_row_count > profile.row_count
                    || summary.unparsed_row_count
                        != profile.row_count.saturating_sub(summary.parsed_row_count)
                    || summary.periods.iter().any(|period| {
                        period.period.is_empty()
                            || period.period.chars().count() > 64
                            || period.row_count > summary.parsed_row_count
                            || !period.percentage.is_finite()
                            || !(0.0..=100.0).contains(&period.percentage)
                    })
                    || summary
                        .periods
                        .iter()
                        .map(|period| period.row_count)
                        .sum::<usize>()
                        != summary.parsed_row_count
            })
        {
            return Err("El perfil guardado contiene tendencias temporales no válidas.".to_owned());
        }
    }
    if profile.row_count != row_count
        || profile.columns.len() != frame.width()
        || profile
            .columns
            .iter()
            .zip(frame.columns())
            .any(|(stored, column)| {
                stored.name != column.name().as_str()
                    || stored.data_type != column.dtype().to_string()
                    || stored.null_count > profile.row_count
                    || stored.unique_count > profile.row_count
            })
    {
        return Err("El perfil guardado no coincide con el dataset.".to_owned());
    }
    Ok(())
}
