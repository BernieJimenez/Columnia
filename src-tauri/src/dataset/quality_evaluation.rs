use super::*;

pub(super) fn validate_quality_rule_definition(
    frame: &DataFrame,
    rule: &QualityRule,
) -> Result<(), String> {
    if rule.kind != QualityRuleKind::ColumnCompare && rule.operator.is_some() {
        return Err(format!(
            "El operator de '{}' solo aplica a column_compare.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::DateRange
        && (rule.min_date.is_some() || rule.max_date.is_some())
    {
        return Err(format!(
            "Los límites de fecha de '{}' solo aplican a date_range.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::Conditional && (rule.when.is_some() || rule.then.is_some()) {
        return Err(format!(
            "when y then solo aplican a la regla conditional de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::SchemaContract
        && (rule.allow_additional.is_some() || rule.required_order.is_some())
    {
        return Err(format!(
            "allowAdditional y requiredOrder solo aplican a schema_contract de '{}'.",
            rule.column
        ));
    }
    let is_aggregate_rule = matches!(
        rule.kind,
        QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation
    );
    let is_distribution_drift = rule.kind == QualityRuleKind::DistributionDrift;
    let is_aggregate_or_drift = is_aggregate_rule || is_distribution_drift;
    if !is_distribution_drift && (rule.baseline.is_some() || rule.threshold.is_some()) {
        return Err(format!(
            "baseline y threshold solo aplican a distribution_drift de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::ReferentialIntegrity
        && !is_aggregate_or_drift
        && rule.reference_values.is_some()
    {
        return Err(format!(
            "referenceValues solo aplica a referential_integrity, reglas agregadas o distribution_drift de '{}'.",
            rule.column
        ));
    }
    if rule.kind != QualityRuleKind::Monotonic && rule.direction.is_some() {
        return Err(format!(
            "direction solo aplica a monotonic de '{}'.",
            rule.column
        ));
    }
    if !is_aggregate_or_drift
        && (rule.expected.is_some()
            || rule.aggregate.is_some()
            || rule.tolerance_abs.is_some()
            || rule.tolerance_rel.is_some())
    {
        return Err(format!(
            "expected, aggregate y tolerancias numéricas solo aplican a reglas agregadas de '{}'.",
            rule.column
        ));
    }
    if rule
        .reference_values
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La regla de '{}' supera el máximo de {MAX_QUALITY_VALUES} referencias.",
            rule.column
        ));
    }
    if rule
        .baseline
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La línea base de '{}' supera el máximo de {MAX_QUALITY_VALUES} valores.",
            rule.column
        ));
    }
    if rule
        .values
        .as_ref()
        .is_some_and(|values| values.len() > MAX_QUALITY_VALUES)
    {
        return Err(format!(
            "La regla de '{}' supera el máximo de {MAX_QUALITY_VALUES} valores permitidos.",
            rule.column
        ));
    }
    if rule
        .columns
        .as_ref()
        .is_some_and(|columns| columns.is_empty() || columns.len() > MAX_QUALITY_COLUMNS_PER_RULE)
    {
        return Err(format!(
            "La regla de '{}' debe tener entre 1 y {MAX_QUALITY_COLUMNS_PER_RULE} columnas.",
            rule.column
        ));
    }
    let is_dataset_rule = rule.kind == QualityRuleKind::RowCount;
    let is_schema_rule = rule.kind == QualityRuleKind::SchemaContract;
    let is_dataset_level_rule = is_dataset_rule || is_schema_rule;
    if rule.column.trim().is_empty() && !is_dataset_level_rule {
        return Err("La columna de una regla de calidad no puede estar vacía.".to_owned());
    }
    if is_dataset_level_rule && rule.column != QUALITY_DATASET_COLUMN {
        return Err(format!(
            "La regla {} debe usar la columna lógica '{}'.",
            if is_schema_rule {
                "schema_contract"
            } else {
                "row_count"
            },
            QUALITY_DATASET_COLUMN,
        ));
    }
    let column = (!is_dataset_level_rule)
        .then(|| frame.column(&rule.column))
        .transpose()
        .map_err(|_| {
            format!(
                "La columna '{}' de la regla de calidad no existe.",
                rule.column
            )
        })?;
    if rule.max_invalid.is_none() && rule.max_invalid_pct.is_none() {
        return Err(format!(
            "La regla de '{}' debe indicar maxInvalid, maxInvalidPct o ambos.",
            rule.column
        ));
    }
    if let Some(value) = rule.max_invalid_pct {
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            return Err(format!(
                "maxInvalidPct de '{}' debe estar entre 0 y 100.",
                rule.column
            ));
        }
    }
    for (name, value) in [
        ("min", rule.min),
        ("max", rule.max),
        ("expected", rule.expected),
        ("toleranceAbs", rule.tolerance_abs),
        ("toleranceRel", rule.tolerance_rel),
        ("threshold", rule.threshold),
    ] {
        if value.is_some_and(|number| !number.is_finite()) {
            return Err(format!(
                "{name} de '{}' debe ser un número finito.",
                rule.column
            ));
        }
    }
    if rule.tolerance_abs.is_some_and(|value| value < 0.0)
        || rule.tolerance_rel.is_some_and(|value| value < 0.0)
        || rule.threshold.is_some_and(|value| value < 0.0)
    {
        return Err(format!(
            "Las tolerancias y umbral de '{}' deben ser mayores o iguales que cero.",
            rule.column
        ));
    }
    if rule
        .min
        .zip(rule.max)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(format!(
            "min no puede ser mayor que max en la regla de '{}'.",
            rule.column
        ));
    }
    match rule.kind {
        QualityRuleKind::NotNull | QualityRuleKind::Unique
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some() =>
        {
            Err(format!(
                "La regla '{}' no admite parámetros de otra comprobación.",
                rule.column
            ))
        }
        QualityRuleKind::NonEmpty
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some() =>
        {
            Err(format!(
                "La regla '{}' no admite parámetros de otra comprobación.",
                rule.column
            ))
        }
        QualityRuleKind::NonEmpty
            if column
                .as_ref()
                .is_none_or(|column| column.dtype() != &DataType::String) =>
        {
            Err(format!(
                "La regla non_empty solo admite columnas String; '{}' es {}.",
                rule.column,
                column.map_or_else(|| "dataset".to_owned(), |value| value.dtype().to_string())
            ))
        }
        QualityRuleKind::NumericRange
            if column.as_ref().is_none_or(|column| {
                !matches!(column.dtype(), DataType::Int64 | DataType::Float64)
            }) =>
        {
            Err(format!(
                "La regla numeric_range solo admite columnas Int64 o Float64; '{}' es {}.",
                rule.column,
                column.map_or_else(|| "dataset".to_owned(), |value| value.dtype().to_string())
            ))
        }
        QualityRuleKind::NumericRange if rule.min.is_none() && rule.max.is_none() => Err(format!(
            "La regla numeric_range de '{}' debe indicar min, max o ambos.",
            rule.column
        )),
        QualityRuleKind::NumericRange
            if column
                .as_ref()
                .is_some_and(|column| column.dtype() == &DataType::Int64) =>
        {
            for (name, value) in [("min", rule.min), ("max", rule.max)] {
                if value.is_some_and(|number| {
                    number.fract() != 0.0
                        || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&number)
                }) {
                    return Err(format!(
                        "{name} de '{}' debe ser un entero seguro para una columna Int64.",
                        rule.column
                    ));
                }
            }
            Ok(())
        }
        QualityRuleKind::NumericRange => {
            if rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla numeric_range de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::AllowedValues => {
            let column = column.ok_or_else(|| "allowed_values requiere una columna.".to_owned())?;
            let values = rule
                .values
                .as_ref()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla allowed_values de '{}' debe indicar al menos un valor.",
                        rule.column
                    )
                })?;
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La regla allowed_values solo admite columnas String; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla allowed_values de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            if values
                .iter()
                .any(|value| value.chars().count() > MAX_QUALITY_COLUMN_CHARS)
            {
                return Err(format!(
                    "La regla allowed_values de '{}' contiene un valor demasiado largo.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Regex => {
            let column = column.ok_or_else(|| "regex requiere una columna.".to_owned())?;
            if column.dtype() != &DataType::String {
                return Err(format!(
                    "La regla regex solo admite columnas String; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            let pattern = rule
                .pattern
                .as_deref()
                .filter(|pattern| !pattern.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla regex de '{}' debe indicar un patrón.",
                        rule.column
                    )
                })?;
            Regex::new(pattern).map_err(|error| {
                format!(
                    "El patrón regex de '{}' no es válido: {error}.",
                    rule.column
                )
            })?;
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla regex de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Dtype => {
            let column = column.ok_or_else(|| "dtype requiere una columna.".to_owned())?;
            let expected = rule
                .dtype
                .as_deref()
                .filter(|dtype| !dtype.trim().is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla dtype de '{}' debe indicar un tipo esperado.",
                        rule.column
                    )
                })?;
            if !quality_dtype_is_supported(expected) {
                return Err(format!(
                    "El tipo esperado '{}' de '{}' no está soportado.",
                    expected, rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla dtype de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            let _ = column;
            Ok(())
        }
        QualityRuleKind::UniqueTogether => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| columns.len() >= 2)
                .ok_or_else(|| {
                    format!(
                        "La regla unique_together de '{}' debe indicar al menos dos columnas.",
                        rule.column
                    )
                })?;
            for name in columns {
                frame.column(name).map_err(|_| {
                    format!(
                        "La columna '{}' de la regla unique_together no existe.",
                        name
                    )
                })?;
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
            {
                return Err(format!(
                    "La regla unique_together de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::ColumnCompare => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| columns.len() == 2)
                .ok_or_else(|| {
                    format!(
                        "La regla column_compare de '{}' debe indicar exactamente dos columnas.",
                        rule.column
                    )
                })?;
            if columns[0] != rule.column {
                return Err(format!(
                    "La primera columna de column_compare debe coincidir con '{}'.",
                    rule.column
                ));
            }
            if columns[0] == columns[1] {
                return Err(format!(
                    "La regla column_compare de '{}' necesita dos columnas distintas.",
                    rule.column
                ));
            }
            let left = frame.column(&columns[0]).map_err(|_| {
                format!(
                    "La columna '{}' de la regla column_compare no existe.",
                    columns[0]
                )
            })?;
            let right = frame.column(&columns[1]).map_err(|_| {
                format!(
                    "La columna '{}' de la regla column_compare no existe.",
                    columns[1]
                )
            })?;
            if left.dtype() != right.dtype() {
                return Err(format!(
                    "Las columnas de '{}' deben compartir tipo físico; {} y {} no coinciden.",
                    rule.column,
                    left.dtype(),
                    right.dtype()
                ));
            }
            let operator = rule.operator.ok_or_else(|| {
                format!(
                    "La regla column_compare de '{}' debe indicar operator.",
                    rule.column
                )
            })?;
            if matches!(
                operator,
                QualityComparison::Lt
                    | QualityComparison::Lte
                    | QualityComparison::Gt
                    | QualityComparison::Gte
            ) && !matches!(
                left.dtype(),
                DataType::String
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "El operator de orden de '{}' solo admite texto o columnas numéricas.",
                    rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
            {
                return Err(format!(
                    "La regla column_compare de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::ReferentialIntegrity => {
            let columns = rule
                .columns
                .as_ref()
                .filter(|columns| !columns.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla referential_integrity de '{}' debe indicar columns[].",
                        rule.column
                    )
                })?;
            if columns.len() > MAX_QUALITY_COLUMNS_PER_RULE {
                return Err(format!(
                    "La regla referential_integrity de '{}' supera el máximo de {MAX_QUALITY_COLUMNS_PER_RULE} columnas.",
                    rule.column
                ));
            }
            if columns[0] != rule.column {
                return Err(format!(
                    "La primera columna de referential_integrity debe coincidir con '{}'.",
                    rule.column
                ));
            }
            if columns.iter().any(|column| column.trim().is_empty()) {
                return Err("referential_integrity no admite nombres de columna vacíos.".to_owned());
            }
            if columns.iter().collect::<HashSet<_>>().len() != columns.len() {
                return Err(
                    "referential_integrity no admite columnas de clave duplicadas.".to_owned(),
                );
            }
            let key_columns = columns
                .iter()
                .map(|name| {
                    frame.column(name).map_err(|_| {
                        format!(
                            "La columna '{}' de la regla referential_integrity no existe.",
                            name
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if key_columns.iter().any(|column| {
                !matches!(
                    column.dtype(),
                    DataType::String
                        | DataType::Boolean
                        | DataType::Int8
                        | DataType::Int16
                        | DataType::Int32
                        | DataType::Int64
                        | DataType::UInt8
                        | DataType::UInt16
                        | DataType::UInt32
                        | DataType::UInt64
                        | DataType::Float32
                        | DataType::Float64
                )
            }) {
                return Err(
                    "referential_integrity solo admite columnas String, Boolean o numéricas."
                        .to_owned(),
                );
            }
            let references = rule
                .reference_values
                .as_ref()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| {
                    format!(
                        "La regla referential_integrity de '{}' debe indicar referenceValues[].",
                        rule.column
                    )
                })?;
            if references
                .iter()
                .any(|value| value.chars().count() > MAX_QUALITY_COLUMN_CHARS)
            {
                return Err(format!(
                    "La regla referential_integrity de '{}' contiene una referencia demasiado larga.",
                    rule.column
                ));
            }
            if references.iter().collect::<HashSet<_>>().len() != references.len() {
                return Err("referential_integrity no admite referencias duplicadas.".to_owned());
            }
            if columns.len() == 1 {
                if references.iter().any(|value| value.is_empty()) {
                    return Err(
                        "referential_integrity no admite referencias vacías para una columna."
                            .to_owned(),
                    );
                }
            } else {
                for reference in references {
                    let value = serde_json::from_str::<JsonValue>(reference).map_err(|_| {
                        format!(
                            "Cada referencia compuesta de '{}' debe ser un arreglo JSON.",
                            rule.column
                        )
                    })?;
                    let components = value.as_array().ok_or_else(|| {
                        format!(
                            "Cada referencia compuesta de '{}' debe ser un arreglo JSON.",
                            rule.column
                        )
                    })?;
                    if components.len() != columns.len()
                        || components.iter().any(|component| {
                            matches!(
                                component,
                                JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_)
                            )
                        })
                    {
                        return Err(format!(
                            "Cada referencia compuesta de '{}' debe contener {} escalares no nulos.",
                            rule.column,
                            columns.len()
                        ));
                    }
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla referential_integrity de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Monotonic => {
            let column = column.ok_or_else(|| "monotonic requiere una columna.".to_owned())?;
            if !matches!(
                column.dtype(),
                DataType::String
                    | DataType::Date
                    | DataType::Datetime(_, _)
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "La regla monotonic solo admite texto, fechas o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.reference_values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla monotonic de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::DistributionDrift => {
            let column = column.ok_or_else(|| {
                format!(
                    "La regla distribution_drift de '{}' requiere una columna.",
                    rule.column
                )
            })?;
            if !matches!(
                column.dtype(),
                DataType::String
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "distribution_drift solo admite texto numérico, booleanos o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            let baseline = rule
                .baseline
                .as_ref()
                .filter(|values| !values.is_empty())
                .or(rule
                    .reference_values
                    .as_ref()
                    .filter(|values| !values.is_empty()))
                .ok_or_else(|| {
                    format!(
                        "La regla distribution_drift de '{}' debe indicar baseline[].",
                        rule.column
                    )
                })?;
            if baseline
                .iter()
                .any(|value| quality_aggregate_text_value(value).is_none())
            {
                return Err(format!(
                    "La línea base de distribution_drift en '{}' debe contener números finitos.",
                    rule.column
                ));
            }
            if rule.expected.is_some()
                || rule.aggregate.is_some()
                || rule.tolerance_rel.is_some()
                || rule.values.is_some()
                || rule.min.is_some()
                || rule.max.is_some()
                || rule.direction.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla distribution_drift de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation => {
            let column = column.ok_or_else(|| {
                format!(
                    "La regla agregada de '{}' requiere una columna.",
                    rule.column
                )
            })?;
            let supports_aggregate = |value: &Column| {
                matches!(
                    value.dtype(),
                    DataType::String
                        | DataType::Boolean
                        | DataType::Int8
                        | DataType::Int16
                        | DataType::Int32
                        | DataType::Int64
                        | DataType::UInt8
                        | DataType::UInt16
                        | DataType::UInt32
                        | DataType::UInt64
                        | DataType::Float32
                        | DataType::Float64
                )
            };
            if !supports_aggregate(column) {
                return Err(format!(
                    "Las reglas agregadas solo admiten texto numérico, booleanos o columnas numéricas; '{}' es {}.",
                    rule.column,
                    column.dtype()
                ));
            }
            if rule
                .reference_values
                .as_ref()
                .is_some_and(|values| values.is_empty())
            {
                return Err(format!(
                    "La regla agregada de '{}' no admite referenceValues vacío.",
                    rule.column
                ));
            }
            if rule.reference_values.as_ref().is_some_and(|values| {
                values
                    .iter()
                    .any(|value| quality_aggregate_text_value(value).is_none())
            }) {
                return Err(format!(
                    "Las referencias agregadas de '{}' deben ser números finitos.",
                    rule.column
                ));
            }
            let has_expected = rule.expected.is_some();
            let has_references = rule
                .reference_values
                .as_ref()
                .is_some_and(|values| !values.is_empty());
            let has_column_pair = rule.kind == QualityRuleKind::AggregateReconciliation
                && rule
                    .columns
                    .as_ref()
                    .is_some_and(|columns| columns.len() >= 2);
            if !has_column_pair && !has_expected && !has_references {
                return Err(format!(
                    "La regla {} de '{}' necesita expected o referenceValues.",
                    if rule.kind == QualityRuleKind::AggregateCheck {
                        "aggregate_check"
                    } else {
                        "aggregate_reconciliation"
                    },
                    rule.column
                ));
            }
            if let Some(aggregate) = rule.aggregate {
                if rule.kind == QualityRuleKind::AggregateReconciliation
                    && rule
                        .columns
                        .as_ref()
                        .is_some_and(|columns| columns.len() >= 2)
                {
                    return Err(
                        "aggregate_reconciliation por columnas siempre compara sumas y no admite aggregate."
                            .to_owned(),
                    );
                }
                let _ = aggregate;
            }
            if rule.kind == QualityRuleKind::AggregateCheck && rule.columns.is_some() {
                return Err(
                    "aggregate_check no admite columns; selecciona una sola columna.".to_owned(),
                );
            }
            if rule.kind == QualityRuleKind::AggregateReconciliation {
                if let Some(columns) = rule.columns.as_ref() {
                    if columns.len() != 2 {
                        return Err(
                            "aggregate_reconciliation necesita exactamente dos columnas."
                                .to_owned(),
                        );
                    }
                    if columns[0] != rule.column {
                        return Err(
                            "La primera columna de aggregate_reconciliation debe coincidir con la columna principal."
                                .to_owned(),
                        );
                    }
                    if columns[0] == columns[1] {
                        return Err(
                            "aggregate_reconciliation necesita dos columnas distintas.".to_owned()
                        );
                    }
                    let right = frame.column(&columns[1]).map_err(|_| {
                        format!(
                            "La columna '{}' de aggregate_reconciliation no existe.",
                            columns[1]
                        )
                    })?;
                    if !supports_aggregate(right) {
                        return Err(format!(
                            "La columna '{}' de aggregate_reconciliation no es agregable.",
                            columns[1]
                        ));
                    }
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.direction.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
                || rule.when.is_some()
                || rule.then.is_some()
                || rule.allow_additional.is_some()
                || rule.required_order.is_some()
            {
                return Err(format!(
                    "La regla agregada de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::DateRange => {
            let column = column.ok_or_else(|| "date_range requiere una columna.".to_owned())?;
            if !matches!(
                column.dtype(),
                DataType::String | DataType::Date | DataType::Datetime(_, _)
            ) {
                return Err(format!(
                    "La regla date_range de '{}' solo admite texto, date o datetime.",
                    rule.column
                ));
            }
            let parse_bound = |value: Option<&str>, label: &str| {
                value
                    .map(|value| {
                        parse_quality_datetime(value).ok_or_else(|| {
                            format!(
                                "El límite {label} de date_range en '{}' no es una fecha válida.",
                                rule.column
                            )
                        })
                    })
                    .transpose()
            };
            let minimum = parse_bound(rule.min_date.as_deref(), "mínimo")?;
            let maximum = parse_bound(rule.max_date.as_deref(), "máximo")?;
            if minimum.is_none() && maximum.is_none() {
                return Err(format!(
                    "La regla date_range de '{}' debe indicar minDate, maxDate o ambos.",
                    rule.column
                ));
            }
            if minimum
                .zip(maximum)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            {
                return Err(format!(
                    "El mínimo de date_range en '{}' no puede superar el máximo.",
                    rule.column
                ));
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err(format!(
                    "La regla date_range de '{}' no admite parámetros adicionales.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::Conditional => {
            let condition = rule.when.as_ref().ok_or_else(|| {
                format!(
                    "La regla conditional de '{}' debe indicar when.",
                    rule.column
                )
            })?;
            let condition_column = frame.column(&condition.column).map_err(|_| {
                format!(
                    "La columna '{}' de when no existe en conditional.",
                    condition.column
                )
            })?;
            if !matches!(
                condition_column.dtype(),
                DataType::String
                    | DataType::Boolean
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "La condición de '{}' solo admite columnas String, numéricas o Boolean.",
                    rule.column
                ));
            }
            if condition.operator.is_none() {
                return Err(format!(
                    "La condición de '{}' debe indicar operator.",
                    rule.column
                ));
            }
            if matches!(
                condition.operator,
                Some(
                    QualityComparison::Lt
                        | QualityComparison::Lte
                        | QualityComparison::Gt
                        | QualityComparison::Gte
                )
            ) && !matches!(
                condition_column.dtype(),
                DataType::String
                    | DataType::Int8
                    | DataType::Int16
                    | DataType::Int32
                    | DataType::Int64
                    | DataType::UInt8
                    | DataType::UInt16
                    | DataType::UInt32
                    | DataType::UInt64
                    | DataType::Float32
                    | DataType::Float64
            ) {
                return Err(format!(
                    "El operator de orden de la condición de '{}' solo admite texto o columnas numéricas.",
                    rule.column
                ));
            }
            if condition.value.is_none() {
                return Err(format!(
                    "La condición de '{}' debe indicar value.",
                    rule.column
                ));
            }
            let then = rule.then.as_deref().ok_or_else(|| {
                format!(
                    "La regla conditional de '{}' debe indicar then.",
                    rule.column
                )
            })?;
            if !matches!(
                then.kind,
                QualityRuleKind::NotNull
                    | QualityRuleKind::NonEmpty
                    | QualityRuleKind::NumericRange
                    | QualityRuleKind::AllowedValues
                    | QualityRuleKind::Regex
                    | QualityRuleKind::Dtype
            ) {
                return Err(
                    "conditional solo admite subreglas then fila-a-fila: not_null, non_empty, numeric_range, allowed_values, regex o dtype.".to_owned(),
                );
            }
            if then.max_invalid != Some(0) || then.max_invalid_pct.is_some() {
                return Err(
                    "La tolerancia de la subregla then debe ser exactamente 0 inválidos."
                        .to_owned(),
                );
            }
            validate_quality_rule_definition(frame, then)?;
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
            {
                return Err(format!(
                    "La regla conditional de '{}' no admite parámetros de otra comprobación.",
                    rule.column
                ));
            }
            Ok(())
        }
        QualityRuleKind::SchemaContract => {
            let required = rule
                .columns
                .as_ref()
                .filter(|columns| !columns.is_empty())
                .ok_or_else(|| "La regla schema_contract necesita columns[].".to_owned())?;
            if required.iter().any(|column| column.trim().is_empty()) {
                return Err("schema_contract no admite nombres de columna vacíos.".to_owned());
            }
            let unique_required = required.iter().collect::<HashSet<_>>();
            if unique_required.len() != required.len() {
                return Err("schema_contract no admite columnas requeridas duplicadas.".to_owned());
            }
            if let Some(order) = rule.required_order.as_ref() {
                if order.len() > MAX_QUALITY_COLUMNS_PER_RULE {
                    return Err(format!(
                        "requiredOrder de schema_contract supera el máximo de {MAX_QUALITY_COLUMNS_PER_RULE} columnas."
                    ));
                }
                if order.is_empty() || order.iter().any(|column| column.trim().is_empty()) {
                    return Err(
                        "requiredOrder de schema_contract debe contener nombres no vacíos."
                            .to_owned(),
                    );
                }
                let unique_order = order.iter().collect::<HashSet<_>>();
                if unique_order.len() != order.len() {
                    return Err(
                        "requiredOrder de schema_contract no admite columnas duplicadas."
                            .to_owned(),
                    );
                }
            }
            if rule.min.is_some()
                || rule.max.is_some()
                || rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.operator.is_some()
                || rule.min_date.is_some()
                || rule.max_date.is_some()
            {
                return Err(
                    "La regla schema_contract no admite parámetros de otra comprobación."
                        .to_owned(),
                );
            }
            Ok(())
        }
        QualityRuleKind::RowCount => {
            if rule.min.is_none() && rule.max.is_none() {
                return Err("La regla row_count debe indicar min, max o ambos.".to_owned());
            }
            if rule.values.is_some()
                || rule.pattern.is_some()
                || rule.dtype.is_some()
                || rule.columns.is_some()
            {
                return Err("La regla row_count no admite parámetros adicionales.".to_owned());
            }
            Ok(())
        }
        QualityRuleKind::NotNull | QualityRuleKind::NonEmpty | QualityRuleKind::Unique => Ok(()),
    }
}

fn quality_dtype_is_supported(expected: &str) -> bool {
    matches!(
        expected.trim().to_ascii_lowercase().as_str(),
        "string"
            | "text"
            | "integer"
            | "int"
            | "int64"
            | "float"
            | "decimal"
            | "float64"
            | "boolean"
            | "bool"
            | "date"
            | "datetime"
            | "timestamp"
    )
}

fn quality_dtype_matches(actual: &DataType, expected: &str) -> bool {
    match expected.trim().to_ascii_lowercase().as_str() {
        "string" | "text" => actual == &DataType::String,
        "integer" | "int" | "int64" => matches!(
            actual,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        ),
        "float" | "decimal" | "float64" => matches!(actual, DataType::Float32 | DataType::Float64),
        "boolean" | "bool" => actual == &DataType::Boolean,
        "date" => actual == &DataType::Date,
        "datetime" | "timestamp" => matches!(actual, DataType::Datetime(_, _)),
        _ => false,
    }
}

fn ensure_quality_row_not_cancelled<C>(row_index: usize, is_cancelled: &C) -> Result<(), String>
where
    C: Fn() -> bool,
{
    if row_index.is_multiple_of(LOCAL_QUERY_CANCEL_CHECK_ROWS) {
        ensure_not_cancelled(is_cancelled())?;
    }
    Ok(())
}

fn duplicate_combination_count(frame: &DataFrame, columns: &[String]) -> Result<usize, String> {
    duplicate_combination_count_with_cancel(frame, columns, &|| false)
}

fn duplicate_combination_count_with_cancel<C>(
    frame: &DataFrame,
    columns: &[String],
    is_cancelled: &C,
) -> Result<usize, String>
where
    C: Fn() -> bool,
{
    let mut seen = HashSet::new();
    let mut duplicate_count = 0;
    for row_index in 0..frame.height() {
        ensure_quality_row_not_cancelled(row_index, is_cancelled)?;
        let key = columns
            .iter()
            .map(|name| {
                frame
                    .column(name)
                    .and_then(|column| column.get(row_index))
                    .map(|value| format!("{value:?}"))
                    .map_err(|error| format!("No se pudo evaluar unique_together: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !seen.insert(key) {
            duplicate_count += 1;
        }
    }
    ensure_not_cancelled(is_cancelled())?;
    Ok(duplicate_count)
}

fn quality_value_ordering(left: AnyValue<'_>, right: AnyValue<'_>) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (AnyValue::Int8(left), AnyValue::Int8(right)) => Some(left.cmp(&right)),
        (AnyValue::Int16(left), AnyValue::Int16(right)) => Some(left.cmp(&right)),
        (AnyValue::Int32(left), AnyValue::Int32(right)) => Some(left.cmp(&right)),
        (AnyValue::Int64(left), AnyValue::Int64(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt8(left), AnyValue::UInt8(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt16(left), AnyValue::UInt16(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt32(left), AnyValue::UInt32(right)) => Some(left.cmp(&right)),
        (AnyValue::UInt64(left), AnyValue::UInt64(right)) => Some(left.cmp(&right)),
        (AnyValue::Float32(left), AnyValue::Float32(right)) => left.partial_cmp(&right),
        (AnyValue::Float64(left), AnyValue::Float64(right)) => left.partial_cmp(&right),
        (AnyValue::Boolean(left), AnyValue::Boolean(right)) => Some(left.cmp(&right)),
        (AnyValue::String(left), AnyValue::String(right)) => Some(left.cmp(right)),
        (AnyValue::String(left), AnyValue::StringOwned(right)) => Some(left.cmp(right.as_str())),
        (AnyValue::StringOwned(left), AnyValue::String(right)) => Some(left.as_str().cmp(right)),
        (AnyValue::StringOwned(left), AnyValue::StringOwned(right)) => {
            Some(left.as_str().cmp(right.as_str()))
        }
        _ => None,
    }
}

pub(super) fn quality_datetime_value(value: AnyValue<'_>) -> Option<NaiveDateTime> {
    match value {
        AnyValue::String(value) => {
            parse_quality_datetime(value).or_else(|| parse_supported_datetime(value))
        }
        AnyValue::StringOwned(value) => parse_quality_datetime(value.as_str())
            .or_else(|| parse_supported_datetime(value.as_str())),
        AnyValue::Date(days) => NaiveDate::from_ymd_opt(1970, 1, 1)
            .and_then(|epoch| epoch.checked_add_signed(chrono::Duration::days(days.into())))
            .and_then(|date| date.and_hms_opt(0, 0, 0)),
        AnyValue::Datetime(raw, unit, _) => {
            let divisor = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            DateTime::from_timestamp(
                raw.div_euclid(divisor),
                (raw.rem_euclid(divisor) as u64 * (1_000_000_000 / divisor as u64)) as u32,
            )
            .map(|date| date.naive_utc())
        }
        _ => None,
    }
}

fn quality_comparison_matches(
    left: AnyValue<'_>,
    right: AnyValue<'_>,
    operator: QualityComparison,
) -> bool {
    match operator {
        QualityComparison::Eq => format!("{left:?}") == format!("{right:?}"),
        QualityComparison::Ne => format!("{left:?}") != format!("{right:?}"),
        QualityComparison::Lt => quality_value_ordering(left, right)
            .is_some_and(|ordering| ordering == std::cmp::Ordering::Less),
        QualityComparison::Lte => quality_value_ordering(left, right).is_some_and(|ordering| {
            matches!(
                ordering,
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal
            )
        }),
        QualityComparison::Gt => quality_value_ordering(left, right)
            .is_some_and(|ordering| ordering == std::cmp::Ordering::Greater),
        QualityComparison::Gte => quality_value_ordering(left, right).is_some_and(|ordering| {
            matches!(
                ordering,
                std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
            )
        }),
    }
}

fn quality_comparison_ordering_matches(
    ordering: std::cmp::Ordering,
    operator: QualityComparison,
) -> bool {
    match operator {
        QualityComparison::Eq => ordering == std::cmp::Ordering::Equal,
        QualityComparison::Ne => ordering != std::cmp::Ordering::Equal,
        QualityComparison::Lt => ordering == std::cmp::Ordering::Less,
        QualityComparison::Lte => {
            matches!(
                ordering,
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal
            )
        }
        QualityComparison::Gt => ordering == std::cmp::Ordering::Greater,
        QualityComparison::Gte => matches!(
            ordering,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
        ),
    }
}

fn quality_condition_matches(value: AnyValue<'_>, condition: &QualityCondition) -> bool {
    let Some(operator) = condition.operator else {
        return false;
    };
    let Some(expected) = condition.value.as_deref() else {
        return false;
    };
    match value {
        AnyValue::String(actual) => {
            quality_comparison_ordering_matches(actual.cmp(expected), operator)
        }
        AnyValue::StringOwned(actual) => {
            quality_comparison_ordering_matches(actual.as_str().cmp(expected), operator)
        }
        AnyValue::Boolean(actual) => expected.parse::<bool>().is_ok_and(|expected| {
            quality_comparison_ordering_matches(actual.cmp(&expected), operator)
        }),
        AnyValue::Null => false,
        value if quality_numeric_value(value.clone()).is_some() => {
            let actual = quality_numeric_value(value).expect("se verificó el tipo numérico");
            expected.parse::<f64>().ok().is_some_and(|expected| {
                actual
                    .partial_cmp(&expected)
                    .is_some_and(|ordering| quality_comparison_ordering_matches(ordering, operator))
            })
        }
        _ => false,
    }
}

fn quality_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    match value {
        AnyValue::Int8(value) => Some(value as f64),
        AnyValue::Int16(value) => Some(value as f64),
        AnyValue::Int32(value) => Some(value as f64),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::UInt8(value) => Some(value as f64),
        AnyValue::UInt16(value) => Some(value as f64),
        AnyValue::UInt32(value) => Some(value as f64),
        AnyValue::UInt64(value) => Some(value as f64),
        AnyValue::Float32(value) => Some(value as f64),
        AnyValue::Float64(value) => Some(value),
        _ => None,
    }
}

fn quality_reference_scalar_matches(value: AnyValue<'_>, expected: &str) -> bool {
    match value {
        AnyValue::String(actual) => actual == expected,
        AnyValue::StringOwned(actual) => actual.as_str() == expected,
        AnyValue::Boolean(actual) => expected
            .parse::<bool>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int8(actual) => expected
            .parse::<i8>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int16(actual) => expected
            .parse::<i16>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int32(actual) => expected
            .parse::<i32>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Int64(actual) => expected
            .parse::<i64>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt8(actual) => expected
            .parse::<u8>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt16(actual) => expected
            .parse::<u16>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt32(actual) => expected
            .parse::<u32>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::UInt64(actual) => expected
            .parse::<u64>()
            .is_ok_and(|expected| actual == expected),
        AnyValue::Float32(actual) => expected
            .parse::<f32>()
            .is_ok_and(|expected| expected.is_finite() && actual.is_finite() && actual == expected),
        AnyValue::Float64(actual) => expected
            .parse::<f64>()
            .is_ok_and(|expected| expected.is_finite() && actual.is_finite() && actual == expected),
        AnyValue::Null => false,
        _ => false,
    }
}

fn quality_reference_json_component_matches(value: AnyValue<'_>, expected: &JsonValue) -> bool {
    match expected {
        JsonValue::String(expected) => quality_reference_scalar_matches(value, expected),
        JsonValue::Bool(expected) => quality_reference_scalar_matches(value, &expected.to_string()),
        JsonValue::Number(expected) => {
            quality_reference_scalar_matches(value, &expected.to_string())
        }
        JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => false,
    }
}

pub(super) fn quality_monotonic_ordering(
    left: AnyValue<'_>,
    right: AnyValue<'_>,
) -> Option<std::cmp::Ordering> {
    if matches!(&left, AnyValue::Date(_) | AnyValue::Datetime(_, _, _))
        || matches!(&right, AnyValue::Date(_) | AnyValue::Datetime(_, _, _))
    {
        return quality_datetime_value(left)
            .zip(quality_datetime_value(right))
            .map(|(left, right)| left.cmp(&right));
    }
    quality_value_ordering(left, right)
}

fn quality_aggregate_text_value(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

pub(super) fn quality_aggregate_numeric_value(value: AnyValue<'_>) -> Option<f64> {
    match value {
        AnyValue::Int8(value) => Some(value as f64),
        AnyValue::Int16(value) => Some(value as f64),
        AnyValue::Int32(value) => Some(value as f64),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::UInt8(value) => Some(value as f64),
        AnyValue::UInt16(value) => Some(value as f64),
        AnyValue::UInt32(value) => Some(value as f64),
        AnyValue::UInt64(value) => (value as f64).is_finite().then_some(value as f64),
        AnyValue::UInt128(value) => (value as f64).is_finite().then_some(value as f64),
        AnyValue::Float32(value) => value.is_finite().then_some(value as f64),
        AnyValue::Float64(value) => value.is_finite().then_some(value),
        AnyValue::Int128(value) => (value as f64).is_finite().then_some(value as f64),
        AnyValue::Boolean(value) => Some(if value { 1.0 } else { 0.0 }),
        AnyValue::String(value) => quality_aggregate_text_value(value),
        AnyValue::StringOwned(value) => quality_aggregate_text_value(value.as_str()),
        _ => None,
    }
}

pub(super) fn quality_aggregate_observation<C>(
    column: &Column,
    row_count: usize,
    is_cancelled: &C,
) -> Result<(usize, f64, Option<f64>, Option<f64>), String>
where
    C: Fn() -> bool,
{
    let mut count = 0;
    let mut sum = 0.0;
    let mut minimum = None;
    let mut maximum = None;
    for row_index in 0..row_count {
        if row_index % 1024 == 0 {
            ensure_not_cancelled(is_cancelled())?;
        }
        let value = column.get(row_index).map_err(|error| error.to_string())?;
        let Some(number) = quality_aggregate_numeric_value(value) else {
            continue;
        };
        count += 1;
        sum += number;
        minimum = Some(minimum.map_or(number, |current: f64| current.min(number)));
        maximum = Some(maximum.map_or(number, |current: f64| current.max(number)));
    }
    Ok((count, sum, minimum, maximum))
}

fn quality_aggregate_expected(rule: &QualityRule, aggregate: QualityAggregate) -> Option<f64> {
    if let Some(expected) = rule.expected {
        return Some(expected);
    }
    let references = rule.reference_values.as_deref()?;
    if aggregate == QualityAggregate::Sum {
        let mut total = 0.0;
        for reference in references {
            total += quality_aggregate_text_value(reference)?;
        }
        Some(total)
    } else {
        references
            .first()
            .and_then(|reference| quality_aggregate_text_value(reference))
    }
}

fn quality_aggregate_tolerance(rule: &QualityRule, reference: f64) -> f64 {
    let relative = rule
        .tolerance_rel
        .map_or(0.0, |value| reference.abs() * value);
    rule.tolerance_abs.unwrap_or(0.0).max(relative)
}

fn quality_distribution_baseline_mean(rule: &QualityRule) -> Option<f64> {
    let values = rule
        .baseline
        .as_deref()
        .filter(|values| !values.is_empty())
        .or(rule
            .reference_values
            .as_deref()
            .filter(|values| !values.is_empty()))?;
    let mut count = 0usize;
    let mut sum = 0.0;
    for value in values {
        let number = quality_aggregate_text_value(value)?;
        count += 1;
        sum += number;
    }
    (count > 0).then_some(sum / count as f64)
}

fn quality_conditional_row_invalid(
    frame: &DataFrame,
    row_index: usize,
    rule: &QualityRule,
) -> Result<bool, String> {
    let column = frame
        .column(&rule.column)
        .map_err(|error| format!("No se pudo evaluar conditional.then: {error}"))?;
    let value = column
        .get(row_index)
        .map_err(|error| format!("No se pudo leer conditional.then: {error}"))?;
    match rule.kind {
        QualityRuleKind::NotNull => Ok(matches!(value, AnyValue::Null)),
        QualityRuleKind::NonEmpty => Ok(match value {
            AnyValue::String(text) => text.trim().is_empty(),
            AnyValue::StringOwned(text) => text.trim().is_empty(),
            AnyValue::Null => true,
            _ => true,
        }),
        QualityRuleKind::NumericRange => {
            let number = quality_numeric_value(value);
            Ok(number.is_none_or(|number| {
                !number.is_finite()
                    || rule.min.is_some_and(|minimum| number < minimum)
                    || rule.max.is_some_and(|maximum| number > maximum)
            }))
        }
        QualityRuleKind::AllowedValues => {
            let allowed = rule.values.as_ref().expect("values validados");
            Ok(match value {
                AnyValue::String(text) => !allowed.iter().any(|item| item == text),
                AnyValue::StringOwned(text) => !allowed.iter().any(|item| item == text.as_str()),
                AnyValue::Null => true,
                _ => true,
            })
        }
        QualityRuleKind::Regex => {
            let regex = Regex::new(rule.pattern.as_deref().expect("pattern validado"))
                .expect("pattern validado");
            Ok(match value {
                AnyValue::String(text) => !regex.is_match(text),
                AnyValue::StringOwned(text) => !regex.is_match(text.as_str()),
                AnyValue::Null => true,
                _ => true,
            })
        }
        QualityRuleKind::Dtype => Ok(!quality_dtype_matches(
            column.dtype(),
            rule.dtype.as_deref().expect("dtype validado"),
        )),
        _ => Err(
            "conditional contiene una subregla then que no es fila-a-fila o no fue validada."
                .to_owned(),
        ),
    }
}

fn quality_rule_result(
    rule: &QualityRule,
    checked_count: usize,
    invalid_count: usize,
) -> QualityRuleResult {
    let invalid_pct = if checked_count == 0 {
        0.0
    } else {
        invalid_count as f64 * 100.0 / checked_count as f64
    };
    let passed = rule
        .max_invalid
        .is_none_or(|maximum| invalid_count <= maximum)
        && rule
            .max_invalid_pct
            .is_none_or(|maximum| invalid_pct <= maximum);
    QualityRuleResult {
        column: rule.column.clone(),
        kind: rule.kind,
        max_invalid: rule.max_invalid,
        max_invalid_pct: rule.max_invalid_pct,
        min: rule.min,
        max: rule.max,
        values: rule.values.clone(),
        reference_values: rule.reference_values.clone(),
        baseline: rule.baseline.clone(),
        direction: rule.direction,
        expected: rule.expected,
        aggregate: rule.aggregate,
        tolerance_abs: rule.tolerance_abs,
        tolerance_rel: rule.tolerance_rel,
        threshold: rule.threshold,
        pattern: rule.pattern.clone(),
        dtype: rule.dtype.clone(),
        columns: rule.columns.clone(),
        operator: rule.operator,
        min_date: rule.min_date.clone(),
        max_date: rule.max_date.clone(),
        when: rule.when.clone(),
        then: rule.then.clone(),
        allow_additional: rule.allow_additional,
        required_order: rule.required_order.clone(),
        checked_count,
        invalid_count,
        invalid_pct,
        passed,
    }
}

pub(super) fn source_quality_rule_is_incremental(rule: &QualityRule) -> bool {
    matches!(
        rule.kind,
        QualityRuleKind::NotNull
            | QualityRuleKind::NonEmpty
            | QualityRuleKind::NumericRange
            | QualityRuleKind::AllowedValues
            | QualityRuleKind::Regex
            | QualityRuleKind::Dtype
            | QualityRuleKind::ColumnCompare
            | QualityRuleKind::ReferentialIntegrity
            | QualityRuleKind::DateRange
            | QualityRuleKind::Conditional
            | QualityRuleKind::SchemaContract
            | QualityRuleKind::RowCount
            | QualityRuleKind::Unique
            | QualityRuleKind::UniqueTogether
            | QualityRuleKind::Monotonic
            | QualityRuleKind::DistributionDrift
            | QualityRuleKind::AggregateCheck
            | QualityRuleKind::AggregateReconciliation
    )
}

pub(super) fn evaluate_source_quality_rules_with_cancel<C>(
    source_path: &Path,
    extension: &str,
    expected_file_size: u64,
    row_count: usize,
    quality_rules: &[QualityRule],
    is_cancelled: C,
) -> Result<QualityValidationResult, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    let (_snapshot_directory, snapshot_path) =
        source_profile_snapshot_with_cancel(source_path, extension, is_cancelled.clone())?;
    ensure_not_cancelled(is_cancelled())?;
    let schema = read_parquet_schema_frame(&snapshot_path)?;

    for rule in quality_rules {
        ensure_not_cancelled(is_cancelled())?;
        validate_quality_rule_definition(&schema, rule)?;
    }

    let mut counts = vec![(0_usize, 0_usize); quality_rules.len()];
    let streaming_rule_indices = quality_rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            !matches!(
                rule.kind,
                QualityRuleKind::Dtype
                    | QualityRuleKind::SchemaContract
                    | QualityRuleKind::RowCount
                    | QualityRuleKind::Unique
                    | QualityRuleKind::UniqueTogether
                    | QualityRuleKind::Monotonic
                    | QualityRuleKind::DistributionDrift
                    | QualityRuleKind::AggregateCheck
                    | QualityRuleKind::AggregateReconciliation
            )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    for (index, rule) in quality_rules.iter().enumerate() {
        ensure_not_cancelled(is_cancelled())?;
        match rule.kind {
            QualityRuleKind::RowCount => {
                let minimum_ok = rule.min.is_none_or(|minimum| row_count as f64 >= minimum);
                let maximum_ok = rule.max.is_none_or(|maximum| row_count as f64 <= maximum);
                counts[index] = (1, usize::from(!(minimum_ok && maximum_ok)));
            }
            QualityRuleKind::Dtype | QualityRuleKind::SchemaContract => {
                let result = evaluate_quality_rules_with_cancel(
                    &schema,
                    std::slice::from_ref(rule),
                    &is_cancelled,
                )?;
                let rule_result = result.rules.first().ok_or_else(|| {
                    "La validación source-backed no devolvió la regla esperada.".to_owned()
                })?;
                counts[index] = (rule_result.checked_count, rule_result.invalid_count);
            }
            QualityRuleKind::Unique => {
                let invalid_count = count_unique_invalid_from_parquet(
                    &snapshot_path,
                    row_count,
                    &rule.column,
                    &is_cancelled,
                )?;
                counts[index] = (row_count, invalid_count);
            }
            QualityRuleKind::UniqueTogether => {
                let columns = rule.columns.as_deref().expect("columns validadas");
                let invalid_count = count_duplicate_combinations_from_parquet(
                    &snapshot_path,
                    row_count,
                    columns,
                    &is_cancelled,
                )?;
                counts[index] = (row_count, invalid_count);
            }
            QualityRuleKind::Monotonic => {
                let invalid_count = count_monotonic_invalid_from_parquet(
                    &snapshot_path,
                    row_count,
                    &rule.column,
                    rule.direction
                        .unwrap_or(QualityMonotonicDirection::Increasing),
                    &is_cancelled,
                )?;
                counts[index] = (row_count, invalid_count);
            }
            QualityRuleKind::DistributionDrift => {
                let (count, sum, _, _) = quality_aggregate_observation_from_parquet(
                    &snapshot_path,
                    row_count,
                    &rule.column,
                    &is_cancelled,
                )?;
                let observed_mean = if count == 0 { 0.0 } else { sum / count as f64 };
                let baseline_mean = quality_distribution_baseline_mean(rule)
                    .expect("baseline validado para distribution_drift");
                let threshold = rule.tolerance_abs.or(rule.threshold).unwrap_or(0.0);
                counts[index] = (
                    row_count,
                    usize::from((observed_mean - baseline_mean).abs() > threshold),
                );
            }
            QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation => {
                let is_reconciliation = rule.kind == QualityRuleKind::AggregateReconciliation;
                let has_column_pair = is_reconciliation
                    && rule
                        .columns
                        .as_ref()
                        .is_some_and(|columns| columns.len() >= 2);
                if has_column_pair {
                    let columns = rule.columns.as_deref().expect("columns validadas");
                    let (_, left_sum, _, _) = quality_aggregate_observation_from_parquet(
                        &snapshot_path,
                        row_count,
                        &columns[0],
                        &is_cancelled,
                    )?;
                    let (_, right_sum, _, _) = quality_aggregate_observation_from_parquet(
                        &snapshot_path,
                        row_count,
                        &columns[1],
                        &is_cancelled,
                    )?;
                    let reference = left_sum.abs().max(right_sum.abs());
                    let tolerance = quality_aggregate_tolerance(rule, reference);
                    counts[index] = (
                        row_count,
                        usize::from((left_sum - right_sum).abs() > tolerance),
                    );
                } else {
                    let aggregate = rule.aggregate.unwrap_or(QualityAggregate::Sum);
                    let (count, sum, minimum, maximum) =
                        quality_aggregate_observation_from_parquet(
                            &snapshot_path,
                            row_count,
                            &rule.column,
                            &is_cancelled,
                        )?;
                    let observed = match aggregate {
                        QualityAggregate::Count => Some(count as f64),
                        QualityAggregate::Sum => Some(sum),
                        QualityAggregate::Min => minimum,
                        QualityAggregate::Max => maximum,
                    };
                    let expected = quality_aggregate_expected(rule, aggregate)
                        .expect("expected o referenceValues validados");
                    let tolerance = quality_aggregate_tolerance(rule, expected);
                    counts[index] = (
                        row_count,
                        usize::from(
                            observed.is_none_or(|value| (value - expected).abs() > tolerance),
                        ),
                    );
                }
            }
            _ => {}
        }
    }

    if !streaming_rule_indices.is_empty() {
        let streaming_rules = streaming_rule_indices
            .iter()
            .map(|index| quality_rules[*index].clone())
            .collect::<Vec<_>>();
        let mut streaming_counts = vec![(0_usize, 0_usize); streaming_rules.len()];
        for_each_parquet_block_with_cancel(
            &snapshot_path,
            row_count,
            &is_cancelled,
            |_, block| {
                ensure_not_cancelled(is_cancelled())?;
                let result =
                    evaluate_quality_rules_with_cancel(block, &streaming_rules, &is_cancelled)?;
                for (index, rule_result) in result.rules.iter().enumerate() {
                    streaming_counts[index].0 = streaming_counts[index]
                        .0
                        .saturating_add(rule_result.checked_count);
                    streaming_counts[index].1 = streaming_counts[index]
                        .1
                        .saturating_add(rule_result.invalid_count);
                }
                Ok(())
            },
        )?;
        for (streaming_index, rule_index) in streaming_rule_indices.iter().enumerate() {
            counts[*rule_index] = streaming_counts[streaming_index];
        }
    }

    ensure_not_cancelled(is_cancelled())?;
    let results = quality_rules
        .iter()
        .enumerate()
        .map(|(index, rule)| quality_rule_result(rule, counts[index].0, counts[index].1))
        .collect::<Vec<_>>();
    let failed_rules = results.iter().filter(|result| !result.passed).count();
    let final_size = fs::metadata(source_path)
        .map_err(|error| format!("No se pudieron verificar los metadatos source-backed: {error}"))?
        .len();
    if final_size != expected_file_size {
        return Err("El archivo source-backed cambió durante la validación de calidad.".to_owned());
    }
    Ok(QualityValidationResult {
        passed: failed_rules == 0,
        row_count,
        total_rules: results.len(),
        failed_rules,
        rules: results,
    })
}

pub(super) fn enforce_source_quality_with_cancel<C>(
    source_path: &Path,
    extension: &str,
    expected_file_size: u64,
    row_count: usize,
    quality_rules: &[QualityRule],
    allow_unvalidated: bool,
    is_cancelled: C,
) -> Result<Option<QualityValidationResult>, String>
where
    C: Fn() -> bool + Clone + Send + Sync + 'static,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    if quality_rules.is_empty() {
        return if allow_unvalidated {
            Ok(None)
        } else {
            Err("La exportación sin reglas de calidad requiere confirmación explícita.".to_owned())
        };
    }
    let validation = evaluate_source_quality_rules_with_cancel(
        source_path,
        extension,
        expected_file_size,
        row_count,
        quality_rules,
        is_cancelled,
    )?;
    if !validation.passed {
        return Err(format!(
            "La exportación fue bloqueada: {} de {} reglas de calidad fallaron.",
            validation.failed_rules, validation.total_rules
        ));
    }
    Ok(Some(validation))
}

pub(super) fn evaluate_quality_rules(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
) -> Result<QualityValidationResult, String> {
    evaluate_quality_rules_with_cancel(frame, quality_rules, || false)
}

pub(super) fn evaluate_quality_rules_with_cancel<C>(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    is_cancelled: C,
) -> Result<QualityValidationResult, String>
where
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    for rule in quality_rules {
        ensure_not_cancelled(is_cancelled())?;
        validate_quality_rule_definition(frame, rule)?;
    }

    let row_count = frame.height();
    let mut results = Vec::with_capacity(quality_rules.len());
    for rule in quality_rules {
        ensure_not_cancelled(is_cancelled())?;
        let (checked_count, invalid_count) = match rule.kind {
            QualityRuleKind::Conditional => {
                let condition = rule.when.as_ref().expect("when validado");
                let then = rule.then.as_deref().expect("then validado");
                let condition_column = frame
                    .column(&condition.column)
                    .map_err(|error| error.to_string())?;
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                    let condition_value = condition_column
                        .get(row_index)
                        .map_err(|error| error.to_string())?;
                    if !matches!(condition_value, AnyValue::Null)
                        && quality_condition_matches(condition_value, condition)
                        && quality_conditional_row_invalid(frame, row_index, then)?
                    {
                        invalid_count += 1;
                    }
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::DateRange => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let minimum = rule.min_date.as_deref().and_then(parse_quality_datetime);
                let maximum = rule.max_date.as_deref().and_then(parse_quality_datetime);
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                    let value = column.get(row_index).ok().and_then(quality_datetime_value);
                    if value.is_none_or(|value| {
                        minimum.is_some_and(|minimum| value < minimum)
                            || maximum.is_some_and(|maximum| value > maximum)
                    }) {
                        invalid_count += 1;
                    }
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::SchemaContract => {
                let required = rule.columns.as_deref().expect("columns validadas");
                let actual = frame
                    .get_column_names()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let missing_count = required
                    .iter()
                    .filter(|required_name| !actual.iter().any(|name| name == *required_name))
                    .count();
                let additional_count = if rule.allow_additional.unwrap_or(true) {
                    0
                } else {
                    actual
                        .iter()
                        .filter(|name| {
                            !required
                                .iter()
                                .any(|required_name| *required_name == **name)
                        })
                        .count()
                };
                let order_count = rule.required_order.as_ref().map_or(0, |required_order| {
                    usize::from(actual.as_slice() != required_order.as_slice())
                });
                (1, missing_count + additional_count + order_count)
            }
            QualityRuleKind::RowCount => {
                let minimum_ok = rule.min.is_none_or(|minimum| row_count as f64 >= minimum);
                let maximum_ok = rule.max.is_none_or(|maximum| row_count as f64 <= maximum);
                (1, usize::from(!(minimum_ok && maximum_ok)))
            }
            QualityRuleKind::Dtype => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let expected = rule.dtype.as_deref().expect("dtype validado");
                (
                    1,
                    usize::from(!quality_dtype_matches(column.dtype(), expected)),
                )
            }
            QualityRuleKind::UniqueTogether => (
                row_count,
                duplicate_combination_count_with_cancel(
                    frame,
                    rule.columns.as_deref().expect("columns validadas"),
                    &is_cancelled,
                )?,
            ),
            QualityRuleKind::ColumnCompare => {
                let columns = rule.columns.as_deref().expect("columns validadas");
                let left = frame
                    .column(&columns[0])
                    .map_err(|error| error.to_string())?;
                let right = frame
                    .column(&columns[1])
                    .map_err(|error| error.to_string())?;
                let operator = rule.operator.expect("operator validado");
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                    let left_value = left.get(row_index).map_err(|error| error.to_string())?;
                    let right_value = right.get(row_index).map_err(|error| error.to_string())?;
                    if matches!(left_value, AnyValue::Null)
                        || matches!(right_value, AnyValue::Null)
                        || !quality_comparison_matches(left_value, right_value, operator)
                    {
                        invalid_count += 1;
                    }
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::ReferentialIntegrity => {
                let columns = rule.columns.as_deref().expect("columns validadas");
                let references = rule
                    .reference_values
                    .as_deref()
                    .expect("referenceValues validados");
                let key_columns = columns
                    .iter()
                    .map(|name| frame.column(name).map_err(|error| error.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let invalid_count = if key_columns.len() == 1 {
                    let column = key_columns[0];
                    let mut invalid_count = 0;
                    for row_index in 0..row_count {
                        ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                        let valid = column.get(row_index).ok().is_some_and(|value| {
                            references.iter().any(|reference| {
                                quality_reference_scalar_matches(value.clone(), reference)
                            })
                        });
                        invalid_count += usize::from(!valid);
                    }
                    invalid_count
                } else {
                    let parsed_references = references
                        .iter()
                        .map(|reference| {
                            serde_json::from_str::<JsonValue>(reference)
                                .expect("referencias compuestas validadas")
                        })
                        .collect::<Vec<_>>();
                    let mut invalid_count = 0;
                    for row_index in 0..row_count {
                        ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                        let values = key_columns
                            .iter()
                            .map(|column| column.get(row_index))
                            .collect::<Result<Vec<_>, _>>();
                        let Ok(values) = values else {
                            invalid_count += 1;
                            continue;
                        };
                        let valid = parsed_references.iter().any(|reference| {
                            let Some(components) = reference.as_array() else {
                                return false;
                            };
                            components.len() == values.len()
                                && values.iter().zip(components).all(|(value, expected)| {
                                    quality_reference_json_component_matches(
                                        value.clone(),
                                        expected,
                                    )
                                })
                        });
                        invalid_count += usize::from(!valid);
                    }
                    invalid_count
                };
                (row_count, invalid_count)
            }
            QualityRuleKind::Monotonic => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let direction = rule
                    .direction
                    .unwrap_or(QualityMonotonicDirection::Increasing);
                let mut previous: Option<AnyValue<'_>> = None;
                let mut invalid_count = 0;
                for row_index in 0..row_count {
                    ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                    let value = column.get(row_index).map_err(|error| error.to_string())?;
                    if matches!(&value, AnyValue::Null) {
                        previous = None;
                        continue;
                    }
                    if let Some(previous_value) = previous.as_ref() {
                        let invalid =
                            match quality_monotonic_ordering(previous_value.clone(), value.clone())
                            {
                                Some(ordering) => match direction {
                                    QualityMonotonicDirection::Increasing => {
                                        ordering == std::cmp::Ordering::Greater
                                    }
                                    QualityMonotonicDirection::Decreasing => {
                                        ordering == std::cmp::Ordering::Less
                                    }
                                },
                                None => true,
                            };
                        invalid_count += usize::from(invalid);
                    }
                    previous = Some(value);
                }
                (row_count, invalid_count)
            }
            QualityRuleKind::DistributionDrift => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let (count, sum, _, _) =
                    quality_aggregate_observation(column, row_count, &is_cancelled)?;
                let observed_mean = if count == 0 { 0.0 } else { sum / count as f64 };
                let baseline_mean = quality_distribution_baseline_mean(rule)
                    .expect("baseline validado para distribution_drift");
                let threshold = rule.tolerance_abs.or(rule.threshold).unwrap_or(0.0);
                (
                    row_count,
                    usize::from((observed_mean - baseline_mean).abs() > threshold),
                )
            }
            QualityRuleKind::AggregateCheck | QualityRuleKind::AggregateReconciliation => {
                let is_reconciliation = rule.kind == QualityRuleKind::AggregateReconciliation;
                let has_column_pair = is_reconciliation
                    && rule
                        .columns
                        .as_ref()
                        .is_some_and(|columns| columns.len() >= 2);
                if has_column_pair {
                    let columns = rule.columns.as_deref().expect("columns validadas");
                    let left = frame
                        .column(&columns[0])
                        .map_err(|error| error.to_string())?;
                    let right = frame
                        .column(&columns[1])
                        .map_err(|error| error.to_string())?;
                    let (_, left_sum, _, _) =
                        quality_aggregate_observation(left, row_count, &is_cancelled)?;
                    let (_, right_sum, _, _) =
                        quality_aggregate_observation(right, row_count, &is_cancelled)?;
                    let reference = left_sum.abs().max(right_sum.abs());
                    let tolerance = quality_aggregate_tolerance(rule, reference);
                    (
                        row_count,
                        usize::from((left_sum - right_sum).abs() > tolerance),
                    )
                } else {
                    let column = frame
                        .column(&rule.column)
                        .map_err(|error| error.to_string())?;
                    let aggregate = rule.aggregate.unwrap_or(QualityAggregate::Sum);
                    let (count, sum, minimum, maximum) =
                        quality_aggregate_observation(column, row_count, &is_cancelled)?;
                    let observed = match aggregate {
                        QualityAggregate::Count => Some(count as f64),
                        QualityAggregate::Sum => Some(sum),
                        QualityAggregate::Min => minimum,
                        QualityAggregate::Max => maximum,
                    };
                    let expected = quality_aggregate_expected(rule, aggregate)
                        .expect("expected o referenceValues validados");
                    let tolerance = quality_aggregate_tolerance(rule, expected);
                    let invalid = usize::from(
                        observed.is_none_or(|value| (value - expected).abs() > tolerance),
                    );
                    (row_count, invalid)
                }
            }
            _ => {
                let column = frame
                    .column(&rule.column)
                    .map_err(|error| error.to_string())?;
                let invalid_count = match rule.kind {
                    QualityRuleKind::NotNull => column.null_count(),
                    QualityRuleKind::NonEmpty => {
                        let values = column.str().map_err(|error| error.to_string())?;
                        let mut invalid_count = 0;
                        for (row_index, value) in values.iter().enumerate() {
                            ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                            invalid_count +=
                                usize::from(value.is_none_or(|text| text.trim().is_empty()));
                        }
                        invalid_count
                    }
                    QualityRuleKind::Unique => {
                        let distinct_including_null = column
                            .n_unique()
                            .map_err(|error| format!("No se pudo evaluar unique: {error}"))?;
                        ensure_not_cancelled(is_cancelled())?;
                        let distinct_non_null = distinct_including_null
                            .saturating_sub(usize::from(column.null_count() > 0));
                        row_count.saturating_sub(distinct_non_null)
                    }
                    QualityRuleKind::NumericRange => match column.dtype() {
                        DataType::Int64 => {
                            let values = column.i64().map_err(|error| error.to_string())?;
                            let mut invalid_count = 0;
                            for (row_index, value) in values.iter().enumerate() {
                                ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                                invalid_count += usize::from(value.is_none_or(|number| {
                                    rule.min.is_some_and(|minimum| number < minimum as i64)
                                        || rule.max.is_some_and(|maximum| number > maximum as i64)
                                }));
                            }
                            invalid_count
                        }
                        DataType::Float64 => {
                            let values = column.f64().map_err(|error| error.to_string())?;
                            let mut invalid_count = 0;
                            for (row_index, value) in values.iter().enumerate() {
                                ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                                invalid_count += usize::from(value.is_none_or(|number| {
                                    !number.is_finite()
                                        || rule.min.is_some_and(|minimum| number < minimum)
                                        || rule.max.is_some_and(|maximum| number > maximum)
                                }));
                            }
                            invalid_count
                        }
                        _ => unreachable!("el tipo numérico ya fue validado"),
                    },
                    QualityRuleKind::AllowedValues => {
                        let allowed = rule.values.as_ref().expect("values validados");
                        let values = column.str().map_err(|error| error.to_string())?;
                        let mut invalid_count = 0;
                        for (row_index, value) in values.iter().enumerate() {
                            ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                            invalid_count += usize::from(
                                value.is_none_or(|text| !allowed.iter().any(|item| item == text)),
                            );
                        }
                        invalid_count
                    }
                    QualityRuleKind::Regex => {
                        let pattern = rule.pattern.as_deref().expect("pattern validado");
                        let regex = Regex::new(pattern).expect("pattern validado");
                        let values = column.str().map_err(|error| error.to_string())?;
                        let mut invalid_count = 0;
                        for (row_index, value) in values.iter().enumerate() {
                            ensure_quality_row_not_cancelled(row_index, &is_cancelled)?;
                            invalid_count +=
                                usize::from(value.is_none_or(|text| !regex.is_match(text)));
                        }
                        invalid_count
                    }
                    QualityRuleKind::Dtype
                    | QualityRuleKind::UniqueTogether
                    | QualityRuleKind::ColumnCompare
                    | QualityRuleKind::ReferentialIntegrity
                    | QualityRuleKind::Monotonic
                    | QualityRuleKind::AggregateCheck
                    | QualityRuleKind::AggregateReconciliation
                    | QualityRuleKind::DistributionDrift
                    | QualityRuleKind::DateRange
                    | QualityRuleKind::Conditional
                    | QualityRuleKind::SchemaContract
                    | QualityRuleKind::RowCount => unreachable!("la regla se evaluó arriba"),
                };
                (row_count, invalid_count)
            }
        };
        ensure_not_cancelled(is_cancelled())?;
        results.push(quality_rule_result(rule, checked_count, invalid_count));
    }
    ensure_not_cancelled(is_cancelled())?;
    let failed_rules = results.iter().filter(|result| !result.passed).count();
    Ok(QualityValidationResult {
        passed: failed_rules == 0,
        row_count,
        total_rules: results.len(),
        failed_rules,
        rules: results,
    })
}

pub(super) fn enforce_export_quality_with_cancel<C>(
    frame: &DataFrame,
    quality_rules: &[QualityRule],
    allow_unvalidated: bool,
    is_cancelled: C,
) -> Result<Option<QualityValidationResult>, String>
where
    C: Fn() -> bool,
{
    ensure_not_cancelled(is_cancelled())?;
    validate_quality_rules_payload(quality_rules)?;
    if quality_rules.is_empty() {
        return if allow_unvalidated {
            Ok(None)
        } else {
            Err("La exportación sin reglas de calidad requiere confirmación explícita.".to_owned())
        };
    }
    let validation = evaluate_quality_rules_with_cancel(frame, quality_rules, is_cancelled)?;
    if !validation.passed {
        return Err(format!(
            "La exportación fue bloqueada: {} de {} reglas de calidad fallaron.",
            validation.failed_rules, validation.total_rules
        ));
    }
    Ok(Some(validation))
}
