use super::*;

pub(super) fn strict_column_text(column: &Column) -> Result<Vec<Option<String>>, String> {
    (0..column.len())
        .map(|row| {
            column
                .get(row)
                .map(preview_value)
                .map_err(|error| format!("No se pudo leer la fila {}: {error}", row + 1))
        })
        .collect()
}

pub(super) fn recipe_column<'a>(frame: &'a DataFrame, name: &str) -> Result<&'a Column, String> {
    frame
        .column(name)
        .map_err(|_| format!("La columna '{name}' no existe en el dataset activo."))
}

pub(super) fn strict_cast_column(
    column: &Column,
    target: RecipeCastTarget,
) -> Result<Column, String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let invalid = |row: usize, value: &str, target: &str| {
        format!(
            "La columna '{}' no se puede convertir a {target}: fila {}, valor '{}'.",
            name,
            row + 1,
            value
        )
    };

    match target {
        RecipeCastTarget::String => Ok(Series::new(name, values).into_column()),
        RecipeCastTarget::Integer => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| {
                            value
                                .trim()
                                .parse::<i64>()
                                .map_err(|_| invalid(row, value, "entero"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
        RecipeCastTarget::Decimal => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| {
                            value
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .filter(|number| number.is_finite())
                                .ok_or_else(|| invalid(row, value, "decimal"))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
        RecipeCastTarget::Boolean => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                            "true" => Ok(true),
                            "false" => Ok(false),
                            _ => Err(invalid(row, value, "booleano (true/false)")),
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Series::new(name, parsed).into_column())
        }
    }
}

pub(super) fn parse_recipe_datetime(
    value: &str,
    format: RecipeDateFormat,
) -> Result<NaiveDateTime, ()> {
    let value = value.trim();
    let date_format = match format {
        RecipeDateFormat::Ymd => Some("%Y-%m-%d"),
        RecipeDateFormat::Dmy => Some("%d/%m/%Y"),
        RecipeDateFormat::Mdy => Some("%m/%d/%Y"),
        RecipeDateFormat::Iso8601 => None,
    };
    if let Some(format) = date_format {
        return NaiveDate::parse_from_str(value, format)
            .ok()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
            .ok_or(());
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0).ok_or(());
    }
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.naive_utc())
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map_err(|_| ())
}

pub(super) fn recipe_filter_is_ordered(operator: RecipeFilterOperator) -> bool {
    matches!(
        operator,
        RecipeFilterOperator::Gt
            | RecipeFilterOperator::Lt
            | RecipeFilterOperator::Gte
            | RecipeFilterOperator::Lte
    )
}

pub(super) fn recipe_filter_uses_temporal_literal(operator: RecipeFilterOperator) -> bool {
    recipe_filter_is_ordered(operator)
        || matches!(
            operator,
            RecipeFilterOperator::Eq | RecipeFilterOperator::Neq
        )
}

pub(super) fn parse_recipe_filter_datetime(
    value: &str,
    column: &str,
) -> Result<NaiveDateTime, String> {
    parse_recipe_datetime(value, RecipeDateFormat::Iso8601).map_err(|_| {
        format!("El valor del filtro para '{column}' debe ser una fecha ISO 8601 válida.")
    })
}

pub(super) fn datetime_timestamp_in_unit(
    value: NaiveDateTime,
    unit: TimeUnit,
) -> Result<i64, String> {
    match unit {
        TimeUnit::Nanoseconds => value
            .and_utc()
            .timestamp_nanos_opt()
            .ok_or_else(|| "La fecha del filtro está fuera del rango admitido.".to_owned()),
        TimeUnit::Microseconds => Ok(value.and_utc().timestamp_micros()),
        TimeUnit::Milliseconds => Ok(value.and_utc().timestamp_millis()),
    }
}

pub(super) fn strict_date_column(
    column: &Column,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
) -> Result<Column, String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let parsed = values
        .iter()
        .enumerate()
        .map(|(row, value)| {
            value
                .as_deref()
                .map(|value| {
                    parse_recipe_datetime(value, format).map_err(|_| {
                        format!(
                            "La columna '{}' contiene una fecha inválida en la fila {}: '{}'.",
                            name,
                            row + 1,
                            value
                        )
                    })
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    match target {
        RecipeDateTarget::Date => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
            let days = parsed
                .into_iter()
                .map(|value| value.map(|value| (value.date() - epoch).num_days() as i32))
                .collect::<Vec<_>>();
            Ok(Series::new(name, days)
                .cast(&polars::prelude::DataType::Date)
                .map_err(|error| format!("No se pudo crear la columna de fecha: {error}"))?
                .into_column())
        }
        RecipeDateTarget::Datetime => {
            let milliseconds = parsed
                .into_iter()
                .map(|value| value.map(|value| value.and_utc().timestamp_millis()))
                .collect::<Vec<_>>();
            Ok(Series::new(name, milliseconds)
                .cast(&polars::prelude::DataType::Datetime(
                    TimeUnit::Milliseconds,
                    None,
                ))
                .map_err(|error| format!("No se pudo crear la columna de fecha y hora: {error}"))?
                .into_column())
        }
    }
}

pub(super) fn cast_column_with_invalid_values_nullified(
    column: &Column,
    target: RecipeCastTarget,
) -> Result<(Column, Vec<bool>), String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let mut invalid_rows = vec![false; values.len()];
    let converted = match target {
        RecipeCastTarget::String => {
            return Ok((strict_cast_column(column, target)?, invalid_rows));
        }
        RecipeCastTarget::Integer => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| match value.as_deref() {
                    None => None,
                    Some(value) => match value.trim().parse::<i64>() {
                        Ok(value) => Some(value),
                        Err(_) => {
                            invalid_rows[row] = true;
                            None
                        }
                    },
                })
                .collect::<Vec<_>>();
            Series::new(name, parsed).into_column()
        }
        RecipeCastTarget::Decimal => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| match value.as_deref() {
                    None => None,
                    Some(value) => match value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|number| number.is_finite())
                    {
                        Some(value) => Some(value),
                        None => {
                            invalid_rows[row] = true;
                            None
                        }
                    },
                })
                .collect::<Vec<_>>();
            Series::new(name, parsed).into_column()
        }
        RecipeCastTarget::Boolean => {
            let parsed = values
                .iter()
                .enumerate()
                .map(|(row, value)| match value.as_deref() {
                    None => None,
                    Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => {
                            invalid_rows[row] = true;
                            None
                        }
                    },
                })
                .collect::<Vec<_>>();
            Series::new(name, parsed).into_column()
        }
    };
    Ok((converted, invalid_rows))
}

pub(super) fn date_column_with_invalid_values_nullified(
    column: &Column,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
) -> Result<(Column, Vec<bool>), String> {
    let name = column.name().clone();
    let values = strict_column_text(column)?;
    let mut invalid_rows = vec![false; values.len()];
    let parsed = values
        .iter()
        .enumerate()
        .map(|(row, value)| match value.as_deref() {
            None => None,
            Some(value) => match parse_recipe_datetime(value, format) {
                Ok(value) => Some(value),
                Err(()) => {
                    invalid_rows[row] = true;
                    None
                }
            },
        })
        .collect::<Vec<_>>();

    let converted = match target {
        RecipeDateTarget::Date => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
            let days = parsed
                .into_iter()
                .map(|value| value.map(|value| (value.date() - epoch).num_days() as i32))
                .collect::<Vec<_>>();
            Series::new(name, days)
                .cast(&polars::prelude::DataType::Date)
                .map_err(|error| format!("No se pudo crear la columna de fecha: {error}"))?
                .into_column()
        }
        RecipeDateTarget::Datetime => {
            let milliseconds = parsed
                .into_iter()
                .map(|value| value.map(|value| value.and_utc().timestamp_millis()))
                .collect::<Vec<_>>();
            Series::new(name, milliseconds)
                .cast(&polars::prelude::DataType::Datetime(
                    TimeUnit::Milliseconds,
                    None,
                ))
                .map_err(|error| format!("No se pudo crear la columna de fecha y hora: {error}"))?
                .into_column()
        }
    };
    Ok((converted, invalid_rows))
}

pub(super) fn exclude_invalid_conversion_rows(
    frame: DataFrame,
    invalid_rows: &[bool],
) -> Result<(DataFrame, usize), String> {
    if invalid_rows.len() != frame.height() {
        return Err("La máscara de excepciones no coincide con las filas activas.".to_owned());
    }
    let keep = invalid_rows
        .iter()
        .map(|invalid| !invalid)
        .collect::<Vec<_>>();
    let removed = invalid_rows.iter().filter(|invalid| **invalid).count();
    if removed == 0 {
        return Ok((frame, 0));
    }
    let filtered = frame
        .filter(&BooleanChunked::from_slice(
            "invalid_conversion_rows".into(),
            &keep,
        ))
        .map_err(|error| {
            format!("No se pudieron excluir filas con conversiones inválidas: {error}")
        })?;
    Ok((filtered, removed))
}

pub(super) fn exception_action_for_cast(
    policy: Option<&ImportExceptionPolicy>,
    column: &str,
    target: RecipeCastTarget,
) -> Result<InvalidConversionAction, String> {
    let Some(policy) = policy else {
        return Ok(InvalidConversionAction::Review);
    };
    policy
        .conversions
        .iter()
        .find_map(|conversion| match conversion {
            ImportExceptionConversion::Cast {
                column: candidate,
                target: candidate_target,
                on_invalid,
            } if candidate == column && *candidate_target == target => Some(*on_invalid),
            _ => None,
        })
        .ok_or_else(|| format!("La política no contiene una decisión para convertir '{column}'."))
}

pub(super) fn exception_action_for_date(
    policy: Option<&ImportExceptionPolicy>,
    column: &str,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
) -> Result<InvalidConversionAction, String> {
    let Some(policy) = policy else {
        return Ok(InvalidConversionAction::Review);
    };
    policy
        .conversions
        .iter()
        .find_map(|conversion| match conversion {
            ImportExceptionConversion::Date {
                column: candidate,
                format: candidate_format,
                target: candidate_target,
                on_invalid,
            } if candidate == column
                && *candidate_format == format
                && *candidate_target == target =>
            {
                Some(*on_invalid)
            }
            _ => None,
        })
        .ok_or_else(|| format!("La política no contiene una decisión para interpretar '{column}'."))
}

pub(super) fn remapped_name<'a>(name: &'a str, renames: &HashMap<&'a str, &'a str>) -> &'a str {
    renames.get(name).copied().unwrap_or(name)
}

pub(super) fn strict_f64(value: &str, context: &str) -> Result<f64, String> {
    let trimmed = value.trim();
    if !trimmed.contains(['.', 'e', 'E'])
        && trimmed
            .parse::<i128>()
            .is_ok_and(|integer| integer.unsigned_abs() > (1_u128 << 53))
    {
        return Err(format!(
            "{context} excede la precisión numérica segura: '{value}'."
        ));
    }
    trimmed
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("{context} debe ser un número finito: '{value}'."))
}

pub(super) fn apply_recipe_filters(
    frame: DataFrame,
    filters: &[RecipeFilter],
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize), String> {
    if filters.len() > 3 {
        return Err("La receta admite como máximo tres filtros combinados con AND.".into());
    }
    let original_height = frame.height();
    let mut combined_mask = vec![true; original_height];
    for filter in filters {
        let name = remapped_name(&filter.column, renames);
        let values = strict_column_text(recipe_column(&frame, name)?)?;
        let unary = matches!(
            filter.operator,
            RecipeFilterOperator::IsNull | RecipeFilterOperator::NotNull
        );
        if unary != filter.value.is_none() {
            return Err(format!(
                "El filtro '{}' {} un valor.",
                filter.column,
                if unary { "no acepta" } else { "requiere" }
            ));
        }
        let literal = filter.value.as_deref().unwrap_or_default();
        if matches!(
            filter.operator,
            RecipeFilterOperator::Gt
                | RecipeFilterOperator::Lt
                | RecipeFilterOperator::Gte
                | RecipeFilterOperator::Lte
                | RecipeFilterOperator::Contains
                | RecipeFilterOperator::NotContains
        ) && literal.is_empty()
        {
            return Err(format!(
                "El filtro '{}' requiere un valor no vacío.",
                filter.column
            ));
        }

        let column_dtype = recipe_column(&frame, name)?.dtype();
        let temporal_literal = if recipe_filter_uses_temporal_literal(filter.operator)
            && matches!(
                column_dtype,
                polars::prelude::DataType::Date | polars::prelude::DataType::Datetime(_, None)
            ) {
            Some(parse_recipe_filter_datetime(literal, name)?)
        } else {
            None
        };
        let numeric_literal =
            if recipe_filter_is_ordered(filter.operator) && temporal_literal.is_none() {
                Some(strict_f64(literal, "El valor del filtro")?)
            } else {
                None
            };
        let temporal_values = temporal_literal
            .as_ref()
            .map(|_| {
                values
                    .iter()
                    .enumerate()
                    .map(|(row, value)| {
                        value
                            .as_deref()
                            .map(|value| {
                                parse_recipe_filter_datetime(value, name).map_err(|_| {
                                    format!(
                                        "La fila {} de la columna '{}' contiene una fecha no válida.",
                                        row + 1,
                                        name
                                    )
                                })
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?;
        let numeric_values = if numeric_literal.is_some() {
            Some(
                values
                    .iter()
                    .enumerate()
                    .map(|(row, value)| {
                        value
                            .as_deref()
                            .map(|value| {
                                strict_f64(
                                    value,
                                    &format!("La fila {} de la columna '{name}'", row + 1),
                                )
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            None
        };
        let needle = literal.to_lowercase();
        let mask = values
            .iter()
            .enumerate()
            .map(|(row, value)| match filter.operator {
                RecipeFilterOperator::IsNull => value.is_none(),
                RecipeFilterOperator::NotNull => value.is_some(),
                RecipeFilterOperator::Eq => temporal_values.as_ref().map_or_else(
                    || value.as_deref() == Some(literal),
                    |values| {
                        values[row].is_some_and(|value| {
                            value == temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
                RecipeFilterOperator::Neq => temporal_values.as_ref().map_or_else(
                    || value.as_deref().is_some_and(|value| value != literal),
                    |values| {
                        values[row].is_some_and(|value| {
                            value != temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
                RecipeFilterOperator::Contains => value
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&needle)),
                RecipeFilterOperator::NotContains => value
                    .as_deref()
                    .is_some_and(|value| !value.to_lowercase().contains(&needle)),
                RecipeFilterOperator::Gt => temporal_values.as_ref().map_or_else(
                    || {
                        numeric_values.as_ref().expect("el filtro requiere números")[row]
                            .is_some_and(|value| {
                                value > numeric_literal.expect("el filtro requiere un literal")
                            })
                    },
                    |values| {
                        values[row].is_some_and(|value| {
                            value > temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
                RecipeFilterOperator::Lt => temporal_values.as_ref().map_or_else(
                    || {
                        numeric_values.as_ref().expect("el filtro requiere números")[row]
                            .is_some_and(|value| {
                                value < numeric_literal.expect("el filtro requiere un literal")
                            })
                    },
                    |values| {
                        values[row].is_some_and(|value| {
                            value < temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
                RecipeFilterOperator::Gte => temporal_values.as_ref().map_or_else(
                    || {
                        numeric_values.as_ref().expect("el filtro requiere números")[row]
                            .is_some_and(|value| {
                                value >= numeric_literal.expect("el filtro requiere un literal")
                            })
                    },
                    |values| {
                        values[row].is_some_and(|value| {
                            value >= temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
                RecipeFilterOperator::Lte => temporal_values.as_ref().map_or_else(
                    || {
                        numeric_values.as_ref().expect("el filtro requiere números")[row]
                            .is_some_and(|value| {
                                value <= numeric_literal.expect("el filtro requiere un literal")
                            })
                    },
                    |values| {
                        values[row].is_some_and(|value| {
                            value <= temporal_literal.expect("el filtro requiere una fecha")
                        })
                    },
                ),
            })
            .collect::<Vec<_>>();
        combined_mask
            .iter_mut()
            .zip(mask)
            .for_each(|(combined, current)| *combined &= current);
    }
    let filtered = frame
        .filter(&BooleanChunked::from_slice("filter".into(), &combined_mask))
        .map_err(|error| format!("No se pudieron aplicar los filtros: {error}"))?;
    let removed = original_height.saturating_sub(filtered.height());
    Ok((filtered, removed))
}

pub(super) fn date_parts(
    column: &Column,
    operation: CalculatedOperation,
) -> Result<Vec<Option<i32>>, String> {
    let name = column.name();
    match column.dtype() {
        polars::prelude::DataType::Date => {
            let physical = column
                .cast(&polars::prelude::DataType::Int32)
                .map_err(|error| error.to_string())?;
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
            Ok((0..physical.len())
                .map(|row| {
                    let days = match physical.get(row).map_err(|error| error.to_string())? {
                        AnyValue::Null => None,
                        AnyValue::Int32(value) => Some(value),
                        _ => {
                            return Err(
                                "La fecha no tiene una representación física válida.".into()
                            );
                        }
                    };
                    let date = days
                        .map(|days| {
                            epoch
                                .checked_add_signed(chrono::Duration::days(days.into()))
                                .ok_or_else(|| {
                                    format!(
                                        "La fecha de la fila {} está fuera del rango admitido.",
                                        row + 1
                                    )
                                })
                        })
                        .transpose()?;
                    Ok(date.map(|date| match operation {
                        CalculatedOperation::Year => date.year(),
                        CalculatedOperation::Month => date.month() as i32,
                        CalculatedOperation::Day => date.day() as i32,
                        _ => unreachable!(),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?)
        }
        polars::prelude::DataType::Datetime(unit, _) => {
            let physical = column
                .cast(&polars::prelude::DataType::Int64)
                .map_err(|error| error.to_string())?;
            let divisor = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            Ok((0..physical.len())
                .map(|row| {
                    let raw = match physical.get(row).map_err(|error| error.to_string())? {
                        AnyValue::Null => None,
                        AnyValue::Int64(value) => Some(value),
                        _ => {
                            return Err(
                                "La fecha y hora no tiene una representación física válida.".into(),
                            );
                        }
                    };
                    let date = raw
                        .map(|raw| {
                            DateTime::from_timestamp(
                                raw.div_euclid(divisor),
                                (raw.rem_euclid(divisor) as u64 * (1_000_000_000 / divisor as u64))
                                    as u32,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "La fecha y hora de la fila {} está fuera del rango admitido.",
                                    row + 1
                                )
                            })
                        })
                        .transpose()?;
                    Ok(date.map(|date| match operation {
                        CalculatedOperation::Year => date.year(),
                        CalculatedOperation::Month => date.month() as i32,
                        CalculatedOperation::Day => date.day() as i32,
                        _ => unreachable!(),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?)
        }
        _ => Err(format!(
            "La columna '{name}' debe ser date o datetime para extraer componentes."
        )),
    }
}

pub(super) fn validate_date_parts_column(column: &Column) -> Result<(), String> {
    let name = column.name();
    match column.dtype() {
        polars::prelude::DataType::Date => {
            let physical = column
                .cast(&polars::prelude::DataType::Int32)
                .map_err(|error| error.to_string())?;
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
            for row in 0..physical.len() {
                let value = physical.get(row).map_err(|error| error.to_string())?;
                let AnyValue::Int32(days) = value else {
                    if matches!(value, AnyValue::Null) {
                        continue;
                    }
                    return Err("La fecha no tiene una representación física válida.".into());
                };
                epoch
                    .checked_add_signed(chrono::Duration::days(days.into()))
                    .ok_or_else(|| {
                        format!(
                            "La fecha de la fila {} está fuera del rango admitido.",
                            row + 1
                        )
                    })?;
            }
            Ok(())
        }
        polars::prelude::DataType::Datetime(unit, None) => {
            let physical = column
                .cast(&polars::prelude::DataType::Int64)
                .map_err(|error| error.to_string())?;
            let divisor = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            for row in 0..physical.len() {
                let value = physical.get(row).map_err(|error| error.to_string())?;
                let AnyValue::Int64(raw) = value else {
                    if matches!(value, AnyValue::Null) {
                        continue;
                    }
                    return Err("La fecha y hora no tiene una representación física válida.".into());
                };
                DateTime::from_timestamp(
                    raw.div_euclid(divisor),
                    (raw.rem_euclid(divisor) as u64 * (1_000_000_000 / divisor as u64)) as u32,
                )
                .ok_or_else(|| {
                    format!(
                        "La fecha y hora de la fila {} está fuera del rango admitido.",
                        row + 1
                    )
                })?;
            }
            Ok(())
        }
        _ => Err(format!(
            "La columna '{name}' debe ser date o datetime para extraer componentes."
        )),
    }
}

pub(super) fn add_calculated_column(
    frame: &mut DataFrame,
    calculation: &CalculatedColumnRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(), String> {
    if calculation.name.trim().is_empty() || calculation.name != calculation.name.trim() {
        return Err(
            "El nombre de la columna calculada no puede estar vacío ni tener espacios exteriores."
                .into(),
        );
    }
    if frame.column(&calculation.name).is_ok() {
        return Err(format!(
            "La columna calculada '{}' ya existe.",
            calculation.name
        ));
    }
    let source_name = remapped_name(&calculation.source, renames);
    let source = recipe_column(frame, source_name)?;
    let unary = matches!(
        calculation.operation,
        CalculatedOperation::Year | CalculatedOperation::Month | CalculatedOperation::Day
    );
    if unary != calculation.operand.is_none() {
        return Err(if unary {
            "year, month y day no aceptan operando.".into()
        } else {
            "La operación calculada requiere un operando.".into()
        });
    }
    let column = if unary {
        Series::new(
            calculation.name.clone().into(),
            date_parts(source, calculation.operation)?,
        )
        .into_column()
    } else {
        let source_values = strict_column_text(source)?;
        let operand = calculation.operand.as_ref().unwrap();
        let operand_values = match operand.kind {
            CalculatedOperandKind::Literal => vec![Some(operand.value.clone()); frame.height()],
            CalculatedOperandKind::Column => strict_column_text(recipe_column(
                frame,
                remapped_name(&operand.value, renames),
            )?)?,
        };
        match calculation.operation {
            CalculatedOperation::Concat => Series::new(
                calculation.name.clone().into(),
                source_values
                    .iter()
                    .zip(&operand_values)
                    .map(|(left, right)| {
                        left.as_ref()
                            .zip(right.as_ref())
                            .map(|(left, right)| format!("{left}{right}"))
                    })
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            CalculatedOperation::Add
            | CalculatedOperation::Subtract
            | CalculatedOperation::Multiply
            | CalculatedOperation::Divide => {
                if operand.kind == CalculatedOperandKind::Literal && operand.value.is_empty() {
                    return Err("El operando numérico no puede estar vacío.".into());
                }
                let values = source_values
                    .iter()
                    .zip(&operand_values)
                    .enumerate()
                    .map(|(row, (left, right))| {
                        left.as_deref()
                            .zip(right.as_deref())
                            .map(|(left, right)| {
                                let left = strict_f64(
                                    left,
                                    &format!("La fila {} de '{source_name}'", row + 1),
                                )?;
                                let right = strict_f64(
                                    right,
                                    &format!("El operando de la fila {}", row + 1),
                                )?;
                                if calculation.operation == CalculatedOperation::Divide
                                    && right == 0.0
                                {
                                    return Err(format!(
                                        "División por cero en la fila {}.",
                                        row + 1
                                    ));
                                }
                                let result = match calculation.operation {
                                    CalculatedOperation::Add => left + right,
                                    CalculatedOperation::Subtract => left - right,
                                    CalculatedOperation::Multiply => left * right,
                                    CalculatedOperation::Divide => left / right,
                                    _ => unreachable!(),
                                };
                                if !result.is_finite() {
                                    return Err(format!(
                                        "El cálculo produjo un valor no finito en la fila {}.",
                                        row + 1
                                    ));
                                }
                                Ok(result)
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(calculation.name.clone().into(), values).into_column()
            }
            _ => unreachable!(),
        }
    };
    frame
        .with_column(column)
        .map_err(|error| format!("No se pudo agregar la columna calculada: {error}"))?;
    Ok(())
}

pub(super) fn apply_find_replace(
    frame: &mut DataFrame,
    recipe: &FindReplaceRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<usize, String> {
    let regex = compile_find_replace_pattern(recipe)?;
    let targets = match recipe.scope {
        FindReplaceScope::Column => {
            let column = recipe
                .column
                .as_deref()
                .ok_or_else(|| "La búsqueda por columna requiere una columna.".to_owned())?;
            vec![remapped_name(column, renames).to_owned()]
        }
        FindReplaceScope::AllTextColumns => {
            if recipe.column.is_some() {
                return Err(
                    "La búsqueda en todas las columnas no acepta una columna concreta.".into(),
                );
            }
            frame
                .columns()
                .iter()
                .filter(|column| column.dtype() == &polars::prelude::DataType::String)
                .map(|column| column.name().to_string())
                .collect()
        }
    };
    let mut count = 0;
    for name in targets {
        let column = recipe_column(frame, &name)?;
        if column.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{name}' debe ser de texto para buscar y reemplazar."
            ));
        }
        let values = strict_column_text(column)?;
        let replaced = values
            .into_iter()
            .map(|value| {
                value.map(|value| {
                    let updated = regex
                        .as_ref()
                        .map(|regex| {
                            regex
                                .replace_all(value.as_str(), recipe.replace.as_str())
                                .into_owned()
                        })
                        .unwrap_or_else(|| value.replace(&recipe.find, &recipe.replace));
                    if updated != value {
                        count += 1;
                        updated
                    } else {
                        value
                    }
                })
            })
            .collect::<Vec<_>>();
        frame
            .replace(
                &name,
                Series::new(name.clone().into(), replaced).into_column(),
            )
            .map_err(|error| format!("No se pudo reemplazar texto en '{name}': {error}"))?;
    }
    Ok(count)
}

pub(super) fn validate_find_replace_pattern(recipe: &FindReplaceRecipe) -> Result<(), String> {
    if recipe.regex && !recipe.find.is_empty() {
        Regex::new(&recipe.find).map_err(|_| {
            "El patrón de búsqueda no es una expresión regular válida compatible con Rust."
                .to_owned()
        })?;
    }
    Ok(())
}

pub(super) fn compile_find_replace_pattern(
    recipe: &FindReplaceRecipe,
) -> Result<Option<Regex>, String> {
    if recipe.find.is_empty() {
        return Err("El texto buscado no puede estar vacío.".into());
    }
    validate_find_replace_pattern(recipe)?;
    if recipe.regex {
        Regex::new(&recipe.find).map(Some).map_err(|_| {
            "El patrón de búsqueda no es una expresión regular válida compatible con Rust."
                .to_owned()
        })
    } else {
        Ok(None)
    }
}

pub(super) fn resolve_keep_column_names(
    frame: &DataFrame,
    keep_columns: Option<&[String]>,
    renames: &HashMap<&str, &str>,
) -> Result<(Vec<String>, usize, bool), String> {
    let Some(keep_columns) = keep_columns else {
        return Ok((Vec::new(), 0, false));
    };
    if keep_columns.is_empty() {
        return Err("Debes conservar al menos una columna.".into());
    }
    let mut seen = HashSet::new();
    let names = keep_columns
        .iter()
        .map(|name| {
            if !seen.insert(name) {
                return Err(format!(
                    "La columna '{name}' aparece más de una vez en la selección."
                ));
            }
            let effective = remapped_name(name, renames).to_owned();
            recipe_column(frame, &effective)?;
            Ok(effective)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let order_changed = frame
        .get_column_names()
        .iter()
        .map(|name| name.as_str())
        .ne(names.iter().map(String::as_str));
    Ok((
        names,
        frame.width().saturating_sub(keep_columns.len()),
        order_changed,
    ))
}

pub(super) fn apply_keep_columns(
    frame: DataFrame,
    keep_columns: Option<&[String]>,
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, bool), String> {
    let (names, dropped_count, order_changed) =
        resolve_keep_column_names(&frame, keep_columns, renames)?;
    if keep_columns.is_none() {
        return Ok((frame, 0, false));
    }
    let selected = frame
        .select(&names)
        .map_err(|error| format!("No se pudieron conservar las columnas seleccionadas: {error}"))?;
    Ok((selected, dropped_count, order_changed))
}

pub(super) fn apply_split_column(
    frame: &mut DataFrame,
    split: &SplitColumnRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if split.delimiter.is_empty() {
        return Err("El delimitador de división no puede estar vacío.".into());
    }
    if !(2..=16).contains(&split.names.len()) {
        return Err("La división requiere entre 2 y 16 columnas de destino.".into());
    }
    let source_name = remapped_name(&split.source, renames);
    let source = recipe_column(frame, source_name)?;
    if source.dtype() != &polars::prelude::DataType::String {
        return Err(format!(
            "La columna '{source_name}' debe ser de texto para dividirse."
        ));
    }
    let mut unique = HashSet::new();
    let names = split
        .names
        .iter()
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed != name {
                return Err(
                    "Los nombres divididos no pueden estar vacíos ni tener espacios exteriores."
                        .into(),
                );
            }
            if !unique.insert(trimmed) {
                return Err(format!("El nombre dividido '{trimmed}' está duplicado."));
            }
            if frame.column(trimmed).is_ok() {
                return Err(format!("La columna dividida '{trimmed}' ya existe."));
            }
            Ok(trimmed.to_owned())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let source_values = strict_column_text(source)?;
    let mut outputs = vec![Vec::with_capacity(frame.height()); names.len()];
    for value in source_values {
        if let Some(value) = value {
            let parts = value
                .splitn(names.len(), &split.delimiter)
                .collect::<Vec<_>>();
            for (index, output) in outputs.iter_mut().enumerate() {
                output.push(parts.get(index).map(|part| (*part).to_owned()));
            }
        } else {
            outputs.iter_mut().for_each(|output| output.push(None));
        }
    }
    for (name, values) in names.into_iter().zip(outputs) {
        frame
            .with_column(Series::new(name.into(), values).into_column())
            .map_err(|error| format!("No se pudo crear una columna dividida: {error}"))?;
    }
    let dropped = if split.drop_source {
        frame
            .drop_in_place(source_name)
            .map_err(|error| format!("No se pudo descartar '{source_name}': {error}"))?;
        1
    } else {
        0
    };
    Ok((split.names.len(), dropped))
}

pub(super) fn apply_merge_columns(
    frame: &mut DataFrame,
    merge: &MergeColumnsRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if !(2..=16).contains(&merge.sources.len()) {
        return Err("La unión requiere entre 2 y 16 columnas fuente.".into());
    }
    if merge.name.trim().is_empty() || merge.name != merge.name.trim() {
        return Err(
            "El nombre de la columna unida no puede estar vacío ni tener espacios exteriores."
                .into(),
        );
    }
    if frame.column(&merge.name).is_ok() {
        return Err(format!("La columna unida '{}' ya existe.", merge.name));
    }
    let mut unique = HashSet::new();
    let sources = merge
        .sources
        .iter()
        .map(|source| {
            if !unique.insert(source) {
                return Err(format!("La columna fuente '{source}' está duplicada."));
            }
            let effective = remapped_name(source, renames).to_owned();
            let column = recipe_column(frame, &effective)?;
            if column.dtype() != &polars::prelude::DataType::String {
                return Err(format!(
                    "La columna '{effective}' debe ser de texto para unirse."
                ));
            }
            Ok(effective)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let columns = sources
        .iter()
        .map(|name| strict_column_text(recipe_column(frame, name)?))
        .collect::<Result<Vec<_>, _>>()?;
    let merged = (0..frame.height())
        .map(|row| {
            let values = columns
                .iter()
                .filter_map(|column| column[row].as_deref())
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(&merge.separator))
        })
        .collect::<Vec<_>>();
    frame
        .with_column(Series::new(merge.name.clone().into(), merged).into_column())
        .map_err(|error| format!("No se pudo crear la columna unida: {error}"))?;
    let dropped = if merge.drop_sources {
        for source in &sources {
            frame
                .drop_in_place(source)
                .map_err(|error| format!("No se pudo descartar '{source}': {error}"))?;
        }
        sources.len()
    } else {
        0
    };
    Ok((1, dropped))
}

pub(super) fn outlier_linear_quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

pub(super) fn physical_numeric_values(column: &Column) -> Result<Vec<Option<f64>>, String> {
    let name = column.name();
    (0..column.len())
        .map(|row| {
            let value = column.get(row).map_err(|error| error.to_string())?;
            match value {
                AnyValue::Null => Ok(None),
                AnyValue::Int64(value) if value.unsigned_abs() <= (1_u64 << 53) => Ok(Some(value as f64)),
                AnyValue::Int64(value) => Err(format!("La columna '{name}' contiene un entero fuera de la precisión segura en la fila {}: {value}.", row + 1)),
                AnyValue::Float64(value) if value.is_finite() => Ok(Some(value)),
                AnyValue::Float64(_) => Err(format!(
                    "La columna '{name}' contiene NaN o infinito en la fila {}.",
                    row + 1
                )),
                _ => Err(format!("La columna '{name}' debe ser Int64 o Float64 para tratar valores numéricos.")),
            }
        })
        .collect()
}

pub(super) type PreparedOutlierTreatment = (
    String,
    OutlierAction,
    Vec<Option<f64>>,
    f64,
    f64,
    f64,
    DataType,
);

pub(super) fn prepare_outlier_treatments(
    frame: &DataFrame,
    treatments: &[OutlierTreatment],
    renames: &HashMap<&str, &str>,
) -> Result<Vec<PreparedOutlierTreatment>, String> {
    if treatments.len() > 16 {
        return Err("La receta admite como máximo 16 tratamientos de atípicos.".into());
    }
    let mut unique = HashSet::new();
    let mut prepared = Vec::with_capacity(treatments.len());
    for treatment in treatments {
        if !unique.insert(treatment.column.as_str()) {
            return Err(format!(
                "La columna '{}' tiene más de un tratamiento de atípicos.",
                treatment.column
            ));
        }
        let name = remapped_name(&treatment.column, renames).to_owned();
        let column = recipe_column(frame, &name)?;
        if !matches!(
            column.dtype(),
            polars::prelude::DataType::Int64 | polars::prelude::DataType::Float64
        ) {
            return Err(format!(
                "La columna '{name}' debe ser Int64 o Float64 para tratar valores numéricos."
            ));
        }
        let values = physical_numeric_values(column)?;
        let mut valid = values.iter().flatten().copied().collect::<Vec<_>>();
        if valid.len() < 4 {
            return Err(format!(
                "La columna '{name}' requiere al menos cuatro valores numéricos válidos."
            ));
        }
        valid.sort_by(f64::total_cmp);
        let q1 = outlier_linear_quantile(&valid, 0.25);
        let q3 = outlier_linear_quantile(&valid, 0.75);
        let iqr = q3 - q1;
        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;
        if ![q1, q3, iqr, lower, upper].into_iter().all(f64::is_finite) {
            return Err(format!(
                "Los umbrales IQR de '{name}' exceden el rango numérico finito."
            ));
        }
        let median = if column.dtype() == &DataType::Int64 {
            valid[(valid.len() - 1) / 2]
        } else {
            outlier_linear_quantile(&valid, 0.5)
        };
        prepared.push((
            name,
            treatment.action,
            values,
            lower,
            upper,
            median,
            column.dtype().clone(),
        ));
    }
    Ok(prepared)
}

pub(super) fn apply_outlier_treatments(
    mut frame: DataFrame,
    treatments: &[OutlierTreatment],
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, usize, usize), String> {
    let prepared = prepare_outlier_treatments(&frame, treatments, renames)?;

    let baseline_height = frame.height();
    let mut drop_mask = vec![false; baseline_height];
    let mut adjusted = 0;
    for (name, action, values, lower, upper, median, dtype) in prepared {
        match action {
            OutlierAction::Cap => {
                let capped = values
                    .into_iter()
                    .map(|value| {
                        value.map(|value| {
                            let capped = value.clamp(lower, upper);
                            if capped != value {
                                adjusted += 1;
                            }
                            capped
                        })
                    })
                    .collect::<Vec<_>>();
                if capped
                    .iter()
                    .zip((0..baseline_height).map(|row| {
                        frame
                            .column(&name)
                            .unwrap()
                            .get(row)
                            .ok()
                            .and_then(numeric_value)
                    }))
                    .any(|(left, right)| *left != right)
                {
                    frame
                        .replace(
                            &name,
                            Series::new(name.clone().into(), capped).into_column(),
                        )
                        .map_err(|error| format!("No se pudo limitar '{name}': {error}"))?;
                }
            }
            OutlierAction::Drop => {
                for (row, value) in values.into_iter().enumerate() {
                    if value.is_some_and(|value| value < lower || value > upper) {
                        drop_mask[row] = true;
                    }
                }
            }
            OutlierAction::Impute => {
                let imputed = values
                    .into_iter()
                    .map(|value| {
                        value.map(|value| {
                            if value < lower || value > upper {
                                adjusted += 1;
                                median
                            } else {
                                value
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                let replacement_column = Column::new(name.clone().into(), imputed)
                    .cast(&dtype)
                    .map_err(|error| format!("No se pudo imputar '{name}': {error}"))?;
                frame
                    .replace(&name, replacement_column)
                    .map_err(|error| format!("No se pudo actualizar '{name}': {error}"))?;
            }
        }
    }
    let keep = drop_mask.iter().map(|drop| !drop).collect::<Vec<_>>();
    let removed = drop_mask.iter().filter(|drop| **drop).count();
    if removed > 0 {
        frame = frame
            .filter(&BooleanChunked::from_slice("outliers".into(), &keep))
            .map_err(|error| format!("No se pudieron retirar filas atípicas: {error}"))?;
    }
    Ok((frame, adjusted, removed, treatments.len()))
}

pub(super) fn apply_group_summary(
    frame: DataFrame,
    summary: &GroupSummaryRecipe,
    renames: &HashMap<&str, &str>,
) -> Result<(DataFrame, usize, usize, usize), String> {
    if summary.group_by.is_empty() || summary.group_by.len() > 8 {
        return Err("Agrupar requiere entre 1 y 8 columnas clave.".into());
    }
    if summary.aggregations.is_empty() || summary.aggregations.len() > 32 {
        return Err("Resumir requiere entre 1 y 32 agregaciones.".into());
    }
    let groups = summary
        .group_by
        .iter()
        .map(|name| remapped_name(name, renames).to_owned())
        .collect::<Vec<_>>();
    let mut group_unique = HashSet::new();
    for name in &groups {
        if !group_unique.insert(name) {
            return Err(format!("La clave de grupo '{name}' está duplicada."));
        }
        recipe_column(&frame, name)?;
    }
    let mut aggregation_unique = HashSet::new();
    let mut output_names = HashSet::new();
    let aggregations = summary
        .aggregations
        .iter()
        .map(|aggregation| {
            let name = remapped_name(&aggregation.column, renames).to_owned();
            if !aggregation_unique.insert((name.clone(), aggregation.operation)) {
                return Err(format!(
                    "La agregación '{}_{}' está duplicada.",
                    name,
                    aggregation.operation.suffix()
                ));
            }
            let output = format!("{}_{}", name, aggregation.operation.suffix());
            if group_unique.contains(&output) || !output_names.insert(output.clone()) {
                return Err(format!(
                    "El nombre de salida '{output}' colisiona con otra columna."
                ));
            }
            let column = recipe_column(&frame, &name)?;
            match aggregation.operation {
                SummaryOperation::Sum | SummaryOperation::Mean
                    if !matches!(
                        column.dtype(),
                        polars::prelude::DataType::Int64 | polars::prelude::DataType::Float64
                    ) =>
                {
                    return Err(format!(
                        "La agregación {} requiere que '{name}' sea Int64 o Float64.",
                        aggregation.operation.suffix()
                    ));
                }
                SummaryOperation::Min | SummaryOperation::Max
                    if !matches!(
                        column.dtype(),
                        polars::prelude::DataType::Int64
                            | polars::prelude::DataType::Float64
                            | polars::prelude::DataType::String
                            | polars::prelude::DataType::Date
                            | polars::prelude::DataType::Datetime(_, _)
                    ) =>
                {
                    return Err(format!(
                        "La agregación {} no admite el tipo de '{name}'.",
                        aggregation.operation.suffix()
                    ));
                }
                _ => {}
            }
            Ok((name, output, aggregation.operation))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut positions: HashMap<Vec<Option<String>>, usize> = HashMap::new();
    let mut buckets: Vec<Vec<IdxSize>> = Vec::new();
    for row in 0..frame.height() {
        let key = groups
            .iter()
            .map(|name| {
                match frame
                    .column(name)
                    .unwrap()
                    .get(row)
                    .map_err(|error| error.to_string())?
                {
                    AnyValue::Null => Ok(None),
                    AnyValue::Float64(value) if !value.is_finite() => Err(format!(
                        "La clave de grupo '{name}' contiene NaN o infinito."
                    )),
                    AnyValue::Float64(0.0) => Ok(Some("0".into())),
                    value => Ok(Some(value.to_string())),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        let index = if let Some(index) = positions.get(&key) {
            *index
        } else {
            let index = buckets.len();
            positions.insert(key, index);
            buckets.push(Vec::new());
            index
        };
        buckets[index].push(row as IdxSize);
    }
    let first_rows = buckets.iter().map(|rows| rows[0]).collect::<Vec<_>>();
    let mut columns = groups
        .iter()
        .map(|name| {
            frame
                .column(name)
                .unwrap()
                .take_slice(&first_rows)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (name, output, operation) in aggregations {
        let source = frame.column(&name).map_err(|error| error.to_string())?;
        let result = match operation {
            SummaryOperation::Count => Series::new(
                output.clone().into(),
                buckets
                    .iter()
                    .map(|rows| rows.len() as i64)
                    .collect::<Vec<_>>(),
            )
            .into_column(),
            SummaryOperation::CountUnique => {
                let counts = buckets
                    .iter()
                    .map(|rows| {
                        let mut unique = HashSet::new();
                        for row in rows {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => {}
                                AnyValue::Float64(value) if !value.is_finite() => {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ));
                                }
                                AnyValue::Float64(0.0) => {
                                    unique.insert("0".to_owned());
                                }
                                value => {
                                    unique.insert(value.to_string());
                                }
                            }
                        }
                        Ok(unique.len() as i64)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), counts).into_column()
            }
            SummaryOperation::Sum if source.dtype() == &polars::prelude::DataType::Int64 => {
                let values = buckets
                    .iter()
                    .map(|rows| {
                        rows.iter().try_fold(0_i64, |total, row| {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => Ok(total),
                                AnyValue::Int64(value) => total
                                    .checked_add(value)
                                    .ok_or_else(|| format!("La suma de '{name}' desbordó Int64.")),
                                _ => unreachable!(),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), values).into_column()
            }
            SummaryOperation::Sum | SummaryOperation::Mean => {
                let is_mean = operation == SummaryOperation::Mean;
                let values = buckets
                    .iter()
                    .map(|rows| {
                        let mut sum = 0.0;
                        let mut count = 0;
                        for row in rows {
                            match source.get(*row as usize).unwrap() {
                                AnyValue::Null => {}
                                AnyValue::Int64(value) => {
                                    if value.unsigned_abs() > (1_u64 << 53) {
                                        return Err(format!(
                                            "La media de '{name}' excede la precisión segura."
                                        ));
                                    }
                                    sum += value as f64;
                                    count += 1;
                                }
                                AnyValue::Float64(value) if value.is_finite() => {
                                    sum += value;
                                    count += 1;
                                }
                                AnyValue::Float64(_) => {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ));
                                }
                                _ => unreachable!(),
                            }
                        }
                        if !sum.is_finite() {
                            return Err(format!(
                                "La agregación de '{name}' produjo un valor no finito."
                            ));
                        }
                        Ok(if is_mean {
                            (count > 0).then(|| sum / count as f64)
                        } else {
                            Some(sum)
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Series::new(output.clone().into(), values).into_column()
            }
            SummaryOperation::Min | SummaryOperation::Max => {
                let take_max = operation == SummaryOperation::Max;
                let selected = buckets
                    .iter()
                    .map(|rows| {
                        let mut best: Option<(IdxSize, AnyValue<'_>)> = None;
                        for row in rows {
                            let value = source.get(*row as usize).unwrap();
                            if !matches!(value, AnyValue::Null) {
                                if matches!(value, AnyValue::Float64(v) if !v.is_finite()) {
                                    return Err(format!(
                                        "La columna '{name}' contiene NaN o infinito."
                                    ));
                                }
                                let better = best.as_ref().is_none_or(|(_, current)| {
                                    match (&value, current) {
                                        (AnyValue::Int64(a), AnyValue::Int64(b)) => {
                                            if take_max {
                                                a > b
                                            } else {
                                                a < b
                                            }
                                        }
                                        (AnyValue::Float64(a), AnyValue::Float64(b)) => {
                                            if take_max {
                                                a > b
                                            } else {
                                                a < b
                                            }
                                        }
                                        _ => {
                                            if take_max {
                                                value.to_string() > current.to_string()
                                            } else {
                                                value.to_string() < current.to_string()
                                            }
                                        }
                                    }
                                });
                                if better {
                                    best = Some((*row, value));
                                }
                            }
                        }
                        Ok(best.map(|(row, _)| row))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                match source.dtype() {
                    polars::prelude::DataType::Int64 => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::Int64(value) => value,
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::Float64 => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::Float64(value) => value,
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::String => Series::new(
                        output.clone().into(),
                        selected
                            .iter()
                            .map(|row| {
                                row.map(|row| match source.get(row as usize).unwrap() {
                                    AnyValue::String(value) => value.to_owned(),
                                    AnyValue::StringOwned(value) => value.to_string(),
                                    _ => unreachable!(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .into_column(),
                    polars::prelude::DataType::Date => {
                        let physical = source
                            .cast(&polars::prelude::DataType::Int32)
                            .map_err(|error| error.to_string())?;
                        Series::new(
                            output.clone().into(),
                            selected
                                .iter()
                                .map(|row| {
                                    row.map(|row| match physical.get(row as usize).unwrap() {
                                        AnyValue::Int32(value) => value,
                                        _ => unreachable!(),
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                        .cast(&polars::prelude::DataType::Date)
                        .map_err(|error| error.to_string())?
                        .into_column()
                    }
                    polars::prelude::DataType::Datetime(unit, zone) => {
                        let physical = source
                            .cast(&polars::prelude::DataType::Int64)
                            .map_err(|error| error.to_string())?;
                        Series::new(
                            output.clone().into(),
                            selected
                                .iter()
                                .map(|row| {
                                    row.map(|row| match physical.get(row as usize).unwrap() {
                                        AnyValue::Int64(value) => value,
                                        _ => unreachable!(),
                                    })
                                })
                                .collect::<Vec<_>>(),
                        )
                        .cast(&polars::prelude::DataType::Datetime(*unit, zone.clone()))
                        .map_err(|error| error.to_string())?
                        .into_column()
                    }
                    _ => unreachable!(),
                }
            }
        };
        columns.push(result);
    }
    let group_count = buckets.len();
    let collapsed = frame.height().saturating_sub(group_count);
    Ok((
        DataFrame::new(group_count, columns).map_err(|error| error.to_string())?,
        group_count,
        summary.aggregations.len(),
        collapsed,
    ))
}

pub(super) fn apply_contact_normalizations(
    frame: &mut DataFrame,
    treatments: &[ContactNormalization],
    renames: &HashMap<&str, &str>,
) -> Result<(usize, usize), String> {
    if treatments.len() > 16 {
        return Err("La receta admite como máximo 16 normalizaciones de contacto.".into());
    }
    let mut unique = HashSet::new();
    let mut changed_cells = 0;
    for treatment in treatments {
        if !unique.insert(treatment.column.as_str()) {
            return Err(format!(
                "La columna '{}' tiene más de una normalización de contacto.",
                treatment.column
            ));
        }
        let name = remapped_name(&treatment.column, renames).to_owned();
        let column = recipe_column(frame, &name)?;
        if column.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{name}' debe ser de texto para normalizar contactos."
            ));
        }
        let values = strict_column_text(column)?
            .into_iter()
            .map(|value| {
                value.map(|value| {
                    let normalized = match treatment.kind {
                        ContactKind::Email => value.trim().to_lowercase(),
                        ContactKind::Phone => {
                            let trimmed = value.trim();
                            let plus = trimmed.starts_with('+');
                            let digits = trimmed
                                .chars()
                                .filter(char::is_ascii_digit)
                                .collect::<String>();
                            if plus {
                                format!("+{digits}")
                            } else {
                                digits
                            }
                        }
                        ContactKind::Address => {
                            value.split_whitespace().collect::<Vec<_>>().join(" ")
                        }
                    };
                    if normalized != value {
                        changed_cells += 1;
                    }
                    normalized
                })
            })
            .collect::<Vec<_>>();
        frame
            .replace(
                &name,
                Series::new(name.clone().into(), values).into_column(),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok((changed_cells, treatments.len()))
}

pub(super) fn first_run(value: &str, matches: impl Fn(char) -> bool) -> Option<String> {
    let mut output = String::new();
    let mut started = false;
    for character in value.chars() {
        if matches(character) {
            started = true;
            output.push(character);
        } else if started {
            break;
        }
    }
    started.then_some(output)
}

pub(super) fn apply_text_extractions(
    frame: &mut DataFrame,
    extractions: &[TextExtraction],
    renames: &HashMap<&str, &str>,
) -> Result<usize, String> {
    if extractions.len() > 16 {
        return Err("La receta admite como máximo 16 extracciones de texto.".into());
    }
    let mut names = HashSet::new();
    for extraction in extractions {
        if extraction.name.trim().is_empty() || extraction.name != extraction.name.trim() {
            return Err(
                "El nombre extraído no puede estar vacío ni tener espacios exteriores.".into(),
            );
        }
        if !names.insert(extraction.name.as_str()) {
            return Err(format!(
                "La columna extraída '{}' está duplicada.",
                extraction.name
            ));
        }
        if frame.column(&extraction.name).is_ok() {
            return Err(format!(
                "La columna extraída '{}' ya existe.",
                extraction.name
            ));
        }
        let delimiter_based = matches!(
            extraction.kind,
            ExtractionKind::Before | ExtractionKind::After
        );
        if delimiter_based != extraction.delimiter.is_some() {
            return Err(format!(
                "La extracción '{}' {} delimitador.",
                extraction.name,
                if delimiter_based {
                    "requiere"
                } else {
                    "no acepta"
                }
            ));
        }
        if extraction.delimiter.as_deref().is_some_and(str::is_empty) {
            return Err("El delimitador de extracción no puede estar vacío.".into());
        }
        let source_name = remapped_name(&extraction.source, renames);
        let source = recipe_column(frame, source_name)?;
        if source.dtype() != &polars::prelude::DataType::String {
            return Err(format!(
                "La columna '{source_name}' debe ser de texto para extraerse."
            ));
        }
        let values = strict_column_text(source)?
            .into_iter()
            .map(|value| {
                value.and_then(|value| match extraction.kind {
                    ExtractionKind::FirstToken => {
                        value.split_whitespace().next().map(str::to_owned)
                    }
                    ExtractionKind::LastToken => value.split_whitespace().last().map(str::to_owned),
                    ExtractionKind::Digits => {
                        first_run(&value, |character| character.is_ascii_digit())
                    }
                    ExtractionKind::Letters => first_run(&value, char::is_alphabetic),
                    ExtractionKind::Before => value
                        .find(extraction.delimiter.as_deref().unwrap())
                        .map(|index| value[..index].to_owned()),
                    ExtractionKind::After => value
                        .find(extraction.delimiter.as_deref().unwrap())
                        .map(|index| {
                            value[index + extraction.delimiter.as_deref().unwrap().len()..]
                                .to_owned()
                        }),
                })
            })
            .collect::<Vec<_>>();
        frame
            .with_column(Series::new(extraction.name.clone().into(), values).into_column())
            .map_err(|error| error.to_string())?;
    }
    Ok(extractions.len())
}

pub(super) type RecipeFrameOutcome = (
    DataFrame,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    bool,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
);

pub(super) fn apply_eager_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    apply_eager_recipe_to_frame_with_exception_policy(source, recipe, None)
}

pub(super) fn apply_eager_recipe_to_frame_with_exception_policy(
    source: &DataFrame,
    recipe: &TransformRecipe,
    exception_policy: Option<&ImportExceptionPolicy>,
) -> Result<RecipeFrameOutcome, String> {
    let mut candidate = source.clone();
    let mut rename_sources = HashSet::new();
    let rename_map = recipe
        .renames
        .iter()
        .map(|rename| {
            if rename.from.trim().is_empty() || rename.to.trim().is_empty() {
                return Err("Los nombres de columna no pueden estar vacíos.".to_owned());
            }
            if rename.to != rename.to.trim() {
                return Err(
                    "El nuevo nombre de columna no puede tener espacios exteriores.".to_owned(),
                );
            }
            if !rename_sources.insert(rename.from.as_str()) {
                return Err(format!(
                    "La columna '{}' aparece en más de un renombrado.",
                    rename.from
                ));
            }
            recipe_column(source, &rename.from)?;
            Ok((rename.from.as_str(), rename.to.as_str()))
        })
        .collect::<Result<HashMap<_, _>, String>>()?;
    for cast in &recipe.casts {
        recipe_column(source, &cast.column)?;
    }
    for parse in &recipe.date_parses {
        recipe_column(source, &parse.column)?;
    }
    for filter in &recipe.filters {
        recipe_column(source, &filter.column)?;
    }
    if let Some(find_replace) = &recipe.find_replace {
        if let Some(column) = &find_replace.column {
            recipe_column(source, column)?;
        }
    }
    if let Some(keep_columns) = &recipe.keep_columns {
        for column in keep_columns {
            recipe_column(source, column)?;
        }
    }
    if let Some(split) = &recipe.split_column {
        recipe_column(source, &split.source)?;
    }
    if let Some(merge) = &recipe.merge_columns {
        for source_name in &merge.sources {
            recipe_column(source, source_name)?;
        }
    }
    for treatment in &recipe.outlier_treatments {
        recipe_column(source, &treatment.column)?;
    }
    if let Some(summary) = &recipe.group_summary {
        let mut derived_names = recipe
            .text_extractions
            .iter()
            .map(|extraction| extraction.name.as_str())
            .collect::<HashSet<_>>();
        if let Some(calculation) = &recipe.calculated_column {
            derived_names.insert(calculation.name.as_str());
        }
        if let Some(split) = &recipe.split_column {
            derived_names.extend(split.names.iter().map(String::as_str));
        }
        if let Some(merge) = &recipe.merge_columns {
            derived_names.insert(merge.name.as_str());
        }
        for name in &summary.group_by {
            if !derived_names.contains(name.as_str()) {
                recipe_column(source, name)?;
            }
        }
        for aggregation in &summary.aggregations {
            if !derived_names.contains(aggregation.column.as_str()) {
                recipe_column(source, &aggregation.column)?;
            }
        }
    }
    for treatment in &recipe.contact_normalizations {
        recipe_column(source, &treatment.column)?;
    }
    for extraction in &recipe.text_extractions {
        recipe_column(source, &extraction.source)?;
    }
    if let Some(calculation) = &recipe.calculated_column {
        recipe_column(source, &calculation.source)?;
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            recipe_column(source, value)?;
        }
    }
    let final_names = source
        .get_column_names()
        .iter()
        .map(|name| {
            rename_map
                .get(name.as_str())
                .copied()
                .unwrap_or(name.as_str())
                .to_owned()
        })
        .collect::<Vec<_>>();
    let mut unique_names = HashSet::new();
    if final_names.iter().any(|name| !unique_names.insert(name)) {
        return Err("Los renombrados producirían nombres de columna duplicados.".to_owned());
    }
    let renamed_count = source
        .get_column_names()
        .iter()
        .zip(&final_names)
        .filter(|(before, after)| before.as_str() != after.as_str())
        .count();
    if renamed_count > 0 {
        candidate
            .set_column_names(&final_names)
            .map_err(|error| format!("No se pudieron aplicar los renombrados: {error}"))?;
    }

    let mut cast_columns = HashSet::new();
    let mut excluded_invalid_rows = vec![false; source.height()];
    let mut cast_count = 0;
    for cast in &recipe.casts {
        let effective_name = rename_map
            .get(cast.column.as_str())
            .copied()
            .unwrap_or(cast.column.as_str());
        if !cast_columns.insert(effective_name) {
            return Err(format!(
                "La columna '{}' aparece en más de una conversión.",
                cast.column
            ));
        }
        let column = recipe_column(&candidate, effective_name)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (polars::prelude::DataType::String, RecipeCastTarget::String)
                | (polars::prelude::DataType::Int64, RecipeCastTarget::Integer)
                | (
                    polars::prelude::DataType::Float64,
                    RecipeCastTarget::Decimal
                )
                | (
                    polars::prelude::DataType::Boolean,
                    RecipeCastTarget::Boolean
                )
        );
        if !already_target {
            let action = exception_action_for_cast(exception_policy, &cast.column, cast.target)?;
            let (converted, invalid_rows) = match action {
                InvalidConversionAction::Review => (
                    strict_cast_column(column, cast.target)?,
                    vec![false; source.height()],
                ),
                InvalidConversionAction::Nullify | InvalidConversionAction::ExcludeRow => {
                    cast_column_with_invalid_values_nullified(column, cast.target)?
                }
            };
            if action == InvalidConversionAction::ExcludeRow {
                for (excluded, invalid) in excluded_invalid_rows.iter_mut().zip(invalid_rows) {
                    *excluded |= invalid;
                }
            }
            candidate
                .replace(effective_name, converted)
                .map_err(|error| format!("No se pudo convertir '{}': {error}", effective_name))?;
            cast_count += 1;
        }
    }

    let mut date_columns = HashSet::new();
    let mut date_count = 0;
    for parse in &recipe.date_parses {
        let effective_name = rename_map
            .get(parse.column.as_str())
            .copied()
            .unwrap_or(parse.column.as_str());
        if cast_columns.contains(effective_name) {
            return Err(format!(
                "La columna '{}' no puede convertirse y parsearse como fecha en la misma receta.",
                parse.column
            ));
        }
        if !date_columns.insert(effective_name) {
            return Err(format!(
                "La columna '{}' aparece en más de un parseo de fecha.",
                parse.column
            ));
        }
        let column = recipe_column(&candidate, effective_name)?;
        let already_target = matches!(
            (column.dtype(), parse.target),
            (polars::prelude::DataType::Date, RecipeDateTarget::Date)
                | (
                    polars::prelude::DataType::Datetime(_, _),
                    RecipeDateTarget::Datetime
                )
        );
        if !already_target {
            let action = exception_action_for_date(
                exception_policy,
                &parse.column,
                parse.format,
                parse.target,
            )?;
            let (converted, invalid_rows) = match action {
                InvalidConversionAction::Review => (
                    strict_date_column(column, parse.format, parse.target)?,
                    vec![false; source.height()],
                ),
                InvalidConversionAction::Nullify | InvalidConversionAction::ExcludeRow => {
                    date_column_with_invalid_values_nullified(column, parse.format, parse.target)?
                }
            };
            if action == InvalidConversionAction::ExcludeRow {
                for (excluded, invalid) in excluded_invalid_rows.iter_mut().zip(invalid_rows) {
                    *excluded |= invalid;
                }
            }
            candidate
                .replace(effective_name, converted)
                .map_err(|error| {
                    format!(
                        "No se pudo convertir la fecha '{}': {error}",
                        effective_name
                    )
                })?;
            date_count += 1;
        }
    }

    let (candidate, exception_removed_row_count) =
        exclude_invalid_conversion_rows(candidate, &excluded_invalid_rows)?;
    let (mut candidate, recipe_removed_row_count) =
        apply_recipe_filters(candidate, &recipe.filters, &rename_map)?;
    let removed_row_count = exception_removed_row_count + recipe_removed_row_count;
    let replaced_cell_count = if let Some(find_replace) = &recipe.find_replace {
        apply_find_replace(&mut candidate, find_replace, &rename_map)?
    } else {
        0
    };
    let (mut candidate, dropped_column_count, kept_order_changed) =
        apply_keep_columns(candidate, recipe.keep_columns.as_deref(), &rename_map)?;
    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        let source_name = remapped_name(&calculation.source, &rename_map);
        recipe_column(&candidate, source_name).map_err(|_| {
            format!("La columna fuente calculada '{source_name}' fue descartada por keepColumns.")
        })?;
        if let Some(CalculatedOperand {
            kind: CalculatedOperandKind::Column,
            value,
        }) = &calculation.operand
        {
            let operand_name = remapped_name(value, &rename_map);
            recipe_column(&candidate, operand_name).map_err(|_| {
                format!(
                    "La columna operando calculada '{operand_name}' fue descartada por keepColumns."
                )
            })?;
        }
        add_calculated_column(&mut candidate, calculation, &rename_map)?;
        1
    } else {
        0
    };
    if let (Some(split), Some(merge)) = (&recipe.split_column, &recipe.merge_columns) {
        if split.drop_source && merge.sources.contains(&split.source) {
            return Err(format!(
                "La unión necesita '{}', pero la división la descartaría.",
                split.source
            ));
        }
    }
    let (split_column_count, split_dropped) = if let Some(split) = &recipe.split_column {
        let effective = remapped_name(&split.source, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!("La columna '{effective}' requerida por split fue descartada por keepColumns.")
        })?;
        apply_split_column(&mut candidate, split, &rename_map)?
    } else {
        (0, 0)
    };
    let (merged_column_count, merge_dropped) = if let Some(merge) = &recipe.merge_columns {
        for source in &merge.sources {
            let effective = remapped_name(source, &rename_map);
            recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' requerida por merge fue descartada por keepColumns o split."))?;
        }
        apply_merge_columns(&mut candidate, merge, &rename_map)?
    } else {
        (0, 0)
    };
    let dropped_source_column_count = split_dropped + merge_dropped;
    for treatment in &recipe.outlier_treatments {
        let effective = remapped_name(&treatment.column, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' para tratar atípicos no sobrevivió las etapas estructurales."))?;
    }
    let (candidate, adjusted_outlier_cell_count, outlier_removed_row_count, outlier_column_count) =
        apply_outlier_treatments(candidate, &recipe.outlier_treatments, &rename_map)?;
    let mut candidate = candidate;
    for treatment in &recipe.contact_normalizations {
        let effective = remapped_name(&treatment.column, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!(
                "La columna '{effective}' para contactos no sobrevivió las etapas estructurales."
            )
        })?;
    }
    for extraction in &recipe.text_extractions {
        let effective = remapped_name(&extraction.source, &rename_map);
        recipe_column(&candidate, effective).map_err(|_| {
            format!(
                "La columna '{effective}' para extracción no sobrevivió las etapas estructurales."
            )
        })?;
    }
    let (normalized_contact_cell_count, normalized_contact_column_count) =
        apply_contact_normalizations(&mut candidate, &recipe.contact_normalizations, &rename_map)?;
    let extracted_column_count =
        apply_text_extractions(&mut candidate, &recipe.text_extractions, &rename_map)?;
    let (candidate, group_count, aggregated_column_count, collapsed_row_count) = if let Some(
        summary,
    ) =
        &recipe.group_summary
    {
        for name in summary.group_by.iter().chain(
            summary
                .aggregations
                .iter()
                .map(|aggregation| &aggregation.column),
        ) {
            let effective = remapped_name(name, &rename_map);
            recipe_column(&candidate, effective).map_err(|_| format!("La columna '{effective}' requerida por agrupar/resumir no sobrevivió las etapas anteriores."))?;
        }
        apply_group_summary(candidate, summary, &rename_map)?
    } else {
        (candidate, 0, 0, 0)
    };

    Ok((
        candidate,
        renamed_count,
        cast_count,
        date_count,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        kept_order_changed,
        split_column_count,
        merged_column_count,
        dropped_source_column_count,
        adjusted_outlier_cell_count,
        outlier_removed_row_count,
        outlier_column_count,
        group_count,
        aggregated_column_count,
        collapsed_row_count,
        normalized_contact_cell_count,
        normalized_contact_column_count,
        extracted_column_count,
    ))
}
