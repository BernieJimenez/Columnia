use super::*;

pub(super) fn source_backed_projection_recipe_supported(
    schema: &DataFrame,
    recipe: &TransformRecipe,
) -> bool {
    (!recipe.renames.is_empty()
        || recipe.keep_columns.is_some()
        || !recipe.filters.is_empty()
        || !recipe.casts.is_empty()
        || !recipe.date_parses.is_empty()
        || recipe.calculated_column.is_some()
        || recipe.find_replace.is_some()
        || recipe.split_column.is_some()
        || recipe.merge_columns.is_some()
        || !recipe.contact_normalizations.is_empty()
        || !recipe.text_extractions.is_empty()
        || recipe.group_summary.is_some()
        || !recipe.outlier_treatments.is_empty())
        && recipe.date_parses.iter().all(|parse| {
            matches!(
                parse.format,
                RecipeDateFormat::Ymd
                    | RecipeDateFormat::Dmy
                    | RecipeDateFormat::Mdy
                    | RecipeDateFormat::Iso8601
            )
        })
        && recipe
            .calculated_column
            .as_ref()
            .is_none_or(|calculation| match calculation.operation {
                CalculatedOperation::Add
                | CalculatedOperation::Subtract
                | CalculatedOperation::Multiply
                | CalculatedOperation::Concat => true,
                CalculatedOperation::Year
                | CalculatedOperation::Month
                | CalculatedOperation::Day => {
                    recipe
                        .casts
                        .iter()
                        .all(|cast| cast.column != calculation.source)
                        && (recipe_column(schema, &calculation.source).is_ok_and(|column| {
                            matches!(column.dtype(), DataType::Date | DataType::Datetime(_, None))
                        }) || recipe.date_parses.iter().any(|parse| {
                            parse.column == calculation.source
                                && matches!(
                                    parse.target,
                                    RecipeDateTarget::Date | RecipeDateTarget::Datetime
                                )
                        }))
                }
                CalculatedOperation::Divide => calculation.operand.is_some(),
            })
        && recipe.find_replace.as_ref().is_none_or(|replacement| {
            !replacement.find.contains('\0')
                && !replacement.replace.contains('\0')
                && (!replacement.regex
                    || (Regex::new(&replacement.find).is_ok()
                        && source_backed_regex_replacement(&replacement.replace).is_ok()))
        })
        && recipe
            .merge_columns
            .as_ref()
            .is_none_or(|merge| !merge.name.contains('\0') && !merge.separator.contains('\0'))
        && recipe.split_column.as_ref().is_none_or(|split| {
            !split.delimiter.contains('\0') && split.names.iter().all(|name| !name.contains('\0'))
        })
        && recipe.text_extractions.iter().all(|extraction| {
            !extraction.name.contains('\0')
                && extraction.source.chars().all(|character| character != '\0')
                && extraction
                    .delimiter
                    .as_deref()
                    .is_none_or(|delimiter| !delimiter.contains('\0'))
        })
        && recipe
            .contact_normalizations
            .iter()
            .all(|normalization| !normalization.column.contains('\0'))
}

struct SourceBackedProjectionPlan {
    source_columns: Vec<String>,
    output_columns: Vec<String>,
    selected_columns: Vec<String>,
    renamed_column_count: usize,
    converted_column_count: usize,
    parsed_date_column_count: usize,
    calculated_column_count: usize,
    replacement_columns: Vec<String>,
    split_columns: Option<SourceBackedSplitPlan>,
    merge_columns: Option<SourceBackedMergePlan>,
    contact_normalizations: Vec<SourceBackedContactNormalizationPlan>,
    text_extractions: Vec<SourceBackedTextExtractionPlan>,
    outlier_treatments: Vec<SourceBackedOutlierPlan>,
    group_summary: Option<SourceBackedGroupSummaryPlan>,
    dropped_column_count: usize,
    kept_order_changed: bool,
}

struct SourceBackedSplitPlan {
    source: String,
    names: Vec<String>,
    delimiter: String,
    drop_source: bool,
}

struct SourceBackedMergePlan {
    sources: Vec<String>,
    name: String,
    separator: String,
    drop_sources: bool,
}

struct SourceBackedTextExtractionPlan {
    source: String,
    kind: ExtractionKind,
    name: String,
    delimiter: Option<String>,
}

struct SourceBackedContactNormalizationPlan {
    column: String,
    kind: ContactKind,
}

struct SourceBackedOutlierPlan {
    column: String,
    action: OutlierAction,
    dtype: DataType,
}

struct SourceBackedGroupSummaryPlan {
    group_by: Vec<String>,
    group_dtypes: Vec<DataType>,
    aggregations: Vec<SourceBackedSummaryAggregationPlan>,
}

struct SourceBackedSummaryAggregationPlan {
    column: String,
    output: String,
    operation: SummaryOperation,
    dtype: DataType,
}

fn source_backed_target_dtype(
    schema: &DataFrame,
    column: &str,
    recipe: &TransformRecipe,
    rename_map: &HashMap<String, String>,
) -> Result<DataType, String> {
    let source_column = recipe_column(schema, column)?;
    let effective = rename_map.get(column).map(String::as_str).unwrap_or(column);
    if let Some(cast) = recipe.casts.iter().find(|cast| {
        rename_map
            .get(&cast.column)
            .map(String::as_str)
            .unwrap_or(cast.column.as_str())
            == effective
    }) {
        return Ok(match cast.target {
            RecipeCastTarget::String => DataType::String,
            RecipeCastTarget::Integer => DataType::Int64,
            RecipeCastTarget::Decimal => DataType::Float64,
            RecipeCastTarget::Boolean => DataType::Boolean,
        });
    }
    if let Some(parse) = recipe.date_parses.iter().find(|parse| {
        rename_map
            .get(&parse.column)
            .map(String::as_str)
            .unwrap_or(parse.column.as_str())
            == effective
    }) {
        return Ok(match parse.target {
            RecipeDateTarget::Date => DataType::Date,
            RecipeDateTarget::Datetime => DataType::Datetime(TimeUnit::Milliseconds, None),
        });
    }
    Ok(source_column.dtype().clone())
}

fn source_backed_group_summary_plan(
    summary: &GroupSummaryRecipe,
    rename_map: &HashMap<String, String>,
    available_columns: &[String],
    available_types: &HashMap<String, DataType>,
) -> Result<SourceBackedGroupSummaryPlan, String> {
    if summary.group_by.is_empty() || summary.group_by.len() > 8 {
        return Err("Agrupar requiere entre 1 y 8 columnas clave.".into());
    }
    if summary.aggregations.is_empty() || summary.aggregations.len() > 32 {
        return Err("Resumir requiere entre 1 y 32 agregaciones.".into());
    }

    let effective_name = |name: &str| {
        rename_map
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    };
    let groups = summary
        .group_by
        .iter()
        .map(|name| effective_name(name))
        .collect::<Vec<_>>();
    let mut group_dtypes = Vec::with_capacity(groups.len());
    let mut group_unique = HashSet::new();
    for name in &groups {
        if !group_unique.insert(name.clone()) {
            return Err(format!("La clave de grupo '{name}' está duplicada."));
        }
        if !available_columns.iter().any(|column| column == name) {
            return Err(format!(
                "La columna '{name}' requerida por agrupar no sobrevivió las etapas anteriores."
            ));
        }
        if !available_types.contains_key(name) {
            return Err(format!(
                "No se pudo inferir el tipo de la clave de grupo '{name}'."
            ));
        }
        group_dtypes.push(
            available_types
                .get(name)
                .expect("la presencia del tipo fue validada")
                .clone(),
        );
    }

    let mut aggregation_unique = HashSet::new();
    let mut output_names = HashSet::new();
    let aggregations = summary
        .aggregations
        .iter()
        .map(|aggregation| {
            let name = effective_name(&aggregation.column);
            if !aggregation_unique.insert((name.clone(), aggregation.operation)) {
                return Err(format!(
                    "La agregación '{}_{}' está duplicada.",
                    name,
                    aggregation.operation.suffix()
                ));
            }
            if !available_columns.iter().any(|column| column == &name) {
                return Err(format!(
                    "La columna '{name}' requerida por resumir no sobrevivió las etapas anteriores."
                ));
            }
            let dtype = available_types.get(&name).ok_or_else(|| {
                format!("No se pudo inferir el tipo de la columna '{name}' para resumir.")
            })?;
            match aggregation.operation {
                SummaryOperation::Sum | SummaryOperation::Mean
                    if !matches!(dtype, DataType::Int64 | DataType::Float64) =>
                {
                    return Err(format!(
                        "La agregación {} requiere que '{name}' sea Int64 o Float64.",
                        aggregation.operation.suffix()
                    ));
                }
                SummaryOperation::Min | SummaryOperation::Max
                    if !matches!(
                        dtype,
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
            let output = format!("{}_{}", name, aggregation.operation.suffix());
            if group_unique.contains(&output) || !output_names.insert(output.clone()) {
                return Err(format!(
                    "El nombre de salida '{output}' colisiona con otra columna."
                ));
            }
            Ok(SourceBackedSummaryAggregationPlan {
                column: name,
                output,
                operation: aggregation.operation,
                dtype: dtype.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(SourceBackedGroupSummaryPlan {
        group_by: groups,
        group_dtypes,
        aggregations,
    })
}

fn source_backed_projection_plan(
    schema: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<SourceBackedProjectionPlan, String> {
    if recipe.filters.len() > 3 {
        return Err("La receta admite como máximo tres filtros combinados con AND.".into());
    }
    if !lazy_renames_have_no_cycles(recipe) {
        return Err("Los renombrados contienen un ciclo.".to_owned());
    }

    let mut rename_map = HashMap::new();
    let mut rename_sources = HashSet::new();
    for rename in &recipe.renames {
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
        recipe_column(schema, &rename.from)?;
        rename_map.insert(rename.from.clone(), rename.to.clone());
    }

    let source_columns = schema
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let output_columns = source_columns
        .iter()
        .map(|name| {
            rename_map
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.clone())
        })
        .collect::<Vec<_>>();
    let mut unique_output_columns = HashSet::new();
    if output_columns
        .iter()
        .any(|name| !unique_output_columns.insert(name.as_str()))
    {
        return Err("Los renombrados producirían nombres de columna duplicados.".to_owned());
    }

    let renamed_column_count = source_columns
        .iter()
        .zip(&output_columns)
        .filter(|(before, after)| before != after)
        .count();
    let selected_columns = if let Some(keep_columns) = &recipe.keep_columns {
        if keep_columns.is_empty() {
            return Err("Debes conservar al menos una columna.".into());
        }
        let mut selected = Vec::with_capacity(keep_columns.len());
        let mut seen = HashSet::new();
        for name in keep_columns {
            let effective = rename_map
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.clone());
            if !output_columns.iter().any(|column| column == &effective) {
                return Err(format!(
                    "La columna '{effective}' no existe en la selección source-backed."
                ));
            }
            if !seen.insert(effective.clone()) {
                return Err(format!(
                    "La columna '{name}' aparece más de una vez en la selección."
                ));
            }
            selected.push(effective);
        }
        selected
    } else {
        output_columns.clone()
    };

    let mut cast_columns = HashSet::new();
    let mut converted_column_count = 0;
    for cast in &recipe.casts {
        let effective = rename_map
            .get(&cast.column)
            .cloned()
            .unwrap_or_else(|| cast.column.clone());
        if !cast_columns.insert(effective) {
            return Err(format!(
                "La columna '{}' aparece en más de una conversión.",
                cast.column
            ));
        }
        let column = recipe_column(schema, &cast.column)?;
        let already_target = matches!(
            (column.dtype(), cast.target),
            (DataType::String, RecipeCastTarget::String)
                | (DataType::Int64, RecipeCastTarget::Integer)
                | (DataType::Float64, RecipeCastTarget::Decimal)
                | (DataType::Boolean, RecipeCastTarget::Boolean)
        );
        converted_column_count += usize::from(!already_target);
    }

    let mut date_columns = HashSet::new();
    let mut parsed_date_column_count = 0;
    for parse in &recipe.date_parses {
        let effective = rename_map
            .get(&parse.column)
            .cloned()
            .unwrap_or_else(|| parse.column.clone());
        if cast_columns.contains(&effective) {
            return Err(format!(
                "La columna '{}' no puede convertirse y parsearse como fecha en la misma receta.",
                parse.column
            ));
        }
        if !date_columns.insert(effective) {
            return Err(format!(
                "La columna '{}' aparece en más de un parseo de fecha.",
                parse.column
            ));
        }
        let column = recipe_column(schema, &parse.column)?;
        let already_target = matches!(
            (column.dtype(), parse.target),
            (DataType::Date, RecipeDateTarget::Date)
                | (DataType::Datetime(_, _), RecipeDateTarget::Datetime)
        );
        if !already_target {
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La columna '{}' debe ser de texto para parsearse como fecha.",
                    parse.column
                ));
            }
            parsed_date_column_count += 1;
        }
    }

    let calculated_column_count = if let Some(calculation) = &recipe.calculated_column {
        if calculation.name.trim().is_empty() || calculation.name != calculation.name.trim() {
            return Err(
                "El nombre calculado no puede estar vacío ni tener espacios exteriores.".into(),
            );
        }
        if output_columns.iter().any(|name| name == &calculation.name) {
            return Err(format!(
                "La columna calculada '{}' ya existe.",
                calculation.name
            ));
        }
        let source_column = recipe_column(schema, &calculation.source)?;
        let unary = matches!(
            calculation.operation,
            CalculatedOperation::Year | CalculatedOperation::Month | CalculatedOperation::Day
        );
        if unary {
            if calculation.operand.is_some() {
                return Err("year, month y day no aceptan operando.".to_owned());
            }
            let source_name = rename_map
                .get(&calculation.source)
                .cloned()
                .unwrap_or_else(|| calculation.source.clone());
            let source_is_date = matches!(
                source_column.dtype(),
                DataType::Date | DataType::Datetime(_, None)
            ) || date_columns.contains(&source_name);
            if !source_is_date {
                return Err(format!(
                    "La columna '{}' debe ser date o datetime para extraer componentes.",
                    calculation.source
                ));
            }
            if cast_columns.contains(&source_name) {
                return Err(format!(
                    "La columna '{}' no puede convertirse y usarse como fecha en la misma receta.",
                    calculation.source
                ));
            }
        } else if calculation.operand.is_none() {
            return Err("La operación calculada requiere un operando.".to_owned());
        }
        if matches!(
            calculation.operation,
            CalculatedOperation::Add
                | CalculatedOperation::Subtract
                | CalculatedOperation::Multiply
                | CalculatedOperation::Divide
        ) && source_column.dtype() == &DataType::Boolean
        {
            return Err(format!(
                "La columna fuente calculada '{}' debe ser numérica.",
                calculation.source
            ));
        }
        let source_name = rename_map
            .get(&calculation.source)
            .cloned()
            .unwrap_or_else(|| calculation.source.clone());
        if recipe.keep_columns.is_some()
            && !selected_columns.iter().any(|name| name == &source_name)
        {
            return Err(format!(
                "La columna fuente calculada '{source_name}' fue descartada por keepColumns."
            ));
        }
        if let Some(operand) = &calculation.operand {
            match operand.kind {
                CalculatedOperandKind::Literal
                    if matches!(
                        calculation.operation,
                        CalculatedOperation::Add
                            | CalculatedOperation::Subtract
                            | CalculatedOperation::Multiply
                            | CalculatedOperation::Divide
                    ) =>
                {
                    strict_f64(&operand.value, "El operando numérico")?;
                }
                CalculatedOperandKind::Column => {
                    let operand_name = rename_map
                        .get(&operand.value)
                        .cloned()
                        .unwrap_or_else(|| operand.value.clone());
                    recipe_column(schema, &operand.value)?;
                    if recipe.keep_columns.is_some()
                        && !selected_columns.iter().any(|name| name == &operand_name)
                    {
                        return Err(format!(
                            "La columna operando calculada '{operand_name}' fue descartada por keepColumns."
                        ));
                    }
                }
                _ => {}
            }
        } else if !unary {
            return Err("La operación calculada requiere un operando.".to_owned());
        }
        1
    } else {
        0
    };

    let text_after_cast = |source_name: &str, source_column: &Column| {
        recipe
            .casts
            .iter()
            .find(|cast| {
                rename_map
                    .get(&cast.column)
                    .map(String::as_str)
                    .unwrap_or(cast.column.as_str())
                    == source_name
            })
            .map(|cast| cast.target == RecipeCastTarget::String)
            .unwrap_or(source_column.dtype() == &DataType::String)
    };

    let replacement_columns = if let Some(replacement) = &recipe.find_replace {
        if replacement.find.is_empty() {
            return Err("El texto buscado no puede estar vacío.".to_owned());
        }
        match replacement.scope {
            FindReplaceScope::Column => {
                let column = replacement
                    .column
                    .as_deref()
                    .ok_or_else(|| "La búsqueda por columna requiere una columna.".to_owned())?;
                let source_column = recipe_column(schema, column)?;
                let effective = rename_map
                    .get(column)
                    .cloned()
                    .unwrap_or_else(|| column.to_owned());
                if !text_after_cast(&effective, source_column) {
                    return Err(format!(
                        "La columna '{effective}' debe ser de texto para buscar y reemplazar."
                    ));
                }
                vec![effective]
            }
            FindReplaceScope::AllTextColumns => {
                if replacement.column.is_some() {
                    return Err(
                        "La búsqueda en todas las columnas no acepta una columna concreta."
                            .to_owned(),
                    );
                }
                source_columns
                    .iter()
                    .zip(schema.columns())
                    .filter(|(name, column)| {
                        let effective = rename_map
                            .get(*name)
                            .map(String::as_str)
                            .unwrap_or(name.as_str());
                        text_after_cast(effective, column)
                    })
                    .map(|(name, _)| {
                        rename_map
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| name.clone())
                    })
                    .collect()
            }
        }
    } else {
        Vec::new()
    };

    let split_columns = if let Some(split) = &recipe.split_column {
        if split.delimiter.is_empty() {
            return Err("El delimitador de división no puede estar vacío.".into());
        }
        if !(2..=16).contains(&split.names.len()) {
            return Err("La división requiere entre 2 y 16 columnas de destino.".into());
        }
        let source_column = recipe_column(schema, &split.source)?;
        let source = rename_map
            .get(&split.source)
            .cloned()
            .unwrap_or_else(|| split.source.clone());
        if recipe.keep_columns.is_some() && !selected_columns.iter().any(|name| name == &source) {
            return Err(format!(
                "La columna '{source}' requerida por split fue descartada por keepColumns."
            ));
        }
        if !text_after_cast(&source, source_column) {
            return Err(format!(
                "La columna '{source}' debe ser de texto para dividirse."
            ));
        }
        let mut unique_names = HashSet::new();
        for name in &split.names {
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed != name {
                return Err(
                    "Los nombres divididos no pueden estar vacíos ni tener espacios exteriores."
                        .into(),
                );
            }
            if !unique_names.insert(trimmed) {
                return Err(format!("El nombre dividido '{trimmed}' está duplicado."));
            }
            if output_columns.iter().any(|existing| existing == trimmed)
                || recipe
                    .calculated_column
                    .as_ref()
                    .is_some_and(|calculation| calculation.name == trimmed)
                || recipe
                    .merge_columns
                    .as_ref()
                    .is_some_and(|merge| merge.name == trimmed)
            {
                return Err(format!("La columna dividida '{trimmed}' ya existe."));
            }
        }
        if split.drop_source
            && recipe
                .merge_columns
                .as_ref()
                .is_some_and(|merge| merge.sources.iter().any(|name| name == &split.source))
        {
            return Err(format!(
                "La unión necesita '{source}', pero la división la descartaría."
            ));
        }
        Some(SourceBackedSplitPlan {
            source,
            names: split.names.clone(),
            delimiter: split.delimiter.clone(),
            drop_source: split.drop_source,
        })
    } else {
        None
    };

    let merge_columns = if let Some(merge) = &recipe.merge_columns {
        if !(2..=16).contains(&merge.sources.len()) {
            return Err("La unión requiere entre 2 y 16 columnas fuente.".into());
        }
        if merge.name.trim().is_empty() || merge.name != merge.name.trim() {
            return Err(
                "El nombre de la columna unida no puede estar vacío ni tener espacios exteriores."
                    .into(),
            );
        }
        if output_columns.iter().any(|name| name == &merge.name)
            || split_columns
                .as_ref()
                .is_some_and(|split| split.names.iter().any(|name| name == &merge.name))
        {
            return Err(format!("La columna unida '{}' ya existe.", merge.name));
        }
        if recipe
            .calculated_column
            .as_ref()
            .is_some_and(|calculation| calculation.name == merge.name)
        {
            return Err(format!("La columna unida '{}' ya existe.", merge.name));
        }
        let mut unique_sources = HashSet::new();
        let sources = merge
            .sources
            .iter()
            .map(|source_name| {
                if !unique_sources.insert(source_name) {
                    return Err(format!(
                        "La columna fuente '{source_name}' está duplicada."
                    ));
                }
                let source_column = recipe_column(schema, source_name)?;
                let effective_name = rename_map
                    .get(source_name)
                    .cloned()
                    .unwrap_or_else(|| source_name.clone());
                if recipe.keep_columns.is_some()
                    && !selected_columns.iter().any(|name| name == &effective_name)
                {
                    return Err(format!(
                        "La columna '{effective_name}' requerida por merge fue descartada por keepColumns."
                    ));
                }
                if !text_after_cast(&effective_name, source_column) {
                    return Err(format!(
                        "La columna '{effective_name}' debe ser de texto para unirse."
                    ));
                }
                Ok(effective_name)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Some(SourceBackedMergePlan {
            sources,
            name: merge.name.clone(),
            separator: merge.separator.clone(),
            drop_sources: merge.drop_sources,
        })
    } else {
        None
    };

    let contact_normalizations = if recipe.contact_normalizations.is_empty() {
        Vec::new()
    } else {
        let mut unique_columns = HashSet::new();
        recipe
            .contact_normalizations
            .iter()
            .map(|normalization| {
                if !unique_columns.insert(normalization.column.as_str()) {
                    return Err(format!(
                        "La columna '{}' tiene más de una normalización de contacto.",
                        normalization.column
                    ));
                }
                let source_column = recipe_column(schema, &normalization.column)?;
                let column = rename_map
                    .get(&normalization.column)
                    .cloned()
                    .unwrap_or_else(|| normalization.column.clone());
                if recipe.keep_columns.is_some()
                    && !selected_columns.iter().any(|name| name == &column)
                {
                    return Err(format!(
                        "La columna '{column}' requerida por contactos fue descartada por keepColumns."
                    ));
                }
                if split_columns
                    .as_ref()
                    .is_some_and(|split| split.drop_source && split.source == column)
                {
                    return Err(format!(
                        "La normalización necesita '{column}', pero la división la descartaría."
                    ));
                }
                if merge_columns.as_ref().is_some_and(|merge| {
                    merge.drop_sources && merge.sources.iter().any(|source| source == &column)
                }) {
                    return Err(format!(
                        "La normalización necesita '{column}', pero la unión la descartaría."
                    ));
                }
                if !text_after_cast(&column, source_column) {
                    return Err(format!(
                        "La columna '{column}' debe ser de texto para normalizar contactos."
                    ));
                }
                Ok(SourceBackedContactNormalizationPlan {
                    column,
                    kind: normalization.kind,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    };

    let text_extractions = if recipe.text_extractions.is_empty() {
        Vec::new()
    } else {
        let mut unique_names = HashSet::new();
        recipe
            .text_extractions
            .iter()
            .map(|extraction| {
                if extraction.name.trim().is_empty() || extraction.name != extraction.name.trim() {
                    return Err(
                        "El nombre extraído no puede estar vacío ni tener espacios exteriores."
                            .to_owned(),
                    );
                }
                if !unique_names.insert(extraction.name.as_str()) {
                    return Err(format!(
                        "La columna extraída '{}' está duplicada.",
                        extraction.name
                    ));
                }
                if output_columns.iter().any(|name| name == &extraction.name)
                    || calculated_column_count > 0
                        && recipe
                            .calculated_column
                            .as_ref()
                            .is_some_and(|calculation| calculation.name == extraction.name)
                    || split_columns.as_ref().is_some_and(|split| {
                        split.names.iter().any(|name| name == &extraction.name)
                    })
                    || merge_columns
                        .as_ref()
                        .is_some_and(|merge| merge.name == extraction.name)
                {
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
                        if delimiter_based { "requiere" } else { "no acepta" }
                    ));
                }
                if extraction.delimiter.as_deref().is_some_and(str::is_empty) {
                    return Err("El delimitador de extracción no puede estar vacío.".to_owned());
                }
                let source_column = recipe_column(schema, &extraction.source)?;
                let source = rename_map
                    .get(&extraction.source)
                    .cloned()
                    .unwrap_or_else(|| extraction.source.clone());
                if recipe.keep_columns.is_some()
                    && !selected_columns.iter().any(|name| name == &source)
                {
                    return Err(format!(
                        "La columna '{source}' requerida por extracción fue descartada por keepColumns."
                    ));
                }
                if split_columns
                    .as_ref()
                    .is_some_and(|split| split.drop_source && split.source == source)
                {
                    return Err(format!(
                        "La extracción necesita '{source}', pero la división la descartaría."
                    ));
                }
                if merge_columns.as_ref().is_some_and(|merge| {
                    merge.drop_sources && merge.sources.iter().any(|name| name == &source)
                }) {
                    return Err(format!(
                        "La extracción necesita '{source}', pero la unión la descartaría."
                    ));
                }
                if !text_after_cast(&source, source_column) {
                    return Err(format!(
                        "La columna '{source}' debe ser de texto para extraerse."
                    ));
                }
                Ok(SourceBackedTextExtractionPlan {
                    source,
                    kind: extraction.kind,
                    name: extraction.name.clone(),
                    delimiter: extraction.delimiter.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    };

    let mut available_types = source_columns
        .iter()
        .zip(&output_columns)
        .map(|(source_name, output_name)| {
            (
                output_name.clone(),
                source_backed_target_dtype(schema, source_name, recipe, &rename_map)
                    .expect("las columnas fuente ya fueron validadas"),
            )
        })
        .collect::<HashMap<_, _>>();
    if let Some(calculation) = &recipe.calculated_column {
        let dtype = match calculation.operation {
            CalculatedOperation::Concat => DataType::String,
            CalculatedOperation::Year | CalculatedOperation::Month | CalculatedOperation::Day => {
                DataType::Int32
            }
            CalculatedOperation::Add
            | CalculatedOperation::Subtract
            | CalculatedOperation::Multiply
            | CalculatedOperation::Divide => DataType::Float64,
        };
        available_types.insert(calculation.name.clone(), dtype);
    }
    if let Some(split) = &split_columns {
        available_types.extend(
            split
                .names
                .iter()
                .cloned()
                .map(|name| (name, DataType::String)),
        );
    }
    if let Some(merge) = &merge_columns {
        available_types.insert(merge.name.clone(), DataType::String);
    }
    for normalization in &contact_normalizations {
        available_types.insert(normalization.column.clone(), DataType::String);
    }
    available_types.extend(
        text_extractions
            .iter()
            .map(|extraction| (extraction.name.clone(), DataType::String)),
    );

    let mut available_columns = selected_columns.clone();
    if let Some(calculation) = &recipe.calculated_column {
        available_columns.push(calculation.name.clone());
    }
    if let Some(split) = &split_columns {
        if split.drop_source {
            available_columns.retain(|column| column != &split.source);
        }
        available_columns.extend(split.names.iter().cloned());
    }
    if let Some(merge) = &merge_columns {
        if merge.drop_sources {
            available_columns.retain(|column| !merge.sources.iter().any(|source| source == column));
        }
        available_columns.push(merge.name.clone());
    }
    if split_columns
        .as_ref()
        .is_some_and(|split| split.drop_source)
    {
        available_types.remove(&split_columns.as_ref().expect("split está presente").source);
    }
    if let Some(merge) = &merge_columns {
        if merge.drop_sources {
            for source in &merge.sources {
                available_types.remove(source);
            }
        }
    }
    let outlier_treatments = if recipe.outlier_treatments.is_empty() {
        Vec::new()
    } else {
        if recipe.outlier_treatments.len() > 16 {
            return Err("La receta admite como máximo 16 tratamientos de atípicos.".into());
        }
        let mut unique_columns = HashSet::new();
        recipe
            .outlier_treatments
            .iter()
            .map(|treatment| {
                if !unique_columns.insert(treatment.column.as_str()) {
                    return Err(format!(
                        "La columna '{}' tiene más de un tratamiento de atípicos.",
                        treatment.column
                    ));
                }
                let column = rename_map
                    .get(&treatment.column)
                    .cloned()
                    .unwrap_or_else(|| treatment.column.clone());
                let dtype = available_types.get(&column).cloned().ok_or_else(|| {
                    format!(
                        "La columna '{column}' para tratar atípicos no sobrevivió las etapas estructurales."
                    )
                })?;
                if !matches!(&dtype, DataType::Int64 | DataType::Float64) {
                    return Err(format!(
                        "La columna '{column}' debe ser Int64 o Float64 para tratar valores numéricos."
                    ));
                }
                if treatment.action == OutlierAction::Cap {
                    available_types.insert(column.clone(), DataType::Float64);
                }
                Ok(SourceBackedOutlierPlan {
                    column,
                    action: treatment.action,
                    dtype,
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    let group_summary = recipe
        .group_summary
        .as_ref()
        .map(|summary| {
            source_backed_group_summary_plan(
                summary,
                &rename_map,
                &available_columns,
                &available_types,
            )
        })
        .transpose()?;

    for filter in &recipe.filters {
        let effective = rename_map
            .get(&filter.column)
            .cloned()
            .unwrap_or_else(|| filter.column.clone());
        if !output_columns.iter().any(|column| column == &effective) {
            return Err(format!(
                "La columna '{}' no existe para el filtro source-backed.",
                filter.column
            ));
        }
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
        if literal.contains('\0') {
            return Err("El valor del filtro contiene un carácter no válido.".to_owned());
        }
        if recipe_filter_uses_temporal_literal(filter.operator) {
            let dtype = source_backed_target_dtype(schema, &filter.column, recipe, &rename_map)?;
            if matches!(&dtype, DataType::Date | DataType::Datetime(_, None)) {
                parse_recipe_filter_datetime(literal, &filter.column)?;
            } else if matches!(&dtype, DataType::Datetime(_, Some(_))) {
                return Err(format!(
                    "La comparación de '{}' no admite fechas con zona horaria.",
                    filter.column
                ));
            } else if recipe_filter_is_ordered(filter.operator) {
                strict_f64(literal, "El valor del filtro")?;
            }
        }
    }

    let dropped_column_count = output_columns.len().saturating_sub(selected_columns.len());
    let kept_order_changed = output_columns != selected_columns;
    Ok(SourceBackedProjectionPlan {
        source_columns,
        output_columns,
        selected_columns,
        renamed_column_count,
        converted_column_count,
        parsed_date_column_count,
        calculated_column_count,
        replacement_columns,
        split_columns,
        merge_columns,
        contact_normalizations,
        text_extractions,
        outlier_treatments,
        group_summary,
        dropped_column_count,
        kept_order_changed,
    })
}

pub(super) fn duckdb_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn source_backed_regex_replacement(value: &str) -> Result<String, String> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            return Err(
                "La sustitución regex source-backed no admite barras invertidas literales."
                    .to_owned(),
            );
        }
        if character != '$' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('$') => output.push('$'),
            Some(group @ '1'..='9') => {
                output.push('\\');
                output.push(group);
            }
            Some('{') => {
                let group = characters.next();
                if !matches!(group, Some('1'..='9')) || characters.next() != Some('}') {
                    return Err(
                        "La sustitución regex source-backed solo admite grupos $1 a $9.".to_owned(),
                    );
                }
                output.push('\\');
                output.push(group.expect("se validó el grupo regex"));
            }
            _ => return Err(
                "La sustitución regex source-backed solo admite texto literal y grupos $1 a $9."
                    .to_owned(),
            ),
        }
    }
    Ok(output)
}

fn source_backed_replace_expression(
    value: &str,
    replacement: &FindReplaceRecipe,
) -> Result<String, String> {
    if replacement.regex {
        let replacement_text = source_backed_regex_replacement(&replacement.replace)?;
        Ok(format!(
            "regexp_replace(CAST({value} AS VARCHAR), {}, {}, 'g')",
            duckdb_string_literal(&replacement.find),
            duckdb_string_literal(&replacement_text)
        ))
    } else {
        Ok(format!(
            "replace(CAST({value} AS VARCHAR), {}, {})",
            duckdb_string_literal(&replacement.find),
            duckdb_string_literal(&replacement.replace)
        ))
    }
}

fn source_backed_filter_expression(
    filter: &RecipeFilter,
    rename_map: &HashMap<String, String>,
    dtype: &DataType,
) -> Result<String, String> {
    let effective = rename_map
        .get(&filter.column)
        .map(String::as_str)
        .unwrap_or(filter.column.as_str());
    let column = format!("t.{}", duckdb_identifier(effective));
    let literal = filter.value.as_deref().unwrap_or_default();
    let expression = match filter.operator {
        RecipeFilterOperator::IsNull => format!("{column} IS NULL"),
        RecipeFilterOperator::NotNull => format!("{column} IS NOT NULL"),
        RecipeFilterOperator::Eq | RecipeFilterOperator::Neq
            if matches!(dtype, DataType::Date | DataType::Datetime(_, None)) =>
        {
            let datetime = parse_recipe_filter_datetime(literal, &filter.column)?;
            let operator = if filter.operator == RecipeFilterOperator::Eq {
                "="
            } else {
                "<>"
            };
            let literal = match dtype {
                DataType::Date => format!(
                    "DATE {}",
                    duckdb_string_literal(&datetime.date().format("%Y-%m-%d").to_string())
                ),
                DataType::Datetime(_, None) => format!(
                    "TIMESTAMP {}",
                    duckdb_string_literal(&datetime.format("%Y-%m-%d %H:%M:%S%.f").to_string(),)
                ),
                _ => unreachable!("el match exterior limita esta rama a fechas"),
            };
            format!("{column} {operator} {literal}")
        }
        RecipeFilterOperator::Eq => format!(
            "CAST({column} AS VARCHAR) = {}",
            duckdb_string_literal(literal)
        ),
        RecipeFilterOperator::Neq => format!(
            "CAST({column} AS VARCHAR) <> {}",
            duckdb_string_literal(literal)
        ),
        RecipeFilterOperator::Contains => format!(
            "strpos(lower(CAST({column} AS VARCHAR)), lower({})) > 0",
            duckdb_string_literal(literal)
        ),
        RecipeFilterOperator::NotContains => format!(
            "strpos(lower(CAST({column} AS VARCHAR)), lower({})) = 0",
            duckdb_string_literal(literal)
        ),
        RecipeFilterOperator::Gt
        | RecipeFilterOperator::Lt
        | RecipeFilterOperator::Gte
        | RecipeFilterOperator::Lte => {
            let operator = match filter.operator {
                RecipeFilterOperator::Gt => ">",
                RecipeFilterOperator::Lt => "<",
                RecipeFilterOperator::Gte => ">=",
                RecipeFilterOperator::Lte => "<=",
                _ => unreachable!("el match exterior limita los operadores numéricos"),
            };
            if matches!(dtype, DataType::Date | DataType::Datetime(_, None)) {
                let datetime = parse_recipe_filter_datetime(literal, &filter.column)?;
                let literal = match dtype {
                    DataType::Date => format!(
                        "DATE {}",
                        duckdb_string_literal(&datetime.date().format("%Y-%m-%d").to_string())
                    ),
                    DataType::Datetime(_, None) => format!(
                        "TIMESTAMP {}",
                        duckdb_string_literal(&datetime.format("%Y-%m-%d %H:%M:%S%.f").to_string(),)
                    ),
                    _ => unreachable!("el match exterior limita esta rama a fechas"),
                };
                format!("{column} {operator} {literal}")
            } else if matches!(dtype, DataType::Datetime(_, Some(_))) {
                return Err(format!(
                    "La comparación de '{}' no admite fechas con zona horaria.",
                    filter.column
                ));
            } else {
                let numeric_literal = strict_f64(literal, "El valor del filtro")?;
                format!("CAST({column} AS DOUBLE) {operator} {numeric_literal}")
            }
        }
    };
    Ok(expression)
}

fn duckdb_cast_type(target: RecipeCastTarget) -> &'static str {
    match target {
        RecipeCastTarget::String => "VARCHAR",
        RecipeCastTarget::Integer => "BIGINT",
        RecipeCastTarget::Decimal => "DOUBLE",
        RecipeCastTarget::Boolean => "BOOLEAN",
    }
}

fn duckdb_date_format(format: RecipeDateFormat) -> Result<&'static str, String> {
    match format {
        RecipeDateFormat::Ymd => Ok("%Y-%m-%d"),
        RecipeDateFormat::Dmy => Ok("%d/%m/%Y"),
        RecipeDateFormat::Mdy => Ok("%m/%d/%Y"),
        RecipeDateFormat::Iso8601 => Err("ISO no usa un formato único.".to_owned()),
    }
}

pub(super) fn duckdb_iso8601_expression(column: &str, target: RecipeDateTarget) -> String {
    let raw = format!("TRIM(CAST({column} AS VARCHAR))");
    let without_utc_suffix = format!(
        "CASE WHEN RIGHT({raw}, 1) = 'Z' THEN SUBSTR({raw}, 1, LENGTH({raw}) - 1) ELSE {raw} END"
    );
    let value = format!("NULLIF({without_utc_suffix}, '')");
    let parsed = [
        "%Y-%m-%d",
        "%Y-%m-%dT%H:%M:%S.%f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S.%f",
        "%Y-%m-%d %H:%M:%S",
    ]
    .into_iter()
    .map(|format| format!("try_strptime({value}, {})", duckdb_string_literal(format)))
    .collect::<Vec<_>>()
    .join(", ");
    let parsed = format!("COALESCE({parsed})");
    match target {
        RecipeDateTarget::Date => format!("CAST({parsed} AS DATE)"),
        RecipeDateTarget::Datetime => parsed,
    }
}

fn duckdb_date_expression(
    column: &str,
    format: RecipeDateFormat,
    target: RecipeDateTarget,
) -> Result<String, String> {
    if format == RecipeDateFormat::Iso8601 {
        return Ok(duckdb_iso8601_expression(column, target));
    }
    let parsed = format!(
        "strptime(NULLIF(TRIM(CAST({column} AS VARCHAR)), ''), {})",
        duckdb_string_literal(duckdb_date_format(format)?)
    );
    Ok(match target {
        RecipeDateTarget::Date => format!("CAST({parsed} AS DATE)"),
        RecipeDateTarget::Datetime => parsed,
    })
}

fn duckdb_calculation_expression(
    calculation: &CalculatedColumnRecipe,
    rename_map: &HashMap<String, String>,
) -> Result<String, String> {
    let source_name = rename_map
        .get(&calculation.source)
        .map(String::as_str)
        .unwrap_or(calculation.source.as_str());
    let source = format!("CAST(t.{} AS DOUBLE)", duckdb_identifier(source_name));
    match calculation.operation {
        CalculatedOperation::Concat => {
            let operand = calculation
                .operand
                .as_ref()
                .ok_or_else(|| "La operación calculada requiere un operando.".to_owned())?;
            let source = format!("CAST(t.{} AS VARCHAR)", duckdb_identifier(source_name));
            let operand = match operand.kind {
                CalculatedOperandKind::Literal => duckdb_string_literal(&operand.value),
                CalculatedOperandKind::Column => {
                    let operand_name = rename_map
                        .get(&operand.value)
                        .map(String::as_str)
                        .unwrap_or(operand.value.as_str());
                    format!("CAST(t.{} AS VARCHAR)", duckdb_identifier(operand_name))
                }
            };
            Ok(format!("{source} || {operand}"))
        }
        CalculatedOperation::Add
        | CalculatedOperation::Subtract
        | CalculatedOperation::Multiply
        | CalculatedOperation::Divide => {
            let operand = calculation
                .operand
                .as_ref()
                .ok_or_else(|| "La operación calculada requiere un operando.".to_owned())?;
            let operand = match operand.kind {
                CalculatedOperandKind::Literal => {
                    let value = strict_f64(&operand.value, "El operando numérico")?;
                    if calculation.operation == CalculatedOperation::Divide && value == 0.0 {
                        return Err("División por cero en el operando numérico.".to_owned());
                    }
                    value.to_string()
                }
                CalculatedOperandKind::Column => {
                    let operand_name = rename_map
                        .get(&operand.value)
                        .map(String::as_str)
                        .unwrap_or(operand.value.as_str());
                    format!("CAST(t.{} AS DOUBLE)", duckdb_identifier(operand_name))
                }
            };
            let operator = match calculation.operation {
                CalculatedOperation::Add => "+",
                CalculatedOperation::Subtract => "-",
                CalculatedOperation::Multiply => "*",
                CalculatedOperation::Divide => "/",
                _ => {
                    unreachable!("el match exterior limita las operaciones numéricas source-backed")
                }
            };
            let operand = if calculation.operation == CalculatedOperation::Divide {
                format!("NULLIF({operand}, 0)")
            } else {
                operand
            };
            Ok(format!("{source} {operator} {operand}"))
        }
        CalculatedOperation::Year => Ok(format!(
            "CAST(year(t.{}) AS INTEGER)",
            duckdb_identifier(source_name)
        )),
        CalculatedOperation::Month => Ok(format!(
            "CAST(month(t.{}) AS INTEGER)",
            duckdb_identifier(source_name)
        )),
        CalculatedOperation::Day => Ok(format!(
            "CAST(day(t.{}) AS INTEGER)",
            duckdb_identifier(source_name)
        )),
    }
}

fn source_backed_text_extraction_expression(
    extraction: &SourceBackedTextExtractionPlan,
) -> Result<String, String> {
    let value = format!(
        "CAST(t.{} AS VARCHAR)",
        duckdb_identifier(&extraction.source)
    );
    match extraction.kind {
        ExtractionKind::FirstToken
        | ExtractionKind::LastToken
        | ExtractionKind::Digits
        | ExtractionKind::Letters => {
            let pattern = match extraction.kind {
                ExtractionKind::FirstToken => r"^\s*(\S+)",
                ExtractionKind::LastToken => r"(\S+)\s*$",
                ExtractionKind::Digits => r"([0-9]+)",
                ExtractionKind::Letters => r"(\p{L}+)",
                ExtractionKind::Before | ExtractionKind::After => {
                    unreachable!("el match exterior limita las extracciones regex")
                }
            };
            let pattern = duckdb_string_literal(pattern);
            Ok(format!(
                "CASE WHEN {value} IS NOT NULL AND regexp_matches({value}, {pattern}) THEN regexp_extract({value}, {pattern}, 1) ELSE NULL END AS {}",
                duckdb_identifier(&extraction.name)
            ))
        }
        ExtractionKind::Before | ExtractionKind::After => {
            let delimiter = extraction.delimiter.as_deref().ok_or_else(|| {
                "La extracción antes/después del delimitador requiere delimitador.".to_owned()
            })?;
            let delimiter = duckdb_string_literal(delimiter);
            let position = format!("strpos({value}, {delimiter})");
            let expression = match extraction.kind {
                ExtractionKind::Before => {
                    format!("left({value}, {position} - 1)")
                }
                ExtractionKind::After => {
                    format!("substr({value}, {position} + length({delimiter}))")
                }
                ExtractionKind::FirstToken
                | ExtractionKind::LastToken
                | ExtractionKind::Digits
                | ExtractionKind::Letters => {
                    unreachable!("el match exterior limita las extracciones por delimitador")
                }
            };
            Ok(format!(
                "CASE WHEN {value} IS NOT NULL AND {position} > 0 THEN {expression} ELSE NULL END AS {}",
                duckdb_identifier(&extraction.name)
            ))
        }
    }
}

fn source_backed_contact_normalization_expression(
    normalization: &SourceBackedContactNormalizationPlan,
) -> String {
    let value = format!(
        "CAST(t.{} AS VARCHAR)",
        duckdb_identifier(&normalization.column)
    );
    let trimmed = format!(
        "regexp_replace({value}, {}, '', 'g')",
        duckdb_string_literal(r"^[\s\p{Z}]+|[\s\p{Z}]+$")
    );
    match normalization.kind {
        ContactKind::Email => format!("lower({trimmed})"),
        ContactKind::Phone => {
            let digits = format!(
                "regexp_replace({value}, {}, '', 'g')",
                duckdb_string_literal(r"[^0-9]+")
            );
            format!(
                "CASE WHEN {value} IS NULL THEN NULL WHEN left({trimmed}, 1) = '+' THEN '+' || {digits} ELSE {digits} END"
            )
        }
        ContactKind::Address => format!(
            "trim(regexp_replace({value}, {}, ' ', 'g'))",
            duckdb_string_literal(r"[\s\p{Z}]+")
        ),
    }
}

fn source_backed_summary_aggregate_expression(
    aggregation: &SourceBackedSummaryAggregationPlan,
) -> String {
    let identifier = duckdb_identifier(&aggregation.column);
    let value = format!("t.{identifier}");
    let expression = match aggregation.operation {
        SummaryOperation::Sum if aggregation.dtype == DataType::Int64 => {
            format!("CAST(SUM({value}) AS BIGINT)")
        }
        SummaryOperation::Sum => format!("SUM({value})"),
        SummaryOperation::Mean => format!("AVG({value})"),
        SummaryOperation::Min => format!("MIN({value})"),
        SummaryOperation::Max => format!("MAX({value})"),
        SummaryOperation::Count => "COUNT(*)".to_owned(),
        SummaryOperation::CountUnique => format!("COUNT(DISTINCT {value})"),
    };
    format!("{expression} AS {}", duckdb_identifier(&aggregation.output))
}

fn source_backed_outlier_alias(index: usize, metric: &str) -> String {
    duckdb_identifier(&format!("__columnia_outlier_{metric}_{index}"))
}

fn source_backed_outlier_value(plan: &SourceBackedOutlierPlan) -> String {
    format!("CAST(t.{} AS DOUBLE)", duckdb_identifier(&plan.column))
}

fn source_backed_outlier_condition(plan: &SourceBackedOutlierPlan, index: usize) -> String {
    let value = source_backed_outlier_value(plan);
    let lower = source_backed_outlier_alias(index, "lower");
    let upper = source_backed_outlier_alias(index, "upper");
    format!("{value} IS NOT NULL AND ({value} < s.{lower} OR {value} > s.{upper})")
}

fn source_backed_projection_query(
    schema: &DataFrame,
    recipe: &TransformRecipe,
) -> Result<SourceBackedProjectionQueries, String> {
    let plan = source_backed_projection_plan(schema, recipe)?;
    let rename_map = recipe
        .renames
        .iter()
        .map(|rename| (rename.from.clone(), rename.to.clone()))
        .collect::<HashMap<_, _>>();
    let renamed_expressions = plan
        .source_columns
        .iter()
        .zip(&plan.output_columns)
        .map(|(source_name, output_name)| {
            if output_name == source_name {
                duckdb_identifier(source_name)
            } else {
                format!(
                    "{} AS {}",
                    duckdb_identifier(source_name),
                    duckdb_identifier(output_name)
                )
            }
        })
        .collect::<Vec<_>>();
    let cast_targets = recipe
        .casts
        .iter()
        .filter_map(|cast| {
            let column = recipe_column(schema, &cast.column).ok()?;
            let already_target = matches!(
                (column.dtype(), cast.target),
                (DataType::String, RecipeCastTarget::String)
                    | (DataType::Int64, RecipeCastTarget::Integer)
                    | (DataType::Float64, RecipeCastTarget::Decimal)
                    | (DataType::Boolean, RecipeCastTarget::Boolean)
            );
            if already_target {
                None
            } else {
                let effective = rename_map
                    .get(&cast.column)
                    .cloned()
                    .unwrap_or_else(|| cast.column.clone());
                Some((effective, cast.target))
            }
        })
        .collect::<HashMap<_, _>>();
    let date_targets = recipe
        .date_parses
        .iter()
        .filter_map(|parse| {
            let column = recipe_column(schema, &parse.column).ok()?;
            let already_target = matches!(
                (column.dtype(), parse.target),
                (DataType::Date, RecipeDateTarget::Date)
                    | (DataType::Datetime(_, _), RecipeDateTarget::Datetime)
            );
            if already_target {
                None
            } else {
                let effective = rename_map
                    .get(&parse.column)
                    .cloned()
                    .unwrap_or_else(|| parse.column.clone());
                Some((effective, (parse.format, parse.target)))
            }
        })
        .collect::<HashMap<_, _>>();
    let transformed_expressions = plan
        .output_columns
        .iter()
        .map(|column| -> Result<String, String> {
            let input = format!("t.{}", duckdb_identifier(column));
            let expression = if let Some(target) = cast_targets.get(column) {
                format!(
                    "CAST({input} AS {}) AS {}",
                    duckdb_cast_type(*target),
                    duckdb_identifier(column)
                )
            } else if let Some((format, target)) = date_targets.get(column) {
                format!(
                    "{} AS {}",
                    duckdb_date_expression(&input, *format, *target)?,
                    duckdb_identifier(column)
                )
            } else {
                format!("{input} AS {}", duckdb_identifier(column))
            };
            Ok(expression)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let filters = recipe
        .filters
        .iter()
        .map(|filter| {
            let dtype = source_backed_target_dtype(schema, &filter.column, recipe, &rename_map)?;
            source_backed_filter_expression(filter, &rename_map, &dtype)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let filtered_source = if filters.is_empty() {
        "transformed"
    } else {
        "filtered"
    };
    let filter_cte = if filters.is_empty() {
        String::new()
    } else {
        format!(
            ", filtered AS (SELECT * FROM transformed AS t WHERE {})",
            filters.join(" AND ")
        )
    };
    let replacement_cte = if plan.replacement_columns.is_empty() {
        String::new()
    } else {
        let replacement = recipe
            .find_replace
            .as_ref()
            .expect("las columnas de reemplazo requieren una receta de reemplazo");
        let expressions = plan
            .output_columns
            .iter()
            .map(|column| -> Result<String, String> {
                let identifier = duckdb_identifier(column);
                if plan
                    .replacement_columns
                    .iter()
                    .any(|target| target == column)
                {
                    Ok(format!(
                        "{} AS {identifier}",
                        source_backed_replace_expression(&format!("t.{identifier}"), replacement,)?
                    ))
                } else {
                    Ok(format!("t.{identifier} AS {identifier}"))
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        format!(
            ", replaced AS (SELECT {} FROM {} AS t)",
            expressions.join(", "),
            filtered_source
        )
    };
    let mut result_source = if plan.replacement_columns.is_empty() {
        filtered_source
    } else {
        "replaced"
    };
    let calculation_input_source = if plan.replacement_columns.is_empty() {
        filtered_source
    } else {
        "replaced"
    };
    let calculation_validation = recipe
        .calculated_column
        .as_ref()
        .filter(|calculation| calculation.operation == CalculatedOperation::Divide)
        .and_then(|calculation| {
            let CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value,
            } = calculation.operand.as_ref()?
            else {
                return None;
            };
            let source_name = rename_map
                .get(&calculation.source)
                .map(String::as_str)
                .unwrap_or(calculation.source.as_str());
            let operand_name = rename_map
                .get(value)
                .map(String::as_str)
                .unwrap_or(value.as_str());
            let source = format!("t.{}", duckdb_identifier(source_name));
            let operand = format!("t.{}", duckdb_identifier(operand_name));
            Some(format!(
                "WITH renamed AS (SELECT {} FROM dataset), transformed AS (SELECT {} FROM renamed AS t){}{} SELECT CASE WHEN EXISTS (SELECT 1 FROM {calculation_input_source} AS t WHERE {source} IS NOT NULL AND {operand} IS NOT NULL AND CAST({operand} AS DOUBLE) = 0) THEN 1 ELSE 0 END",
                renamed_expressions.join(", "),
                transformed_expressions.join(", "),
                filter_cte,
                replacement_cte
            ))
        });
    let calculation_cte = if let Some(calculation) = &recipe.calculated_column {
        result_source = "calculated";
        format!(
            ", calculated AS (SELECT *, {} AS {} FROM {} AS t)",
            duckdb_calculation_expression(calculation, &rename_map)?,
            duckdb_identifier(&calculation.name),
            calculation_input_source
        )
    } else {
        String::new()
    };
    let split_cte = if let Some(split) = &plan.split_columns {
        let source = result_source;
        let value = format!("CAST(t.{} AS VARCHAR)", duckdb_identifier(&split.source));
        let parts = format!(
            "string_split({value}, {})",
            duckdb_string_literal(&split.delimiter)
        );
        let part_count = format!("array_length({parts})");
        let expressions = split
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let position = index + 1;
                let value = if position == split.names.len() {
                    format!(
                        "CASE WHEN {part_count} >= {position} THEN array_to_string(list_slice({parts}, {position}, {part_count}), {}) ELSE NULL END",
                        duckdb_string_literal(&split.delimiter)
                    )
                } else {
                    format!(
                        "CASE WHEN {part_count} >= {position} THEN split_part({value}, {}, {position}) ELSE NULL END",
                        duckdb_string_literal(&split.delimiter)
                    )
                };
                format!("{value} AS {}", duckdb_identifier(name))
            })
            .collect::<Vec<_>>();
        result_source = "split";
        format!(
            ", split AS (SELECT *, {} FROM {source} AS t)",
            expressions.join(", ")
        )
    } else {
        String::new()
    };
    let merge_cte = if let Some(merge) = &plan.merge_columns {
        let source_expressions = merge
            .sources
            .iter()
            .map(|column| format!("CAST(t.{} AS VARCHAR)", duckdb_identifier(column)))
            .collect::<Vec<_>>();
        let any_source_present = source_expressions
            .iter()
            .map(|expression| format!("{expression} IS NOT NULL"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let joined = format!(
            "concat_ws({}, {})",
            duckdb_string_literal(&merge.separator),
            source_expressions.join(", ")
        );
        let merge_expression = format!(
            "CASE WHEN {any_source_present} THEN {joined} ELSE NULL END AS {}",
            duckdb_identifier(&merge.name)
        );
        let source = result_source;
        result_source = "merged";
        format!(", merged AS (SELECT *, {merge_expression} FROM {source} AS t)")
    } else {
        String::new()
    };
    let mut output_columns = plan.selected_columns.clone();
    if let Some(calculation) = &recipe.calculated_column {
        output_columns.push(calculation.name.clone());
    }
    if let Some(split) = &plan.split_columns {
        if split.drop_source {
            output_columns.retain(|column| column != &split.source);
        }
        output_columns.extend(split.names.iter().cloned());
    }
    if let Some(merge) = &plan.merge_columns {
        if merge.drop_sources {
            output_columns.retain(|column| !merge.sources.iter().any(|source| source == column));
        }
        output_columns.push(merge.name.clone());
    }
    let outlier_input_source = result_source;
    let pre_outlier_ctes = format!(
        "WITH renamed AS (SELECT {} FROM dataset), transformed AS (SELECT {} FROM renamed AS t){}{}{}{}{}",
        renamed_expressions.join(", "),
        transformed_expressions.join(", "),
        filter_cte,
        replacement_cte,
        calculation_cte,
        split_cte,
        merge_cte
    );
    let outlier_quantile_cte = if plan.outlier_treatments.is_empty() {
        String::new()
    } else {
        let expressions = plan
            .outlier_treatments
            .iter()
            .enumerate()
            .flat_map(|(index, treatment)| {
                let value = format!("CAST(t.{} AS DOUBLE)", duckdb_identifier(&treatment.column));
                let median = if treatment.dtype == DataType::Int64 {
                    format!("quantile_disc({value}, 0.5)")
                } else {
                    format!("quantile_cont({value}, 0.5)")
                };
                [
                    format!(
                        "quantile_cont({value}, 0.25) AS {}",
                        source_backed_outlier_alias(index, "q1")
                    ),
                    format!(
                        "quantile_cont({value}, 0.75) AS {}",
                        source_backed_outlier_alias(index, "q3")
                    ),
                    format!(
                        "{median} AS {}",
                        source_backed_outlier_alias(index, "median")
                    ),
                    format!(
                        "COUNT(t.{}) AS {}",
                        duckdb_identifier(&treatment.column),
                        source_backed_outlier_alias(index, "valid_count")
                    ),
                ]
            })
            .collect::<Vec<_>>();
        format!(
            ", outlier_quantiles AS (SELECT {} FROM {outlier_input_source} AS t)",
            expressions.join(", ")
        )
    };
    let outlier_stats_cte = if plan.outlier_treatments.is_empty() {
        String::new()
    } else {
        let expressions = plan
            .outlier_treatments
            .iter()
            .enumerate()
            .flat_map(|(index, _)| {
                let q1 = source_backed_outlier_alias(index, "q1");
                let q3 = source_backed_outlier_alias(index, "q3");
                let median = source_backed_outlier_alias(index, "median");
                [
                    format!("q.{q1} AS {q1}"),
                    format!("q.{q3} AS {q3}"),
                    format!(
                        "q.{q1} - 1.5 * (q.{q3} - q.{q1}) AS {}",
                        source_backed_outlier_alias(index, "lower")
                    ),
                    format!(
                        "q.{q3} + 1.5 * (q.{q3} - q.{q1}) AS {}",
                        source_backed_outlier_alias(index, "upper")
                    ),
                    format!("q.{median} AS {median}"),
                    format!(
                        "q.{} AS {}",
                        source_backed_outlier_alias(index, "valid_count"),
                        source_backed_outlier_alias(index, "valid_count")
                    ),
                ]
            })
            .collect::<Vec<_>>();
        format!(
            ", outlier_stats AS (SELECT {} FROM outlier_quantiles AS q)",
            expressions.join(", ")
        )
    };
    let outlier_cte = if plan.outlier_treatments.is_empty() {
        String::new()
    } else {
        let expressions = output_columns
            .iter()
            .map(|column| {
                let identifier = duckdb_identifier(column);
                plan.outlier_treatments
                    .iter()
                    .enumerate()
                    .find(|(_, treatment)| treatment.column == *column)
                    .map_or_else(
                        || format!("t.{identifier} AS {identifier}"),
                        |(index, treatment)| {
                            let value = format!("CAST(t.{identifier} AS DOUBLE)");
                            let condition = source_backed_outlier_condition(treatment, index);
                            let lower = format!(
                                "s.{}",
                                source_backed_outlier_alias(index, "lower")
                            );
                            let upper = format!(
                                "s.{}",
                                source_backed_outlier_alias(index, "upper")
                            );
                            let median = format!(
                                "s.{}",
                                source_backed_outlier_alias(index, "median")
                            );
                            let expression = match treatment.action {
                                OutlierAction::Cap => format!(
                                    "CASE WHEN {value} IS NULL THEN NULL WHEN {value} < {lower} THEN {lower} WHEN {value} > {upper} THEN {upper} ELSE {value} END"
                                ),
                                OutlierAction::Impute => {
                                    let replacement = if treatment.dtype == DataType::Int64 {
                                        format!("CAST({median} AS BIGINT)")
                                    } else {
                                        median
                                    };
                                    let original = if treatment.dtype == DataType::Int64 {
                                        format!("t.{identifier}")
                                    } else {
                                        value.clone()
                                    };
                                    format!(
                                        "CASE WHEN {value} IS NULL THEN NULL WHEN {condition} THEN {replacement} ELSE {original} END"
                                    )
                                }
                                OutlierAction::Drop => format!("t.{identifier}"),
                            };
                            format!("{expression} AS {identifier}")
                        },
                    )
            })
            .collect::<Vec<_>>();
        let drop_conditions = plan
            .outlier_treatments
            .iter()
            .enumerate()
            .filter(|(_, treatment)| treatment.action == OutlierAction::Drop)
            .map(|(index, treatment)| source_backed_outlier_condition(treatment, index))
            .collect::<Vec<_>>();
        let where_clause = if drop_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE NOT ({})", drop_conditions.join(" OR "))
        };
        result_source = "outliers";
        format!(
            "{outlier_quantile_cte}{outlier_stats_cte}, outliers AS (SELECT {} FROM {outlier_input_source} AS t CROSS JOIN outlier_stats AS s{where_clause})",
            expressions.join(", ")
        )
    };
    let contact_input_source = result_source;
    let contact_normalization_cte = if plan.contact_normalizations.is_empty() {
        String::new()
    } else {
        let expressions = output_columns
            .iter()
            .map(|column| {
                let identifier = duckdb_identifier(column);
                if let Some(normalization) = plan
                    .contact_normalizations
                    .iter()
                    .find(|normalization| normalization.column == *column)
                {
                    format!(
                        "{} AS {identifier}",
                        source_backed_contact_normalization_expression(normalization)
                    )
                } else {
                    format!("t.{identifier} AS {identifier}")
                }
            })
            .collect::<Vec<_>>();
        result_source = "normalized_contacts";
        format!(
            ", normalized_contacts AS (SELECT {} FROM {contact_input_source} AS t)",
            expressions.join(", ")
        )
    };
    let text_extraction_cte = if plan.text_extractions.is_empty() {
        String::new()
    } else {
        let source = result_source;
        let expressions = plan
            .text_extractions
            .iter()
            .map(source_backed_text_extraction_expression)
            .collect::<Result<Vec<_>, String>>()?;
        result_source = "text_extracted";
        format!(
            ", text_extracted AS (SELECT *, {} FROM {source} AS t)",
            expressions.join(", ")
        )
    };
    output_columns.extend(
        plan.text_extractions
            .iter()
            .map(|extraction| extraction.name.clone()),
    );
    let pre_group_ctes = format!(
        "WITH renamed AS (SELECT {} FROM dataset), transformed AS (SELECT {} FROM renamed AS t){}{}{}{}{}{}{}{}",
        renamed_expressions.join(", "),
        transformed_expressions.join(", "),
        filter_cte,
        replacement_cte,
        calculation_cte,
        split_cte,
        merge_cte,
        outlier_cte,
        contact_normalization_cte,
        text_extraction_cte
    );
    let group_input_source = result_source;
    let mut final_ctes = pre_group_ctes.clone();
    let mut final_source = result_source;
    let mut final_columns = output_columns;
    let mut final_order_column = None;
    let mut group_input_count = None;
    let mut group_validation = None;
    let mut output_validation = None;
    if let Some(summary) = &plan.group_summary {
        const GROUP_ORDER_COLUMN: &str = "__columnia_group_order";
        if final_columns
            .iter()
            .any(|column| column == GROUP_ORDER_COLUMN)
        {
            return Err(format!(
                "La receta no puede usar la columna reservada '{GROUP_ORDER_COLUMN}' para agrupar."
            ));
        }
        let order_identifier = duckdb_identifier(GROUP_ORDER_COLUMN);
        final_ctes.push_str(&format!(
            ", ordered AS (SELECT *, row_number() OVER () AS {order_identifier} FROM {group_input_source} AS t)"
        ));
        let group_select = summary
            .group_by
            .iter()
            .map(|column| {
                let identifier = duckdb_identifier(column);
                format!("t.{identifier} AS {identifier}")
            })
            .chain(
                summary
                    .aggregations
                    .iter()
                    .map(source_backed_summary_aggregate_expression),
            )
            .chain(std::iter::once(format!(
                "MIN(t.{order_identifier}) AS {order_identifier}"
            )))
            .collect::<Vec<_>>();
        let group_by = summary
            .group_by
            .iter()
            .map(|column| format!("t.{}", duckdb_identifier(column)))
            .collect::<Vec<_>>()
            .join(", ");
        final_ctes.push_str(&format!(
            ", grouped AS (SELECT {} FROM ordered AS t GROUP BY {})",
            group_select.join(", "),
            group_by
        ));
        final_source = "grouped";
        final_order_column = Some(GROUP_ORDER_COLUMN);
        final_columns = summary
            .group_by
            .iter()
            .cloned()
            .chain(
                summary
                    .aggregations
                    .iter()
                    .map(|aggregation| aggregation.output.clone()),
            )
            .collect();

        group_input_count = Some(format!(
            "{pre_group_ctes} SELECT CAST(COUNT(*) AS BIGINT) FROM {group_input_source}"
        ));

        let mut invalid_input_terms = Vec::new();
        for (column, dtype) in summary.group_by.iter().zip(&summary.group_dtypes) {
            if dtype == &DataType::Float64 {
                let identifier = duckdb_identifier(column);
                invalid_input_terms.push(format!(
                    "t.{identifier} IS NOT NULL AND NOT isfinite(CAST(t.{identifier} AS DOUBLE))"
                ));
            }
        }
        let mut integer_sum_overflow_terms = Vec::new();
        for aggregation in &summary.aggregations {
            let identifier = duckdb_identifier(&aggregation.column);
            if aggregation.dtype == DataType::Float64
                && matches!(
                    aggregation.operation,
                    SummaryOperation::Sum
                        | SummaryOperation::Mean
                        | SummaryOperation::Min
                        | SummaryOperation::Max
                        | SummaryOperation::CountUnique
                )
            {
                invalid_input_terms.push(format!(
                    "t.{identifier} IS NOT NULL AND NOT isfinite(CAST(t.{identifier} AS DOUBLE))"
                ));
            }
            if aggregation.dtype == DataType::Int64
                && aggregation.operation == SummaryOperation::Mean
            {
                invalid_input_terms.push(format!(
                    "t.{identifier} IS NOT NULL AND abs(CAST(t.{identifier} AS DOUBLE)) > 9007199254740992"
                ));
            }
            if aggregation.dtype == DataType::Int64
                && aggregation.operation == SummaryOperation::Sum
            {
                integer_sum_overflow_terms.push(format!(
                    "SUM(CAST(t.{identifier} AS HUGEINT)) > 9223372036854775807 OR SUM(CAST(t.{identifier} AS HUGEINT)) < -9223372036854775808"
                ));
            }
        }
        let raw_validation = if invalid_input_terms.is_empty() {
            "FALSE".to_owned()
        } else {
            format!(
                "EXISTS (SELECT 1 FROM {group_input_source} AS t WHERE {})",
                invalid_input_terms.join(" OR ")
            )
        };
        let overflow_validation = if integer_sum_overflow_terms.is_empty() {
            "FALSE".to_owned()
        } else {
            format!(
                "EXISTS (SELECT 1 FROM {group_input_source} AS t GROUP BY {group_by} HAVING {})",
                integer_sum_overflow_terms.join(" OR ")
            )
        };
        if raw_validation != "FALSE" || overflow_validation != "FALSE" {
            group_validation = Some(format!(
                "{pre_group_ctes} SELECT CASE WHEN {raw_validation} OR {overflow_validation} THEN 1 ELSE 0 END"
            ));
        }

        let output_invalid_terms = summary
            .group_by
            .iter()
            .zip(&summary.group_dtypes)
            .filter(|(_, dtype)| **dtype == DataType::Float64)
            .map(|(column, _)| {
                let identifier = duckdb_identifier(column);
                format!(
                    "t.{identifier} IS NOT NULL AND NOT isfinite(CAST(t.{identifier} AS DOUBLE))"
                )
            })
            .chain(
                summary
                    .aggregations
                    .iter()
                    .filter(|aggregation| {
                        aggregation.dtype == DataType::Float64
                            && matches!(
                                aggregation.operation,
                                SummaryOperation::Sum
                                    | SummaryOperation::Mean
                                    | SummaryOperation::Min
                                    | SummaryOperation::Max
                            )
                    })
                    .map(|aggregation| {
                        let identifier = duckdb_identifier(&aggregation.output);
                        format!(
                            "t.{identifier} IS NOT NULL AND NOT isfinite(CAST(t.{identifier} AS DOUBLE))"
                        )
                    }),
            )
            .collect::<Vec<_>>();
        if !output_invalid_terms.is_empty() {
            output_validation = Some(format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM dataset AS t WHERE {}) THEN 1 ELSE 0 END",
                output_invalid_terms.join(" OR ")
            ));
        }
    }
    let selected = final_columns
        .iter()
        .map(|column| format!("t.{}", duckdb_identifier(column)))
        .collect::<Vec<_>>();
    let order_clause = final_order_column
        .map(|column| format!(" ORDER BY t.{}", duckdb_identifier(column)))
        .unwrap_or_default();
    let output = format!(
        "{final_ctes} SELECT {} FROM {final_source} AS t{order_clause}",
        selected.join(", ")
    );
    let replacement_count = if plan.replacement_columns.is_empty() {
        None
    } else {
        let replacement = recipe
            .find_replace
            .as_ref()
            .expect("las columnas de reemplazo requieren una receta de reemplazo");
        let changed_terms = plan
            .replacement_columns
            .iter()
            .map(|column| -> Result<String, String> {
                let identifier = duckdb_identifier(column);
                Ok(format!(
                    "CASE WHEN t.{identifier} IS DISTINCT FROM {} THEN 1 ELSE 0 END",
                    source_backed_replace_expression(&format!("t.{identifier}"), replacement,)?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Some(format!(
            "WITH renamed AS (SELECT {} FROM dataset), transformed AS (SELECT {} FROM renamed AS t){} SELECT CAST(COALESCE(SUM({}), 0) AS BIGINT) FROM {} AS t",
            renamed_expressions.join(", "),
            transformed_expressions.join(", "),
            filter_cte,
            changed_terms.join(" + "),
            filtered_source
        ))
    };
    let normalization_count = if plan.contact_normalizations.is_empty() {
        None
    } else {
        let changed_terms = plan
            .contact_normalizations
            .iter()
            .map(|normalization| {
                let identifier = duckdb_identifier(&normalization.column);
                format!(
                    "CASE WHEN t.{identifier} IS DISTINCT FROM {} THEN 1 ELSE 0 END",
                    source_backed_contact_normalization_expression(normalization)
                )
            })
            .collect::<Vec<_>>();
        Some(format!(
            "WITH renamed AS (SELECT {} FROM dataset), transformed AS (SELECT {} FROM renamed AS t){}{}{}{}{}{} SELECT CAST(COALESCE(SUM({}), 0) AS BIGINT) FROM {} AS t",
            renamed_expressions.join(", "),
            transformed_expressions.join(", "),
            filter_cte,
            replacement_cte,
            calculation_cte,
            split_cte,
            merge_cte,
            outlier_cte,
            changed_terms.join(" + "),
            contact_input_source
        ))
    };
    let outlier_validation = if plan.outlier_treatments.is_empty() {
        None
    } else {
        let mut invalid_terms = Vec::new();
        for (index, treatment) in plan.outlier_treatments.iter().enumerate() {
            let identifier = duckdb_identifier(&treatment.column);
            if treatment.dtype == DataType::Float64 {
                invalid_terms.push(format!(
                    "EXISTS (SELECT 1 FROM {outlier_input_source} AS t WHERE t.{identifier} IS NOT NULL AND NOT isfinite(CAST(t.{identifier} AS DOUBLE)))"
                ));
            } else {
                invalid_terms.push(format!(
                    "EXISTS (SELECT 1 FROM {outlier_input_source} AS t WHERE t.{identifier} IS NOT NULL AND abs(CAST(t.{identifier} AS DOUBLE)) > 9007199254740992)"
                ));
            }
            let q1 = source_backed_outlier_alias(index, "q1");
            let q3 = source_backed_outlier_alias(index, "q3");
            let median = source_backed_outlier_alias(index, "median");
            let lower = source_backed_outlier_alias(index, "lower");
            let upper = source_backed_outlier_alias(index, "upper");
            let valid_count = source_backed_outlier_alias(index, "valid_count");
            invalid_terms.push(format!(
                "EXISTS (SELECT 1 FROM outlier_stats AS s WHERE s.{valid_count} < 4 OR NOT isfinite(CAST(s.{q1} AS DOUBLE)) OR NOT isfinite(CAST(s.{q3} AS DOUBLE)) OR NOT isfinite(CAST(s.{median} AS DOUBLE)) OR NOT isfinite(CAST(s.{lower} AS DOUBLE)) OR NOT isfinite(CAST(s.{upper} AS DOUBLE)))"
            ));
        }
        Some(format!(
            "{pre_outlier_ctes}{outlier_quantile_cte}{outlier_stats_cte} SELECT CASE WHEN {} THEN 1 ELSE 0 END",
            invalid_terms.join(" OR ")
        ))
    };
    let outlier_adjusted_count = if plan.outlier_treatments.is_empty() {
        None
    } else {
        let changed_terms = plan
            .outlier_treatments
            .iter()
            .enumerate()
            .filter(|(_, treatment)| treatment.action != OutlierAction::Drop)
            .map(|(index, treatment)| {
                format!(
                    "CASE WHEN {} THEN 1 ELSE 0 END",
                    source_backed_outlier_condition(treatment, index)
                )
            })
            .collect::<Vec<_>>();
        if changed_terms.is_empty() {
            None
        } else {
            Some(format!(
                "{pre_outlier_ctes}{outlier_quantile_cte}{outlier_stats_cte} SELECT CAST(COALESCE(SUM({}), 0) AS BIGINT) FROM {outlier_input_source} AS t CROSS JOIN outlier_stats AS s",
                changed_terms.join(" + ")
            ))
        }
    };
    let outlier_removed_count = if plan.outlier_treatments.is_empty() {
        None
    } else {
        let drop_conditions = plan
            .outlier_treatments
            .iter()
            .enumerate()
            .filter(|(_, treatment)| treatment.action == OutlierAction::Drop)
            .map(|(index, treatment)| source_backed_outlier_condition(treatment, index))
            .collect::<Vec<_>>();
        if drop_conditions.is_empty() {
            None
        } else {
            Some(format!(
                "{pre_outlier_ctes}{outlier_quantile_cte}{outlier_stats_cte} SELECT CAST(COUNT(*) AS BIGINT) FROM {outlier_input_source} AS t CROSS JOIN outlier_stats AS s WHERE {}",
                drop_conditions.join(" OR ")
            ))
        }
    };
    let input_row_count = if plan.outlier_treatments.is_empty() && plan.group_summary.is_none() {
        None
    } else {
        Some(format!(
            "{pre_group_ctes} SELECT CAST(COUNT(*) AS BIGINT) FROM {filtered_source}"
        ))
    };
    Ok(SourceBackedProjectionQueries {
        output,
        replacement_count,
        normalization_count,
        outlier_adjusted_count,
        outlier_removed_count,
        outlier_validation,
        input_row_count,
        group_input_count,
        group_validation,
        output_validation,
        calculation_validation,
    })
}

struct SourceBackedProjectionQueries {
    output: String,
    replacement_count: Option<String>,
    normalization_count: Option<String>,
    outlier_adjusted_count: Option<String>,
    outlier_removed_count: Option<String>,
    outlier_validation: Option<String>,
    input_row_count: Option<String>,
    group_input_count: Option<String>,
    group_validation: Option<String>,
    output_validation: Option<String>,
    calculation_validation: Option<String>,
}

pub(super) fn apply_source_backed_projection_recipe_with_cancellation(
    dataset: &mut LoadedDataset,
    recipe: &TransformRecipe,
    cancellation: Option<&PrepareCancellation>,
) -> Result<TransformRecipeResult, String> {
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    let source_reference = dataset
        .source_path
        .as_deref()
        .ok_or_else(|| "La fuente source-backed ya no está disponible.".to_owned())?;
    let (original_source_path, source_size, original_extension) =
        validate_dataset_file(source_reference)?;
    if source_size != dataset.file_size_bytes {
        return Err("El archivo source-backed cambió después de la carga.".to_owned());
    }
    let (source_path, source_format, extension) = if let Some(snapshot_path) =
        dataset.history.source_snapshot_path.as_deref()
    {
        let (snapshot_path, _, snapshot_extension) = validate_dataset_file(snapshot_path)?;
        if snapshot_extension != "parquet" {
            return Err("El snapshot source-backed no es un Parquet válido.".to_owned());
        }
        (
            snapshot_path,
            crate::duckdb_query::DuckDbFileFormat::Parquet,
            snapshot_extension,
        )
    } else {
        let source_format = match original_extension.as_str() {
            "parquet" => crate::duckdb_query::DuckDbFileFormat::Parquet,
            "csv" | "tsv" | "txt" => {
                let delimiter = detect_delimiter(&original_source_path, &original_extension)?;
                crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter }
            }
            "json" | "jsonl" | "ndjson" => crate::duckdb_query::DuckDbFileFormat::Json,
            _ => {
                return Err("La receta source-backed requiere una fuente compatible.".to_owned());
            }
        };
        (
            original_source_path.clone(),
            source_format,
            original_extension.clone(),
        )
    };
    let schema = dataset.frame.clone();
    let plan = source_backed_projection_plan(&schema, recipe)?;
    let split_column_count = plan
        .split_columns
        .as_ref()
        .map_or(0, |split| split.names.len());
    let split_dropped_source_count = plan
        .split_columns
        .as_ref()
        .map_or(0, |split| usize::from(split.drop_source));
    let merged_column_count = plan.merge_columns.as_ref().map_or(0, |_| 1);
    let merge_dropped_source_count = plan.merge_columns.as_ref().map_or(0, |merge| {
        usize::from(merge.drop_sources) * merge.sources.len()
    });
    let normalized_contact_column_count = plan.contact_normalizations.len();
    let extracted_column_count = plan.text_extractions.len();
    let dropped_source_column_count = split_dropped_source_count + merge_dropped_source_count;
    let structural_change = plan.renamed_column_count
        + plan.converted_column_count
        + plan.parsed_date_column_count
        + plan.calculated_column_count
        + split_column_count
        + merged_column_count
        + extracted_column_count
        + dropped_source_column_count
        + plan.dropped_column_count
        + usize::from(plan.kept_order_changed)
        + usize::from(plan.group_summary.is_some())
        > 0;

    if !structural_change
        && recipe.filters.is_empty()
        && recipe.find_replace.is_none()
        && recipe.contact_normalizations.is_empty()
        && recipe.outlier_treatments.is_empty()
        && recipe.group_summary.is_none()
    {
        let page_frame = collect_lazy_frame_streaming(
            source_scan(&source_path, &extension)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa source-backed",
        )?;
        let expected_page_rows = dataset.row_count.min(PREVIEW_ROW_LIMIT);
        if page_frame.height() != expected_page_rows {
            return Err(
                "La fuente source-backed cambió durante la lectura de la vista previa.".to_owned(),
            );
        }
        let preview = dataset_preview_from_schema_and_page(
            &dataset.file_name,
            dataset.file_size_bytes,
            dataset.row_count,
            &schema,
            &page_frame,
        )?;
        if let Some(cancellation) = cancellation {
            cancellation.ensure()?;
        }
        return Ok(TransformRecipeResult {
            dataset: preview,
            renamed_column_count: plan.renamed_column_count,
            converted_column_count: plan.converted_column_count,
            parsed_date_column_count: plan.parsed_date_column_count,
            removed_row_count: 0,
            calculated_column_count: plan.calculated_column_count,
            replaced_cell_count: 0,
            dropped_column_count: plan.dropped_column_count,
            split_column_count,
            merged_column_count,
            dropped_source_column_count,
            adjusted_outlier_cell_count: 0,
            outlier_removed_row_count: 0,
            outlier_column_count: 0,
            group_count: 0,
            aggregated_column_count: 0,
            collapsed_row_count: 0,
            normalized_contact_cell_count: 0,
            normalized_contact_column_count,
            extracted_column_count,
            changed: false,
        });
    }

    let queries = source_backed_projection_query(&schema, recipe)?;
    if let Some(query) = queries.group_validation.as_deref() {
        let invalid = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
        if invalid != 0 {
            return Err(
                "El resumen source-backed contiene claves, agregaciones o sumas no válidas."
                    .to_owned(),
            );
        }
    }
    if let Some(query) = queries.calculation_validation.as_deref() {
        let invalid = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
        if invalid != 0 {
            return Err("La división source-backed contiene división por cero.".to_owned());
        }
    }
    if let Some(query) = queries.outlier_validation.as_deref() {
        let invalid = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
        if invalid != 0 {
            return Err(
                "Los tratamientos IQR source-backed requieren al menos cuatro valores finitos y dentro de precisión segura."
                    .to_owned(),
            );
        }
    }
    let scalar_count = |query: Option<&String>, message: &str| {
        query
            .map(|query| {
                let count =
                    crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
                usize::try_from(count).map_err(|_| message.to_owned())
            })
            .transpose()
    };
    let adjusted_outlier_cell_count = scalar_count(
        queries.outlier_adjusted_count.as_ref(),
        "El conteo de celdas atípicas ajustadas excede el límite de memoria.",
    )?
    .unwrap_or(0);
    let outlier_removed_row_count = scalar_count(
        queries.outlier_removed_count.as_ref(),
        "El conteo de filas atípicas retiradas excede el límite de memoria.",
    )?
    .unwrap_or(0);
    let filtered_input_row_count = scalar_count(
        queries.input_row_count.as_ref(),
        "El conteo de filas de entrada excede el límite de memoria.",
    )?;
    let group_summary_input_rows = queries
        .group_input_count
        .as_deref()
        .map(|query| {
            let count = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
            usize::try_from(count).map_err(|_| {
                "El conteo de filas para agrupar excede el límite de memoria.".to_owned()
            })
        })
        .transpose()?;
    let temporary =
        tempfile::NamedTempFile::with_suffix_in(".parquet", dataset.history.directory.path())
            .map_err(|error| {
                format!(
                    "No se pudo preparar la salida temporal de la receta source-backed: {error}"
                )
            })?;
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let mut output_guard = SourceBackedJoinOutputGuard::new(&output_path);
    let materialize_result = if let Some(cancellation) = cancellation {
        crate::duckdb_query::materialize_file_query_to_parquet_with_cancel(
            &source_path,
            source_format,
            &queries.output,
            &output_path,
            cancellation.callback(),
        )
    } else {
        crate::duckdb_query::materialize_file_query_to_parquet(
            &source_path,
            source_format,
            &queries.output,
            &output_path,
        )
    };
    if let Err(error) = materialize_result {
        let _ = fs::remove_file(&output_path);
        return Err(error);
    }

    let output_size = fs::metadata(&output_path)
        .map_err(|error| format!("No se pudo verificar la receta source-backed: {error}"))?
        .len();
    let output_schema = read_parquet_schema_frame(&output_path)?;
    let output_row_count = crate::duckdb_query::count_file_rows(
        &output_path,
        crate::duckdb_query::DuckDbFileFormat::Parquet,
        || false,
    )?;
    if output_row_count > dataset.row_count {
        let _ = fs::remove_file(&output_path);
        return Err(
            "La receta source-backed aumentó inesperadamente el conteo de filas.".to_owned(),
        );
    }
    if let Some(query) = queries.output_validation.as_deref() {
        let invalid = crate::duckdb_query::query_file_scalar(
            &output_path,
            crate::duckdb_query::DuckDbFileFormat::Parquet,
            query,
        )
        .inspect_err(|_| {
            let _ = fs::remove_file(&output_path);
        })?;
        if invalid != 0 {
            let _ = fs::remove_file(&output_path);
            return Err("El resumen source-backed produjo un valor numérico no finito.".to_owned());
        }
    }
    let replaced_cell_count = queries
        .replacement_count
        .as_deref()
        .map(|query| {
            let count = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
            usize::try_from(count)
                .map_err(|_| "El conteo de reemplazos excede el límite de memoria.".to_owned())
        })
        .transpose()?
        .unwrap_or(0);
    let normalized_contact_cell_count = queries
        .normalization_count
        .as_deref()
        .map(|query| {
            let count = crate::duckdb_query::query_file_scalar(&source_path, source_format, query)?;
            usize::try_from(count)
                .map_err(|_| "El conteo de normalizaciones excede el límite de memoria.".to_owned())
        })
        .transpose()?
        .unwrap_or(0);
    let (_, current_source_size, _) = validate_dataset_file(&original_source_path)?;
    if current_source_size != dataset.file_size_bytes {
        let _ = fs::remove_file(&output_path);
        return Err("El archivo source-backed cambió durante la receta.".to_owned());
    }
    let removed_row_count = filtered_input_row_count
        .map(|input_rows| dataset.row_count.saturating_sub(input_rows))
        .unwrap_or_else(|| dataset.row_count.saturating_sub(output_row_count));
    let group_count = usize::from(plan.group_summary.is_some()) * output_row_count;
    let aggregated_column_count = plan
        .group_summary
        .as_ref()
        .map_or(0, |summary| summary.aggregations.len());
    let collapsed_row_count = group_summary_input_rows
        .map(|input_rows| input_rows.saturating_sub(output_row_count))
        .unwrap_or(0);
    let changed = structural_change
        || removed_row_count > 0
        || replaced_cell_count > 0
        || normalized_contact_cell_count > 0
        || adjusted_outlier_cell_count > 0
        || outlier_removed_row_count > 0;
    if !changed {
        let _ = fs::remove_file(&output_path);
        let page_frame = collect_lazy_frame_streaming(
            source_scan(&source_path, &extension)?.slice(0, PREVIEW_ROW_LIMIT as IdxSize),
            "No se pudo leer la vista previa source-backed",
        )?;
        if let Some(cancellation) = cancellation {
            cancellation.ensure()?;
        }
        let preview = dataset_preview_from_schema_and_page(
            &dataset.file_name,
            dataset.file_size_bytes,
            dataset.row_count,
            &schema,
            &page_frame,
        )?;
        return Ok(TransformRecipeResult {
            dataset: preview,
            renamed_column_count: plan.renamed_column_count,
            converted_column_count: 0,
            parsed_date_column_count: 0,
            removed_row_count: 0,
            calculated_column_count: 0,
            replaced_cell_count: 0,
            dropped_column_count: plan.dropped_column_count,
            split_column_count,
            merged_column_count,
            dropped_source_column_count,
            adjusted_outlier_cell_count: 0,
            outlier_removed_row_count: 0,
            outlier_column_count: plan.outlier_treatments.len(),
            group_count: 0,
            aggregated_column_count: 0,
            collapsed_row_count: 0,
            normalized_contact_cell_count: 0,
            normalized_contact_column_count,
            extracted_column_count,
            changed: false,
        });
    }
    let page = dataset_page_from_parquet(&output_path, output_row_count, 0, PREVIEW_ROW_LIMIT)?;
    let page_frame = read_parquet_query_block(&output_path, 0, page.rows.len())?;
    let preview = dataset_preview_from_schema_and_page(
        &dataset.file_name,
        output_size,
        output_row_count,
        &output_schema,
        &page_frame,
    )?;
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    let baseline_snapshot = if dataset.history.snapshots_enabled {
        None
    } else {
        let cancellation_for_baseline = cancellation.cloned();
        let is_cancelled = move || {
            cancellation_for_baseline
                .as_ref()
                .is_some_and(PrepareCancellation::is_cancelled)
        };
        Some(prepare_source_backed_baseline_snapshot(
            &dataset.history,
            &source_path,
            source_format,
            is_cancelled,
        )?)
    };
    let prepared_output = dataset.history.prepare_parquet_snapshot(&output_path, || {
        cancellation.is_some_and(PrepareCancellation::is_cancelled)
    })?;
    if let Some(cancellation) = cancellation {
        cancellation.ensure()?;
    }
    let previous_source_path = dataset.source_path.clone();
    let previous_source_snapshot = dataset.history.source_snapshot_path.clone();
    let mut keep_output = false;
    let mut retired_paths = Vec::new();
    let publish = || {
        let history_commit = commit_source_backed_recipe_history(
            &mut dataset.history,
            baseline_snapshot,
            prepared_output,
            "Aplicar receta de transformación",
        )?;
        retired_paths = history_commit.retired_paths;
        let (active_source_path, active_file_size) =
            history_commit.active_snapshot.unwrap_or_else(|| {
                keep_output = true;
                (output_path.clone(), output_size)
            });

        dataset.source_path = Some(active_source_path.clone());
        dataset.file_size_bytes = active_file_size;
        dataset.row_count = output_row_count;
        dataset.frame = output_schema;
        dataset.source_backed = true;
        dataset.history.source_snapshot_path = None;
        dataset.profile = None;
        dataset.history.current_label = "Aplicar receta de transformación".to_owned();
        Ok(TransformRecipeResult {
            dataset: DatasetPreview {
                file_size_bytes: active_file_size,
                ..preview
            },
            renamed_column_count: plan.renamed_column_count,
            converted_column_count: plan.converted_column_count,
            parsed_date_column_count: plan.parsed_date_column_count,
            removed_row_count,
            calculated_column_count: plan.calculated_column_count,
            replaced_cell_count,
            dropped_column_count: plan.dropped_column_count,
            split_column_count,
            merged_column_count,
            dropped_source_column_count,
            adjusted_outlier_cell_count,
            outlier_removed_row_count,
            outlier_column_count: plan.outlier_treatments.len(),
            group_count,
            aggregated_column_count,
            collapsed_row_count,
            normalized_contact_cell_count,
            normalized_contact_column_count,
            extracted_column_count,
            changed,
        })
    };
    let result = if let Some(cancellation) = cancellation {
        cancellation.commit(publish)?
    } else {
        publish()?
    };
    for path in retired_paths.into_iter().chain(
        [previous_source_path, previous_source_snapshot]
            .into_iter()
            .flatten(),
    ) {
        if path.parent() == Some(dataset.history.directory.path())
            && dataset.source_path.as_deref() != Some(path.as_path())
            && !dataset
                .history
                .entries
                .iter()
                .any(|entry| entry.path == path)
        {
            let _ = fs::remove_file(path);
        }
    }
    if keep_output {
        output_guard.keep();
    }
    Ok(result)
}
