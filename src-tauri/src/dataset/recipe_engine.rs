use super::*;

type LazySummaryAggregation = (String, String, SummaryOperation);
type LazySummaryPlan = (Vec<String>, Vec<LazySummaryAggregation>);
type LazyContactTarget = (String, ContactKind);
type LazyTextExtractionTarget = (String, ExtractionKind, Option<String>);

pub(super) fn lazy_renames_have_no_cycles(recipe: &TransformRecipe) -> bool {
    recipe.renames.iter().all(|rename| {
        let mut current = rename.from.as_str();
        let mut visited = HashSet::new();
        while let Some(next) = recipe
            .renames
            .iter()
            .find(|candidate| candidate.from == current)
            .map(|candidate| candidate.to.as_str())
        {
            if !visited.insert(current) {
                return false;
            }
            current = next;
        }
        true
    })
}

fn lazy_outlier_columns_survive_keep(recipe: &TransformRecipe) -> bool {
    let remap = |name: &str| {
        recipe
            .renames
            .iter()
            .find(|rename| rename.from == name)
            .map_or_else(|| name.to_owned(), |rename| rename.to.clone())
    };
    recipe.keep_columns.as_ref().is_none_or(|keep_columns| {
        recipe.outlier_treatments.iter().all(|treatment| {
            let effective_name = remap(&treatment.column);
            keep_columns
                .iter()
                .any(|name| remap(name) == effective_name)
        })
    })
}

fn lazy_iso8601_value_supported(value: &str) -> bool {
    let trimmed = value.trim();
    parse_recipe_datetime(trimmed, RecipeDateFormat::Iso8601).is_ok()
        && (DateTime::parse_from_rfc3339(trimmed).is_err() || trimmed.ends_with('Z'))
}

fn lazy_iso8601_column_supported(column: &Column) -> bool {
    column.dtype() == &DataType::String
        && strict_column_text(column).is_ok_and(|values| {
            values
                .iter()
                .flatten()
                .all(|value| lazy_iso8601_value_supported(value))
        })
}

pub(super) fn lazy_recipe_supported(source: &DataFrame, recipe: &TransformRecipe) -> bool {
    lazy_renames_have_no_cycles(recipe)
        && lazy_outlier_columns_survive_keep(recipe)
        && recipe
            .date_parses
            .iter()
            .all(|parse| match recipe_column(source, &parse.column) {
                Ok(column)
                    if matches!(
                        parse.format,
                        RecipeDateFormat::Ymd
                            | RecipeDateFormat::Dmy
                            | RecipeDateFormat::Mdy
                            | RecipeDateFormat::Iso8601
                    ) =>
                {
                    (if matches!(parse.format, RecipeDateFormat::Iso8601) {
                        lazy_iso8601_column_supported(column)
                    } else {
                        matches!(column.dtype(), DataType::String)
                    }) || matches!(
                        (column.dtype(), parse.target),
                        (DataType::Date, RecipeDateTarget::Date)
                            | (DataType::Datetime(_, _), RecipeDateTarget::Datetime)
                    )
                }
                _ => false,
            })
        && !recipe
            .date_parses
            .iter()
            .any(|parse| recipe.casts.iter().any(|cast| cast.column == parse.column))
        && (recipe.outlier_treatments.is_empty()
            || (recipe.casts.is_empty()
                && recipe.date_parses.is_empty()
                && recipe.find_replace.is_none()
                && recipe.calculated_column.is_none()
                && recipe.split_column.is_none()
                && recipe.merge_columns.is_none()
                && recipe.group_summary.is_none()
                && recipe.contact_normalizations.is_empty()
                && recipe.text_extractions.is_empty()
                && recipe.outlier_treatments.iter().all(|treatment| {
                    recipe_column(source, &treatment.column).is_ok_and(|column| {
                        matches!(column.dtype(), DataType::Int64 | DataType::Float64)
                    })
                })))
        && recipe.calculated_column.as_ref().is_none_or(|calculation| {
            matches!(
                calculation.operation,
                CalculatedOperation::Add
                    | CalculatedOperation::Subtract
                    | CalculatedOperation::Multiply
                    | CalculatedOperation::Divide
                    | CalculatedOperation::Concat
            ) || (matches!(
                calculation.operation,
                CalculatedOperation::Year | CalculatedOperation::Month | CalculatedOperation::Day
            ) && recipe
                .casts
                .iter()
                .all(|cast| cast.column != calculation.source)
                && (recipe_column(source, &calculation.source).is_ok_and(|column| {
                    matches!(column.dtype(), DataType::Date | DataType::Datetime(_, None))
                }) || recipe.date_parses.iter().any(|parse| {
                    parse.column == calculation.source
                        && matches!(
                            parse.target,
                            RecipeDateTarget::Date | RecipeDateTarget::Datetime
                        )
                })))
        })
}

fn validate_lazy_recipe_inputs(source: &DataFrame, recipe: &TransformRecipe) -> Result<(), String> {
    for cast in &recipe.casts {
        let column = recipe_column(source, &cast.column)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (DataType::String, RecipeCastTarget::String)
                | (DataType::Int64, RecipeCastTarget::Integer)
                | (DataType::Float64, RecipeCastTarget::Decimal)
                | (DataType::Boolean, RecipeCastTarget::Boolean)
        );
        if !already_target {
            strict_cast_column(column, cast.target)?;
        }
    }

    for filter in &recipe.filters {
        let name = filter.column.as_str();
        let column = recipe_column(source, name)?;
        let date_parse = recipe.date_parses.iter().find(|parse| parse.column == name);
        let native_temporal =
            matches!(column.dtype(), DataType::Date | DataType::Datetime(_, None));
        if !recipe_filter_is_ordered(filter.operator)
            && !(recipe_filter_uses_temporal_literal(filter.operator) && date_parse.is_some())
            && !native_temporal
        {
            continue;
        }
        let values = strict_column_text(column)?;
        for (row, value) in values.iter().enumerate() {
            if let Some(value) = value {
                if let Some(parse) = date_parse {
                    parse_recipe_datetime(value, parse.format).map_err(|_| {
                        format!(
                            "La fila {} de la columna '{}' contiene una fecha no válida.",
                            row + 1,
                            name
                        )
                    })?;
                } else if matches!(column.dtype(), DataType::Date | DataType::Datetime(_, None)) {
                    parse_recipe_filter_datetime(value, name).map_err(|_| {
                        format!(
                            "La fila {} de la columna '{}' contiene una fecha no válida.",
                            row + 1,
                            name
                        )
                    })?;
                } else if recipe_filter_is_ordered(filter.operator) {
                    strict_f64(
                        value,
                        &format!("La fila {} de la columna '{name}'", row + 1),
                    )?;
                }
            }
        }
    }

    if let Some(calculation) = &recipe.calculated_column {
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
        if unary {
            let source_column = recipe_column(source, &calculation.source)?;
            let parsed_as_date = recipe.date_parses.iter().any(|parse| {
                parse.column == calculation.source
                    && matches!(
                        parse.target,
                        RecipeDateTarget::Date | RecipeDateTarget::Datetime
                    )
            });
            if !parsed_as_date {
                validate_date_parts_column(source_column)?;
            }
        }
        if matches!(
            calculation.operation,
            CalculatedOperation::Add
                | CalculatedOperation::Subtract
                | CalculatedOperation::Multiply
                | CalculatedOperation::Divide
        ) {
            let source_name = calculation.source.as_str();
            let source_values = strict_column_text(recipe_column(source, source_name)?)?;
            let source_numbers = source_values
                .iter()
                .enumerate()
                .map(|(row, value)| {
                    value
                        .as_deref()
                        .map(|value| {
                            strict_f64(value, &format!("La fila {} de '{source_name}'", row + 1))
                        })
                        .transpose()
                })
                .collect::<Result<Vec<_>, String>>()?;
            let operand_numbers = match calculation.operand.as_ref().unwrap() {
                CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value,
                } => {
                    if value.is_empty() {
                        return Err("El operando numérico no puede estar vacío.".into());
                    }
                    vec![Some(strict_f64(value, "El operando numérico")?); source.height()]
                }
                CalculatedOperand {
                    kind: CalculatedOperandKind::Column,
                    value,
                } => strict_column_text(recipe_column(source, value)?)?
                    .iter()
                    .enumerate()
                    .map(|(row, value)| {
                        value
                            .as_deref()
                            .map(|value| {
                                strict_f64(value, &format!("El operando de la fila {}", row + 1))
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            };
            for (row, (left, right)) in source_numbers.iter().zip(&operand_numbers).enumerate() {
                let (Some(left), Some(right)) = (left, right) else {
                    continue;
                };
                if calculation.operation == CalculatedOperation::Divide && *right == 0.0 {
                    return Err(format!("División por cero en la fila {}.", row + 1));
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
            }
        }
    }
    Ok(())
}

fn lazy_filter_expression(
    column_dtype: &DataType,
    filter: &RecipeFilter,
    effective_name: &str,
) -> Result<Expr, String> {
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

    let value = col(effective_name);
    Ok(match filter.operator {
        RecipeFilterOperator::IsNull => value.is_null(),
        RecipeFilterOperator::NotNull => value.is_not_null(),
        RecipeFilterOperator::Eq | RecipeFilterOperator::Neq
            if matches!(column_dtype, DataType::Date | DataType::Datetime(_, None)) =>
        {
            let datetime = parse_recipe_filter_datetime(literal, effective_name)?;
            let (value, literal) = match column_dtype {
                DataType::Date => {
                    let epoch =
                        NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
                    let days =
                        i32::try_from((datetime.date() - epoch).num_days()).map_err(|_| {
                            "La fecha del filtro está fuera del rango admitido.".to_owned()
                        })?;
                    (value.cast(DataType::Int32), lit(days))
                }
                DataType::Datetime(unit, None) => (
                    value.cast(DataType::Int64),
                    lit(datetime_timestamp_in_unit(datetime, *unit)?),
                ),
                _ => unreachable!("el match exterior limita esta rama a fechas"),
            };
            if filter.operator == RecipeFilterOperator::Eq {
                value.eq(literal)
            } else {
                value.neq(literal)
            }
        }
        RecipeFilterOperator::Eq => value.cast(DataType::String).eq(lit(literal.to_owned())),
        RecipeFilterOperator::Neq => value.cast(DataType::String).neq(lit(literal.to_owned())),
        RecipeFilterOperator::Contains => value
            .cast(DataType::String)
            .str()
            .to_lowercase()
            .str()
            .contains_literal(lit(literal.to_lowercase())),
        RecipeFilterOperator::NotContains => value
            .cast(DataType::String)
            .str()
            .to_lowercase()
            .str()
            .contains_literal(lit(literal.to_lowercase()))
            .not(),
        RecipeFilterOperator::Gt
        | RecipeFilterOperator::Lt
        | RecipeFilterOperator::Gte
        | RecipeFilterOperator::Lte => {
            if matches!(column_dtype, DataType::Date | DataType::Datetime(_, None)) {
                let datetime = parse_recipe_filter_datetime(literal, effective_name)?;
                let (value, literal) = match column_dtype {
                    DataType::Date => {
                        let epoch =
                            NaiveDate::from_ymd_opt(1970, 1, 1).expect("la época Unix es válida");
                        let days =
                            i32::try_from((datetime.date() - epoch).num_days()).map_err(|_| {
                                "La fecha del filtro está fuera del rango admitido.".to_owned()
                            })?;
                        (value.cast(DataType::Int32), lit(days))
                    }
                    DataType::Datetime(unit, None) => (
                        value.cast(DataType::Int64),
                        lit(datetime_timestamp_in_unit(datetime, *unit)?),
                    ),
                    _ => unreachable!("el match exterior limita esta rama a fechas"),
                };
                match filter.operator {
                    RecipeFilterOperator::Gt => value.gt(literal),
                    RecipeFilterOperator::Lt => value.lt(literal),
                    RecipeFilterOperator::Gte => value.gt_eq(literal),
                    RecipeFilterOperator::Lte => value.lt_eq(literal),
                    _ => unreachable!("el match exterior limita esta rama a comparaciones"),
                }
            } else {
                let numeric_literal = strict_f64(literal, "El valor del filtro")?;
                let value = value.strict_cast(DataType::Float64);
                match filter.operator {
                    RecipeFilterOperator::Gt => value.gt(lit(numeric_literal)),
                    RecipeFilterOperator::Lt => value.lt(lit(numeric_literal)),
                    RecipeFilterOperator::Gte => value.gt_eq(lit(numeric_literal)),
                    RecipeFilterOperator::Lte => value.lt_eq(lit(numeric_literal)),
                    _ => {
                        unreachable!("el match exterior limita esta rama a comparaciones numéricas")
                    }
                }
            }
        }
    })
}

fn lazy_find_replace_targets(
    source: &DataFrame,
    recipe: &FindReplaceRecipe,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Vec<String>, String> {
    if recipe.find.is_empty() {
        return Err("El texto buscado no puede estar vacío.".into());
    }
    validate_find_replace_pattern(recipe)?;
    let cast_target = |effective_name: &str| {
        casts
            .iter()
            .find(|cast| remapped_name(&cast.column, renames) == effective_name)
            .map(|cast| cast.target)
    };
    let is_text_after_cast = |source_name: &str, source_dtype: &DataType| {
        cast_target(remapped_name(source_name, renames))
            .map(|target| target == RecipeCastTarget::String)
            .unwrap_or(source_dtype == &DataType::String)
    };

    match recipe.scope {
        FindReplaceScope::Column => {
            let column = recipe
                .column
                .as_deref()
                .ok_or_else(|| "La búsqueda por columna requiere una columna.".to_owned())?;
            let source_column = recipe_column(source, column)?;
            let effective_name = remapped_name(column, renames);
            if !is_text_after_cast(column, source_column.dtype()) {
                return Err(format!(
                    "La columna '{effective_name}' debe ser de texto para buscar y reemplazar."
                ));
            }
            Ok(vec![effective_name.to_owned()])
        }
        FindReplaceScope::AllTextColumns => {
            if recipe.column.is_some() {
                return Err(
                    "La búsqueda en todas las columnas no acepta una columna concreta.".into(),
                );
            }
            Ok(source
                .get_column_names()
                .iter()
                .zip(source.columns())
                .filter(|(name, column)| is_text_after_cast(name, column.dtype()))
                .map(|(name, _)| remapped_name(name, renames).to_owned())
                .collect())
        }
    }
}

fn lazy_summary_column(
    source: &DataFrame,
    name: &str,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Column, String> {
    let column = recipe_column(source, name)?;
    let effective_name = remapped_name(name, renames);
    let Some(cast) = casts
        .iter()
        .find(|cast| remapped_name(&cast.column, renames) == effective_name)
    else {
        return Ok(column.clone());
    };
    let already_target = matches!(
        (column.dtype(), cast.target),
        (DataType::String, RecipeCastTarget::String)
            | (DataType::Int64, RecipeCastTarget::Integer)
            | (DataType::Float64, RecipeCastTarget::Decimal)
            | (DataType::Boolean, RecipeCastTarget::Boolean)
    );
    if already_target {
        Ok(column.clone())
    } else {
        strict_cast_column(column, cast.target)
    }
}

fn lazy_summary_validation_column(
    source: &DataFrame,
    validation: Option<&DataFrame>,
    name: &str,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Column, String> {
    if let Some(validation) = validation {
        let effective_name = remapped_name(name, renames);
        return validation
            .column(effective_name)
            .cloned()
            .map_err(|_| format!("La columna '{effective_name}' no existe en el preflight."));
    }
    lazy_summary_column(source, name, renames, casts)
}

fn lazy_contact_targets(
    source: &DataFrame,
    treatments: &[ContactNormalization],
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Vec<LazyContactTarget>, String> {
    if treatments.len() > 16 {
        return Err("La receta admite como máximo 16 normalizaciones de contacto.".into());
    }
    let mut unique = HashSet::new();
    treatments
        .iter()
        .map(|treatment| {
            if !unique.insert(treatment.column.as_str()) {
                return Err(format!(
                    "La columna '{}' tiene más de una normalización de contacto.",
                    treatment.column
                ));
            }
            let effective_name = remapped_name(&treatment.column, renames).to_owned();
            let column = lazy_summary_column(source, &treatment.column, renames, casts)?;
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La columna '{effective_name}' debe ser de texto para normalizar contactos."
                ));
            }
            Ok((effective_name, treatment.kind))
        })
        .collect()
}

fn lazy_contact_expression(name: &str, kind: ContactKind) -> Expr {
    let value = col(name);
    match kind {
        ContactKind::Email => value.str().strip_chars(lit(NULL)).str().to_lowercase(),
        ContactKind::Phone => {
            let trimmed = value.clone().str().strip_chars(lit(NULL));
            let digits = value.str().replace_all(lit(r"[^0-9]+"), lit(""), false);
            when(trimmed.str().starts_with(lit("+")))
                .then(concat_str(vec![lit("+"), digits.clone()], "", false))
                .otherwise(digits)
        }
        ContactKind::Address => value
            .str()
            .replace_all(lit(r"(?u)\s+"), lit(" "), false)
            .str()
            .strip_chars(lit(NULL)),
    }
}

fn lazy_text_extraction_targets(
    source: &DataFrame,
    extractions: &[TextExtraction],
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
) -> Result<Vec<LazyTextExtractionTarget>, String> {
    if extractions.len() > 16 {
        return Err("La receta admite como máximo 16 extracciones de texto.".into());
    }
    let mut names = HashSet::new();
    extractions
        .iter()
        .map(|extraction| {
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
            let effective_source = remapped_name(&extraction.source, renames).to_owned();
            let source_column = lazy_summary_column(source, &extraction.source, renames, casts)?;
            if source_column.dtype() != &DataType::String {
                return Err(format!(
                    "La columna '{effective_source}' debe ser de texto para extraerse."
                ));
            }
            Ok((
                effective_source,
                extraction.kind,
                extraction.delimiter.clone(),
            ))
        })
        .collect()
}

fn lazy_text_extraction_expression(
    name: &str,
    kind: ExtractionKind,
    delimiter: Option<&str>,
) -> Result<Expr, String> {
    let pattern = match kind {
        ExtractionKind::FirstToken => r"(?u)^\s*(\S+)".to_owned(),
        ExtractionKind::LastToken => r"(?u)(\S+)\s*$".to_owned(),
        ExtractionKind::Digits => r"([0-9]+)".to_owned(),
        ExtractionKind::Letters => r"(?u)(\p{Alphabetic}+)".to_owned(),
        ExtractionKind::Before => format!(
            r"(?us)^(.*?){}",
            regex::escape(delimiter.ok_or_else(|| {
                "La extracción antes del delimitador requiere delimitador.".to_owned()
            })?)
        ),
        ExtractionKind::After => format!(
            r"(?us)^.*?{}(.*)$",
            regex::escape(delimiter.ok_or_else(|| {
                "La extracción después del delimitador requiere delimitador.".to_owned()
            })?)
        ),
    };
    Regex::new(&pattern)
        .map_err(|error| format!("No se pudo preparar la extracción de texto: {error}"))?;
    Ok(col(name).str().extract(lit(pattern), 1))
}

fn lazy_summary_group_key(
    column: &Column,
    row: usize,
    name: &str,
) -> Result<Option<String>, String> {
    match column
        .get(row)
        .map_err(|error| format!("No se pudo leer la clave de grupo '{name}': {error}"))?
    {
        AnyValue::Null => Ok(None),
        AnyValue::Float64(value) if !value.is_finite() => Err(format!(
            "La clave de grupo '{name}' contiene NaN o infinito."
        )),
        AnyValue::Float64(0.0) => Ok(Some("0".into())),
        value => Ok(Some(value.to_string())),
    }
}

fn validate_lazy_group_summary(
    source: &DataFrame,
    summary: &GroupSummaryRecipe,
    renames: &HashMap<&str, &str>,
    casts: &[RecipeCast],
    validation: Option<&DataFrame>,
) -> Result<LazySummaryPlan, String> {
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
    let group_columns = groups
        .iter()
        .zip(&summary.group_by)
        .map(|(effective_name, source_name)| {
            if !group_unique.insert(effective_name.clone()) {
                return Err(format!(
                    "La clave de grupo '{effective_name}' está duplicada."
                ));
            }
            let column =
                lazy_summary_validation_column(source, validation, source_name, renames, casts)?;
            for row in 0..column.len() {
                lazy_summary_group_key(&column, row, effective_name)?;
            }
            Ok(column)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut aggregation_unique = HashSet::new();
    let mut output_names = HashSet::new();
    let mut aggregations = Vec::with_capacity(summary.aggregations.len());
    let mut integer_sum_columns = Vec::new();
    for aggregation in &summary.aggregations {
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
        let column = lazy_summary_validation_column(
            source,
            validation,
            &aggregation.column,
            renames,
            casts,
        )?;
        match aggregation.operation {
            SummaryOperation::Sum | SummaryOperation::Mean
                if !matches!(column.dtype(), DataType::Int64 | DataType::Float64) =>
            {
                return Err(format!(
                    "La agregación {} requiere que '{name}' sea Int64 o Float64.",
                    aggregation.operation.suffix()
                ));
            }
            SummaryOperation::Min | SummaryOperation::Max
                if !matches!(
                    column.dtype(),
                    DataType::Int64
                        | DataType::Float64
                        | DataType::String
                        | DataType::Date
                        | DataType::Datetime(_, _)
                ) =>
            {
                return Err(format!(
                    "La agregación {} no admite el tipo de '{name}'.",
                    aggregation.operation.suffix()
                ));
            }
            _ => {}
        }

        if matches!(
            aggregation.operation,
            SummaryOperation::Sum
                | SummaryOperation::Mean
                | SummaryOperation::Min
                | SummaryOperation::Max
                | SummaryOperation::CountUnique
        ) {
            for row in 0..column.len() {
                match column.get(row).map_err(|error| error.to_string())? {
                    AnyValue::Float64(value) if !value.is_finite() => {
                        return Err(format!("La columna '{name}' contiene NaN o infinito."));
                    }
                    AnyValue::Int64(value)
                        if aggregation.operation == SummaryOperation::Mean
                            && value.unsigned_abs() > (1_u64 << 53) =>
                    {
                        return Err(format!("La media de '{name}' excede la precisión segura."));
                    }
                    _ => {}
                }
            }
        }
        if aggregation.operation == SummaryOperation::Sum && column.dtype() == &DataType::Int64 {
            integer_sum_columns.push((name.clone(), column.clone()));
        }
        aggregations.push((name, output, aggregation.operation));
    }

    if !integer_sum_columns.is_empty() {
        let mut sums = vec![HashMap::<Vec<Option<String>>, i64>::new(); integer_sum_columns.len()];
        let validation_height = validation.map_or(source.height(), DataFrame::height);
        for row in 0..validation_height {
            let key = group_columns
                .iter()
                .zip(&groups)
                .map(|(column, name)| lazy_summary_group_key(column, row, name))
                .collect::<Result<Vec<_>, String>>()?;
            for (index, (name, column)) in integer_sum_columns.iter().enumerate() {
                let Some(value) = (match column
                    .get(row)
                    .map_err(|error| format!("No se pudo leer '{name}': {error}"))?
                {
                    AnyValue::Null => None,
                    AnyValue::Int64(value) => Some(value),
                    _ => unreachable!("la validación de tipo garantiza Int64"),
                }) else {
                    continue;
                };
                let total = sums[index].entry(key.clone()).or_insert(0);
                *total = total
                    .checked_add(value)
                    .ok_or_else(|| format!("La suma de '{name}' desbordó Int64."))?;
            }
        }
    }

    Ok((groups, aggregations))
}

pub(super) fn apply_lazy_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    if recipe.filters.len() > 3 {
        return Err("La receta admite como máximo tres filtros combinados con AND.".into());
    }
    validate_lazy_recipe_inputs(source, recipe)?;

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
    for filter in &recipe.filters {
        recipe_column(source, &filter.column)?;
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
        if calculation.name.trim().is_empty() || calculation.name != calculation.name.trim() {
            return Err(
                "El nombre calculado no puede estar vacío ni tener espacios exteriores.".into(),
            );
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

    let mut plan = source.clone().lazy();
    if renamed_count > 0 {
        let old_names = source
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>();
        let new_names = final_names.iter().map(String::as_str).collect::<Vec<_>>();
        plan = plan.rename(old_names, new_names, true);
    }

    let mut cast_columns = HashSet::new();
    let mut cast_expressions = Vec::new();
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
        let column = recipe_column(source, &cast.column)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (DataType::String, RecipeCastTarget::String)
                | (DataType::Int64, RecipeCastTarget::Integer)
                | (DataType::Float64, RecipeCastTarget::Decimal)
                | (DataType::Boolean, RecipeCastTarget::Boolean)
        );
        if !already_target {
            let target = match cast.target {
                RecipeCastTarget::String => DataType::String,
                RecipeCastTarget::Integer => DataType::Int64,
                RecipeCastTarget::Decimal => DataType::Float64,
                RecipeCastTarget::Boolean => DataType::Boolean,
            };
            cast_expressions.push(
                col(effective_name)
                    .strict_cast(target)
                    .alias(effective_name),
            );
            cast_count += 1;
        }
    }
    if !cast_expressions.is_empty() {
        plan = plan.with_columns(cast_expressions);
    }

    let mut date_columns = HashSet::new();
    let mut date_expressions = Vec::new();
    let mut parsed_date_column_count = 0;
    for parse in &recipe.date_parses {
        let effective_name = remapped_name(&parse.column, &rename_map);
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
        let column = recipe_column(source, &parse.column)?;
        let already_target = matches!(
            (column.dtype(), parse.target),
            (DataType::Date, RecipeDateTarget::Date)
                | (DataType::Datetime(_, _), RecipeDateTarget::Datetime)
        );
        if already_target {
            continue;
        }
        if column.dtype() != &DataType::String {
            return Err(format!(
                "La columna '{}' debe ser de texto para parsearse como fecha.",
                parse.column
            ));
        }
        let expression = if parse.format == RecipeDateFormat::Iso8601 {
            let value = col(effective_name)
                .str()
                .strip_chars(lit(NULL))
                .str()
                .replace_all(lit("Z"), lit(""), true);
            let datetime_target = DataType::Datetime(TimeUnit::Milliseconds, None);
            let parsed = [
                "%Y-%m-%d",
                "%Y-%m-%dT%H:%M:%S%.f",
                "%Y-%m-%dT%H:%M:%S",
                "%Y-%m-%d %H:%M:%S%.f",
                "%Y-%m-%d %H:%M:%S",
            ]
            .into_iter()
            .map(|format| {
                value.clone().str().strptime(
                    datetime_target.clone(),
                    StrptimeOptions {
                        format: Some(format.into()),
                        strict: false,
                        ..Default::default()
                    },
                    lit("raise"),
                )
            })
            .collect::<Vec<_>>();
            let parsed = coalesce(&parsed);
            match parse.target {
                RecipeDateTarget::Date => parsed.cast(DataType::Date),
                RecipeDateTarget::Datetime => parsed,
            }
            .alias(effective_name)
        } else {
            let format = match parse.format {
                RecipeDateFormat::Ymd => "%Y-%m-%d",
                RecipeDateFormat::Dmy => "%d/%m/%Y",
                RecipeDateFormat::Mdy => "%m/%d/%Y",
                RecipeDateFormat::Iso8601 => unreachable!(),
            };
            let target = match parse.target {
                RecipeDateTarget::Date => DataType::Date,
                RecipeDateTarget::Datetime => DataType::Datetime(TimeUnit::Milliseconds, None),
            };
            col(effective_name)
                .str()
                .strip_chars(lit(NULL))
                .str()
                .strptime(
                    target,
                    StrptimeOptions {
                        format: Some(format.into()),
                        ..Default::default()
                    },
                    lit("raise"),
                )
                .alias(effective_name)
        };
        date_expressions.push(expression);
        parsed_date_column_count += 1;
    }
    if !date_expressions.is_empty() {
        plan = plan.with_columns(date_expressions);
    }

    for filter in &recipe.filters {
        let effective_name = remapped_name(&filter.column, &rename_map);
        let mut filter_dtype = recipe_column(source, &filter.column)?.dtype().clone();
        if let Some(cast) = recipe
            .casts
            .iter()
            .find(|cast| remapped_name(&cast.column, &rename_map) == effective_name)
        {
            filter_dtype = match cast.target {
                RecipeCastTarget::String => DataType::String,
                RecipeCastTarget::Integer => DataType::Int64,
                RecipeCastTarget::Decimal => DataType::Float64,
                RecipeCastTarget::Boolean => DataType::Boolean,
            };
        }
        if let Some(parse) = recipe
            .date_parses
            .iter()
            .find(|parse| remapped_name(&parse.column, &rename_map) == effective_name)
        {
            filter_dtype = match parse.target {
                RecipeDateTarget::Date => DataType::Date,
                RecipeDateTarget::Datetime => DataType::Datetime(TimeUnit::Milliseconds, None),
            };
        }
        plan = plan.filter(lazy_filter_expression(
            &filter_dtype,
            filter,
            effective_name,
        )?);
    }

    let replaced_cell_count = if let Some(find_replace) = &recipe.find_replace {
        let targets = lazy_find_replace_targets(source, find_replace, &rename_map, &recipe.casts)?;
        if targets.is_empty() {
            0
        } else {
            let count_aliases = targets
                .iter()
                .enumerate()
                .map(|(index, _)| format!("__columnia_replaced_{index}"))
                .collect::<Vec<_>>();
            let count_expressions = targets
                .iter()
                .zip(&count_aliases)
                .map(|(name, alias)| {
                    let original = col(name);
                    let replaced = original.clone().str().replace_all(
                        lit(find_replace.find.clone()),
                        lit(find_replace.replace.clone()),
                        !find_replace.regex,
                    );
                    replaced
                        .neq_missing(original)
                        .cast(DataType::UInt64)
                        .sum()
                        .alias(alias)
                })
                .collect::<Vec<_>>();
            let count_frame = collect_lazy_frame_streaming(
                plan.clone().select(count_expressions),
                "No se pudo contar el reemplazo de texto",
            )?;
            let count = count_aliases
                .iter()
                .map(|alias| {
                    let value = count_frame
                        .column(alias)
                        .map_err(|error| {
                            format!("No se pudo leer el conteo de reemplazos: {error}")
                        })?
                        .get(0)
                        .map_err(|error| {
                            format!("No se pudo leer el conteo de reemplazos: {error}")
                        })?;
                    match value {
                        AnyValue::UInt64(value) => usize::try_from(value).map_err(|_| {
                            "El conteo de reemplazos excede el límite de memoria.".to_owned()
                        }),
                        AnyValue::Int64(value) if value >= 0 => usize::try_from(value as u64)
                            .map_err(|_| {
                                "El conteo de reemplazos excede el límite de memoria.".to_owned()
                            }),
                        AnyValue::Null => Ok(0),
                        _ => Err("El conteo de reemplazos devolvió un tipo inválido.".to_owned()),
                    }
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .sum();
            let replacement_expressions = targets
                .iter()
                .map(|name| {
                    col(name)
                        .str()
                        .replace_all(
                            lit(find_replace.find.clone()),
                            lit(find_replace.replace.clone()),
                            !find_replace.regex,
                        )
                        .alias(name)
                })
                .collect::<Vec<_>>();
            plan = plan.with_columns(replacement_expressions);
            count
        }
    } else {
        0
    };

    let renamed_source = if recipe.keep_columns.is_some() {
        let mut frame = source.clone();
        frame
            .set_column_names(&final_names)
            .map_err(|error| format!("No se pudieron validar las columnas conservadas: {error}"))?;
        Some(frame)
    } else {
        None
    };
    let keep_frame = renamed_source.as_ref().unwrap_or(source);
    let (keep_names, dropped_column_count, kept_order_changed) =
        resolve_keep_column_names(keep_frame, recipe.keep_columns.as_deref(), &rename_map)?;
    if recipe.keep_columns.is_some() {
        plan = plan.select(keep_names.iter().map(col).collect::<Vec<_>>());
    }

    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        let source_name = remapped_name(&calculation.source, &rename_map);
        if recipe.keep_columns.is_some() && !keep_names.iter().any(|name| name == source_name) {
            return Err(format!(
                "La columna fuente calculada '{source_name}' fue descartada por keepColumns."
            ));
        }
        let source_column = recipe_column(source, &calculation.source)?;
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
        let expression = match calculation.operation {
            CalculatedOperation::Add
            | CalculatedOperation::Subtract
            | CalculatedOperation::Multiply
            | CalculatedOperation::Divide => {
                if source_column.dtype() == &DataType::Boolean {
                    return Err(format!(
                        "La columna fuente calculada '{}' debe ser numérica.",
                        calculation.source
                    ));
                }
                let left = col(source_name).strict_cast(DataType::Float64);
                let operand = calculation.operand.as_ref().unwrap();
                let right = match operand.kind {
                    CalculatedOperandKind::Literal => {
                        lit(strict_f64(&operand.value, "El operando numérico")?)
                    }
                    CalculatedOperandKind::Column => {
                        let operand_name = remapped_name(&operand.value, &rename_map);
                        recipe_column(source, &operand.value)?;
                        col(operand_name).strict_cast(DataType::Float64)
                    }
                };
                match calculation.operation {
                    CalculatedOperation::Add => left + right,
                    CalculatedOperation::Subtract => left - right,
                    CalculatedOperation::Multiply => left * right,
                    CalculatedOperation::Divide => left / right,
                    _ => unreachable!(),
                }
            }
            CalculatedOperation::Concat => {
                let operand = calculation.operand.as_ref().unwrap();
                let right = match operand.kind {
                    CalculatedOperandKind::Literal => lit(operand.value.clone()),
                    CalculatedOperandKind::Column => {
                        let operand_name = remapped_name(&operand.value, &rename_map);
                        recipe_column(source, &operand.value)?;
                        col(operand_name).cast(DataType::String)
                    }
                };
                concat_str(
                    vec![col(source_name).cast(DataType::String), right],
                    "",
                    false,
                )
            }
            CalculatedOperation::Year => col(source_name).dt().year(),
            CalculatedOperation::Month => col(source_name).dt().month(),
            CalculatedOperation::Day => col(source_name).dt().day(),
        };
        if recipe_column(source, &calculation.name).is_ok() {
            return Err(format!(
                "La columna calculada '{}' ya existe.",
                calculation.name
            ));
        }
        plan = plan.with_columns(vec![expression.alias(calculation.name.clone())]);
        1
    } else {
        0
    };

    let (split_column_count, split_dropped_source_count) = if let Some(split) = &recipe.split_column
    {
        if split.delimiter.is_empty() {
            return Err("El delimitador de división no puede estar vacío.".into());
        }
        if !(2..=16).contains(&split.names.len()) {
            return Err("La división requiere entre 2 y 16 columnas de destino.".into());
        }
        let source_name = remapped_name(&split.source, &rename_map).to_owned();
        let mut output_names = if recipe.keep_columns.is_some() {
            keep_names.clone()
        } else {
            final_names.clone()
        };
        if let Some(calculation) = &recipe.calculated_column {
            output_names.push(calculation.name.clone());
        }
        if !output_names.iter().any(|name| name == &source_name) {
            return Err(format!(
                "La columna '{source_name}' requerida por split fue descartada por keepColumns."
            ));
        }
        let source_column = recipe_column(source, &split.source)?;
        let cast_target = recipe
            .casts
            .iter()
            .find(|cast| remapped_name(&cast.column, &rename_map) == source_name)
            .map(|cast| cast.target);
        let is_text = cast_target
            .map(|target| target == RecipeCastTarget::String)
            .unwrap_or(source_column.dtype() == &DataType::String);
        if !is_text {
            return Err(format!(
                "La columna '{source_name}' debe ser de texto para dividirse."
            ));
        }
        let mut unique = HashSet::new();
        for name in &split.names {
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
            if output_names.iter().any(|existing| existing == trimmed) {
                return Err(format!("La columna dividida '{trimmed}' ya existe."));
            }
        }
        let split_expression = col(source_name.as_str())
            .str()
            .splitn(lit(split.delimiter.clone()), split.names.len());
        let split_expressions = split
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                split_expression
                    .clone()
                    .struct_()
                    .field_by_name(&format!("field_{index}"))
                    .alias(name.clone())
            })
            .collect::<Vec<_>>();
        plan = plan.with_columns(split_expressions);
        output_names.extend(split.names.iter().cloned());
        let split_dropped_source_count = if split.drop_source {
            plan = plan.select(
                output_names
                    .iter()
                    .filter(|name| name.as_str() != source_name)
                    .map(col)
                    .collect::<Vec<_>>(),
            );
            1
        } else {
            0
        };
        (split.names.len(), split_dropped_source_count)
    } else {
        (0, 0)
    };

    let (merged_column_count, dropped_source_column_count) = if let Some(merge) =
        &recipe.merge_columns
    {
        if !(2..=16).contains(&merge.sources.len()) {
            return Err("La unión requiere entre 2 y 16 columnas fuente.".into());
        }
        if merge.name.trim().is_empty() || merge.name != merge.name.trim() {
            return Err(
                "El nombre de la columna unida no puede estar vacío ni tener espacios exteriores."
                    .into(),
            );
        }
        let mut output_names = if recipe.keep_columns.is_some() {
            keep_names.clone()
        } else {
            final_names.clone()
        };
        if let Some(calculation) = &recipe.calculated_column {
            output_names.push(calculation.name.clone());
        }
        if let Some(split) = &recipe.split_column {
            let split_source = remapped_name(&split.source, &rename_map);
            if split.drop_source && merge.sources.contains(&split.source) {
                return Err(format!(
                    "La unión necesita '{split_source}', pero la división la descartaría."
                ));
            }
            if split.drop_source {
                output_names.retain(|name| name != split_source);
            }
            output_names.extend(split.names.iter().cloned());
        }
        if output_names.iter().any(|name| name == &merge.name) {
            return Err(format!("La columna unida '{}' ya existe.", merge.name));
        }
        let mut unique = HashSet::new();
        let sources = merge
                .sources
                .iter()
                .map(|source_name| {
                    if !unique.insert(source_name) {
                        return Err(format!(
                            "La columna fuente '{source_name}' está duplicada."
                        ));
                    }
                    let effective_name = remapped_name(source_name, &rename_map).to_owned();
                    if !output_names.iter().any(|name| name == &effective_name) {
                        return Err(format!(
                            "La columna '{effective_name}' requerida por merge fue descartada por keepColumns."
                        ));
                    }
                    let source_column = recipe_column(source, source_name)?;
                    let cast_target = recipe
                        .casts
                        .iter()
                        .find(|cast| {
                            remapped_name(&cast.column, &rename_map) == effective_name
                        })
                        .map(|cast| cast.target);
                    let is_text = cast_target
                        .map(|target| target == RecipeCastTarget::String)
                        .unwrap_or(source_column.dtype() == &DataType::String);
                    if !is_text {
                        return Err(format!(
                            "La columna '{effective_name}' debe ser de texto para unirse."
                        ));
                    }
                    Ok(effective_name)
                })
                .collect::<Result<Vec<_>, String>>()?;
        let source_expressions = sources.iter().map(col).collect::<Vec<_>>();
        let expression = when(coalesce(&source_expressions).is_not_null())
            .then(concat_str(source_expressions, &merge.separator, true))
            .otherwise(lit(NULL))
            .alias(merge.name.clone());
        plan = plan.with_columns(vec![expression]);
        output_names.push(merge.name.clone());
        let dropped_source_column_count = if merge.drop_sources {
            let dropped = sources.iter().collect::<HashSet<_>>();
            plan = plan.select(
                output_names
                    .iter()
                    .filter(|name| !dropped.contains(name))
                    .map(col)
                    .collect::<Vec<_>>(),
            );
            sources.len()
        } else {
            0
        };
        (1, dropped_source_column_count)
    } else {
        (0, 0)
    };

    let (adjusted_outlier_cell_count, outlier_removed_row_count, outlier_column_count) = if recipe
        .outlier_treatments
        .is_empty()
    {
        (0, 0, 0)
    } else {
        let filtered_outlier_values = if recipe.filters.is_empty() {
            None
        } else {
            let outlier_columns = recipe
                .outlier_treatments
                .iter()
                .map(|treatment| remapped_name(&treatment.column, &rename_map).to_owned())
                .collect::<Vec<_>>();
            Some(collect_lazy_frame_streaming(
                plan.clone()
                    .select(outlier_columns.iter().map(col).collect::<Vec<_>>()),
                "No se pudieron preparar los valores de atípicos después de los filtros",
            )?)
        };
        let outlier_source = filtered_outlier_values.as_ref().unwrap_or(source);
        let prepared =
            prepare_outlier_treatments(outlier_source, &recipe.outlier_treatments, &rename_map)?;
        let mut drop_mask = vec![false; outlier_source.height()];
        let mut drop_expression: Option<Expr> = None;
        let mut replacement_expressions = Vec::new();
        let mut adjusted = 0;

        for (name, action, values, lower, upper, median, dtype) in prepared {
            let outlier_rows = values
                .iter()
                .enumerate()
                .filter_map(|(row, value)| {
                    value.and_then(|value| (value < lower || value > upper).then_some(row))
                })
                .collect::<Vec<_>>();
            let value = col(name.as_str());
            let outlier = value.clone().is_not_null().and(
                value
                    .clone()
                    .lt(lit(lower))
                    .or(value.clone().gt(lit(upper))),
            );
            match action {
                OutlierAction::Cap => {
                    adjusted += outlier_rows.len();
                    if !outlier_rows.is_empty() {
                        replacement_expressions.push(
                            when(value.clone().lt(lit(lower)))
                                .then(lit(lower))
                                .when(value.clone().gt(lit(upper)))
                                .then(lit(upper))
                                .otherwise(value)
                                .alias(name),
                        );
                    }
                }
                OutlierAction::Drop => {
                    for row in outlier_rows {
                        drop_mask[row] = true;
                    }
                    drop_expression = Some(match drop_expression {
                        Some(existing) => existing.or(outlier),
                        None => outlier,
                    });
                }
                OutlierAction::Impute => {
                    adjusted += outlier_rows.len();
                    replacement_expressions.push(
                        when(outlier)
                            .then(lit(median))
                            .otherwise(value)
                            .cast(dtype)
                            .alias(name),
                    );
                }
            }
        }
        if !replacement_expressions.is_empty() {
            plan = plan.with_columns(replacement_expressions);
        }
        if let Some(drop_expression) = drop_expression {
            plan = plan.filter(drop_expression.not());
        }
        (
            adjusted,
            drop_mask.iter().filter(|drop| **drop).count(),
            recipe.outlier_treatments.len(),
        )
    };

    let (normalized_contact_cell_count, normalized_contact_column_count) = if recipe
        .contact_normalizations
        .is_empty()
    {
        (0, 0)
    } else {
        let schema = plan.collect_schema().map_err(|error| {
            format!("No se pudo validar el esquema de normalización de contactos: {error}")
        })?;
        let targets = lazy_contact_targets(
            source,
            &recipe.contact_normalizations,
            &rename_map,
            &recipe.casts,
        )?;
        for (name, _) in &targets {
            if schema.get(name).is_none() {
                return Err(format!(
                    "La columna '{name}' para contactos no sobrevivió las etapas estructurales."
                ));
            }
        }
        let count_aliases = targets
            .iter()
            .enumerate()
            .map(|(index, _)| format!("__columnia_contact_changed_{index}"))
            .collect::<Vec<_>>();
        let count_expressions = targets
            .iter()
            .zip(&count_aliases)
            .map(|((name, kind), alias)| {
                lazy_contact_expression(name, *kind)
                    .neq_missing(col(name))
                    .cast(DataType::UInt64)
                    .sum()
                    .alias(alias)
            })
            .collect::<Vec<_>>();
        let count_frame = collect_lazy_frame_streaming(
            plan.clone().select(count_expressions),
            "No se pudo contar la normalización de contactos",
        )?;
        let changed_cells = count_aliases
            .iter()
            .map(|alias| {
                let value = count_frame
                    .column(alias)
                    .map_err(|error| format!("No se pudo leer el conteo de contactos: {error}"))?
                    .get(0)
                    .map_err(|error| format!("No se pudo leer el conteo de contactos: {error}"))?;
                match value {
                    AnyValue::UInt64(value) => usize::try_from(value).map_err(|_| {
                        "El conteo de normalizaciones excede el límite de memoria.".to_owned()
                    }),
                    AnyValue::Int64(value) if value >= 0 => {
                        usize::try_from(value as u64).map_err(|_| {
                            "El conteo de normalizaciones excede el límite de memoria.".to_owned()
                        })
                    }
                    AnyValue::Null => Ok(0),
                    _ => Err("El conteo de contactos devolvió un tipo inválido.".to_owned()),
                }
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .sum();
        let expressions = targets
            .iter()
            .map(|(name, kind)| lazy_contact_expression(name, *kind).alias(name))
            .collect::<Vec<_>>();
        plan = plan.with_columns(expressions);
        (changed_cells, targets.len())
    };

    let extracted_column_count = if recipe.text_extractions.is_empty() {
        0
    } else {
        let schema = plan.collect_schema().map_err(|error| {
            format!("No se pudo validar el esquema de extracción de texto: {error}")
        })?;
        let targets = lazy_text_extraction_targets(
            source,
            &recipe.text_extractions,
            &rename_map,
            &recipe.casts,
        )?;
        for ((source_name, _, _), extraction) in targets.iter().zip(&recipe.text_extractions) {
            if schema.get(source_name).is_none() {
                return Err(format!(
                    "La columna '{source_name}' para extracción no sobrevivió las etapas estructurales."
                ));
            }
            if schema.get(&extraction.name).is_some() {
                return Err(format!(
                    "La columna extraída '{}' ya existe.",
                    extraction.name
                ));
            }
        }
        let expressions = targets
            .iter()
            .zip(&recipe.text_extractions)
            .map(|((source_name, kind, delimiter), extraction)| {
                lazy_text_extraction_expression(source_name, *kind, delimiter.as_deref())
                    .map(|expression| expression.alias(&extraction.name))
            })
            .collect::<Result<Vec<_>, String>>()?;
        plan = plan.with_columns(expressions);
        targets.len()
    };

    let (group_summary_input_rows, summary_aggregations) = if let Some(summary) =
        &recipe.group_summary
    {
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
        for name in summary.group_by.iter().chain(
            summary
                .aggregations
                .iter()
                .map(|aggregation| &aggregation.column),
        ) {
            if !derived_names.contains(name.as_str()) {
                recipe_column(source, name)?;
            }
        }
        let schema = plan
            .collect_schema()
            .map_err(|error| format!("No se pudo validar el esquema del resumen: {error}"))?;
        let summary_names = summary
            .group_by
            .iter()
            .chain(
                summary
                    .aggregations
                    .iter()
                    .map(|aggregation| &aggregation.column),
            )
            .map(|name| remapped_name(name, &rename_map).to_owned())
            .collect::<HashSet<_>>();
        for name in &summary_names {
            if schema.get(name).is_none() {
                return Err(format!(
                    "La columna '{name}' requerida por agrupar/resumir no sobrevivió las etapas anteriores."
                ));
            }
        }
        let validation = if recipe.filters.is_empty()
            && recipe.contact_normalizations.is_empty()
            && recipe.text_extractions.is_empty()
            && recipe.calculated_column.is_none()
            && recipe.split_column.is_none()
            && recipe.merge_columns.is_none()
        {
            None
        } else {
            Some(collect_lazy_frame_streaming(
                plan.clone()
                    .select(summary_names.iter().map(col).collect::<Vec<_>>()),
                "No se pudo validar el resumen después de las etapas previas",
            )?)
        };
        let (groups, aggregations) = validate_lazy_group_summary(
            source,
            summary,
            &rename_map,
            &recipe.casts,
            validation.as_ref(),
        )?;
        let group_expressions = groups.iter().map(col).collect::<Vec<_>>();
        let aggregate_expressions = aggregations
            .iter()
            .map(|(name, output, operation)| {
                let expression = match operation {
                    SummaryOperation::Sum => col(name).sum(),
                    SummaryOperation::Mean => col(name).mean(),
                    SummaryOperation::Min => col(name).min(),
                    SummaryOperation::Max => col(name).max(),
                    SummaryOperation::Count => len().cast(DataType::Int64),
                    SummaryOperation::CountUnique => {
                        col(name).drop_nulls().n_unique().cast(DataType::Int64)
                    }
                };
                expression.alias(output)
            })
            .collect::<Vec<_>>();
        plan = plan
            .group_by_stable(group_expressions)
            .agg(aggregate_expressions);
        (
            Some(
                validation
                    .as_ref()
                    .map_or(source.height(), DataFrame::height),
            ),
            Some(aggregations),
        )
    } else {
        (None, None)
    };

    let candidate = collect_lazy_frame_streaming(plan, "No se pudo ejecutar la receta lazy")?;
    if let Some(aggregations) = &summary_aggregations {
        for (name, output, operation) in aggregations {
            if !matches!(
                operation,
                SummaryOperation::Sum
                    | SummaryOperation::Mean
                    | SummaryOperation::Min
                    | SummaryOperation::Max
            ) {
                continue;
            }
            let column = candidate
                .column(output)
                .map_err(|error| format!("No se pudo leer '{output}': {error}"))?;
            if column.dtype() == &DataType::Float64
                && (0..column.len()).any(|row| {
                    matches!(
                        column.get(row),
                        Ok(AnyValue::Float64(value)) if !value.is_finite()
                    )
                })
            {
                return Err(format!(
                    "La agregación de '{name}' produjo un valor no finito."
                ));
            }
        }
    }
    let removed_row_count = group_summary_input_rows
        .map(|input_rows| source.height().saturating_sub(input_rows))
        .unwrap_or_else(|| source.height().saturating_sub(candidate.height()));
    let group_count = summary_aggregations
        .as_ref()
        .map_or(0, |_| candidate.height());
    let collapsed_row_count =
        group_summary_input_rows.map_or(0, |input_rows| input_rows.saturating_sub(group_count));
    Ok((
        candidate,
        renamed_count,
        cast_count,
        parsed_date_column_count,
        removed_row_count,
        calculated_column_count,
        replaced_cell_count,
        dropped_column_count,
        kept_order_changed,
        split_column_count,
        merged_column_count,
        split_dropped_source_count + dropped_source_column_count,
        adjusted_outlier_cell_count,
        outlier_removed_row_count,
        outlier_column_count,
        group_count,
        summary_aggregations.as_ref().map_or(0, Vec::len),
        collapsed_row_count,
        normalized_contact_cell_count,
        normalized_contact_column_count,
        extracted_column_count,
    ))
}

pub(super) fn apply_recipe_to_frame(
    source: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<RecipeFrameOutcome, String> {
    if lazy_recipe_supported(source, recipe) {
        return apply_lazy_recipe_to_frame(source, recipe);
    }
    apply_eager_recipe_to_frame(source, recipe)
}
