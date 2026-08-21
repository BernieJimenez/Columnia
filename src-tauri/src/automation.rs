use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::dataset::{self, ExportFormat};

const GENERAL_HELP: &str = "Columnia CLI\n\nUSO:\n  columnia-cli inspect --input <ruta>\n  columnia-cli transform --input <ruta> --recipe <ruta> --output <ruta> --format csv|parquet\n\nFORMATOS DE ENTRADA:\n  CSV, TSV, JSON y Parquet. Los libros requieren selección de hoja y deben procesarse en la aplicación de escritorio.\n\nSALIDA:\n  JSON por stdout. Los errores se escriben en stderr sin rutas ni valores de celdas.\n";
const INSPECT_HELP: &str = "USO:\n  columnia-cli inspect --input <ruta>\n\nInspecciona CSV, TSV, JSON o Parquet y emite esquema y dimensiones como JSON, sin filas ni rutas.\n";
const TRANSFORM_HELP: &str = "USO:\n  columnia-cli transform --input <ruta> --recipe <ruta> --output <ruta> --format csv|parquet\n\nAplica una receta Columnia y publica la salida atómicamente. CSV conserva la protección contra fórmulas de hojas de cálculo.\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomationFormat {
    Csv,
    Parquet,
}

impl AutomationFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }

    fn dataset_format(self) -> ExportFormat {
        match self {
            Self::Csv => ExportFormat::Csv,
            Self::Parquet => ExportFormat::Parquet,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Help(&'static str),
    Inspect {
        input: PathBuf,
    },
    Transform {
        input: PathBuf,
        recipe: PathBuf,
        output: PathBuf,
        format: AutomationFormat,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationError {
    message: String,
}

impl AutomationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AutomationError {}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InspectionColumn {
    name: String,
    data_type: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InspectOutput {
    schema_version: u8,
    command: &'static str,
    file_name: String,
    row_count: usize,
    column_count: usize,
    columns: Vec<InspectionColumn>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransformSummary {
    input_row_count: usize,
    output_row_count: usize,
    input_column_count: usize,
    output_column_count: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransformOutput {
    schema_version: u8,
    command: &'static str,
    output_file_name: String,
    file_size_bytes: u64,
    format: &'static str,
    changed: bool,
    summary: TransformSummary,
}

fn parse_flags(
    arguments: &[OsString],
    allowed: &[&'static str],
) -> Result<HashMap<&'static str, OsString>, AutomationError> {
    let mut parsed = HashMap::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index]
            .to_str()
            .ok_or_else(|| AutomationError::new("La opción no contiene texto válido."))?;
        let Some(known_flag) = allowed.iter().copied().find(|candidate| *candidate == flag) else {
            return Err(AutomationError::new("Se recibió una opción desconocida."));
        };
        if parsed.contains_key(known_flag) {
            return Err(AutomationError::new(format!(
                "La opción {known_flag} está duplicada."
            )));
        }
        let value = arguments
            .get(index + 1)
            .filter(|value| !value.is_empty() && !value.to_string_lossy().starts_with("--"))
            .ok_or_else(|| {
                AutomationError::new(format!("La opción {known_flag} requiere un valor."))
            })?;
        parsed.insert(known_flag, value.clone());
        index += 2;
    }
    Ok(parsed)
}

fn required_flag(
    flags: &mut HashMap<&'static str, OsString>,
    name: &'static str,
) -> Result<OsString, AutomationError> {
    flags
        .remove(name)
        .ok_or_else(|| AutomationError::new(format!("Falta la opción requerida {name}.")))
}

pub fn parse_cli_args<I, T>(arguments: I) -> Result<CliCommand, AutomationError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
    if matches!(arguments.as_slice(), [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
    {
        return Ok(CliCommand::Help(GENERAL_HELP));
    }
    let subcommand = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| AutomationError::new("Falta el comando inspect o transform."))?;
    let rest = &arguments[1..];

    match subcommand {
        "inspect" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(INSPECT_HELP));
            }
            let mut flags = parse_flags(rest, &["--input"])?;
            let input = PathBuf::from(required_flag(&mut flags, "--input")?);
            Ok(CliCommand::Inspect { input })
        }
        "transform" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(TRANSFORM_HELP));
            }
            let mut flags = parse_flags(rest, &["--input", "--recipe", "--output", "--format"])?;
            let input = PathBuf::from(required_flag(&mut flags, "--input")?);
            let recipe = PathBuf::from(required_flag(&mut flags, "--recipe")?);
            let output = PathBuf::from(required_flag(&mut flags, "--output")?);
            let format = match required_flag(&mut flags, "--format")?.to_str() {
                Some("csv") => AutomationFormat::Csv,
                Some("parquet") => AutomationFormat::Parquet,
                _ => {
                    return Err(AutomationError::new(
                        "La opción --format debe ser csv o parquet.",
                    ));
                }
            };
            Ok(CliCommand::Transform {
                input,
                recipe,
                output,
                format,
            })
        }
        _ => Err(AutomationError::new(
            "Comando desconocido. Usa inspect, transform o --help.",
        )),
    }
}

fn validate_input_format(input: &Path) -> Result<(), AutomationError> {
    let extension = input
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("csv" | "tsv" | "json" | "parquet") => Ok(()),
        Some("xlsx" | "xls" | "xlsb" | "ods") => Err(AutomationError::new(
            "Los libros requieren selección de hoja y deben procesarse en la aplicación de escritorio.",
        )),
        _ => Err(AutomationError::new(
            "La entrada debe ser un archivo CSV, TSV, JSON o Parquet.",
        )),
    }
}

pub fn inspect(input: &Path) -> Result<InspectOutput, AutomationError> {
    validate_input_format(input)?;
    let (_, preview) = dataset::load_dataset_for_automation(input).map_err(|_| {
        AutomationError::new(
            "No se pudo inspeccionar el dataset. Verifica que sea un archivo regular y válido.",
        )
    })?;
    Ok(InspectOutput {
        schema_version: 1,
        command: "inspect",
        file_name: preview.file_name,
        row_count: preview.row_count,
        column_count: preview.column_count,
        columns: preview
            .columns
            .into_iter()
            .map(|column| InspectionColumn {
                name: column.name,
                data_type: column.data_type,
            })
            .collect(),
    })
}

pub fn transform(
    input: &Path,
    recipe: &Path,
    output: &Path,
    format: AutomationFormat,
) -> Result<TransformOutput, AutomationError> {
    validate_input_format(input)?;
    let output_matches_format = output
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()));
    if !output_matches_format {
        return Err(AutomationError::new(
            "La extensión de --output debe coincidir con --format.",
        ));
    }

    let (source, _) = dataset::load_dataset_for_automation(input).map_err(|_| {
        AutomationError::new(
            "No se pudo cargar el dataset. Verifica que sea un archivo regular y válido.",
        )
    })?;
    let stored_recipe = dataset::load_recipe_for_automation(recipe)
        .map_err(|_| AutomationError::new("No se pudo cargar una receta Columnia válida."))?;
    let summary_before = (source.height(), source.width());
    let (candidate, changed) = dataset::apply_recipe_for_automation(&source, &stored_recipe)
        .map_err(|_| AutomationError::new("La receta no es válida para el dataset de entrada."))?;
    let summary_after = (candidate.height(), candidate.width());
    let exported =
        dataset::export_frame_for_automation(&candidate, output, format.dataset_format()).map_err(
            |_| AutomationError::new("No se pudo publicar el archivo de salida de forma atómica."),
        )?;

    Ok(TransformOutput {
        schema_version: 1,
        command: "transform",
        output_file_name: exported.file_name,
        file_size_bytes: exported.file_size_bytes,
        format: exported.format,
        changed,
        summary: TransformSummary {
            input_row_count: summary_before.0,
            output_row_count: summary_after.0,
            input_column_count: summary_before.1,
            output_column_count: summary_after.1,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_recipe(path: &Path, from: &str, to: &str) {
        let document = serde_json::json!({
            "version": 1,
            "name": "Receta CLI",
            "savedAt": "2026-08-21T00:00:00Z",
            "recipe": {
                "renames": [{ "from": from, "to": to }]
            }
        });
        fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
    }

    #[test]
    fn parser_is_strict_and_help_is_available() {
        assert!(matches!(
            parse_cli_args(["--help"]).unwrap(),
            CliCommand::Help(text) if text.contains("columnia-cli inspect")
        ));
        assert_eq!(
            parse_cli_args(["inspect", "--input", "dataset.csv"]).unwrap(),
            CliCommand::Inspect {
                input: PathBuf::from("dataset.csv")
            }
        );
        assert!(
            parse_cli_args(["inspect", "--input", "a.csv", "--input", "b.csv"])
                .unwrap_err()
                .to_string()
                .contains("duplicada")
        );
        assert!(parse_cli_args(["inspect", "--unknown", "value"]).is_err());
        assert!(parse_cli_args(["transform", "--input", "dataset.csv"]).is_err());
        assert!(parse_cli_args([
            "transform",
            "--input",
            "dataset.csv",
            "--recipe",
            "recipe.json",
            "--output",
            "result.csv",
            "--format",
            "xml"
        ])
        .is_err());
    }

    #[test]
    fn inspect_emits_schema_without_rows_or_paths() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        fs::write(&input, "name,count\nA,1\nB,2\n").unwrap();

        let output = inspect(&input).unwrap();
        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["command"], "inspect");
        assert_eq!(json["fileName"], "source.csv");
        assert_eq!(json["rowCount"], 2);
        assert_eq!(json["columnCount"], 2);
        assert!(json.get("rows").is_none());
        assert!(json.get("path").is_none());
        assert!(!json
            .to_string()
            .contains(directory.path().to_str().unwrap()));
    }

    #[test]
    fn transform_reuses_atomic_export_and_csv_formula_protection() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("recipe.json");
        let output = directory.path().join("result.csv");
        fs::write(&input, "old,formula\nA,=SUM(A1:A2)\n").unwrap();
        write_recipe(&recipe, "old", "new");

        let result = transform(&input, &recipe, &output, AutomationFormat::Csv).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["command"], "transform");
        assert_eq!(json["outputFileName"], "result.csv");
        assert_eq!(json["format"], "CSV");
        assert_eq!(json["changed"], true);
        assert_eq!(json["summary"]["inputRowCount"], 1);
        assert_eq!(json["summary"]["outputColumnCount"], 2);
        assert!(fs::read_to_string(output).unwrap().contains("'=SUM(A1:A2)"));
        assert!(!json
            .to_string()
            .contains(directory.path().to_str().unwrap()));
    }

    #[test]
    fn invalid_recipe_never_creates_or_replaces_output() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("invalid.json");
        let absent_output = directory.path().join("absent.csv");
        let existing_output = directory.path().join("existing.csv");
        fs::write(&input, "value\n1\n").unwrap();
        fs::write(&existing_output, "previous output").unwrap();
        write_recipe(&recipe, "missing", "renamed");

        let error = transform(&input, &recipe, &absent_output, AutomationFormat::Csv).unwrap_err();
        assert!(!absent_output.exists());
        assert_eq!(
            error.to_string(),
            "La receta no es válida para el dataset de entrada."
        );
        assert!(!error
            .to_string()
            .contains(directory.path().to_str().unwrap()));

        assert!(transform(&input, &recipe, &existing_output, AutomationFormat::Csv).is_err());
        assert_eq!(
            fs::read_to_string(existing_output).unwrap(),
            "previous output"
        );
    }

    #[test]
    fn workbook_requires_explicit_desktop_sheet_selection() {
        let error = inspect(Path::new("book.xlsx")).unwrap_err();
        assert!(error.to_string().contains("selección de hoja"));
        assert!(!error.to_string().contains("book.xlsx"));
    }

    #[test]
    fn serialized_contracts_have_only_the_agreed_top_level_fields() {
        let inspect = serde_json::to_value(InspectOutput {
            schema_version: 1,
            command: "inspect",
            file_name: "data.csv".to_owned(),
            row_count: 0,
            column_count: 0,
            columns: vec![],
        })
        .unwrap();
        let keys = inspect
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "columnCount",
                "columns",
                "command",
                "fileName",
                "rowCount",
                "schemaVersion"
            ]
        );
        assert!(inspect.is_object());
    }
}
