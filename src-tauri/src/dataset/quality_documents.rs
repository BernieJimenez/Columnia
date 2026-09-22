use super::*;

fn migration_validate_quality_metadata(
    map: &JsonMap<String, JsonValue>,
    label: &str,
) -> Result<(), String> {
    if migration_has_true_nullable(map)? {
        return Err(format!(
            "{label} declara nullable/allowNulls=true, pero el contrato Columnia no puede conservar esa política sin degradar la regla."
        ));
    }
    if [
        "reference_revision",
        "referenceRevision",
        "reference_dataset",
        "referenceDataset",
    ]
    .iter()
    .any(|key| map.get(*key).is_some_and(|value| !value.is_null()))
    {
        return Err(format!(
            "{label} usa una referencia externa (referenceRevision/referenceDataset) que Columnia no puede resolver de forma segura."
        ));
    }
    Ok(())
}

fn migration_number_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<f64>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let number = value
        .as_f64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<f64>().ok())
        })
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("El campo '{key}' debe ser un número finito."))?;
    Ok(Some(number))
}

fn migration_usize_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<usize>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let number = value
        .as_u64()
        .or_else(|| {
            value.as_f64().and_then(|number| {
                (number.is_finite() && number >= 0.0 && number.fract() == 0.0)
                    .then_some(number as u64)
            })
        })
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| format!("El campo '{key}' debe ser un entero no negativo."))?;
    Ok(Some(number))
}

fn migration_string_array(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Result<Option<Vec<String>>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("El campo '{key}' debe ser una lista."))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Todos los elementos de '{key}' deben ser texto."))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(values))
}

fn migration_reference_values(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
    column_count: usize,
) -> Result<Option<Vec<String>>, String> {
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| map.get(*key).map(|value| (*key, value)))
    else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("El campo '{key}' debe ser una lista."))?;
    if values.len() > MAX_QUALITY_VALUES {
        return Err(format!(
            "El campo '{key}' supera el máximo de {MAX_QUALITY_VALUES} valores."
        ));
    }
    values
        .iter()
        .map(|value| {
            if column_count == 1 {
                match value {
                    JsonValue::String(value) => Ok(value.clone()),
                    JsonValue::Bool(value) => Ok(value.to_string()),
                    JsonValue::Number(value) => Ok(value.to_string()),
                    JsonValue::Null => Err(format!(
                        "Los elementos de '{key}' deben ser valores escalares no nulos."
                    )),
                    JsonValue::Array(_) | JsonValue::Object(_) => Err(format!(
                        "Los elementos de '{key}' deben ser escalares para una columna."
                    )),
                }
            } else {
                let components = value.as_array().ok_or_else(|| {
                    format!(
                        "Los elementos de '{key}' deben ser arreglos para una clave compuesta."
                    )
                })?;
                if components.len() != column_count
                    || components.iter().any(|component| {
                        matches!(component, JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_))
                    })
                {
                    return Err(format!(
                        "Cada referencia de '{key}' debe contener {column_count} escalares no nulos."
                    ));
                }
                serde_json::to_string(value).map_err(|error| {
                    format!("No se pudo normalizar un valor de '{key}': {error}")
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn migration_quality_kind(value: &str) -> Option<QualityRuleKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "not_null" => Some(QualityRuleKind::NotNull),
        "non_empty" => Some(QualityRuleKind::NonEmpty),
        "unique" => Some(QualityRuleKind::Unique),
        "numeric_range" | "range" => Some(QualityRuleKind::NumericRange),
        "allowed_values" => Some(QualityRuleKind::AllowedValues),
        "regex" => Some(QualityRuleKind::Regex),
        "dtype" => Some(QualityRuleKind::Dtype),
        "unique_together" => Some(QualityRuleKind::UniqueTogether),
        "column_compare" | "column_comparison" => Some(QualityRuleKind::ColumnCompare),
        "referential_integrity" | "referential" => Some(QualityRuleKind::ReferentialIntegrity),
        "monotonic" => Some(QualityRuleKind::Monotonic),
        "aggregate_check" | "aggregate" => Some(QualityRuleKind::AggregateCheck),
        "aggregate_reconciliation" | "aggregate_reconcile" | "reconciliation" => {
            Some(QualityRuleKind::AggregateReconciliation)
        }
        "distribution_drift" | "drift" => Some(QualityRuleKind::DistributionDrift),
        "date_range" => Some(QualityRuleKind::DateRange),
        "conditional" => Some(QualityRuleKind::Conditional),
        "schema_contract" | "schema" => Some(QualityRuleKind::SchemaContract),
        "row_count" => Some(QualityRuleKind::RowCount),
        _ => None,
    }
}

fn migration_quality_monotonic_direction(value: &str) -> Option<QualityMonotonicDirection> {
    match value.trim().to_ascii_lowercase().as_str() {
        "increasing" | "increase" | "asc" | "ascending" => {
            Some(QualityMonotonicDirection::Increasing)
        }
        "decreasing" | "decrease" | "desc" | "descending" => {
            Some(QualityMonotonicDirection::Decreasing)
        }
        _ => None,
    }
}

fn migration_quality_aggregate(value: &str) -> Option<QualityAggregate> {
    match value.trim().to_ascii_lowercase().as_str() {
        "count" | "counts" | "n" => Some(QualityAggregate::Count),
        "sum" | "total" => Some(QualityAggregate::Sum),
        "min" | "minimum" => Some(QualityAggregate::Min),
        "max" | "maximum" => Some(QualityAggregate::Max),
        _ => None,
    }
}

fn migration_quality_comparison(value: &str) -> Option<QualityComparison> {
    match value.trim().to_ascii_lowercase().as_str() {
        "eq" | "equal" | "equals" => Some(QualityComparison::Eq),
        "ne" | "neq" | "not_equal" => Some(QualityComparison::Ne),
        "lt" | "less_than" => Some(QualityComparison::Lt),
        "lte" | "le" | "less_or_equal" => Some(QualityComparison::Lte),
        "gt" | "greater_than" => Some(QualityComparison::Gt),
        "gte" | "ge" | "greater_or_equal" => Some(QualityComparison::Gte),
        _ => None,
    }
}

fn migration_condition_value(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn migration_quality_condition(
    value: Option<&JsonValue>,
) -> Result<Option<QualityCondition>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let map = value
        .as_object()
        .ok_or_else(|| "conditional necesita un objeto when.".to_owned())?;
    let column = migration_string_field(map, &["column"])
        .filter(|column| !column.trim().is_empty())
        .ok_or_else(|| "conditional.when necesita column.".to_owned())?;
    let operator = match migration_string_field(map, &["operator", "op"]) {
        Some(value) => Some(migration_quality_comparison(&value).ok_or_else(|| {
            "conditional.when necesita operator eq, ne, lt, lte, gt o gte.".to_owned()
        })?),
        None => Some(QualityComparison::Eq),
    };
    let condition_value = map
        .get("value")
        .or_else(|| map.get("val"))
        .and_then(migration_condition_value);
    if condition_value.is_none() {
        return Err("conditional.when necesita value.".to_owned());
    }
    Ok(Some(QualityCondition {
        column,
        operator,
        value: condition_value,
    }))
}

fn migrate_conditional_then(
    value: Option<&JsonValue>,
    fallback_column: &str,
) -> Result<QualityRule, String> {
    let map = value
        .and_then(JsonValue::as_object)
        .ok_or_else(|| "conditional necesita un objeto then.".to_owned())?;
    let source_kind =
        migration_string_field(map, &["kind", "type"]).unwrap_or_else(|| "not_null".to_owned());
    let kind = migration_quality_kind(&source_kind).ok_or_else(|| {
        "La subregla then de conditional todavía no tiene representación equivalente.".to_owned()
    })?;
    if !matches!(
        kind,
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
    let severity = migration_severity_value(map)?;
    if severity != "blocking" {
        return Err(format!(
            "La severidad de la subregla then ({severity}) no tiene equivalente seguro en Columnia."
        ));
    }
    for (key, expected, label) in [
        ("on_missing", "fail", "La política on_missing"),
        ("null_policy", "invalid", "La política de nulos"),
    ] {
        let value = migration_policy_value(map, key, expected)?;
        if value != expected {
            return Err(format!(
                "{label} de la subregla then ({value}) no tiene equivalente seguro en Columnia."
            ));
        }
    }
    migration_validate_quality_metadata(map, "La subregla then")?;
    let column = migration_string_field(map, &["column"])
        .filter(|column| !column.trim().is_empty())
        .unwrap_or_else(|| fallback_column.to_owned());
    let mut rule = QualityRule {
        column,
        kind,
        max_invalid: Some(0),
        max_invalid_pct: None,
        min: None,
        max: None,
        values: None,
        reference_values: None,
        baseline: None,
        direction: None,
        expected: None,
        aggregate: None,
        tolerance_abs: None,
        tolerance_rel: None,
        threshold: None,
        pattern: None,
        dtype: None,
        columns: None,
        operator: None,
        min_date: None,
        max_date: None,
        when: None,
        then: None,
        allow_additional: None,
        required_order: None,
    };
    match kind {
        QualityRuleKind::NumericRange => {
            rule.min = migration_number_field(map, &["min", "min_value", "minValue"])?;
            rule.max = migration_number_field(map, &["max", "max_value", "maxValue"])?;
            if rule.min.is_none() && rule.max.is_none() {
                return Err("La subregla numeric_range necesita min o max.".to_owned());
            }
        }
        QualityRuleKind::AllowedValues => {
            rule.values = migration_reference_values(map, &["values"], 1)?;
            if rule.values.as_ref().is_none_or(Vec::is_empty) {
                return Err("La subregla allowed_values necesita values[].".to_owned());
            }
        }
        QualityRuleKind::Regex => {
            rule.pattern = migration_string_field(map, &["pattern"]);
            if rule.pattern.as_deref().is_none_or(str::is_empty) {
                return Err("La subregla regex necesita pattern.".to_owned());
            }
        }
        QualityRuleKind::Dtype => {
            let source_dtype = migration_string_field(map, &["dtype"])
                .ok_or_else(|| "La subregla dtype necesita dtype.".to_owned())?;
            rule.dtype = migration_dtype(&source_dtype).map(str::to_owned);
            if rule.dtype.is_none() {
                return Err("La subregla dtype no contiene un tipo soportado.".to_owned());
            }
        }
        QualityRuleKind::NotNull | QualityRuleKind::NonEmpty => {}
        _ => unreachable!("conditional then fue validado arriba"),
    }
    Ok(rule)
}

pub(super) fn parse_quality_datetime(value: &str) -> Option<NaiveDateTime> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    parse_recipe_datetime(value, RecipeDateFormat::Iso8601)
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%d/%m/%Y")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y/%m/%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })
}

fn migration_dtype(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "string" | "text" | "str" => Some("string"),
        "integer" | "int" | "int64" => Some("integer"),
        "numeric" | "number" | "float" | "float64" | "decimal" => Some("float"),
        "boolean" | "bool" => Some("boolean"),
        "date" => Some("date"),
        "datetime" | "timestamp" => Some("datetime"),
        _ => None,
    }
}

fn migration_warning(
    rule_index: usize,
    source_kind: &str,
    severity: &'static str,
    message: impl Into<String>,
) -> QualityMigrationWarning {
    QualityMigrationWarning {
        rule_index,
        source_kind: source_kind.to_owned(),
        severity,
        message: message.into(),
    }
}

fn quality_migration_report(
    artifact_sha256: Option<String>,
    total_items: usize,
    converted_items: usize,
    omitted_items: usize,
    warnings: &[QualityMigrationWarning],
) -> QualityMigrationReport {
    let mut manual_actions = vec!["Validar el contrato convertido antes de exportar.".to_owned()];
    if omitted_items > 0 {
        manual_actions.push(
            "Revisar las reglas omitidas y recrearlas manualmente si siguen siendo necesarias."
                .to_owned(),
        );
    }
    if warnings.iter().any(|warning| warning.severity == "warning") {
        manual_actions.push(
            "Revisar las tolerancias ajustadas o asumidas frente al documento original.".to_owned(),
        );
    }
    QualityMigrationReport {
        artifact_sha256,
        total_items,
        converted_items,
        omitted_items,
        warning_count: warnings.len(),
        manual_actions,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyQualityRulesDocument {
    version: u8,
    rules: Vec<QualityRule>,
}

pub(super) fn build_quality_rules_document(
    rules: Vec<QualityRule>,
) -> Result<QualityRulesDocument, String> {
    let document = QualityRulesDocument {
        format: QUALITY_RULES_DOCUMENT_FORMAT.to_owned(),
        version: QUALITY_RULES_DOCUMENT_VERSION,
        rules,
    };
    validate_quality_rules_document(&document)?;
    Ok(document)
}

fn validate_quality_rules_document(document: &QualityRulesDocument) -> Result<(), String> {
    if document.format != QUALITY_RULES_DOCUMENT_FORMAT {
        return Err(format!(
            "El formato del contrato debe ser '{QUALITY_RULES_DOCUMENT_FORMAT}'."
        ));
    }
    if document.version != QUALITY_RULES_DOCUMENT_VERSION {
        return Err(format!(
            "La versión {} del contrato de calidad no es compatible; Columnia admite la versión {QUALITY_RULES_DOCUMENT_VERSION}.",
            document.version
        ));
    }
    validate_quality_rules_payload(&document.rules)?;
    let encoded = serde_json::to_vec(document)
        .map_err(|error| format!("No se pudo validar el contrato de calidad: {error}"))?;
    if encoded.len() as u64 > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    Ok(())
}

pub(super) fn parse_quality_rules_document(
    document: JsonValue,
    allow_legacy_v1: bool,
) -> Result<QualityRulesDocument, String> {
    if document
        .as_object()
        .is_some_and(|map| map.contains_key("format"))
    {
        let document = serde_json::from_value::<QualityRulesDocument>(document)
            .map_err(|error| format!("El contrato Columnia no es válido: {error}"))?;
        validate_quality_rules_document(&document)?;
        return Ok(document);
    }
    if !allow_legacy_v1 {
        return Err(format!(
            "El contrato Columnia debe declarar format='{QUALITY_RULES_DOCUMENT_FORMAT}'."
        ));
    }
    let legacy = serde_json::from_value::<LegacyQualityRulesDocument>(document)
        .map_err(|error| format!("El contrato de calidad v1 no es válido: {error}"))?;
    if legacy.version != QUALITY_RULES_DOCUMENT_VERSION {
        return Err(format!(
            "La versión {} del contrato de calidad no es compatible; Columnia admite la versión {QUALITY_RULES_DOCUMENT_VERSION}.",
            legacy.version
        ));
    }
    build_quality_rules_document(legacy.rules)
}

fn migration_quality_document_version(
    document: &JsonMap<String, JsonValue>,
) -> Result<Option<u8>, String> {
    let Some((field, raw_version)) = ["version", "schema_version"]
        .iter()
        .find_map(|field| document.get(*field).map(|value| (*field, value)))
    else {
        return Ok(None);
    };
    let version = raw_version
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| raw_version.as_str()?.trim().parse::<u8>().ok())
        .ok_or_else(|| format!("El campo '{field}' debe ser un entero de versión."))?;
    if !(1..=LEGACY_QUALITY_DOCUMENT_MAX_VERSION).contains(&version) {
        return Err(format!(
            "La versión legacy {version} no es compatible; se admiten las versiones 1 a {LEGACY_QUALITY_DOCUMENT_MAX_VERSION}."
        ));
    }
    Ok(Some(version))
}

pub(super) fn migrate_quality_rules_document(
    document: JsonValue,
) -> Result<QualityMigrationResult, String> {
    if document
        .as_object()
        .is_some_and(|map| map.contains_key("format"))
    {
        let document = parse_quality_rules_document(document, false)?;
        let total_items = document.rules.len();
        let warnings = Vec::new();
        return Ok(QualityMigrationResult {
            source_format: "columnia",
            source_version: Some(document.version.to_string()),
            converted_rules: document.rules,
            report: quality_migration_report(None, total_items, total_items, 0, &warnings),
            warnings,
            omitted_rules: 0,
        });
    }

    let (source_format, source_version, raw_rules) = match document {
        JsonValue::Array(rules) => ("legacy", None, rules),
        JsonValue::Object(document) => {
            let source_version = migration_quality_document_version(&document)?;
            let source_format = "legacy";
            let rules = document
                .get("rules")
                .or_else(|| document.get("quality_rules"))
                .and_then(JsonValue::as_array)
                .cloned()
                .ok_or_else(|| "El documento debe contener una lista 'rules'.".to_owned())?;
            (
                source_format,
                source_version.map(|version| version.to_string()),
                rules,
            )
        }
        _ => {
            return Err(
                "Las reglas migradas deben ser una lista o un objeto con 'rules'.".to_owned(),
            );
        }
    };

    if raw_rules.len() > MAX_QUALITY_RULES {
        return Err(format!(
            "El documento contiene {} reglas; Columnia admite como máximo {} en un contrato.",
            raw_rules.len(),
            MAX_QUALITY_RULES
        ));
    }

    let total_items = raw_rules.len();
    let mut converted_rules = Vec::new();
    let mut warnings = Vec::new();
    let mut omitted_rules = 0;
    for (zero_index, raw_rule) in raw_rules.into_iter().enumerate() {
        let rule_index = zero_index + 1;
        let JsonValue::Object(map) = raw_rule else {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                "unknown",
                "omitted",
                "La entrada no es un objeto JSON y se omitió.",
            ));
            continue;
        };
        let source_kind =
            migration_string_field(&map, &["kind", "type"]).unwrap_or_else(|| "unknown".to_owned());
        let Some(kind) = migration_quality_kind(&source_kind) else {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La regla todavía no tiene representación equivalente en Columnia.",
            ));
            continue;
        };
        let mut column = migration_string_field(&map, &["column"])
            .filter(|column| !column.trim().is_empty())
            .unwrap_or_else(|| QUALITY_DATASET_COLUMN.to_owned());
        if kind == QualityRuleKind::ReferentialIntegrity && column == QUALITY_DATASET_COLUMN {
            column = map
                .get("columns")
                .or_else(|| map.get("key_columns"))
                .and_then(JsonValue::as_array)
                .and_then(|columns| columns.first())
                .and_then(JsonValue::as_str)
                .filter(|column| !column.trim().is_empty())
                .unwrap_or(QUALITY_DATASET_COLUMN)
                .to_owned();
        }
        if !matches!(
            kind,
            QualityRuleKind::RowCount | QualityRuleKind::SchemaContract
        ) && column == QUALITY_DATASET_COLUMN
        {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La regla necesita una columna concreta.",
            ));
            continue;
        }

        let severity = match migration_severity_value(&map) {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if severity != "blocking" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La severidad no bloqueante no se convierte porque Columnia aún no tiene severidades por regla; la regla se omitió para no aprobarla silenciosamente.",
            ));
            continue;
        }
        if let Err(error) = migration_validate_quality_metadata(&map, "La regla") {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                error,
            ));
            continue;
        }
        let on_missing = match migration_policy_value(&map, "on_missing", "fail") {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if on_missing != "fail" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La política on_missing no bloqueante no tiene equivalente seguro en Columnia.",
            ));
            continue;
        }
        let null_policy = match migration_policy_value(&map, "null_policy", "invalid") {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        if null_policy != "invalid" {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                "La política de nulos distinta de invalid no tiene equivalente seguro en Columnia.",
            ));
            continue;
        }

        let max_invalid = match migration_usize_field(&map, &["maxInvalid", "max_invalid"]) {
            Ok(value) => value,
            Err(error) => {
                omitted_rules += 1;
                warnings.push(migration_warning(
                    rule_index,
                    &source_kind,
                    "omitted",
                    error,
                ));
                continue;
            }
        };
        let raw_max_invalid_pct =
            match migration_number_field(&map, &["maxInvalidPct", "max_invalid_pct"]) {
                Ok(value) => value,
                Err(error) => {
                    omitted_rules += 1;
                    warnings.push(migration_warning(
                        rule_index,
                        &source_kind,
                        "omitted",
                        error,
                    ));
                    continue;
                }
            };
        let max_invalid_pct = raw_max_invalid_pct.map(|number| number.clamp(0.0, 100.0));
        let (max_invalid, max_invalid_pct) = if max_invalid.is_none() && max_invalid_pct.is_none() {
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "warning",
                "La regla no traía tolerancia; se importó con máximo de inválidos igual a 0.",
            ));
            (Some(0), None)
        } else {
            (max_invalid, max_invalid_pct)
        };
        let mut rule = QualityRule {
            column,
            kind,
            max_invalid,
            max_invalid_pct,
            min: None,
            max: None,
            values: None,
            reference_values: None,
            baseline: None,
            direction: None,
            expected: None,
            aggregate: None,
            tolerance_abs: None,
            tolerance_rel: None,
            threshold: None,
            pattern: None,
            dtype: None,
            columns: None,
            operator: None,
            min_date: None,
            max_date: None,
            when: None,
            then: None,
            allow_additional: None,
            required_order: None,
        };

        let conversion_result = (|| -> Result<Option<String>, String> {
            Ok(match kind {
                QualityRuleKind::NumericRange => {
                    rule.min = migration_number_field(&map, &["min", "min_value", "minValue"])?;
                    rule.max = migration_number_field(&map, &["max", "max_value", "maxValue"])?;
                    (rule.min.is_none() && rule.max.is_none())
                        .then(|| "numeric_range necesita min o max.".to_owned())
                }
                QualityRuleKind::AllowedValues => {
                    rule.values = migration_reference_values(&map, &["values"], 1)?;
                    rule.values
                        .as_ref()
                        .filter(|values| !values.is_empty())
                        .is_none()
                        .then(|| "allowed_values necesita values[].".to_owned())
                }
                QualityRuleKind::Regex => {
                    rule.pattern = migration_string_field(&map, &["pattern"]);
                    rule.pattern
                        .as_ref()
                        .filter(|pattern| !pattern.is_empty())
                        .is_none()
                        .then(|| "regex necesita pattern.".to_owned())
                }
                QualityRuleKind::Dtype => {
                    let source_dtype =
                        migration_string_field(&map, &["dtype", "expected_type", "expectedType"]);
                    rule.dtype = source_dtype
                        .as_deref()
                        .and_then(migration_dtype)
                        .map(str::to_owned);
                    source_dtype
                        .as_ref()
                        .filter(|_| rule.dtype.is_some())
                        .is_none()
                        .then(|| "dtype no contiene un tipo soportado.".to_owned())
                }
                QualityRuleKind::UniqueTogether => {
                    rule.columns = migration_string_array(
                        &map,
                        &["columns", "required_columns", "requiredColumns"],
                    )?;
                    rule.columns
                        .as_ref()
                        .filter(|columns| columns.len() >= 2)
                        .is_none()
                        .then(|| "unique_together necesita al menos dos columnas.".to_owned())
                }
                QualityRuleKind::ColumnCompare => {
                    rule.columns = migration_string_array(&map, &["columns"])?;
                    if rule.columns.is_none() {
                        if let Some(other_column) = migration_string_field(
                            &map,
                            &["other_column", "otherColumn", "right_column", "rightColumn"],
                        ) {
                            rule.columns = Some(vec![rule.column.clone(), other_column]);
                        }
                    }
                    rule.operator = migration_string_field(&map, &["operator", "comparison"])
                        .and_then(|value| migration_quality_comparison(&value));
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.len() != 2)
                    {
                        Some("column_compare necesita exactamente dos columnas.".to_owned())
                    } else if rule.operator.is_none() {
                        Some(
                            "column_compare necesita operator eq, ne, lt, lte, gt o gte."
                                .to_owned(),
                        )
                    } else {
                        None
                    }
                }
                QualityRuleKind::ReferentialIntegrity => {
                    rule.columns =
                        migration_string_array(&map, &["columns", "key_columns", "keyColumns"])?;
                    if rule.columns.is_none() {
                        rule.columns = Some(vec![rule.column.clone()]);
                    }
                    if let Some(columns) = rule.columns.as_ref() {
                        if let Some(first) = columns.first() {
                            rule.column = first.clone();
                        }
                    }
                    let column_count = rule.columns.as_ref().map_or(0, Vec::len);
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        column_count,
                    )?;
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.is_empty())
                    {
                        Some("referential_integrity necesita columns[].".to_owned())
                    } else if rule.reference_values.as_ref().is_none_or(Vec::is_empty) {
                        Some("referential_integrity necesita reference_values[].".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::Monotonic => {
                    let source_direction = migration_string_field(&map, &["direction", "order"]);
                    rule.direction = match source_direction.as_deref() {
                        Some(value) => {
                            Some(migration_quality_monotonic_direction(value).ok_or_else(|| {
                                "monotonic necesita direction increasing o decreasing.".to_owned()
                            })?)
                        }
                        None => Some(QualityMonotonicDirection::Increasing),
                    };
                    None
                }
                QualityRuleKind::AggregateCheck => {
                    rule.expected = migration_number_field(
                        &map,
                        &["expected", "expected_value", "expectedValue"],
                    )?;
                    rule.aggregate = migration_string_field(&map, &["aggregate", "aggregation"])
                        .map(|value| {
                            migration_quality_aggregate(&value).ok_or_else(|| {
                                "aggregate_check necesita aggregate count, sum, min o max."
                                    .to_owned()
                            })
                        })
                        .transpose()?;
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        1,
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    rule.tolerance_rel = migration_number_field(
                        &map,
                        &["tolerance_rel", "toleranceRel", "relative_tolerance"],
                    )?;
                    if rule.expected.is_none()
                        && rule.reference_values.as_ref().is_none_or(Vec::is_empty)
                    {
                        Some("aggregate_check necesita expected o reference_values.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::AggregateReconciliation => {
                    rule.columns = migration_string_array(&map, &["columns", "source_columns"])?;
                    rule.expected = migration_number_field(
                        &map,
                        &["expected", "expected_value", "expectedValue"],
                    )?;
                    rule.reference_values = migration_reference_values(
                        &map,
                        &[
                            "reference_values",
                            "referenceValues",
                            "reference",
                            "references",
                        ],
                        1,
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    rule.tolerance_rel = migration_number_field(
                        &map,
                        &["tolerance_rel", "toleranceRel", "relative_tolerance"],
                    )?;
                    if rule
                        .columns
                        .as_ref()
                        .is_none_or(|columns| columns.len() < 2)
                        && rule.expected.is_none()
                        && rule.reference_values.as_ref().is_none_or(Vec::is_empty)
                    {
                        Some(
                            "aggregate_reconciliation necesita dos columnas o expected/reference_values."
                                .to_owned(),
                        )
                    } else {
                        None
                    }
                }
                QualityRuleKind::DistributionDrift => {
                    rule.baseline =
                        migration_reference_values(&map, &["baseline", "baselineValues"], 1)?;
                    if rule.baseline.as_ref().is_none_or(Vec::is_empty) {
                        rule.baseline = migration_reference_values(
                            &map,
                            &[
                                "reference_values",
                                "referenceValues",
                                "reference",
                                "references",
                            ],
                            1,
                        )?;
                    }
                    rule.threshold = migration_number_field(
                        &map,
                        &["threshold", "drift_threshold", "driftThreshold"],
                    )?;
                    rule.tolerance_abs = migration_number_field(
                        &map,
                        &["tolerance_abs", "toleranceAbs", "absolute_tolerance"],
                    )?;
                    if rule.baseline.as_ref().is_none_or(Vec::is_empty) {
                        Some("distribution_drift necesita baseline[].".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::DateRange => {
                    rule.min_date = migration_string_field(
                        &map,
                        &["min_date", "minDate", "min_value", "minValue", "min"],
                    );
                    rule.max_date = migration_string_field(
                        &map,
                        &["max_date", "maxDate", "max_value", "maxValue", "max"],
                    );
                    let has_invalid_bound = rule
                        .min_date
                        .as_deref()
                        .is_some_and(|value| parse_quality_datetime(value).is_none())
                        || rule
                            .max_date
                            .as_deref()
                            .is_some_and(|value| parse_quality_datetime(value).is_none());
                    if rule.min_date.is_none() && rule.max_date.is_none() {
                        Some("date_range necesita min_value/min o max_value/max.".to_owned())
                    } else if has_invalid_bound {
                        Some("date_range necesita límites de fecha válidos.".to_owned())
                    } else if rule
                        .min_date
                        .as_deref()
                        .zip(rule.max_date.as_deref())
                        .is_some_and(|(min, max)| {
                            parse_quality_datetime(min)
                                .zip(parse_quality_datetime(max))
                                .is_some_and(|(min, max)| min > max)
                        })
                    {
                        Some("El mínimo de date_range no puede superar el máximo.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::Conditional => {
                    rule.when = migration_quality_condition(
                        map.get("when")
                            .or_else(|| map.get("condition"))
                            .or_else(|| map.get("if")),
                    )?;
                    let then = migrate_conditional_then(map.get("then"), &rule.column)?;
                    rule.then = Some(Box::new(then));
                    if rule.when.is_none() {
                        Some("conditional necesita when.".to_owned())
                    } else {
                        None
                    }
                }
                QualityRuleKind::SchemaContract => {
                    rule.column = QUALITY_DATASET_COLUMN.to_owned();
                    rule.columns = migration_string_array(
                        &map,
                        &["columns", "required_columns", "requiredColumns"],
                    )?;
                    let invalid_columns = rule.columns.as_ref().is_none_or(|columns| {
                        columns.is_empty()
                            || columns.iter().any(|column| column.trim().is_empty())
                            || columns.iter().collect::<HashSet<_>>().len() != columns.len()
                    });
                    if invalid_columns {
                        Some("schema_contract necesita columns[].".to_owned())
                    } else {
                        rule.allow_additional = Some(
                            map.get("allow_additional")
                                .or_else(|| map.get("allowAdditional"))
                                .map(|value| {
                                    value.as_bool().ok_or_else(|| {
                                        "allowAdditional de schema_contract debe ser booleano."
                                            .to_owned()
                                    })
                                })
                                .transpose()?
                                .unwrap_or(true),
                        );
                        rule.required_order =
                            migration_string_array(&map, &["required_order", "requiredOrder"])?;
                        let invalid_order = rule.required_order.as_ref().is_some_and(|order| {
                            order.is_empty()
                                || order.iter().any(|column| column.trim().is_empty())
                                || order.iter().collect::<HashSet<_>>().len() != order.len()
                        });
                        if invalid_order {
                            Some(
                                "requiredOrder de schema_contract no puede estar vacío.".to_owned(),
                            )
                        } else {
                            None
                        }
                    }
                }
                QualityRuleKind::RowCount => {
                    rule.min = migration_number_field(&map, &["min_value", "min"])?;
                    rule.max = migration_number_field(&map, &["max_value", "max"])?;
                    (rule.min.is_none() && rule.max.is_none())
                        .then(|| "row_count necesita min_value/min o max_value/max.".to_owned())
                }
                QualityRuleKind::NotNull | QualityRuleKind::NonEmpty | QualityRuleKind::Unique => {
                    None
                }
            })
        })();
        let conversion_error = match conversion_result {
            Ok(error) => error,
            Err(error) => Some(error),
        };
        let conversion_error = conversion_error.or_else(|| {
            [
                ("toleranceAbs", rule.tolerance_abs),
                ("toleranceRel", rule.tolerance_rel),
                ("threshold", rule.threshold),
            ]
            .into_iter()
            .find_map(|(field, value)| {
                value
                    .filter(|value| *value < 0.0)
                    .map(|_| format!("{field} no puede ser negativo."))
            })
        });
        if let Some(error) = conversion_error {
            omitted_rules += 1;
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "omitted",
                error,
            ));
            continue;
        }
        if raw_max_invalid_pct.is_some_and(|value| value != value.clamp(0.0, 100.0)) {
            warnings.push(migration_warning(
                rule_index,
                &source_kind,
                "warning",
                "La tolerancia porcentual fue ajustada al intervalo 0–100.",
            ));
        }
        converted_rules.push(rule);
    }

    let report = quality_migration_report(
        None,
        total_items,
        converted_rules.len(),
        omitted_rules,
        &warnings,
    );
    Ok(QualityMigrationResult {
        source_format,
        source_version,
        converted_rules,
        warnings,
        omitted_rules,
        report,
    })
}

fn read_quality_rules_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let path = canonicalize_existing_file(path, "el contrato de calidad seleccionado")?;
    if !path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err("El contrato de calidad debe ser JSON.".to_owned());
    }
    let file = File::open(path)
        .map_err(|error| format!("No se pudo abrir el contrato de calidad: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("No se pudo verificar el contrato de calidad: {error}"))?
        .len();
    if size > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(QUALITY_MIGRATION_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer el contrato de calidad: {error}"))?;
    if bytes.len() as u64 > QUALITY_MIGRATION_FILE_LIMIT_BYTES {
        return Err(format!(
            "El contrato de calidad supera el límite local de {} bytes.",
            QUALITY_MIGRATION_FILE_LIMIT_BYTES
        ));
    }
    Ok(bytes)
}

pub(super) fn read_quality_rules_json(path: &Path) -> Result<JsonValue, String> {
    let bytes = read_quality_rules_bytes(path)?;
    serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))
}

pub(super) fn save_quality_rules_atomic(
    document: &QualityRulesDocument,
    destination: &Path,
) -> Result<(), String> {
    validate_quality_rules_document(document)?;
    let destination = canonicalize_write_destination(destination, "el contrato de calidad")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta del contrato de calidad.".to_owned())?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo preparar el contrato de calidad: {error}"))?;
    serde_json::to_writer_pretty(temporary.as_file(), document)
        .map_err(|error| format!("No se pudo escribir el contrato de calidad: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar el contrato de calidad: {error}"))?;
    temporary.persist(&destination).map_err(|error| {
        format!(
            "No se pudo publicar el contrato de calidad: {}",
            error.error
        )
    })?;
    Ok(())
}

pub(super) fn load_quality_migration_file(path: &Path) -> Result<QualityMigrationResult, String> {
    let bytes = read_quality_rules_bytes(path)?;
    let document = serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|error| format!("El contrato de calidad no es JSON válido: {error}"))?;
    let mut result = migrate_quality_rules_document(document)?;
    result.report.artifact_sha256 = Some(hex::encode(Sha256::digest(&bytes)));
    Ok(result)
}

pub(super) fn validate_quality_rules_payload(quality_rules: &[QualityRule]) -> Result<(), String> {
    if quality_rules.len() > MAX_QUALITY_RULES {
        return Err(format!(
            "Se admiten como máximo {MAX_QUALITY_RULES} reglas de calidad."
        ));
    }
    let mut text_fields = Vec::new();
    for rule in quality_rules {
        text_fields.push(("column", rule.column.as_str()));
        if let Some(pattern) = rule.pattern.as_deref() {
            text_fields.push(("pattern", pattern));
        }
        if let Some(values) = rule.values.as_deref() {
            for value in values {
                text_fields.push(("value", value.as_str()));
            }
        }
        if let Some(reference_values) = rule.reference_values.as_deref() {
            for value in reference_values {
                text_fields.push(("referenceValue", value.as_str()));
            }
        }
        if let Some(columns) = rule.columns.as_deref() {
            for column in columns {
                text_fields.push(("columns", column.as_str()));
            }
        }
        if let Some(condition) = rule.when.as_ref() {
            text_fields.push(("when.column", condition.column.as_str()));
            if let Some(value) = condition.value.as_deref() {
                text_fields.push(("when.value", value));
            }
        }
        if let Some(then) = rule.then.as_deref() {
            text_fields.push(("then.column", then.column.as_str()));
            if let Some(pattern) = then.pattern.as_deref() {
                text_fields.push(("then.pattern", pattern));
            }
            if let Some(values) = then.values.as_deref() {
                for value in values {
                    text_fields.push(("then.value", value.as_str()));
                }
            }
        }
        if let Some(required_order) = rule.required_order.as_deref() {
            for column in required_order {
                text_fields.push(("requiredOrder", column.as_str()));
            }
        }
    }
    validate_semantic_text_budget(
        "payload de reglas de calidad",
        text_fields,
        MAX_QUALITY_COLUMN_CHARS,
        MAX_QUALITY_TOTAL_TEXT_CHARS,
    )
}

pub(crate) fn validate_reusable_quality_rules(quality_rules: &[QualityRule]) -> Result<(), String> {
    validate_quality_rules_payload(quality_rules)
}
