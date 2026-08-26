//! Sanitización común para artefactos que pueden salir del proceso local.
//!
//! Las recetas y los manifiestos ejecutables conservan sus referencias privadas
//! en disco para poder funcionar. Cuando se convierten en un reporte público,
//! esta política elimina esas referencias y cualquier ruta absoluta que haya
//! quedado embebida en un mensaje.

use std::io::Write;

use serde::Serialize;
use serde_json::{Map, Value};

const REDACTED: &str = "[redactado]";

/// Serializa una respuesta destinada a stdout o a un conector sin revelar
/// rutas, referencias de entrada/salida ni otros metadatos de filesystem.
pub fn write_sanitized_json<W, T>(writer: W, value: &T) -> serde_json::Result<()>
where
    W: Write,
    T: Serialize,
{
    let sanitized = sanitized_json(value)?;
    serde_json::to_writer(writer, &sanitized)
}

/// Devuelve una copia JSON apta para reportes o conectores externos.
///
/// La función trabaja sobre la representación serializada para que también
/// cubra estructuras futuras sin acoplar este módulo al motor de datasets.
pub fn sanitized_json<T: Serialize>(value: &T) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(value)?;
    sanitize_value(&mut value);
    Ok(value)
}

/// Conserva solo el nombre visible de un archivo y elimina controles o
/// separadores. Es útil para reportes que necesitan identificar un resultado
/// sin revelar su carpeta local.
pub fn safe_file_name(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let filtered = basename
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    let trimmed = filtered.trim();
    if trimmed.is_empty() {
        "[sin-nombre]".to_owned()
    } else {
        trimmed.chars().take(255).collect()
    }
}

/// Redacta referencias privadas incrustadas en errores o mensajes de reporte.
pub fn sanitize_error(value: &str) -> String {
    if looks_like_private_reference(value) {
        REDACTED.to_owned()
    } else {
        value.to_owned()
    }
}

fn sanitize_value(value: &mut Value) {
    match value {
        Value::Object(object) => sanitize_object(object),
        Value::Array(values) => values.iter_mut().for_each(sanitize_value),
        Value::String(text) => {
            *text = sanitize_error(text);
        }
        _ => {}
    }
}

fn sanitize_object(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_private_reference_key(key) {
            *value = Value::String(REDACTED.to_owned());
        } else {
            sanitize_value(value);
        }
    }
}

fn is_private_reference_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "path"
            | "sourcepath"
            | "snapshotpath"
            | "manifestpath"
            | "storepath"
            | "workingdirectory"
            | "currentdirectory"
            | "cwd"
            | "inputpath"
            | "outputpath"
            | "recipepath"
            | "rulespath"
            | "homepath"
            | "input"
            | "output"
            | "recipe"
            | "manifest"
            | "store"
    ) || normalized.ends_with("path")
}

fn looks_like_private_reference(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(character, ',' | '.' | ';' | ':' | ')' | ']')
            })
        })
        .any(looks_like_private_reference_token)
    {
        return true;
    }
    false
}

fn looks_like_private_reference_token(value: &str) -> bool {
    if value.starts_with("file://")
        || value.starts_with("fixture://")
        || value.starts_with("\\\\")
        || value.starts_with('/')
    {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
        && bytes[0].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn sanitizes_report_paths_and_nested_manifest_references() {
        let source = json!({
            "schemaVersion": 1,
            "fileName": "resultado.csv",
            "outputPath": "C:\\Users\\Ana\\Desktop\\resultado.csv",
            "message": "No se pudo abrir file:///Users/ana/datos.csv",
            "jobs": [{
                "input": "C:\\datos\\clientes.csv",
                "recipe": "C:\\datos\\limpieza.json",
                "output": "C:\\salidas\\resultado.csv"
            }]
        });

        let sanitized = sanitized_json(&source).expect("el JSON debe serializarse");
        assert_eq!(sanitized["fileName"], "resultado.csv");
        assert_eq!(sanitized["outputPath"], REDACTED);
        assert_eq!(sanitized["message"], REDACTED);
        assert_eq!(sanitized["jobs"][0]["input"], REDACTED);
        assert_eq!(sanitized["jobs"][0]["recipe"], REDACTED);
        assert_eq!(sanitized["jobs"][0]["output"], REDACTED);
    }

    #[test]
    fn keeps_non_sensitive_recipe_configuration_and_redacts_path_like_values() {
        let source = json!({
            "name": "Limpieza de ventas",
            "recipe": {
                "renames": ["old -> new"],
                "filters": [{"value": "active"}],
                "source": "/private/customer.csv"
            },
            "migrationReport": {
                "sourceFormat": "dataprep",
                "sourceVersion": 3,
                "session": {"hasSourceReference": true}
            }
        });

        let sanitized = sanitized_json(&source).expect("el JSON debe serializarse");
        assert_eq!(sanitized["name"], "Limpieza de ventas");
        assert_eq!(sanitized["recipe"], REDACTED);
        assert_eq!(sanitized["migrationReport"]["sourceFormat"], "dataprep");
        assert_eq!(sanitized["migrationReport"]["sourceVersion"], 3);
    }

    #[test]
    fn safe_file_name_removes_paths_controls_and_empty_values() {
        assert_eq!(safe_file_name(r"C:\\private\\ventas.csv"), "ventas.csv");
        assert_eq!(safe_file_name("/private/ventas\n.csv"), "ventas.csv");
        assert_eq!(safe_file_name("  \0  "), "[sin-nombre]");
    }
}
