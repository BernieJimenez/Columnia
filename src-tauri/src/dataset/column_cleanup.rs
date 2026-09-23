use polars::prelude::{AnyValue, Column, DataFrame};

use super::{privacy_signal, HIGH_NULL_COLUMN_THRESHOLD_PERCENTAGE, REDACTED_VALUE};

pub(super) fn remove_constant_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() <= 1 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| column.name() != "_cambios")
        .filter_map(|column| {
            let null_count = column.null_count();
            let unique_count = column
                .n_unique()
                .ok()?
                .saturating_sub(usize::from(null_count > 0));
            (unique_count <= 1 && null_count < frame.height()).then(|| column.name().to_string())
        })
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas constantes: {error}"))?;
    Ok((cleaned, removed_columns))
}

pub(super) fn remove_empty_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| column.name() != "_cambios")
        .filter(|column| column.null_count() == frame.height())
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas vacías: {error}"))?;
    Ok((cleaned, removed_columns))
}

pub(super) fn remove_high_null_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| column.name() != "_cambios")
        .filter(|column| {
            let null_count = column.null_count();
            null_count > 0
                && null_count < frame.height()
                && null_count.saturating_mul(100)
                    >= frame
                        .height()
                        .saturating_mul(HIGH_NULL_COLUMN_THRESHOLD_PERCENTAGE)
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron eliminar columnas con alta nulidad: {error}"))?;
    Ok((cleaned, removed_columns))
}

pub(super) fn remove_identifier_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| matches!(privacy_signal(column.name()), Some("identifier")))
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame
        .select(&remaining_columns)
        .map_err(|error| format!("No se pudieron retirar columnas identificadoras: {error}"))?;
    Ok((cleaned, removed_columns))
}

pub(super) fn remove_personal_columns_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, Vec<String>), String> {
    if frame.height() == 0 || frame.width() <= 1 {
        return Ok((frame.clone(), Vec::new()));
    }

    let candidates = frame
        .columns()
        .iter()
        .filter(|column| {
            column.name() != "_cambios"
                && matches!(
                    privacy_signal(column.name()),
                    Some("email" | "phone" | "address" | "name")
                )
        })
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let removable_count = candidates.len().min(frame.width().saturating_sub(1));
    let removed_columns = candidates
        .into_iter()
        .take(removable_count)
        .collect::<Vec<_>>();
    if removed_columns.is_empty() {
        return Ok((frame.clone(), removed_columns));
    }

    let remaining_columns = frame
        .get_column_names()
        .iter()
        .filter(|name| {
            !removed_columns
                .iter()
                .any(|removed| removed == name.as_str())
        })
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let cleaned = frame.select(&remaining_columns).map_err(|error| {
        format!("No se pudieron retirar columnas con datos personales: {error}")
    })?;
    Ok((cleaned, removed_columns))
}

pub(super) fn is_personal_privacy_signal(column_name: &str) -> bool {
    matches!(
        privacy_signal(column_name),
        Some("email" | "phone" | "address" | "name")
    )
}

pub(super) fn mask_personal_values_from_frame(
    frame: &DataFrame,
) -> Result<(DataFrame, usize, usize), String> {
    let mut masked = frame.clone();
    let mut changed_cell_count = 0;
    let mut changed_column_count = 0;

    for column in frame
        .columns()
        .iter()
        .filter(|column| column.name() != "_cambios" && is_personal_privacy_signal(column.name()))
    {
        let mut column_changed_cell_count = 0;
        let values = (0..column.len())
            .map(|row_index| {
                column
                    .get(row_index)
                    .map_err(|_| {
                        "No se pudo leer una columna personal para proteger sus valores.".to_owned()
                    })
                    .map(|value| match value {
                        AnyValue::Null => None,
                        value => {
                            let already_redacted = match &value {
                                AnyValue::String(current) => *current == REDACTED_VALUE,
                                AnyValue::StringOwned(current) => {
                                    current.as_str() == REDACTED_VALUE
                                }
                                _ => false,
                            };
                            if !already_redacted {
                                column_changed_cell_count += 1;
                            }
                            Some(REDACTED_VALUE.to_owned())
                        }
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;

        if column_changed_cell_count > 0 {
            changed_cell_count += column_changed_cell_count;
            changed_column_count += 1;
        }
        masked
            .replace(
                column.name().as_str(),
                Column::new(column.name().clone(), values),
            )
            .map_err(|_| "No se pudo proteger una columna de datos personales.".to_owned())?;
    }

    Ok((masked, changed_cell_count, changed_column_count))
}
