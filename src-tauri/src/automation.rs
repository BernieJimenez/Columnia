use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use crate::{
    dataset::{self, DatasetState, ExportFormat, SpreadsheetHeaderMode},
    projects::{self, ProjectSummary, ProjectWorkspace},
};

const WORKBOOK_FLAGS: &str = "Para XLSX, XLS, XLSB u ODS son obligatorios --sheet <nombre-exacto> y --header first-row|generated. En otros formatos están prohibidos.";
const GENERAL_HELP: &str = "Columnia CLI\n\nUSO:\n  columnia-cli inspect --input <ruta> [--sheet <nombre> --header first-row|generated]\n  columnia-cli transform --input <ruta> [--sheet <nombre> --header first-row|generated] --recipe <ruta> --output <ruta> --format csv|json|parquet|sql|excel|sqlite|bundle\n  columnia-cli validate --input <ruta> [--sheet <nombre> --header first-row|generated] --rules <ruta.json>\n  columnia-cli quality-migration-report --rules <ruta.json>\n  columnia-cli batch --manifest <ruta.json>\n  columnia-cli project-list --store <directorio>\n  columnia-cli project-save --store <directorio> --name <nombre> --input <ruta> [--id <id>] [--sheet <nombre> --header first-row|generated] [--recipe <ruta>] [--rules <ruta>] [--profile]\n  columnia-cli project-inspect --store <directorio> --id <id>\n  columnia-cli project-export --store <directorio> --id <id> --output <ruta> --format csv|json|parquet|sql|excel|sqlite|bundle [--allow-unvalidated]\n  columnia-cli project-delete --store <directorio> --id <id> --confirm <id>\n\nFORMATOS DE ENTRADA:\n  CSV, TSV, JSON, Parquet, XLSX, XLS, XLSB y ODS.\n\nLIBROS:\n  Selección estricta por nombre exacto de hoja; no se elige una hoja implícitamente.\n\nSALIDA:\n  JSON v1 por stdout, sin rutas, filas ni muestras. quality-migration-report, validate, un trabajo batch fallido o una exportación bloqueada por calidad terminan con código 2 cuando requieren revisión; los errores de uso, carga o almacenamiento terminan con código 1. Batch hace preflight completo y publica cada trabajo atómicamente, pero no es una transacción global: conserva las salidas ya completadas ante un fallo tardío.\n";
const INSPECT_HELP: &str = "USO:\n  columnia-cli inspect --input <ruta> [--sheet <nombre> --header first-row|generated]\n\nInspecciona un dataset y emite esquema y dimensiones como JSON, sin filas ni rutas.\n";
const TRANSFORM_HELP: &str = "USO:\n  columnia-cli transform --input <ruta> [--sheet <nombre> --header first-row|generated] --recipe <ruta> --output <ruta> --format csv|json|parquet|sql|excel|sqlite|bundle\n\nAplica una receta Columnia y publica la salida atómicamente. CSV y Excel escriben valores como texto seguro; SQL produce un script portable, SQLite una base local con tabla dataset y bundle un ZIP con dataset, diccionario, receta validada, calidad opcional y manifest.\n";
const VALIDATE_HELP: &str = "USO:\n  columnia-cli validate --input <ruta> [--sheet <nombre> --header first-row|generated] --rules <ruta.json>\n\nEvalúa un contrato JSON Columnia con {\"format\":\"columnia-quality-rules\",\"version\":1,\"rules\":[...]}. El documento anterior {\"version\":1,\"rules\":[...]} sigue admitido por compatibilidad. Emite solo conteos; código 0 si pasa y 2 si no pasa.\n";
const QUALITY_MIGRATION_REPORT_HELP: &str = "USO:\n  columnia-cli quality-migration-report --rules <ruta.json>\n\nHace un preflight sanitizado de un contrato Columnia, Legacy v1–v3 o legacy. Resume por regla la severidad y las políticas on_missing/null_policy, identifica omisiones y devuelve código 2 si hace falta revisión manual. No migra ni evalúa filas.\n";
const BATCH_HELP: &str = "USO:\n  columnia-cli batch --manifest <ruta.json> [--force]\n\nEjecuta de 1 a 64 transformaciones declaradas en un manifiesto JSON v1 estricto. Las rutas relativas se resuelven desde la carpeta del manifiesto o outputRoot. Por defecto las salidas quedan confinadas a ese root, no pueden usar rutas absolutas, traversal ni reemplazar archivos existentes. --force permite un destino externo o existente, pero no permite colisionar con el manifiesto, inputs o recetas. El preflight valida todos los trabajos antes de escribir. Cada trabajo publica su salida atómicamente, pero el lote no es una transacción global: si un trabajo falla, conserva las salidas anteriores y termina con código 2. Un manifiesto o uso inválido termina con código 1.\n";
const PROJECT_LIST_HELP: &str = "USO:\n  columnia-cli project-list --store <directorio>\n\nLista resúmenes de proyectos persistidos y emite JSON v1 sin rutas ni muestras.\n";
const PROJECT_SAVE_HELP: &str = "USO:\n  columnia-cli project-save --store <directorio> --name <nombre> --input <ruta> [--id <id>] [--sheet <nombre> --header first-row|generated] [--recipe <ruta>] [--rules <ruta>] [--profile]\n\nCrea o actualiza un proyecto. La receta, las reglas y el perfil son opcionales. --recipe acepta recetas Columnia y pipelines Legacy v1–v3; las operaciones de limpieza seleccionadas del pipeline se reproducen antes de la transformación estructural.\n";
const PROJECT_INSPECT_HELP: &str = "USO:\n  columnia-cli project-inspect --store <directorio> --id <id>\n\nEmite metadatos, flags y conteos del proyecto sin abrir una sesión de escritorio.\n";
const PROJECT_EXPORT_HELP: &str = "USO:\n  columnia-cli project-export --store <directorio> --id <id> --output <ruta> --format csv|json|parquet|sql|excel|sqlite|bundle [--allow-unvalidated]\n\nLas reglas guardadas siempre deben pasar. --allow-unvalidated solo permite exportar proyectos sin reglas. La publicación es atómica; un bundle incluye la receta validada del proyecto cuando existe.\n";
const PROJECT_DELETE_HELP: &str = "USO:\n  columnia-cli project-delete --store <directorio> --id <id> --confirm <id>\n\nElimina el proyecto solo cuando --confirm coincide exactamente con --id.\n";
const BATCH_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const BATCH_MAX_JOBS: usize = 64;
const BATCH_MAX_FIELD_CHARS: usize = 4 * 1024;
const BATCH_MAX_TOTAL_TEXT_CHARS: usize = 64 * 1024;
const QUALITY_MIGRATION_REPORT_FILE_LIMIT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomationFormat {
    Csv,
    Json,
    Parquet,
    Sql,
    Excel,
    Sqlite,
    Bundle,
}

impl AutomationFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Parquet => "parquet",
            Self::Sql => "sql",
            Self::Excel => "xlsx",
            Self::Sqlite => "sqlite",
            Self::Bundle => "zip",
        }
    }

    fn dataset_format(self) -> ExportFormat {
        match self {
            Self::Csv => ExportFormat::Csv,
            Self::Json => ExportFormat::Json,
            Self::Parquet => ExportFormat::Parquet,
            Self::Sql => ExportFormat::Sql,
            Self::Excel => ExportFormat::Excel,
            Self::Sqlite => ExportFormat::Sqlite,
            Self::Bundle => ExportFormat::Bundle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Parquet => "Parquet",
            Self::Sql => "SQL",
            Self::Excel => "Excel",
            Self::Sqlite => "SQLite",
            Self::Bundle => "Paquete Columnia",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Help(&'static str),
    Inspect {
        input: PathBuf,
        sheet: Option<String>,
        header: Option<SpreadsheetHeaderMode>,
    },
    Transform {
        input: PathBuf,
        sheet: Option<String>,
        header: Option<SpreadsheetHeaderMode>,
        recipe: PathBuf,
        output: PathBuf,
        format: AutomationFormat,
    },
    Validate {
        input: PathBuf,
        sheet: Option<String>,
        header: Option<SpreadsheetHeaderMode>,
        rules: PathBuf,
    },
    QualityMigrationReport {
        rules: PathBuf,
    },
    Batch {
        manifest: PathBuf,
        force: bool,
    },
    ProjectList {
        store: PathBuf,
    },
    ProjectSave {
        store: PathBuf,
        name: String,
        input: PathBuf,
        id: Option<String>,
        sheet: Option<String>,
        header: Option<SpreadsheetHeaderMode>,
        recipe: Option<PathBuf>,
        rules: Option<PathBuf>,
        profile: bool,
    },
    ProjectInspect {
        store: PathBuf,
        id: String,
    },
    ProjectExport {
        store: PathBuf,
        id: String,
        output: PathBuf,
        format: AutomationFormat,
        allow_unvalidated: bool,
    },
    ProjectDelete {
        store: PathBuf,
        id: String,
        confirm: String,
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

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateOutput {
    schema_version: u8,
    command: &'static str,
    passed: bool,
    row_count: usize,
    total_rules: usize,
    passed_rules: usize,
    failed_rules: usize,
    total_invalid_count: usize,
}

impl ValidateOutput {
    pub fn passed(&self) -> bool {
        self.passed
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationRuleReport {
    rule_index: usize,
    source_kind: String,
    status: &'static str,
    severity: &'static str,
    on_missing: &'static str,
    null_policy: &'static str,
    issues: Vec<&'static str>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationPolicySummary {
    blocking_rules: usize,
    non_blocking_rules: usize,
    unsupported_policy_rules: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMigrationReportOutput {
    schema_version: u8,
    command: &'static str,
    source_format: &'static str,
    source_version: Option<String>,
    artifact_sha256: String,
    total_rules: usize,
    convertible_rules: usize,
    omitted_rules: usize,
    warning_count: usize,
    policies: QualityMigrationPolicySummary,
    rules: Vec<QualityMigrationRuleReport>,
    manual_actions: Vec<&'static str>,
}

impl QualityMigrationReportOutput {
    pub fn requires_manual_review(&self) -> bool {
        self.omitted_rules > 0 || self.policies.unsupported_policy_rules > 0
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchManifest {
    version: u8,
    #[serde(default)]
    output_root: Option<String>,
    jobs: Vec<BatchJobDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchJobDocument {
    input: String,
    recipe: String,
    output: String,
    format: BatchFormat,
    sheet: Option<String>,
    header: Option<SpreadsheetHeaderMode>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BatchFormat {
    Csv,
    Json,
    Parquet,
    Sql,
    Excel,
    Sqlite,
    Bundle,
}

impl From<BatchFormat> for AutomationFormat {
    fn from(value: BatchFormat) -> Self {
        match value {
            BatchFormat::Csv => Self::Csv,
            BatchFormat::Json => Self::Json,
            BatchFormat::Parquet => Self::Parquet,
            BatchFormat::Sql => Self::Sql,
            BatchFormat::Excel => Self::Excel,
            BatchFormat::Sqlite => Self::Sqlite,
            BatchFormat::Bundle => Self::Bundle,
        }
    }
}

struct PreparedBatchJob {
    input: PathBuf,
    recipe: PathBuf,
    output: PathBuf,
    format: AutomationFormat,
    sheet: Option<String>,
    header: Option<SpreadsheetHeaderMode>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchOutput {
    schema_version: u8,
    command: &'static str,
    status: &'static str,
    total_jobs: usize,
    completed_jobs: usize,
    changed_jobs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed_job_number: Option<usize>,
}

impl BatchOutput {
    pub fn failed(&self) -> bool {
        self.failed_job_number.is_some()
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListOutput {
    schema_version: u8,
    command: &'static str,
    projects: Vec<ProjectSummary>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSaveOutput {
    schema_version: u8,
    command: &'static str,
    created: bool,
    project: ProjectSummary,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHistoryOutput {
    entry_count: usize,
    current_index: usize,
    can_undo: bool,
    can_redo: bool,
    snapshots_enabled: bool,
    degraded: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspectOutput {
    schema_version: u8,
    command: &'static str,
    project: ProjectSummary,
    profile_cached: bool,
    quality_rule_count: usize,
    recipe_draft_present: bool,
    history: ProjectHistoryOutput,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectQualityOutput {
    validated: bool,
    passed: Option<bool>,
    row_count: usize,
    total_rules: usize,
    passed_rules: usize,
    failed_rules: usize,
    total_invalid_count: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportOutput {
    schema_version: u8,
    command: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_size_bytes: Option<u64>,
    format: &'static str,
    quality: ProjectQualityOutput,
}

impl ProjectExportOutput {
    pub fn blocked(&self) -> bool {
        self.status == "blocked"
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeleteOutput {
    schema_version: u8,
    command: &'static str,
    id: String,
    deleted: bool,
}

fn parse_flags(
    arguments: &[OsString],
    value_flags: &[&'static str],
    switches: &[&'static str],
) -> Result<(HashMap<&'static str, OsString>, HashSet<&'static str>), AutomationError> {
    let mut parsed = HashMap::new();
    let mut enabled = HashSet::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index]
            .to_str()
            .ok_or_else(|| AutomationError::new("La opción no contiene texto válido."))?;
        if let Some(known_switch) = switches
            .iter()
            .copied()
            .find(|candidate| *candidate == flag)
        {
            if !enabled.insert(known_switch) {
                return Err(AutomationError::new(format!(
                    "La opción {known_switch} está duplicada."
                )));
            }
            index += 1;
            continue;
        }
        let Some(known_flag) = value_flags
            .iter()
            .copied()
            .find(|candidate| *candidate == flag)
        else {
            return Err(AutomationError::new("Se recibió una opción desconocida."));
        };
        if parsed.contains_key(known_flag) || enabled.contains(known_flag) {
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
    Ok((parsed, enabled))
}

fn required_flag(
    flags: &mut HashMap<&'static str, OsString>,
    name: &'static str,
) -> Result<OsString, AutomationError> {
    flags
        .remove(name)
        .ok_or_else(|| AutomationError::new(format!("Falta la opción requerida {name}.")))
}

fn text_flag(
    flags: &mut HashMap<&'static str, OsString>,
    name: &'static str,
) -> Result<String, AutomationError> {
    required_flag(flags, name)?
        .into_string()
        .map_err(|_| AutomationError::new(format!("La opción {name} no contiene texto válido.")))
}

fn optional_text_flag(
    flags: &mut HashMap<&'static str, OsString>,
    name: &'static str,
) -> Result<Option<String>, AutomationError> {
    flags
        .remove(name)
        .map(|value| {
            value.into_string().map_err(|_| {
                AutomationError::new(format!("La opción {name} no contiene texto válido."))
            })
        })
        .transpose()
}

fn migration_report_version(
    map: &JsonMap<String, JsonValue>,
) -> Result<Option<u8>, AutomationError> {
    let Some(value) = map.get("version").or_else(|| map.get("schema_version")) else {
        return Ok(None);
    };
    let parsed = value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<u8>().ok())
        .ok_or_else(|| AutomationError::new("La versión del contrato debe ser un entero."))?;
    Ok(Some(parsed))
}

fn migration_report_document(
    document: &JsonValue,
) -> Result<(&'static str, Option<String>, Vec<JsonValue>), AutomationError> {
    match document {
        JsonValue::Array(rules) => Ok(("legacy", None, rules.clone())),
        JsonValue::Object(map) => {
            if map.contains_key("format") {
                let format = map
                    .get("format")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        AutomationError::new("El formato del contrato debe ser texto.")
                    })?;
                if format != "columnia-quality-rules" {
                    return Err(AutomationError::new(
                        "El formato del contrato de calidad no es compatible.",
                    ));
                }
                let version = migration_report_version(map)?;
                if version != Some(1) {
                    return Err(AutomationError::new(
                        "El contrato Columnia debe usar la versión 1.",
                    ));
                }
                let rules = map
                    .get("rules")
                    .and_then(JsonValue::as_array)
                    .cloned()
                    .ok_or_else(|| {
                        AutomationError::new("El contrato debe contener una lista rules.")
                    })?;
                return Ok(("columnia", Some("1".to_owned()), rules));
            }

            let version = migration_report_version(map)?;
            if version.is_some_and(|version| !(1..=3).contains(&version)) {
                return Err(AutomationError::new(
                    "La versión Legacy no es compatible; se admiten las versiones 1 a 3.",
                ));
            }
            let source_format = "legacy";
            let rules = map
                .get("rules")
                .or_else(|| map.get("quality_rules"))
                .and_then(JsonValue::as_array)
                .cloned()
                .ok_or_else(|| {
                    AutomationError::new("El contrato debe contener una lista rules.")
                })?;
            Ok((
                source_format,
                version.map(|version| version.to_string()),
                rules,
            ))
        }
        _ => Err(AutomationError::new(
            "El contrato debe ser una lista o un objeto JSON.",
        )),
    }
}

fn migration_report_kind(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "not_null" => Some("not_null"),
        "non_empty" => Some("non_empty"),
        "unique" => Some("unique"),
        "numeric_range" | "range" => Some("numeric_range"),
        "allowed_values" => Some("allowed_values"),
        "regex" => Some("regex"),
        "dtype" => Some("dtype"),
        "unique_together" => Some("unique_together"),
        "column_compare" | "column_comparison" => Some("column_compare"),
        "referential_integrity" | "referential" => Some("referential_integrity"),
        "monotonic" => Some("monotonic"),
        "aggregate_check" | "aggregate" => Some("aggregate_check"),
        "aggregate_reconciliation" | "aggregate_reconcile" | "reconciliation" => {
            Some("aggregate_reconciliation")
        }
        "distribution_drift" | "drift" => Some("distribution_drift"),
        "date_range" => Some("date_range"),
        "conditional" => Some("conditional"),
        "schema_contract" | "schema" => Some("schema_contract"),
        "row_count" => Some("row_count"),
        _ => None,
    }
}

fn migration_report_policy(
    map: &JsonMap<String, JsonValue>,
    key: &str,
    default: &'static str,
) -> &'static str {
    let Some(value) = map.get(key) else {
        return default;
    };
    if value.is_null() {
        return default;
    }
    let Some(value) = value.as_str() else {
        return "invalid_policy";
    };
    match (key, value.trim().to_ascii_lowercase().as_str()) {
        ("severity", "blocking") => "blocking",
        ("severity", "warning" | "warn" | "non_blocking" | "non-blocking" | "info") => {
            "non_blocking"
        }
        ("on_missing", "fail") => "fail",
        ("on_missing", "warning" | "warn" | "skip" | "ignore" | "continue") => "non_blocking",
        ("null_policy", "invalid") => "invalid",
        ("null_policy", "warning" | "warn" | "skip" | "ignore" | "allow" | "valid") => {
            "non_blocking"
        }
        _ => "unsupported_policy",
    }
}

fn migration_report_rule(
    rule_index: usize,
    value: &JsonValue,
) -> (QualityMigrationRuleReport, bool, bool, bool) {
    let Some(map) = value.as_object() else {
        return (
            QualityMigrationRuleReport {
                rule_index,
                source_kind: "unknown".to_owned(),
                status: "manual_review",
                severity: "invalid_policy",
                on_missing: "invalid_policy",
                null_policy: "invalid_policy",
                issues: vec!["rule_not_object"],
            },
            false,
            false,
            false,
        );
    };
    let source_kind = map
        .get("kind")
        .or_else(|| map.get("type"))
        .and_then(JsonValue::as_str)
        .and_then(migration_report_kind)
        .map(str::to_owned);
    let kind_supported = source_kind.is_some();
    let source_kind = source_kind.unwrap_or_else(|| {
        if map.get("kind").or_else(|| map.get("type")).is_some() {
            "unsupported".to_owned()
        } else {
            "unknown".to_owned()
        }
    });
    let severity = migration_report_policy(map, "severity", "blocking_default");
    let on_missing = migration_report_policy(map, "on_missing", "fail_default");
    let null_policy = migration_report_policy(map, "null_policy", "invalid_default");
    let policy_compatible = matches!(severity, "blocking" | "blocking_default")
        && matches!(on_missing, "fail" | "fail_default")
        && matches!(null_policy, "invalid" | "invalid_default");
    let mut issues = Vec::new();
    if !kind_supported {
        issues.push("unsupported_kind");
    }
    if !policy_compatible {
        if !matches!(severity, "blocking" | "blocking_default") {
            issues.push("severity_not_blocking");
        }
        if !matches!(on_missing, "fail" | "fail_default") {
            issues.push("on_missing_not_fail");
        }
        if !matches!(null_policy, "invalid" | "invalid_default") {
            issues.push("null_policy_not_invalid");
        }
    }
    let requires_column = source_kind != "row_count" && source_kind != "schema_contract";
    let has_column = map
        .get("column")
        .and_then(JsonValue::as_str)
        .is_some_and(|column| !column.trim().is_empty())
        || (source_kind == "referential_integrity"
            && map
                .get("columns")
                .or_else(|| map.get("key_columns"))
                .and_then(JsonValue::as_array)
                .is_some_and(|columns| !columns.is_empty()));
    if requires_column && !has_column {
        issues.push("column_required");
    }
    let status = if issues.is_empty() {
        "compatible"
    } else {
        "manual_review"
    };
    let policy_issue = !policy_compatible;
    let explicit_non_blocking =
        severity == "non_blocking" || on_missing == "non_blocking" || null_policy == "non_blocking";
    (
        QualityMigrationRuleReport {
            rule_index,
            source_kind,
            status,
            severity,
            on_missing,
            null_policy,
            issues,
        },
        status == "compatible",
        policy_issue,
        explicit_non_blocking,
    )
}

pub fn quality_migration_report(
    input: &Path,
) -> Result<QualityMigrationReportOutput, AutomationError> {
    let bytes = fs::read(input).map_err(|_| {
        AutomationError::new("No se pudo leer el contrato de calidad para el preflight.")
    })?;
    if bytes.len() as u64 > QUALITY_MIGRATION_REPORT_FILE_LIMIT_BYTES {
        return Err(AutomationError::new(
            "El contrato de calidad supera el límite local de 1 MiB.",
        ));
    }
    let document = serde_json::from_slice::<JsonValue>(&bytes)
        .map_err(|_| AutomationError::new("El contrato de calidad no es JSON válido."))?;
    let (source_format, source_version, rules) = migration_report_document(&document)?;
    if rules.len() > 16 {
        return Err(AutomationError::new(
            "El contrato contiene más de 16 reglas; Columnia requiere revisión antes de importar.",
        ));
    }
    let mut rule_reports = Vec::with_capacity(rules.len());
    let mut convertible_rules = 0;
    let mut unsupported_policy_rules = 0;
    let mut non_blocking_rules = 0;
    for (index, rule) in rules.iter().enumerate() {
        let (report, convertible, policy_issue, explicit_non_blocking) =
            migration_report_rule(index + 1, rule);
        convertible_rules += usize::from(convertible);
        unsupported_policy_rules += usize::from(policy_issue);
        non_blocking_rules += usize::from(explicit_non_blocking);
        rule_reports.push(report);
    }
    let omitted_rules = rules.len().saturating_sub(convertible_rules);
    let warning_count = rule_reports
        .iter()
        .filter(|rule| rule.status == "manual_review")
        .count();
    let mut manual_actions = vec!["Validar el contrato convertido antes de exportar."];
    if omitted_rules > 0 {
        manual_actions.push(
            "Revisar y recrear manualmente las reglas omitidas; ninguna omisión se aprueba de forma automática.",
        );
    }
    if unsupported_policy_rules > 0 {
        manual_actions.push(
            "Revisar severidades y políticas on_missing/null_policy antes de aceptar el contrato.",
        );
    }
    Ok(QualityMigrationReportOutput {
        schema_version: 1,
        command: "quality-migration-report",
        source_format,
        source_version,
        artifact_sha256: format!("{:x}", Sha256::digest(&bytes)),
        total_rules: rules.len(),
        convertible_rules,
        omitted_rules,
        warning_count,
        policies: QualityMigrationPolicySummary {
            blocking_rules: rules.len().saturating_sub(non_blocking_rules),
            non_blocking_rules,
            unsupported_policy_rules,
        },
        rules: rule_reports,
        manual_actions,
    })
}

fn parse_format(value: OsString) -> Result<AutomationFormat, AutomationError> {
    match value.to_str() {
        Some("csv") => Ok(AutomationFormat::Csv),
        Some("json") => Ok(AutomationFormat::Json),
        Some("parquet") => Ok(AutomationFormat::Parquet),
        Some("sql") => Ok(AutomationFormat::Sql),
        Some("excel" | "xlsx") => Ok(AutomationFormat::Excel),
        Some("sqlite") => Ok(AutomationFormat::Sqlite),
        Some("bundle" | "zip") => Ok(AutomationFormat::Bundle),
        _ => Err(AutomationError::new(
            "La opción --format debe ser csv, json, parquet, sql, excel, sqlite o bundle.",
        )),
    }
}

fn parse_input_options(
    flags: &mut HashMap<&'static str, OsString>,
) -> Result<(PathBuf, Option<String>, Option<SpreadsheetHeaderMode>), AutomationError> {
    let input = PathBuf::from(required_flag(flags, "--input")?);
    let sheet = flags
        .remove("--sheet")
        .map(|value| {
            value
                .into_string()
                .map_err(|_| AutomationError::new("El nombre de hoja no contiene texto válido."))
        })
        .transpose()?;
    let header = flags
        .remove("--header")
        .map(|value| match value.to_str() {
            Some("first-row") => Ok(SpreadsheetHeaderMode::FirstRow),
            Some("generated") => Ok(SpreadsheetHeaderMode::Generated),
            _ => Err(AutomationError::new(
                "La opción --header debe ser first-row o generated.",
            )),
        })
        .transpose()?;
    validate_input_options(&input, sheet.as_deref(), header)?;
    Ok((input, sheet, header))
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
        .ok_or_else(|| {
            AutomationError::new("Falta el comando inspect, transform, validate o batch.")
        })?;
    let rest = &arguments[1..];

    match subcommand {
        "inspect" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(INSPECT_HELP));
            }
            let (mut flags, _) = parse_flags(rest, &["--input", "--sheet", "--header"], &[])?;
            let (input, sheet, header) = parse_input_options(&mut flags)?;
            Ok(CliCommand::Inspect {
                input,
                sheet,
                header,
            })
        }
        "transform" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(TRANSFORM_HELP));
            }
            let (mut flags, _) = parse_flags(
                rest,
                &[
                    "--input", "--sheet", "--header", "--recipe", "--output", "--format",
                ],
                &[],
            )?;
            let (input, sheet, header) = parse_input_options(&mut flags)?;
            let recipe = PathBuf::from(required_flag(&mut flags, "--recipe")?);
            let output = PathBuf::from(required_flag(&mut flags, "--output")?);
            let format = parse_format(required_flag(&mut flags, "--format")?)?;
            Ok(CliCommand::Transform {
                input,
                sheet,
                header,
                recipe,
                output,
                format,
            })
        }
        "validate" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(VALIDATE_HELP));
            }
            let (mut flags, _) =
                parse_flags(rest, &["--input", "--sheet", "--header", "--rules"], &[])?;
            let (input, sheet, header) = parse_input_options(&mut flags)?;
            let rules = PathBuf::from(required_flag(&mut flags, "--rules")?);
            Ok(CliCommand::Validate {
                input,
                sheet,
                header,
                rules,
            })
        }
        "quality-migration-report" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(QUALITY_MIGRATION_REPORT_HELP));
            }
            let (mut flags, _) = parse_flags(rest, &["--rules"], &[])?;
            Ok(CliCommand::QualityMigrationReport {
                rules: PathBuf::from(required_flag(&mut flags, "--rules")?),
            })
        }
        "batch" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(BATCH_HELP));
            }
            let (mut flags, switches) = parse_flags(rest, &["--manifest"], &["--force"])?;
            Ok(CliCommand::Batch {
                manifest: PathBuf::from(required_flag(&mut flags, "--manifest")?),
                force: switches.contains("--force"),
            })
        }
        "project-list" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(PROJECT_LIST_HELP));
            }
            let (mut flags, _) = parse_flags(rest, &["--store"], &[])?;
            Ok(CliCommand::ProjectList {
                store: PathBuf::from(required_flag(&mut flags, "--store")?),
            })
        }
        "project-save" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(PROJECT_SAVE_HELP));
            }
            let (mut flags, switches) = parse_flags(
                rest,
                &[
                    "--store", "--name", "--input", "--id", "--sheet", "--header", "--recipe",
                    "--rules",
                ],
                &["--profile"],
            )?;
            let store = PathBuf::from(required_flag(&mut flags, "--store")?);
            let name = text_flag(&mut flags, "--name")?;
            let (input, sheet, header) = parse_input_options(&mut flags)?;
            let id = optional_text_flag(&mut flags, "--id")?;
            let recipe = flags.remove("--recipe").map(PathBuf::from);
            let rules = flags.remove("--rules").map(PathBuf::from);
            Ok(CliCommand::ProjectSave {
                store,
                name,
                input,
                id,
                sheet,
                header,
                recipe,
                rules,
                profile: switches.contains("--profile"),
            })
        }
        "project-inspect" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(PROJECT_INSPECT_HELP));
            }
            let (mut flags, _) = parse_flags(rest, &["--store", "--id"], &[])?;
            Ok(CliCommand::ProjectInspect {
                store: PathBuf::from(required_flag(&mut flags, "--store")?),
                id: text_flag(&mut flags, "--id")?,
            })
        }
        "project-export" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(PROJECT_EXPORT_HELP));
            }
            let (mut flags, switches) = parse_flags(
                rest,
                &["--store", "--id", "--output", "--format"],
                &["--allow-unvalidated"],
            )?;
            Ok(CliCommand::ProjectExport {
                store: PathBuf::from(required_flag(&mut flags, "--store")?),
                id: text_flag(&mut flags, "--id")?,
                output: PathBuf::from(required_flag(&mut flags, "--output")?),
                format: parse_format(required_flag(&mut flags, "--format")?)?,
                allow_unvalidated: switches.contains("--allow-unvalidated"),
            })
        }
        "project-delete" => {
            if matches!(rest, [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h"))
            {
                return Ok(CliCommand::Help(PROJECT_DELETE_HELP));
            }
            let (mut flags, _) = parse_flags(rest, &["--store", "--id", "--confirm"], &[])?;
            Ok(CliCommand::ProjectDelete {
                store: PathBuf::from(required_flag(&mut flags, "--store")?),
                id: text_flag(&mut flags, "--id")?,
                confirm: text_flag(&mut flags, "--confirm")?,
            })
        }
        _ => Err(AutomationError::new(
            "Comando desconocido. Usa --help para ver la interfaz admitida.",
        )),
    }
}

fn validate_input_options(
    input: &Path,
    sheet: Option<&str>,
    header: Option<SpreadsheetHeaderMode>,
) -> Result<(), AutomationError> {
    let extension = input
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("csv" | "tsv" | "json" | "parquet") if sheet.is_none() && header.is_none() => Ok(()),
        Some("csv" | "tsv" | "json" | "parquet") => Err(AutomationError::new(WORKBOOK_FLAGS)),
        Some("xlsx" | "xls" | "xlsb" | "ods") if sheet.is_some() && header.is_some() => Ok(()),
        Some("xlsx" | "xls" | "xlsb" | "ods") => Err(AutomationError::new(WORKBOOK_FLAGS)),
        _ => Err(AutomationError::new(
            "La entrada debe ser CSV, TSV, JSON, Parquet, XLSX, XLS, XLSB u ODS.",
        )),
    }
}

pub fn inspect(
    input: &Path,
    sheet: Option<&str>,
    header: Option<SpreadsheetHeaderMode>,
) -> Result<InspectOutput, AutomationError> {
    validate_input_options(input, sheet, header)?;
    let (_, preview) =
        dataset::load_dataset_for_automation(input, sheet, header).map_err(|_| {
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
    sheet: Option<&str>,
    header: Option<SpreadsheetHeaderMode>,
    recipe: &Path,
    output: &Path,
    format: AutomationFormat,
) -> Result<TransformOutput, AutomationError> {
    validate_input_options(input, sheet, header)?;
    let output_matches_format = output
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()));
    if !output_matches_format {
        return Err(AutomationError::new(
            "La extensión de --output debe coincidir con --format.",
        ));
    }

    let (source, _) = dataset::load_dataset_for_automation(input, sheet, header).map_err(|_| {
        AutomationError::new(
            "No se pudo cargar el dataset. Verifica que sea un archivo regular y válido.",
        )
    })?;
    let stored_recipe = dataset::load_stored_recipe_for_automation(recipe)
        .map_err(|_| AutomationError::new("No se pudo cargar una receta Columnia válida."))?;
    let summary_before = (source.height(), source.width());
    let (candidate, changed) = dataset::apply_recipe_for_automation(&source, &stored_recipe.recipe)
        .map_err(|_| AutomationError::new("La receta no es válida para el dataset de entrada."))?;
    let summary_after = (candidate.height(), candidate.width());
    let exported = dataset::export_frame_for_automation_with_recipe(
        &candidate,
        output,
        format.dataset_format(),
        Some(&stored_recipe),
    )
    .map_err(|_| {
        AutomationError::new("No se pudo publicar el archivo de salida de forma atómica.")
    })?;

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

pub fn validate(
    input: &Path,
    sheet: Option<&str>,
    header: Option<SpreadsheetHeaderMode>,
    rules: &Path,
) -> Result<ValidateOutput, AutomationError> {
    validate_input_options(input, sheet, header)?;
    let (source, _) = dataset::load_dataset_for_automation(input, sheet, header).map_err(|_| {
        AutomationError::new(
            "No se pudo cargar el dataset. Verifica que sea un archivo regular y válido.",
        )
    })?;
    let quality_rules = dataset::load_quality_rules_for_automation(rules)
        .map_err(|_| AutomationError::new("No se pudo cargar un contrato de calidad v1 válido."))?;
    let result =
        dataset::evaluate_quality_rules_for_automation(&source, &quality_rules).map_err(|_| {
            AutomationError::new("El contrato de calidad no es válido para el dataset.")
        })?;
    Ok(ValidateOutput {
        schema_version: 1,
        command: "validate",
        passed: result.passed,
        row_count: result.row_count,
        total_rules: result.total_rules,
        passed_rules: result.total_rules.saturating_sub(result.failed_rules),
        failed_rules: result.failed_rules,
        total_invalid_count: result.total_invalid_count(),
    })
}

pub fn project_list(store: &Path) -> Result<ProjectListOutput, AutomationError> {
    let projects = projects::automation_list_projects(store)
        .map_err(|_| AutomationError::new("No se pudo abrir el almacén de proyectos."))?;
    Ok(ProjectListOutput {
        schema_version: 1,
        command: "project-list",
        projects,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn project_save(
    store: &Path,
    name: String,
    input: &Path,
    id: Option<String>,
    sheet: Option<&str>,
    header: Option<SpreadsheetHeaderMode>,
    recipe: Option<&Path>,
    rules: Option<&Path>,
    profile: bool,
) -> Result<ProjectSaveOutput, AutomationError> {
    validate_input_options(input, sheet, header)?;
    let existed = match id.as_deref() {
        Some(id) => projects::automation_list_projects(store)
            .map_err(|_| AutomationError::new("No se pudo abrir el almacén de proyectos."))?
            .iter()
            .any(|project| project.id == id),
        None => false,
    };
    let (frame, preview) =
        dataset::load_dataset_for_automation(input, sheet, header).map_err(|_| {
            AutomationError::new(
                "No se pudo cargar el dataset. Verifica que sea un archivo regular y válido.",
            )
        })?;
    let dataset = DatasetState::for_project_import(frame, preview.file_name)
        .map_err(|_| AutomationError::new("No se pudo preparar el proyecto."))?;
    let recipe_draft = recipe
        .map(|path| {
            dataset::load_stored_recipe_for_automation(path)
                .map_err(|_| AutomationError::new("No se pudo cargar una receta Columnia válida."))
        })
        .transpose()?;
    if let Some(recipe) = recipe_draft.as_ref() {
        dataset
            .apply_project_import_recipe(&recipe.recipe)
            .map_err(|_| {
                AutomationError::new("La receta no es válida para el dataset de entrada.")
            })?;
    }
    let quality_rules = rules
        .map(|path| {
            dataset::load_quality_rules_for_automation(path).map_err(|_| {
                AutomationError::new("No se pudo cargar un contrato de calidad v1 válido.")
            })
        })
        .transpose()?
        .unwrap_or_default();
    if profile {
        dataset
            .cache_project_import_profile()
            .map_err(|_| AutomationError::new("No se pudo calcular el perfil del proyecto."))?;
    }
    let project = projects::automation_import_project(
        store,
        &dataset,
        id,
        name,
        ProjectWorkspace {
            quality_rules,
            recipe_draft,
            sql_history: Vec::new(),
            review_tab: Default::default(),
            preview_offset: Default::default(),
            active_phase: Default::default(),
            query_engine: Default::default(),
            analysis_sample_rows: Default::default(),
            performance_profile: Default::default(),
            export_format: Default::default(),
            privacy_mode: Default::default(),
            comparison_key_columns: Default::default(),
            join_type: Default::default(),
        },
    )
    .map_err(|_| AutomationError::new("No se pudo guardar el proyecto."))?;
    Ok(ProjectSaveOutput {
        schema_version: 1,
        command: "project-save",
        created: !existed,
        project,
    })
}

pub fn project_inspect(store: &Path, id: &str) -> Result<ProjectInspectOutput, AutomationError> {
    let inspection = projects::automation_inspect_project(store, id).map_err(|error| {
        AutomationError::new(format!(
            "No se pudo inspeccionar el proyecto solicitado. Fase durable: {error}"
        ))
    })?;
    Ok(ProjectInspectOutput {
        schema_version: 1,
        command: "project-inspect",
        project: inspection.project,
        profile_cached: inspection.profile_cached,
        quality_rule_count: inspection.quality_rule_count,
        recipe_draft_present: inspection.recipe_draft_present,
        history: ProjectHistoryOutput {
            entry_count: inspection.history.entry_count,
            current_index: inspection.history.current_index,
            can_undo: inspection.history.can_undo,
            can_redo: inspection.history.can_redo,
            snapshots_enabled: inspection.history.snapshots_enabled,
            degraded: inspection.history.degraded,
        },
    })
}

pub fn project_export(
    store: &Path,
    id: &str,
    output: &Path,
    format: AutomationFormat,
    allow_unvalidated: bool,
) -> Result<ProjectExportOutput, AutomationError> {
    if !output
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()))
    {
        return Err(AutomationError::new(
            "La extensión de --output debe coincidir con --format.",
        ));
    }
    let opened = projects::automation_open_project(store, id)
        .map_err(|_| AutomationError::new("No se pudo abrir el proyecto solicitado."))?;
    let total_rules = opened.workspace.quality_rules.len();
    if total_rules == 0 && !allow_unvalidated {
        return Err(AutomationError::new(
            "El proyecto no tiene reglas; usa --allow-unvalidated para autorizar la exportación.",
        ));
    }
    let quality = if total_rules == 0 {
        ProjectQualityOutput {
            validated: false,
            passed: None,
            row_count: opened.frame.height(),
            total_rules: 0,
            passed_rules: 0,
            failed_rules: 0,
            total_invalid_count: 0,
        }
    } else {
        let result = dataset::evaluate_quality_rules_for_automation(
            &opened.frame,
            &opened.workspace.quality_rules,
        )
        .map_err(|_| AutomationError::new("Las reglas guardadas del proyecto no son válidas."))?;
        let quality = ProjectQualityOutput {
            validated: true,
            passed: Some(result.passed),
            row_count: result.row_count,
            total_rules: result.total_rules,
            passed_rules: result.total_rules.saturating_sub(result.failed_rules),
            failed_rules: result.failed_rules,
            total_invalid_count: result.total_invalid_count(),
        };
        if !result.passed {
            return Ok(ProjectExportOutput {
                schema_version: 1,
                command: "project-export",
                status: "blocked",
                file_name: None,
                file_size_bytes: None,
                format: format.label(),
                quality,
            });
        }
        quality
    };
    let exported = dataset::export_frame_for_automation_with_recipe(
        &opened.frame,
        output,
        format.dataset_format(),
        opened.workspace.recipe_draft.as_ref(),
    )
    .map_err(|_| AutomationError::new("No se pudo publicar la salida de forma atómica."))?;
    Ok(ProjectExportOutput {
        schema_version: 1,
        command: "project-export",
        status: "succeeded",
        file_name: Some(exported.file_name),
        file_size_bytes: Some(exported.file_size_bytes),
        format: exported.format,
        quality,
    })
}

pub fn project_delete(
    store: &Path,
    id: String,
    confirm: &str,
) -> Result<ProjectDeleteOutput, AutomationError> {
    if id != confirm {
        return Err(AutomationError::new(
            "La confirmación no coincide exactamente con el identificador.",
        ));
    }
    projects::automation_delete_project(store, &id)
        .map_err(|_| AutomationError::new("No se pudo eliminar el proyecto solicitado."))?;
    Ok(ProjectDeleteOutput {
        schema_version: 1,
        command: "project-delete",
        id,
        deleted: true,
    })
}

fn resolve_manifest_path(base: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn has_unsafe_relative_segments(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}

#[cfg(windows)]
fn output_collision_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn output_collision_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn validate_batch_text_budget(manifest: &BatchManifest) -> Result<(), AutomationError> {
    let mut total = 0_usize;
    for job in &manifest.jobs {
        for value in [
            Some(job.input.as_str()),
            Some(job.recipe.as_str()),
            Some(job.output.as_str()),
            job.sheet.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let chars = value.chars().count();
            if chars == 0 || chars > BATCH_MAX_FIELD_CHARS {
                return Err(AutomationError::new(
                    "Un campo de texto del manifiesto está vacío o supera el límite permitido.",
                ));
            }
            total = total.saturating_add(chars);
            if total > BATCH_MAX_TOTAL_TEXT_CHARS {
                return Err(AutomationError::new(
                    "El texto acumulado del manifiesto supera el límite permitido.",
                ));
            }
        }
    }
    Ok(())
}

fn prepare_batch(
    manifest_path: &Path,
    force: bool,
) -> Result<Vec<PreparedBatchJob>, AutomationError> {
    let canonical_manifest =
        dataset::canonicalize_file_for_automation(manifest_path).map_err(|_| {
            AutomationError::new("No se pudo cargar un manifiesto batch regular y válido.")
        })?;
    let metadata = fs::metadata(&canonical_manifest)
        .map_err(|_| AutomationError::new("No se pudo verificar el manifiesto batch."))?;
    if metadata.len() > BATCH_FILE_LIMIT_BYTES {
        return Err(AutomationError::new(
            "El manifiesto batch supera el límite de tamaño permitido.",
        ));
    }
    let encoded = fs::read(&canonical_manifest)
        .map_err(|_| AutomationError::new("No se pudo leer el manifiesto batch."))?;
    let manifest: BatchManifest = serde_json::from_slice(&encoded).map_err(|_| {
        AutomationError::new("El manifiesto batch no es un documento JSON v1 válido.")
    })?;
    if manifest.version != 1 || manifest.jobs.is_empty() || manifest.jobs.len() > BATCH_MAX_JOBS {
        return Err(AutomationError::new(
            "El manifiesto batch debe usar la versión 1 y declarar entre 1 y 64 trabajos.",
        ));
    }
    validate_batch_text_budget(&manifest)?;
    let base = canonical_manifest
        .parent()
        .expect("un archivo canonicalizado siempre tiene carpeta");
    let output_root = if let Some(requested_root) = manifest.output_root.as_deref() {
        if !force && has_unsafe_relative_segments(requested_root) {
            return Err(AutomationError::new(
                "outputRoot batch debe ser relativo y no puede contener traversal.",
            ));
        }
        resolve_manifest_path(base, requested_root)
    } else {
        base.to_owned()
    };
    let canonical_output_root = fs::canonicalize(&output_root).map_err(|_| {
        AutomationError::new("El outputRoot batch debe ser un directorio existente y válido.")
    })?;
    if !canonical_output_root.is_dir() {
        return Err(AutomationError::new(
            "El outputRoot batch debe ser un directorio existente y válido.",
        ));
    }
    if !force && !canonical_output_root.starts_with(base) {
        return Err(AutomationError::new(
            "El outputRoot batch debe permanecer dentro de la carpeta del manifiesto.",
        ));
    }
    let mut prepared = Vec::with_capacity(manifest.jobs.len());
    let mut output_keys = HashSet::with_capacity(manifest.jobs.len());
    let mut source_keys = HashSet::with_capacity(manifest.jobs.len() * 2 + 1);
    source_keys.insert(output_collision_key(&canonical_manifest));

    for job in manifest.jobs {
        let input = resolve_manifest_path(base, &job.input);
        let recipe = resolve_manifest_path(base, &job.recipe);
        if !force && has_unsafe_relative_segments(&job.output) {
            return Err(AutomationError::new(
                "Una salida batch debe ser relativa y no puede contener traversal.",
            ));
        }
        let output = resolve_manifest_path(&canonical_output_root, &job.output);
        let format = AutomationFormat::from(job.format);
        validate_input_options(&input, job.sheet.as_deref(), job.header)?;
        if output
            .extension()
            .and_then(OsStr::to_str)
            .is_none_or(|extension| !extension.eq_ignore_ascii_case(format.extension()))
        {
            return Err(AutomationError::new(
                "La extensión de una salida batch no coincide con su formato.",
            ));
        }
        let canonical_input = dataset::canonicalize_file_for_automation(&input)
            .map_err(|_| AutomationError::new("Un input batch no es un archivo regular válido."))?;
        let canonical_recipe =
            dataset::canonicalize_file_for_automation(&recipe).map_err(|_| {
                AutomationError::new("Una receta batch no es un archivo regular válido.")
            })?;
        dataset::load_dataset_for_automation(&canonical_input, job.sheet.as_deref(), job.header)
            .map_err(|_| {
                AutomationError::new("Un input o selección de libro batch no es válido.")
            })?;
        dataset::load_recipe_for_automation(&canonical_recipe)
            .map_err(|_| AutomationError::new("Una receta batch no es un documento válido."))?;
        let canonical_output = dataset::canonicalize_output_for_automation(&output)
            .map_err(|_| AutomationError::new("Un destino batch no es válido."))?;
        if !force && !canonical_output.starts_with(&canonical_output_root) {
            return Err(AutomationError::new(
                "Una salida batch debe permanecer dentro del outputRoot.",
            ));
        }
        if !force && fs::symlink_metadata(&output).is_ok() {
            return Err(AutomationError::new(
                "Una salida batch existente requiere --force para reemplazarse.",
            ));
        }
        let output_key = output_collision_key(&canonical_output);
        if !output_keys.insert(output_key) {
            return Err(AutomationError::new(
                "Dos trabajos batch no pueden compartir el mismo destino.",
            ));
        }
        source_keys.insert(output_collision_key(&canonical_input));
        source_keys.insert(output_collision_key(&canonical_recipe));
        prepared.push(PreparedBatchJob {
            input: canonical_input,
            recipe: canonical_recipe,
            output: canonical_output,
            format,
            sheet: job.sheet,
            header: job.header,
        });
    }

    if output_keys
        .iter()
        .any(|output| source_keys.contains(output))
    {
        return Err(AutomationError::new(
            "Una salida batch no puede sobrescribir el manifiesto, un input o una receta del lote.",
        ));
    }
    Ok(prepared)
}

pub fn batch(manifest_path: &Path) -> Result<BatchOutput, AutomationError> {
    batch_with_options(manifest_path, false)
}

pub fn batch_with_options(
    manifest_path: &Path,
    force: bool,
) -> Result<BatchOutput, AutomationError> {
    let jobs = prepare_batch(manifest_path, force)?;
    let total_jobs = jobs.len();
    let mut completed_jobs = 0;
    let mut changed_jobs = 0;
    for (index, job) in jobs.iter().enumerate() {
        match transform(
            &job.input,
            job.sheet.as_deref(),
            job.header,
            &job.recipe,
            &job.output,
            job.format,
        ) {
            Ok(result) => {
                completed_jobs += 1;
                changed_jobs += usize::from(result.changed);
            }
            Err(_) => {
                return Ok(BatchOutput {
                    schema_version: 1,
                    command: "batch",
                    status: "failed",
                    total_jobs,
                    completed_jobs,
                    changed_jobs,
                    failed_job_number: Some(index + 1),
                });
            }
        }
    }
    Ok(BatchOutput {
        schema_version: 1,
        command: "batch",
        status: "succeeded",
        total_jobs,
        completed_jobs,
        changed_jobs,
        failed_job_number: None,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn sanitized_output<T: Serialize>(value: &T) -> JsonValue {
        crate::privacy::sanitized_json(value).expect("el contrato CLI debe sanitizarse")
    }

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
    fn parser_accepts_excel_and_sqlite_destinations() {
        let excel = parse_cli_args([
            "transform",
            "--input",
            "dataset.csv",
            "--recipe",
            "recipe.json",
            "--output",
            "result.xlsx",
            "--format",
            "excel",
        ])
        .unwrap();
        assert!(matches!(
            excel,
            CliCommand::Transform {
                format: AutomationFormat::Excel,
                ..
            }
        ));

        let sqlite = parse_cli_args([
            "transform",
            "--input",
            "dataset.csv",
            "--recipe",
            "recipe.json",
            "--output",
            "result.sqlite",
            "--format",
            "sqlite",
        ])
        .unwrap();
        assert!(matches!(
            sqlite,
            CliCommand::Transform {
                format: AutomationFormat::Sqlite,
                ..
            }
        ));

        let bundle = parse_cli_args([
            "transform",
            "--input",
            "dataset.csv",
            "--recipe",
            "recipe.json",
            "--output",
            "result.zip",
            "--format",
            "bundle",
        ])
        .unwrap();
        assert!(matches!(
            bundle,
            CliCommand::Transform {
                format: AutomationFormat::Bundle,
                ..
            }
        ));
    }

    #[test]
    fn inspect_emits_schema_without_rows_or_paths() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        fs::write(&input, "name,count\nA,1\nB,2\n").unwrap();

        let output = inspect(&input, None, None).unwrap();
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

        let result =
            transform(&input, None, None, &recipe, &output, AutomationFormat::Csv).unwrap();
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
    fn transform_exports_json_with_the_same_atomic_contract() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("recipe.json");
        let output = directory.path().join("result.json");
        fs::write(&input, "city,value\nSanto Domingo,30\n").unwrap();
        write_recipe(&recipe, "city", "place");

        let result =
            transform(&input, None, None, &recipe, &output, AutomationFormat::Json).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["outputFileName"], "result.json");
        assert_eq!(json["format"], "JSON");
        let rows: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(rows[0]["place"], "Santo Domingo");
    }

    #[test]
    fn transform_exports_sql_with_the_same_atomic_contract() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("recipe.json");
        let output = directory.path().join("result.sql");
        fs::write(&input, "city,value\nSanto Domingo,30\n").unwrap();
        write_recipe(&recipe, "city", "place");

        let result =
            transform(&input, None, None, &recipe, &output, AutomationFormat::Sql).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["outputFileName"], "result.sql");
        assert_eq!(json["format"], "SQL");
        let script = fs::read_to_string(output).unwrap();
        assert!(script.contains("\"place\" TEXT"));
        assert!(script.contains("'Santo Domingo'"));
    }

    #[test]
    fn transform_exports_excel_and_sqlite_with_the_same_atomic_contract() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("recipe.json");
        fs::write(&input, "city,value\nSanto Domingo,30\n").unwrap();
        write_recipe(&recipe, "city", "place");

        let excel = directory.path().join("result.xlsx");
        let excel_result =
            transform(&input, None, None, &recipe, &excel, AutomationFormat::Excel).unwrap();
        assert_eq!(excel_result.format, "Excel");
        assert!(fs::read(&excel).unwrap().starts_with(b"PK"));

        let sqlite = directory.path().join("result.sqlite");
        let sqlite_result = transform(
            &input,
            None,
            None,
            &recipe,
            &sqlite,
            AutomationFormat::Sqlite,
        )
        .unwrap();
        assert_eq!(sqlite_result.format, "SQLite");
        let connection = rusqlite::Connection::open(sqlite).unwrap();
        let place: String = connection
            .query_row("SELECT place FROM dataset", [], |row| row.get(0))
            .unwrap();
        assert_eq!(place, "Santo Domingo");
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

        let error = transform(
            &input,
            None,
            None,
            &recipe,
            &absent_output,
            AutomationFormat::Csv,
        )
        .unwrap_err();
        assert!(!absent_output.exists());
        assert_eq!(
            error.to_string(),
            "La receta no es válida para el dataset de entrada."
        );
        assert!(!error
            .to_string()
            .contains(directory.path().to_str().unwrap()));

        assert!(transform(
            &input,
            None,
            None,
            &recipe,
            &existing_output,
            AutomationFormat::Csv,
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(existing_output).unwrap(),
            "previous output"
        );
    }

    #[test]
    fn workbook_requires_explicit_sheet_and_header_selection() {
        let error = inspect(Path::new("book.xlsx"), None, None).unwrap_err();
        assert!(error.to_string().contains("obligatorios"));
        assert!(!error.to_string().contains("book.xlsx"));
    }

    #[test]
    fn validate_emits_counts_only_for_passing_and_failing_contracts() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let passing = directory.path().join("passing.json");
        let failing = directory.path().join("failing.json");
        fs::write(&input, "name,count\nA,1\n,2\n").unwrap();
        fs::write(
            &passing,
            br#"{"version":1,"rules":[{"column":"count","kind":"not_null","maxInvalid":0}]}"#,
        )
        .unwrap();
        fs::write(
            &failing,
            br#"{"version":1,"rules":[{"column":"name","kind":"non_empty","maxInvalid":0}]}"#,
        )
        .unwrap();

        let passed = validate(&input, None, None, &passing).unwrap();
        assert!(passed.passed());
        let failed = validate(&input, None, None, &failing).unwrap();
        assert!(!failed.passed());
        let json = serde_json::to_value(failed).unwrap();
        assert_eq!(json["rowCount"], 2);
        assert_eq!(json["totalRules"], 1);
        assert_eq!(json["passedRules"], 0);
        assert_eq!(json["failedRules"], 1);
        assert_eq!(json["totalInvalidCount"], 1);
        assert!(json.get("rules").is_none());
        assert!(json.get("columns").is_none());
        assert!(!json
            .to_string()
            .contains(directory.path().to_str().unwrap()));
    }

    #[test]
    fn validate_rejects_unknown_versions_and_fields() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.csv");
        let rules = directory.path().join("rules.json");
        fs::write(&input, "value\n1\n").unwrap();
        fs::write(&rules, br#"{"version":2,"rules":[]}"#).unwrap();
        assert!(validate(&input, None, None, &rules).is_err());
        fs::write(&rules, br#"{"version":1,"rules":[],"extra":true}"#).unwrap();
        assert!(validate(&input, None, None, &rules).is_err());
    }

    #[test]
    fn batch_success_resolves_relative_paths_and_emits_only_summary_counts() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("first.csv"), "old\nA\n").unwrap();
        fs::write(directory.path().join("second.csv"), "old\nB\n").unwrap();
        write_recipe(&directory.path().join("recipe.json"), "old", "new");
        fs::write(
            directory.path().join("batch.json"),
            br#"{"version":1,"jobs":[{"input":"first.csv","recipe":"recipe.json","output":"first-output.csv","format":"csv"},{"input":"second.csv","recipe":"recipe.json","output":"second-output.parquet","format":"parquet"}]}"#,
        )
        .unwrap();

        let result = batch(&directory.path().join("batch.json")).unwrap();
        assert!(!result.failed());
        assert!(directory.path().join("first-output.csv").is_file());
        assert!(directory.path().join("second-output.parquet").is_file());
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "succeeded");
        assert_eq!(json["totalJobs"], 2);
        assert_eq!(json["completedJobs"], 2);
        assert_eq!(json["changedJobs"], 2);
        assert_eq!(json.as_object().unwrap().len(), 6);
        assert!(json.get("failedJobNumber").is_none());
        assert!(!json
            .to_string()
            .contains(directory.path().to_str().unwrap()));

        let sanitized = sanitized_output(&result);
        assert_eq!(sanitized["command"], "batch");
        assert_eq!(sanitized["status"], "succeeded");
        assert_eq!(sanitized["totalJobs"], 2);
        assert_eq!(sanitized["completedJobs"], 2);
        assert_eq!(sanitized["changedJobs"], 2);
        assert!(!sanitized.to_string().contains("first-output.csv"));
        assert!(!sanitized
            .to_string()
            .contains(directory.path().to_str().unwrap()));
    }

    #[test]
    fn batch_rejects_unknown_fields_before_writing_any_output() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("input.csv"), "old\nA\n").unwrap();
        write_recipe(&directory.path().join("recipe.json"), "old", "new");
        fs::write(
            directory.path().join("batch.json"),
            br#"{"version":1,"jobs":[{"input":"input.csv","recipe":"recipe.json","output":"output.csv","format":"csv","unknown":true}]}"#,
        )
        .unwrap();

        assert!(batch(&directory.path().join("batch.json")).is_err());
        assert!(!directory.path().join("output.csv").exists());
    }

    #[test]
    fn batch_confines_outputs_and_requires_force_for_external_or_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(directory.path().join("input.csv"), "old\nA\n").unwrap();
        write_recipe(&directory.path().join("recipe.json"), "old", "new");
        let external_output = outside.join("external.csv");
        let manifest = directory.path().join("batch.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "jobs": [{
                    "input": "input.csv",
                    "recipe": "recipe.json",
                    "output": external_output,
                    "format": "csv"
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(batch(&manifest).is_err());
        assert!(!external_output.exists());
        assert!(!batch_with_options(&manifest, true).unwrap().failed());
        assert!(external_output.is_file());

        let existing_output = directory.path().join("existing.csv");
        fs::write(&existing_output, "previous\n").unwrap();
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "jobs": [{
                    "input": "input.csv",
                    "recipe": "recipe.json",
                    "output": "existing.csv",
                    "format": "csv"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(batch(&manifest).is_err());
        assert_eq!(fs::read_to_string(&existing_output).unwrap(), "previous\n");
        assert!(!batch_with_options(&manifest, true).unwrap().failed());
        assert!(fs::read_to_string(existing_output).unwrap().contains("new"));
    }

    #[test]
    fn batch_contract_enforces_root_shape_counts_text_budgets_and_workbook_options() {
        assert!(serde_json::from_str::<BatchManifest>(
            r#"{"version":1,"jobs":[],"unexpected":true}"#
        )
        .is_err());
        let workbook = serde_json::from_str::<BatchManifest>(
            r#"{"version":1,"jobs":[{"input":"book.xlsx","recipe":"recipe.json","output":"out.csv","format":"csv","sheet":"Data","header":"firstRow"}]}"#,
        )
        .unwrap();
        assert_eq!(workbook.jobs[0].sheet.as_deref(), Some("Data"));
        assert_eq!(
            workbook.jobs[0].header,
            Some(SpreadsheetHeaderMode::FirstRow)
        );

        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("batch.json");
        fs::write(&manifest, br#"{"version":1,"jobs":[]}"#).unwrap();
        assert!(batch(&manifest).is_err());

        let jobs = (0..=BATCH_MAX_JOBS)
            .map(|index| {
                serde_json::json!({
                    "input": "input.csv",
                    "recipe": "recipe.json",
                    "output": format!("output-{index}.csv"),
                    "format": "csv"
                })
            })
            .collect::<Vec<_>>();
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({ "version": 1, "jobs": jobs })).unwrap(),
        )
        .unwrap();
        assert!(batch(&manifest).is_err());

        let oversized = "x".repeat(BATCH_MAX_FIELD_CHARS + 1);
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "jobs": [{
                    "input": oversized,
                    "recipe": "recipe.json",
                    "output": "output.csv",
                    "format": "csv"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(batch(&manifest).is_err());
    }

    #[test]
    fn batch_detects_output_collisions_during_preflight() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("input.csv"), "old\nA\n").unwrap();
        write_recipe(&directory.path().join("recipe.json"), "old", "new");
        fs::write(
            directory.path().join("batch.json"),
            br#"{"version":1,"jobs":[{"input":"input.csv","recipe":"recipe.json","output":"same.csv","format":"csv"},{"input":"input.csv","recipe":"recipe.json","output":"./same.csv","format":"csv"}]}"#,
        )
        .unwrap();

        assert!(batch(&directory.path().join("batch.json")).is_err());
        assert!(!directory.path().join("same.csv").exists());
    }

    #[test]
    fn batch_late_failure_keeps_completed_atomic_outputs_and_reports_ordinal() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("input.csv"), "old\nA\n").unwrap();
        write_recipe(&directory.path().join("valid.json"), "old", "new");
        write_recipe(
            &directory.path().join("incompatible.json"),
            "missing",
            "new",
        );
        fs::write(
            directory.path().join("batch.json"),
            br#"{"version":1,"jobs":[{"input":"input.csv","recipe":"valid.json","output":"completed.csv","format":"csv"},{"input":"input.csv","recipe":"incompatible.json","output":"failed.csv","format":"csv"}]}"#,
        )
        .unwrap();

        let result = batch(&directory.path().join("batch.json")).unwrap();
        assert!(result.failed());
        assert!(directory.path().join("completed.csv").is_file());
        assert!(!directory.path().join("failed.csv").exists());
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["totalJobs"], 2);
        assert_eq!(json["completedJobs"], 1);
        assert_eq!(json["changedJobs"], 1);
        assert_eq!(json["failedJobNumber"], 2);
        assert_eq!(json.as_object().unwrap().len(), 7);
    }

    #[test]
    fn project_commands_persist_inspect_export_and_delete_without_paths() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("store");
        let input = directory.path().join("source.csv");
        let recipe = directory.path().join("recipe.json");
        let rules = directory.path().join("rules.json");
        let output = directory.path().join("export.csv");
        fs::write(&input, "old,amount\nA,1\nB,2\n").unwrap();
        write_recipe(&recipe, "old", "name");
        fs::write(
            &rules,
            br#"{"version":1,"rules":[{"column":"name","kind":"non_empty","maxInvalid":0}]}"#,
        )
        .unwrap();

        let saved = project_save(
            &store,
            "Ventas".to_owned(),
            &input,
            None,
            None,
            None,
            Some(&recipe),
            Some(&rules),
            true,
        )
        .unwrap();
        assert!(saved.created);
        assert_eq!(
            serde_json::to_value(&saved)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            4
        );
        let id = saved.project.id.clone();
        let list = project_list(&store).unwrap();
        assert_eq!(list.projects, vec![saved.project.clone()]);
        assert_eq!(
            serde_json::to_value(&list)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            3
        );

        let inspected = project_inspect(&store, &id).unwrap();
        assert!(inspected.profile_cached);
        assert!(inspected.recipe_draft_present);
        assert_eq!(inspected.quality_rule_count, 1);
        assert_eq!(inspected.history.entry_count, 2);
        assert_eq!(inspected.history.current_index, 1);
        assert_eq!(
            serde_json::to_value(&inspected)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            7
        );
        let sanitized_inspection = sanitized_output(&inspected);
        assert_eq!(sanitized_inspection["project"]["id"], id);
        assert_eq!(
            sanitized_inspection["project"]["datasetFileName"],
            "[redactado]"
        );
        assert_eq!(sanitized_inspection["project"]["rowCount"], 2);
        assert_eq!(sanitized_inspection["qualityRuleCount"], 1);
        assert_eq!(sanitized_inspection["history"]["entryCount"], 2);
        assert!(!sanitized_inspection.to_string().contains("source.csv"));

        let exported = project_export(&store, &id, &output, AutomationFormat::Csv, false).unwrap();
        assert!(!exported.blocked());
        assert!(output.is_file());
        assert_eq!(exported.quality.passed, Some(true));
        assert_eq!(exported.quality.total_rules, 1);
        let json = serde_json::to_value(&exported).unwrap();
        assert_eq!(json["command"], "project-export");
        assert_eq!(json["fileName"], "export.csv");
        assert_eq!(json.as_object().unwrap().len(), 7);
        assert_eq!(json["quality"].as_object().unwrap().len(), 7);
        assert!(json.get("path").is_none());
        assert!(!json
            .to_string()
            .contains(directory.path().to_str().unwrap()));
        let sanitized_export = sanitized_output(&exported);
        assert_eq!(sanitized_export["command"], "project-export");
        assert_eq!(sanitized_export["status"], "succeeded");
        assert_eq!(sanitized_export["fileName"], "[redactado]");
        assert_eq!(sanitized_export["quality"]["passed"], true);
        assert_eq!(sanitized_export["quality"]["rowCount"], 2);
        assert!(!sanitized_export.to_string().contains("export.csv"));
        assert!(!sanitized_export
            .to_string()
            .contains(directory.path().to_str().unwrap()));

        assert!(project_delete(&store, id.clone(), "different").is_err());
        assert_eq!(project_list(&store).unwrap().projects.len(), 1);
        let deleted = project_delete(&store, id.clone(), &id).unwrap();
        assert!(deleted.deleted);
        assert_eq!(deleted.id, id);
        assert_eq!(
            serde_json::to_value(&deleted)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            4
        );
        assert!(project_list(&store).unwrap().projects.is_empty());
    }

    #[test]
    fn project_export_quality_gate_never_replaces_or_creates_output() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("store");
        let input = directory.path().join("source.csv");
        let rules = directory.path().join("rules.json");
        let absent = directory.path().join("absent.csv");
        let existing = directory.path().join("existing.csv");
        fs::write(&input, "name,other\nA,x\n,y\n").unwrap();
        fs::write(
            &rules,
            br#"{"version":1,"rules":[{"column":"name","kind":"non_empty","maxInvalid":0}]}"#,
        )
        .unwrap();
        let saved = project_save(
            &store,
            "Bloqueado".to_owned(),
            &input,
            None,
            None,
            None,
            None,
            Some(&rules),
            false,
        )
        .unwrap();
        fs::write(&existing, "previous").unwrap();

        for output in [&absent, &existing] {
            let result = project_export(
                &store,
                &saved.project.id,
                output,
                AutomationFormat::Csv,
                true,
            )
            .unwrap();
            assert!(result.blocked());
            assert_eq!(result.quality.passed, Some(false));
            assert!(result.file_name.is_none());
            assert_eq!(
                serde_json::to_value(&result)
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .len(),
                5
            );
            let sanitized = sanitized_output(&result);
            assert_eq!(sanitized["status"], "blocked");
            assert_eq!(sanitized["quality"]["passed"], false);
            assert_eq!(sanitized["quality"]["rowCount"], 2);
            assert!(sanitized.get("fileName").is_none());
            assert!(!sanitized.to_string().contains("absent.csv"));
            assert!(!sanitized.to_string().contains("existing.csv"));
            assert!(!sanitized
                .to_string()
                .contains(directory.path().to_str().unwrap()));
        }
        assert!(!absent.exists());
        assert_eq!(fs::read_to_string(existing).unwrap(), "previous");
    }

    #[test]
    fn project_without_rules_requires_explicit_unvalidated_permission() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("store");
        let input = directory.path().join("source.csv");
        let output = directory.path().join("output.parquet");
        fs::write(&input, "value\n1\n").unwrap();
        let saved = project_save(
            &store,
            "Sin reglas".to_owned(),
            &input,
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .unwrap();

        assert!(project_export(
            &store,
            &saved.project.id,
            &output,
            AutomationFormat::Parquet,
            false
        )
        .is_err());
        assert!(!output.exists());
        let result = project_export(
            &store,
            &saved.project.id,
            &output,
            AutomationFormat::Parquet,
            true,
        )
        .unwrap();
        assert!(!result.quality.validated);
        assert_eq!(result.quality.passed, None);
        assert!(output.is_file());
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

        let validate = serde_json::to_value(ValidateOutput {
            schema_version: 1,
            command: "validate",
            passed: true,
            row_count: 2,
            total_rules: 1,
            passed_rules: 1,
            failed_rules: 0,
            total_invalid_count: 0,
        })
        .unwrap();
        assert_eq!(
            validate.as_object().unwrap().keys().collect::<Vec<_>>(),
            [
                "command",
                "failedRules",
                "passed",
                "passedRules",
                "rowCount",
                "schemaVersion",
                "totalInvalidCount",
                "totalRules",
            ]
        );
    }

    #[test]
    fn quality_migration_report_accepts_columnia_v1_and_requires_no_review() {
        let directory = tempfile::tempdir().unwrap();
        let rules = directory.path().join("columnia.json");
        fs::write(
            &rules,
            serde_json::to_vec(&serde_json::json!({
                "format": "columnia-quality-rules",
                "version": 1,
                "rules": [{ "kind": "unique", "column": "id" }]
            }))
            .unwrap(),
        )
        .unwrap();

        let report = quality_migration_report(&rules).unwrap();

        assert_eq!(report.source_format, "columnia");
        assert_eq!(report.source_version.as_deref(), Some("1"));
        assert_eq!(report.convertible_rules, 1);
        assert_eq!(report.omitted_rules, 0);
        assert!(!report.requires_manual_review());
    }
}
