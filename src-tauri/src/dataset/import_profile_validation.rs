use std::collections::{HashMap, HashSet};

use super::{
    DataFrame, ImportExceptionBaseline, ImportExceptionConversion, ImportExceptionPolicy,
    ImportProfile, ImportProfileColumn, ImportProfileMismatch, ImportProfileTypeChange,
    StoredTransformRecipe, TransformRecipe,
};

pub const IMPORT_PROFILE_MISMATCH_PREFIX: &str = "__columnia_import_profile_mismatch__:";
const MAX_IMPORT_PROFILE_COLUMNS: usize = 2_000;
const MAX_IMPORT_PROFILE_COLUMN_NAME_CHARS: usize = 512;

pub fn validate_import_profile(profile: &ImportProfile) -> Result<(), String> {
    if profile.version != 1 {
        return Err("La versión del perfil de importación no es compatible.".to_owned());
    }
    if !matches!(
        profile.format.as_str(),
        "csv" | "tsv" | "json" | "parquet" | "excel"
    ) {
        return Err("El formato del perfil de importación no es compatible.".to_owned());
    }
    let has_sheet = profile.sheet_name.is_some();
    let has_header_mode = profile.header_mode.is_some();
    let options_match_format = match profile.format.as_str() {
        "excel" => has_sheet && has_header_mode,
        "csv" | "tsv" => !has_sheet && has_header_mode,
        "json" | "parquet" => !has_sheet && !has_header_mode,
        _ => unreachable!("el formato ya fue validado"),
    };
    if !options_match_format {
        return Err("Las opciones de hoja del perfil no coinciden con su formato.".to_owned());
    }
    if profile.sheet_name.as_deref().is_some_and(|name| {
        name.trim().is_empty()
            || name.chars().count() > MAX_IMPORT_PROFILE_COLUMN_NAME_CHARS
            || name.chars().any(char::is_control)
    }) {
        return Err("El nombre de hoja del perfil no es válido.".to_owned());
    }
    if profile.schema.is_empty() || profile.schema.len() > MAX_IMPORT_PROFILE_COLUMNS {
        return Err("El esquema del perfil de importación no es válido.".to_owned());
    }
    let mut seen = HashSet::with_capacity(profile.schema.len());
    if profile.schema.iter().any(|column| {
        column.name.trim().is_empty()
            || column.name.chars().count() > MAX_IMPORT_PROFILE_COLUMN_NAME_CHARS
            || column.name.chars().any(char::is_control)
            || column.data_type.is_empty()
            || column.data_type.chars().count() > 64
            || column.data_type.chars().any(char::is_control)
            || !seen.insert(column.name.clone())
    }) {
        return Err("Las columnas del perfil de importación no son válidas.".to_owned());
    }
    Ok(())
}

pub(crate) fn validate_import_exception_policy(
    policy: &ImportExceptionPolicy,
    import_schema: &[ImportProfileColumn],
    recipe: Option<&StoredTransformRecipe>,
) -> Result<(), String> {
    let recipe = recipe.ok_or_else(|| {
        "Las decisiones de conversión requieren una receta reutilizable.".to_owned()
    })?;
    validate_import_exception_policy_for_recipe(policy, import_schema, &recipe.recipe)
}

pub(super) fn import_exception_schema_for_frame(frame: &DataFrame) -> Vec<ImportProfileColumn> {
    frame
        .columns()
        .iter()
        .map(|column| ImportProfileColumn {
            name: column.name().to_string(),
            data_type: column.dtype().to_string(),
        })
        .collect()
}

pub(super) fn validate_import_exception_policy_for_recipe(
    policy: &ImportExceptionPolicy,
    import_schema: &[ImportProfileColumn],
    recipe: &TransformRecipe,
) -> Result<(), String> {
    if policy.version != 1 || policy.baseline != ImportExceptionBaseline::Lexical {
        return Err("La versión o base de la política de excepciones no es compatible.".to_owned());
    }
    if policy.schema != import_schema {
        return Err(
            "La política de excepciones debe coincidir con el esquema exacto de entrada."
                .to_owned(),
        );
    }
    if policy.conversions.is_empty() || policy.conversions.len() > 512 {
        return Err("La cantidad de decisiones de conversión no es válida.".to_owned());
    }

    let input_columns = import_schema
        .iter()
        .map(|column| column.name.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::with_capacity(policy.conversions.len());
    for decision in &policy.conversions {
        let column = match decision {
            ImportExceptionConversion::Cast { column, .. }
            | ImportExceptionConversion::Date { column, .. } => column,
        };
        if !input_columns.contains(column.as_str()) || !seen.insert(column.as_str()) {
            return Err("La política menciona una columna inexistente o repetida.".to_owned());
        }
    }

    let expected_columns = recipe
        .casts
        .iter()
        .filter(|cast| input_columns.contains(cast.column.as_str()))
        .map(|cast| cast.column.as_str())
        .chain(
            recipe
                .date_parses
                .iter()
                .filter(|parse| input_columns.contains(parse.column.as_str()))
                .map(|parse| parse.column.as_str()),
        )
        .collect::<HashSet<_>>();
    if seen != expected_columns {
        return Err("Las decisiones no coinciden con las conversiones de la receta.".to_owned());
    }
    for decision in &policy.conversions {
        let matches_recipe = match decision {
            ImportExceptionConversion::Cast { column, target, .. } => recipe
                .casts
                .iter()
                .any(|cast| cast.column == *column && cast.target == *target),
            ImportExceptionConversion::Date {
                column,
                format,
                target,
                ..
            } => recipe.date_parses.iter().any(|parse| {
                parse.column == *column && parse.format == *format && parse.target == *target
            }),
        };
        if !matches_recipe {
            return Err(
                "El destino o formato de una decisión no coincide con la receta.".to_owned(),
            );
        }
    }
    Ok(())
}

pub fn import_profile_schema_mismatch(
    profile: &ImportProfile,
    frame: &DataFrame,
) -> Option<ImportProfileMismatch> {
    let actual = frame
        .columns()
        .iter()
        .map(|column| {
            (
                column.name().as_str(),
                canonical_import_profile_data_type(&column.dtype().to_string()),
            )
        })
        .collect::<HashMap<_, _>>();
    let expected_names = profile
        .schema
        .iter()
        .map(|column| column.name.as_str())
        .collect::<HashSet<_>>();
    let expected = profile
        .schema
        .iter()
        .map(|column| {
            (
                column.name.as_str(),
                canonical_import_profile_data_type(&column.data_type),
            )
        })
        .collect::<HashMap<_, _>>();

    let missing_columns = profile
        .schema
        .iter()
        .filter(|column| !actual.contains_key(column.name.as_str()))
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let added_columns = frame
        .columns()
        .iter()
        .filter(|column| !expected_names.contains(column.name().as_str()))
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let changed_types = profile
        .schema
        .iter()
        .filter_map(|column| {
            let actual_type = actual.get(column.name.as_str())?;
            let expected_type = expected.get(column.name.as_str())?;
            (actual_type != expected_type).then(|| ImportProfileTypeChange {
                column: column.name.clone(),
                expected: expected_type.clone(),
                actual: actual_type.clone(),
            })
        })
        .collect::<Vec<_>>();

    if missing_columns.is_empty() && added_columns.is_empty() && changed_types.is_empty() {
        None
    } else {
        Some(ImportProfileMismatch {
            code: "importProfileSchemaMismatch".to_owned(),
            missing_columns,
            added_columns,
            changed_types,
        })
    }
}

/// Polars uses compact names for primitive dtypes (`str`, `i64`, `f64`), while
/// older saved profiles and UI fixtures may use their display names. Compare
/// only established aliases and keep genuinely distinct dtypes distinct.
fn canonical_import_profile_data_type(data_type: &str) -> String {
    match data_type.trim() {
        "str" | "String" | "string" | "Utf8" | "Utf8View" => "String".to_owned(),
        "bool" | "Boolean" => "Boolean".to_owned(),
        "i8" | "Int8" => "Int8".to_owned(),
        "i16" | "Int16" => "Int16".to_owned(),
        "i32" | "Int32" => "Int32".to_owned(),
        "i64" | "Int64" => "Int64".to_owned(),
        "u8" | "UInt8" => "UInt8".to_owned(),
        "u16" | "UInt16" => "UInt16".to_owned(),
        "u32" | "UInt32" => "UInt32".to_owned(),
        "u64" | "UInt64" => "UInt64".to_owned(),
        "f32" | "Float32" => "Float32".to_owned(),
        "f64" | "Float64" => "Float64".to_owned(),
        "date" | "Date" => "Date".to_owned(),
        "time" | "Time" => "Time".to_owned(),
        "null" | "Null" => "Null".to_owned(),
        other => other.to_owned(),
    }
}
