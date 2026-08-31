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
const MAX_SAFE_ERROR_CHARS: usize = 512;

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
    if looks_like_private_reference(value) || looks_like_sensitive_literal(value) {
        REDACTED.to_owned()
    } else {
        value
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_SAFE_ERROR_CHARS)
            .collect()
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
        let normalized = normalize_key(key);
        let redact_reference = is_private_reference_key(&normalized)
            && !(value.is_object() && is_structured_reference_key(&normalized));
        if redact_reference
            || is_private_container_key(&normalized)
            || is_sensitive_value_key(&normalized)
        {
            *value = Value::String(REDACTED.to_owned());
        } else if is_structured_reference_key(&normalized) && value.is_object() {
            sanitize_value(value);
            redact_reference_names(value);
        } else {
            sanitize_value(value);
        }
    }
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_private_reference_key(normalized: &str) -> bool {
    matches!(
        normalized,
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
            | "file"
            | "source"
            | "snapshot"
            | "dataset"
            | "sheet"
            | "sheetname"
            | "sourcereference"
            | "snapshotreference"
            | "inputreference"
            | "outputreference"
            | "filereference"
            | "uri"
            | "url"
    ) || normalized.ends_with("path")
        || normalized.ends_with("file")
        || normalized.ends_with("filename")
        || normalized.ends_with("directory")
        || normalized.ends_with("dirname")
        || normalized.ends_with("uri")
        || normalized.ends_with("url")
}

fn is_structured_reference_key(normalized: &str) -> bool {
    matches!(normalized, "source" | "snapshot" | "sheet")
}

fn redact_reference_names(value: &mut Value) {
    if let Value::Object(object) = value {
        if let Some(name) = object.get_mut("name") {
            *name = Value::String(REDACTED.to_owned());
        }
    }
}

/// These fields can contain complete user-authored documents or row payloads.
/// Redacting the container also protects future fields that are added to them.
fn is_private_container_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "recipedraft"
            | "recipejson"
            | "manifestjson"
            | "batchmanifest"
            | "qualityrules"
            | "qualityrulesjson"
            | "rows"
            | "records"
            | "samples"
            | "cells"
            | "jobs"
            | "data"
            | "dataframe"
            | "inputdata"
            | "outputdata"
            | "rawdata"
            | "payload"
    )
}

/// Scalar values and free-form text that can carry dataset contents or secrets.
/// Contract metadata such as ids, hashes, statuses and counts deliberately stays
/// visible to keep CLI and bridge responses useful.
fn is_sensitive_value_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "address"
            | "addresses"
            | "allowedvalue"
            | "allowedvalues"
            | "cellvalue"
            | "columnvalue"
            | "content"
            | "contents"
            | "email"
            | "emails"
            | "example"
            | "examples"
            | "inputvalue"
            | "invalidvalue"
            | "invalidvalues"
            | "newvalue"
            | "oldvalue"
            | "originalvalue"
            | "outputvalue"
            | "phone"
            | "phones"
            | "raw"
            | "referencevalue"
            | "referencevalues"
            | "replacement"
            | "sample"
            | "samplevalue"
            | "samplevalues"
            | "secret"
            | "secrets"
            | "token"
            | "tokens"
            | "value"
            | "values"
            | "password"
            | "apikey"
            | "authorization"
            | "cookie"
            | "createdat"
            | "updatedat"
            | "savedat"
            | "lastmodified"
            | "createdby"
            | "owner"
            | "username"
            | "text"
            | "texts"
    ) || normalized.ends_with("value")
        || normalized.ends_with("values")
}

fn looks_like_private_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("file://") || lower.contains("fixture://") {
        return true;
    }
    let bytes = value.as_bytes();
    if bytes.windows(3).any(|window| {
        window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'\\' | b'/')
    }) {
        return true;
    }
    if value.contains("\\\\") {
        return true;
    }
    if value
        .split_whitespace()
        .flat_map(|part| {
            part.split(|character: char| {
                matches!(character, '=' | '(' | '[' | '{' | '"' | '\'' | ';' | ',')
            })
        })
        .any(|part| part.starts_with('/'))
    {
        return true;
    }
    value
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(
                    character,
                    ',' | '.' | ';' | ':' | ')' | ']' | '}' | '"' | '\''
                )
            })
        })
        .any(looks_like_private_reference_token)
}

fn looks_like_sensitive_literal(value: &str) -> bool {
    value
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(
                    character,
                    ',' | '.' | ';' | ':' | ')' | ']' | '}' | '"' | '\''
                )
            })
        })
        .any(|token| {
            let Some(at) = token.find('@') else {
                return false;
            };
            at > 0 && token[at + 1..].contains('.') && at + 1 < token.len()
        })
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
        assert_eq!(sanitized["fileName"], REDACTED);
        assert_eq!(sanitized["outputPath"], REDACTED);
        assert_eq!(sanitized["message"], REDACTED);
        assert_eq!(sanitized["jobs"], REDACTED);
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
                "sourceFormat": "legacy",
                "sourceVersion": 3,
                "session": {"hasSourceReference": true}
            }
        });

        let sanitized = sanitized_json(&source).expect("el JSON debe serializarse");
        assert_eq!(sanitized["name"], "Limpieza de ventas");
        assert_eq!(sanitized["recipe"], REDACTED);
        assert_eq!(sanitized["migrationReport"]["sourceFormat"], "legacy");
        assert_eq!(sanitized["migrationReport"]["sourceVersion"], 3);
    }

    #[test]
    fn redacts_file_names_documents_and_dataset_values_but_keeps_safe_contract_metadata() {
        let source = json!({
            "id": "project-123",
            "artifactSha256": "deadbeef",
            "rowCount": 42,
            "columnCount": 3,
            "fileName": "clientes.csv",
            "sourceFileName": "clientes.csv",
            "datasetFile": "clientes.csv",
            "sheetName": "Clientes privados",
            "createdAt": "2026-08-27T12:00:00Z",
            "recipeDraft": {
                "steps": [{"column": "email", "value": "ana@example.com"}]
            },
            "manifest": {
                "version": 1,
                "jobs": [{
                    "input": "clientes.csv",
                    "recipe": "limpieza.json",
                    "output": "salida.csv"
                }]
            },
            "report": {
                "values": ["cliente-secreto"],
                "sample": "cliente-secreto",
                "rules": [{"column": "email", "referenceValues": ["ana@example.com"], "rowValue": "cliente-secreto"}]
            },
            "recipeSummary": {
                "operationCount": 2,
                "convertedOperations": ["filters"]
            }
        });

        let sanitized = sanitized_json(&source).expect("el JSON debe serializarse");
        assert_eq!(sanitized["id"], "project-123");
        assert_eq!(sanitized["artifactSha256"], "deadbeef");
        assert_eq!(sanitized["rowCount"], 42);
        assert_eq!(sanitized["columnCount"], 3);
        assert_eq!(sanitized["fileName"], REDACTED);
        assert_eq!(sanitized["sourceFileName"], REDACTED);
        assert_eq!(sanitized["datasetFile"], REDACTED);
        assert_eq!(sanitized["sheetName"], REDACTED);
        assert_eq!(sanitized["createdAt"], REDACTED);
        assert_eq!(sanitized["recipeDraft"], REDACTED);
        assert_eq!(sanitized["manifest"], REDACTED);
        assert_eq!(sanitized["report"]["values"], REDACTED);
        assert_eq!(sanitized["report"]["sample"], REDACTED);
        assert_eq!(sanitized["report"]["rules"][0]["referenceValues"], REDACTED);
        assert_eq!(sanitized["report"]["rules"][0]["rowValue"], REDACTED);
        assert_eq!(sanitized["recipeSummary"]["operationCount"], 2);
        assert_eq!(
            sanitized["recipeSummary"]["convertedOperations"][0],
            "filters"
        );
        assert!(!sanitized.to_string().contains("clientes.csv"));
        assert!(!sanitized.to_string().contains("cliente-secreto"));
        assert!(!sanitized.to_string().contains("ana@example.com"));
    }

    #[test]
    fn keeps_reference_status_objects_while_redacting_their_private_members() {
        let source = json!({
            "origin": {
                "source": {
                    "status": "available",
                    "available": true,
                    "path": "C:\\private\\clientes.csv"
                },
                "snapshot": {
                    "status": "missing",
                    "available": false,
                    "fileName": "snapshot.csv"
                }
            },
            "sheet": {
                "status": "selected",
                "name": "Clientes privados"
            }
        });

        let sanitized = sanitized_json(&source).expect("el JSON debe serializarse");
        assert_eq!(sanitized["origin"]["source"]["status"], "available");
        assert_eq!(sanitized["origin"]["source"]["available"], true);
        assert_eq!(sanitized["origin"]["source"]["path"], REDACTED);
        assert_eq!(sanitized["origin"]["snapshot"]["status"], "missing");
        assert_eq!(sanitized["origin"]["snapshot"]["available"], false);
        assert_eq!(sanitized["origin"]["snapshot"]["fileName"], REDACTED);
        assert_eq!(sanitized["sheet"]["status"], "selected");
        assert_eq!(sanitized["sheet"]["name"], REDACTED);
    }

    #[test]
    fn sanitizes_embedded_references_and_email_literals_without_hiding_safe_messages() {
        assert_eq!(
            sanitize_error("No se pudo abrir input=/private/clientes.csv"),
            REDACTED
        );
        assert_eq!(
            sanitize_error("No se pudo abrir FILE:///Users/ana/clientes.csv"),
            REDACTED
        );
        assert_eq!(sanitize_error("Contacto ana@example.com"), REDACTED);
        assert_eq!(
            sanitize_error("La validación terminó correctamente"),
            "La validación terminó correctamente"
        );
        assert_eq!(sanitize_error("mensaje\u{1b}[31m"), "mensaje[31m");
    }

    #[test]
    fn safe_file_name_removes_paths_controls_and_empty_values() {
        assert_eq!(safe_file_name(r"C:\\private\\ventas.csv"), "ventas.csv");
        assert_eq!(safe_file_name("/private/ventas\n.csv"), "ventas.csv");
        assert_eq!(safe_file_name("  \0  "), "[sin-nombre]");
    }
}
