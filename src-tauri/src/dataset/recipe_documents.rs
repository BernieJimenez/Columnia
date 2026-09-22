use super::*;

pub(super) fn recipe_path_with_extension(mut path: PathBuf) -> PathBuf {
    let is_json = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    if !is_json {
        path.set_extension("json");
    }
    path
}

fn validate_recipe_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name != name.trim() {
        return Err(
            "El nombre de la receta no puede estar vacío ni tener espacios externos.".to_owned(),
        );
    }
    if name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err(
            "El nombre de la receta debe tener hasta 120 caracteres imprimibles.".to_owned(),
        );
    }
    Ok(())
}

pub(super) fn recipe_suggested_file_name(name: &str) -> String {
    let safe_name: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            _ => character,
        })
        .collect();
    format!("{safe_name}.json")
}

fn current_recipe_timestamp() -> String {
    DateTime::<chrono::Utc>::from(std::time::SystemTime::now()).to_rfc3339()
}

pub(super) fn build_stored_recipe(
    recipe: TransformRecipe,
    name: String,
) -> Result<StoredTransformRecipe, String> {
    validate_recipe_name(&name)?;
    let document = StoredTransformRecipe {
        version: RECIPE_FILE_VERSION,
        name,
        saved_at: current_recipe_timestamp(),
        recipe,
        source_schema: None,
        export_options: None,
    };
    validate_stored_recipe(&document)?;
    Ok(document)
}

fn validate_recipe_export_options(options: &RecipeExportOptions) -> Result<(), String> {
    if options.formats.is_empty() || options.formats.len() > 6 {
        return Err("Las opciones de entrega deben incluir entre 1 y 6 formatos.".to_owned());
    }
    if options.selected_columns.len() > 512 {
        return Err("La entrega puede seleccionar como máximo 512 columnas.".to_owned());
    }
    validate_semantic_text_budget(
        "opciones de entrega",
        options
            .selected_columns
            .iter()
            .map(|column| ("columna seleccionada", column.as_str())),
        MAX_RECIPE_TEXT_FIELD_CHARS,
        MAX_RECIPE_TOTAL_TEXT_CHARS,
    )
}

pub(super) fn validate_stored_recipe(document: &StoredTransformRecipe) -> Result<(), String> {
    if ![PREVIOUS_RECIPE_FILE_VERSION, RECIPE_FILE_VERSION].contains(&document.version) {
        return Err(format!(
            "La receta usa la versión {}, pero Columnia admite las versiones {} y {}.",
            document.version, PREVIOUS_RECIPE_FILE_VERSION, RECIPE_FILE_VERSION
        ));
    }
    if document.version == PREVIOUS_RECIPE_FILE_VERSION && document.source_schema.is_some() {
        return Err("Una receta v1 no puede incluir el esquema de origen v2.".to_owned());
    }
    if let Some(schema) = &document.source_schema {
        if schema.len() > MAX_RECIPE_SOURCE_COLUMNS {
            return Err(format!(
                "El esquema de origen puede incluir como máximo {MAX_RECIPE_SOURCE_COLUMNS} columnas."
            ));
        }
        let mut names = HashSet::with_capacity(schema.len());
        for column in schema {
            if column.name.is_empty()
                || column.data_type.is_empty()
                || column.name.chars().any(char::is_control)
                || column.data_type.chars().any(char::is_control)
                || column.data_type.chars().count() > 128
                || !names.insert(column.name.as_str())
            {
                return Err("El esquema de origen de la receta contiene columnas vacías, repetidas o fuera de límite.".to_owned());
            }
        }
        validate_semantic_text_budget(
            "esquema de origen de receta",
            schema.iter().flat_map(|column| {
                [
                    ("nombre de columna", column.name.as_str()),
                    ("tipo de columna", column.data_type.as_str()),
                ]
            }),
            MAX_RECIPE_TEXT_FIELD_CHARS,
            MAX_RECIPE_TOTAL_TEXT_CHARS,
        )?;
    }
    validate_recipe_name(&document.name)?;
    DateTime::parse_from_rfc3339(&document.saved_at)
        .map_err(|_| "La receta no incluye una fecha de guardado RFC 3339 válida.".to_owned())?;
    validate_recipe_structure(&document.recipe)?;
    if let Some(export_options) = &document.export_options {
        validate_recipe_export_options(export_options)?;
    }
    let encoded = serde_json::to_vec(document)
        .map_err(|error| format!("No se pudo validar la receta: {error}"))?;
    if encoded.len() as u64 > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    Ok(())
}

pub(crate) fn validate_reusable_recipe(document: &StoredTransformRecipe) -> Result<(), String> {
    validate_stored_recipe(document)
}

pub(super) fn validate_semantic_text_budget<'a, I>(
    payload: &str,
    fields: I,
    maximum_field_chars: usize,
    maximum_total_chars: usize,
) -> Result<(), String>
where
    I: IntoIterator<Item = (&'static str, &'a str)>,
{
    let mut total_chars = 0_usize;
    for (field, value) in fields {
        let field_chars = value.chars().count();
        if field_chars > maximum_field_chars {
            return Err(format!(
                "El campo {field} del {payload} supera el límite de {maximum_field_chars} caracteres."
            ));
        }
        total_chars = total_chars.saturating_add(field_chars);
        if total_chars > maximum_total_chars {
            return Err(format!(
                "El {payload} supera el presupuesto semántico de {maximum_total_chars} caracteres."
            ));
        }
    }
    Ok(())
}

fn validate_recipe_text_budget(recipe: &TransformRecipe) -> Result<(), String> {
    let mut fields: Vec<(&'static str, &str)> = Vec::new();
    for rename in &recipe.renames {
        fields.extend([
            ("from de renombre", rename.from.as_str()),
            ("to de renombre", rename.to.as_str()),
        ]);
    }
    for cast in &recipe.casts {
        fields.push(("column de conversión", cast.column.as_str()));
    }
    for date_parse in &recipe.date_parses {
        fields.push(("column de fecha", date_parse.column.as_str()));
    }
    for filter in &recipe.filters {
        fields.push(("column de filtro", filter.column.as_str()));
        if let Some(value) = &filter.value {
            fields.push(("value de filtro", value.as_str()));
        }
    }
    if let Some(calculation) = &recipe.calculated_column {
        fields.extend([
            ("name de cálculo", calculation.name.as_str()),
            ("source de cálculo", calculation.source.as_str()),
        ]);
        if let Some(operand) = &calculation.operand {
            fields.push(("value de operando", operand.value.as_str()));
        }
    }
    if let Some(replacement) = &recipe.find_replace {
        if let Some(column) = &replacement.column {
            fields.push(("column de reemplazo", column.as_str()));
        }
        fields.extend([
            ("find de reemplazo", replacement.find.as_str()),
            ("replace de reemplazo", replacement.replace.as_str()),
        ]);
    }
    if let Some(columns) = &recipe.keep_columns {
        fields.extend(
            columns
                .iter()
                .map(|column| ("keepColumns", column.as_str())),
        );
    }
    if let Some(split) = &recipe.split_column {
        fields.extend([
            ("source de división", split.source.as_str()),
            ("delimiter de división", split.delimiter.as_str()),
        ]);
        fields.extend(
            split
                .names
                .iter()
                .map(|name| ("names de división", name.as_str())),
        );
    }
    if let Some(merge) = &recipe.merge_columns {
        fields.extend(
            merge
                .sources
                .iter()
                .map(|source| ("sources de combinación", source.as_str())),
        );
        fields.extend([
            ("name de combinación", merge.name.as_str()),
            ("separator de combinación", merge.separator.as_str()),
        ]);
    }
    fields.extend(
        recipe
            .outlier_treatments
            .iter()
            .map(|treatment| ("column de atípicos", treatment.column.as_str())),
    );
    if let Some(summary) = &recipe.group_summary {
        fields.extend(
            summary
                .group_by
                .iter()
                .map(|column| ("groupBy", column.as_str())),
        );
        fields.extend(
            summary
                .aggregations
                .iter()
                .map(|aggregation| ("column de agregación", aggregation.column.as_str())),
        );
    }
    fields.extend(
        recipe
            .contact_normalizations
            .iter()
            .map(|normalization| ("column de contacto", normalization.column.as_str())),
    );
    for extraction in &recipe.text_extractions {
        fields.extend([
            ("source de extracción", extraction.source.as_str()),
            ("name de extracción", extraction.name.as_str()),
        ]);
        if let Some(delimiter) = &extraction.delimiter {
            fields.push(("delimiter de extracción", delimiter.as_str()));
        }
    }

    validate_semantic_text_budget(
        "payload de receta",
        fields,
        MAX_RECIPE_TEXT_FIELD_CHARS,
        MAX_RECIPE_TOTAL_TEXT_CHARS,
    )
}

pub(super) fn validate_recipe_structure(recipe: &TransformRecipe) -> Result<(), String> {
    validate_recipe_text_budget(recipe)?;
    if let Some(find_replace) = &recipe.find_replace {
        validate_find_replace_pattern(find_replace)?;
    }
    let bounded = [
        (recipe.renames.len(), 256, "renombres"),
        (recipe.casts.len(), 256, "conversiones"),
        (recipe.date_parses.len(), 256, "fechas"),
        (recipe.filters.len(), 3, "filtros"),
        (
            recipe.outlier_treatments.len(),
            16,
            "tratamientos de atípicos",
        ),
        (
            recipe.contact_normalizations.len(),
            16,
            "normalizaciones de contacto",
        ),
        (recipe.text_extractions.len(), 16, "extracciones de texto"),
    ];
    for (count, maximum, operation) in bounded {
        if count > maximum {
            return Err(format!(
                "La receta contiene demasiados {operation}: máximo {maximum}."
            ));
        }
    }
    if recipe
        .keep_columns
        .as_ref()
        .is_some_and(|columns| columns.len() > 512)
    {
        return Err("La receta puede conservar como máximo 512 columnas.".to_owned());
    }
    if recipe
        .split_column
        .as_ref()
        .is_some_and(|split| split.names.len() > 16)
    {
        return Err("Una división puede crear como máximo 16 columnas.".to_owned());
    }
    if recipe
        .merge_columns
        .as_ref()
        .is_some_and(|merge| merge.sources.len() > 16)
    {
        return Err("Una combinación puede usar como máximo 16 columnas.".to_owned());
    }
    if let Some(summary) = &recipe.group_summary {
        if summary.group_by.len() > 8 || summary.aggregations.len() > 32 {
            return Err(
                "Un resumen admite hasta 8 columnas de grupo y 32 agregaciones.".to_owned(),
            );
        }
    }
    Ok(())
}

pub(super) fn save_recipe_atomic(
    document: &StoredTransformRecipe,
    destination: &Path,
) -> Result<(), String> {
    validate_stored_recipe(document)?;
    let destination = canonicalize_write_destination(destination, "la receta")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "No se pudo resolver la carpeta de la receta.".to_owned())?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("No se pudo preparar el archivo de receta: {error}"))?;
    serde_json::to_writer_pretty(temporary.as_file(), document)
        .map_err(|error| format!("No se pudo escribir la receta: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("No se pudo sincronizar la receta: {error}"))?;
    temporary
        .persist(&destination)
        .map_err(|error| format!("No se pudo publicar la receta: {}", error.error))?;
    Ok(())
}

pub(super) fn load_recipe_file(path: &Path) -> Result<StoredTransformRecipe, String> {
    let path = canonicalize_existing_file(path, "la receta seleccionada")?;
    let file = File::open(path).map_err(|error| format!("No se pudo abrir la receta: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("No se pudo verificar la receta: {error}"))?
        .len();
    if size > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }

    let mut bytes = Vec::with_capacity(size as usize);
    file.take(RECIPE_FILE_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer la receta: {error}"))?;
    if bytes.len() as u64 > RECIPE_FILE_LIMIT_BYTES {
        return Err(format!(
            "La receta supera el límite local de {} bytes.",
            RECIPE_FILE_LIMIT_BYTES
        ));
    }
    let json = std::str::from_utf8(&bytes)
        .map_err(|_| "La receta no contiene texto UTF-8 válido.".to_owned())?;
    let raw: JsonValue = serde_json::from_str(json)
        .map_err(|error| format!("La receta JSON no es válida: {error}"))?;

    if raw.get("recipe").is_some() {
        let document: StoredTransformRecipe = serde_json::from_value(raw)
            .map_err(|error| format!("La receta Columnia no es válida: {error}"))?;
        validate_stored_recipe(&document)?;
        return Ok(document);
    }

    if let Ok(document) = serde_json::from_value::<StoredTransformRecipe>(raw.clone()) {
        validate_stored_recipe(&document)?;
        return Ok(document);
    }

    Err("El archivo no contiene una receta Columnia válida.".to_owned())
}

pub(super) fn migration_string_field(
    map: &JsonMap<String, JsonValue>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(JsonValue::as_str).map(str::to_owned))
}

pub(super) fn migration_policy_value(
    map: &JsonMap<String, JsonValue>,
    key: &str,
    default: &str,
) -> Result<String, String> {
    let value = match key {
        "on_missing" => map
            .get(key)
            .filter(|value| !value.is_null())
            .or_else(|| map.get("onMissing").filter(|value| !value.is_null())),
        "null_policy" => map
            .get(key)
            .filter(|value| !value.is_null())
            .or_else(|| map.get("nullPolicy").filter(|value| !value.is_null())),
        _ => map.get(key).filter(|value| !value.is_null()),
    };
    match value {
        None | Some(JsonValue::Null) => Ok(default.to_owned()),
        Some(JsonValue::String(value)) => {
            let value = value.trim().to_ascii_lowercase();
            let normalized = match key {
                "on_missing" if matches!(value.as_str(), "error" | "block" | "blocking") => "fail",
                "null_policy" if matches!(value.as_str(), "reject" | "error" | "fail") => "invalid",
                _ => value.as_str(),
            };
            Ok(normalized.to_owned())
        }
        Some(_) => Err(format!("El campo '{key}' debe ser texto.")),
    }
}

pub(super) fn migration_blocking_value(
    map: &JsonMap<String, JsonValue>,
) -> Result<Option<bool>, String> {
    let Some(value) = map
        .get("blocking")
        .or_else(|| map.get("is_blocking"))
        .or_else(|| map.get("isBlocking"))
    else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| "El campo 'blocking' debe ser booleano.".to_owned())
}

pub(super) fn migration_severity_value(map: &JsonMap<String, JsonValue>) -> Result<String, String> {
    let blocking = migration_blocking_value(map)?;
    let severity_is_absent = map.get("severity").is_none_or(JsonValue::is_null);
    let severity = if severity_is_absent && blocking == Some(false) {
        "warning".to_owned()
    } else {
        migration_policy_value(map, "severity", "blocking")?
    };
    let normalized = match severity.as_str() {
        // legacy historically called blocking rules "error" or "critical".
        // They are equivalent to Columnia's blocking gate and can be retained.
        "blocking" | "error" | "critical" | "fatal" => "blocking",
        "warning" | "warn" | "non_blocking" | "non-blocking" | "info" => "non_blocking",
        _ => return Err(format!("La severidad '{severity}' no está soportada.")),
    };
    if let Some(blocking) = blocking {
        let blocking_severity = if blocking { "blocking" } else { "non_blocking" };
        if normalized != blocking_severity {
            return Err(
                "severity y blocking describen políticas contradictorias; la regla se omitió."
                    .to_owned(),
            );
        }
    }
    Ok(normalized.to_owned())
}

pub(super) fn migration_has_true_nullable(
    map: &JsonMap<String, JsonValue>,
) -> Result<bool, String> {
    let Some(value) = map
        .get("nullable")
        .or_else(|| map.get("allow_nulls"))
        .or_else(|| map.get("allowNulls"))
    else {
        return Ok(false);
    };
    value
        .as_bool()
        .ok_or_else(|| "El campo 'nullable' debe ser booleano.".to_owned())
}
