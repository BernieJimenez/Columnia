use std::{io::Write, path::Path};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tempfile::Builder as TempFileBuilder;

const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;
const MAX_ERROR_CODES: usize = 5;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum DiagnosticContract {
    #[serde(rename = "columnia-diagnostic-report")]
    ColumniaDiagnosticReport,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticPhase {
    Load,
    Review,
    Prepare,
    Deliver,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Ready,
    Working,
    IssueReported,
    NoDataset,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum DiagnosticErrorCode {
    DatasetLoadFailed,
    DatasetProfileFailed,
    DatasetReviewFailed,
    TransformApplyFailed,
    LocalExportFailed,
    DatabaseExportFailed,
    ProjectSaveFailed,
    UnknownOperationFailed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRowBucket {
    #[serde(rename = "not_applicable")]
    NotApplicable,
    #[serde(rename = "under_1k")]
    Under1k,
    #[serde(rename = "1k_to_9k")]
    OneKTo9k,
    #[serde(rename = "10k_to_99k")]
    TenKTo99k,
    #[serde(rename = "100k_to_999k")]
    HundredKTo999k,
    #[serde(rename = "1m_or_more")]
    OneMOrMore,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticColumnBucket {
    #[serde(rename = "not_applicable")]
    NotApplicable,
    #[serde(rename = "under_10")]
    Under10,
    #[serde(rename = "10_to_49")]
    TenTo49,
    #[serde(rename = "50_to_199")]
    FiftyTo199,
    #[serde(rename = "200_or_more")]
    TwoHundredOrMore,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSizeBucket {
    #[serde(rename = "not_applicable")]
    NotApplicable,
    #[serde(rename = "under_1_mib")]
    Under1Mib,
    #[serde(rename = "1_to_99_mib")]
    OneTo99Mib,
    #[serde(rename = "100_to_999_mib")]
    HundredTo999Mib,
    #[serde(rename = "1_gib_or_more")]
    OneGibOrMore,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticMetrics {
    dataset_loaded: bool,
    rows: DiagnosticRowBucket,
    columns: DiagnosticColumnBucket,
    source_size: DiagnosticSizeBucket,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticReport {
    contract: DiagnosticContract,
    schema_version: u32,
    app_version: String,
    phase: DiagnosticPhase,
    status: DiagnosticStatus,
    error_codes: Vec<DiagnosticErrorCode>,
    metrics: Option<DiagnosticMetrics>,
}

impl DiagnosticReport {
    fn validate(&self) -> Result<(), &'static str> {
        if self.contract != DiagnosticContract::ColumniaDiagnosticReport {
            return Err("El contrato del diagnóstico no es válido.");
        }
        if self.schema_version != DIAGNOSTIC_SCHEMA_VERSION {
            return Err("La versión del contrato de diagnóstico no es compatible.");
        }
        if self.app_version != env!("CARGO_PKG_VERSION") {
            return Err("La versión de Columnia no coincide con la aplicación activa.");
        }
        if self.error_codes.len() > MAX_ERROR_CODES {
            return Err("El diagnóstico supera el límite de códigos de error.");
        }
        for (index, code) in self.error_codes.iter().enumerate() {
            if self.error_codes[..index].contains(code) {
                return Err("El diagnóstico contiene códigos repetidos.");
            }
        }
        if (self.status == DiagnosticStatus::IssueReported) != !self.error_codes.is_empty() {
            return Err("El estado y los códigos del diagnóstico no coinciden.");
        }
        if let Some(metrics) = &self.metrics {
            let all_buckets_are_not_applicable = metrics.rows == DiagnosticRowBucket::NotApplicable
                && metrics.columns == DiagnosticColumnBucket::NotApplicable
                && metrics.source_size == DiagnosticSizeBucket::NotApplicable;
            let all_buckets_are_applicable = metrics.rows != DiagnosticRowBucket::NotApplicable
                && metrics.columns != DiagnosticColumnBucket::NotApplicable
                && metrics.source_size != DiagnosticSizeBucket::NotApplicable;
            let metrics_are_consistent = if metrics.dataset_loaded {
                all_buckets_are_applicable
            } else {
                all_buckets_are_not_applicable
            };
            if !metrics_are_consistent {
                return Err("Los rangos del diagnóstico no coinciden con el estado del dataset.");
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn save_diagnostic_report(
    app: AppHandle,
    report: DiagnosticReport,
) -> Result<Option<()>, String> {
    report.validate().map_err(str::to_owned)?;
    let selection = app
        .dialog()
        .file()
        .add_filter("Diagnóstico local de Columnia", &["json"])
        .set_file_name("columnia-diagnostic-v1.json")
        .blocking_save_file();
    let Some(selection) = selection else {
        return write_report_if_selected(&report, None);
    };
    let destination = selection
        .into_path()
        .map_err(|_| "No se pudo resolver el destino local del diagnóstico.".to_owned())?;
    let mut destination = destination;
    if destination
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("json")
    {
        destination.set_extension("json");
    }

    tauri::async_runtime::spawn_blocking(move || {
        write_report_if_selected(&report, Some(destination))
    })
    .await
    .map_err(|_| "El guardado local del diagnóstico se interrumpió.".to_owned())?
}

fn write_report_if_selected(
    report: &DiagnosticReport,
    destination: Option<std::path::PathBuf>,
) -> Result<Option<()>, String> {
    report.validate().map_err(str::to_owned)?;
    let Some(destination) = destination else {
        return Ok(None);
    };
    write_report_atomically(&destination, report)?;
    Ok(Some(()))
}

fn write_report_atomically(destination: &Path, report: &DiagnosticReport) -> Result<(), String> {
    report.validate().map_err(str::to_owned)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let json = serde_json::to_vec_pretty(report)
        .map_err(|_| "No se pudo serializar el diagnóstico local.".to_owned())?;
    let mut temporary = TempFileBuilder::new()
        .prefix(".columnia-diagnostic-")
        .tempfile_in(parent)
        .map_err(|_| "No se pudo preparar el guardado local del diagnóstico.".to_owned())?;
    temporary
        .write_all(&json)
        .map_err(|_| "No se pudo escribir el diagnóstico local.".to_owned())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "No se pudo sincronizar el diagnóstico local.".to_owned())?;
    temporary
        .persist(destination)
        .map_err(|_| "No se pudo publicar el archivo local del diagnóstico.".to_owned())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    fn report() -> DiagnosticReport {
        DiagnosticReport {
            contract: DiagnosticContract::ColumniaDiagnosticReport,
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            phase: DiagnosticPhase::Prepare,
            status: DiagnosticStatus::IssueReported,
            error_codes: vec![DiagnosticErrorCode::TransformApplyFailed],
            metrics: Some(DiagnosticMetrics {
                dataset_loaded: true,
                rows: DiagnosticRowBucket::TenKTo99k,
                columns: DiagnosticColumnBucket::Under10,
                source_size: DiagnosticSizeBucket::OneTo99Mib,
            }),
        }
    }

    #[test]
    fn contract_allowlist_rejects_free_text_secrets_and_unknown_fields() {
        let base = serde_json::to_value(report()).unwrap();
        let marker_fields = [
            ("errorMessage", "SECRET_RAW_ERROR"),
            ("path", "C:\\Users\\alice\\salary.csv"),
            ("query", "SELECT * FROM payroll"),
            ("datasetName", "CONFIDENTIAL_DATASET_NAME"),
            ("value", "PERSONAL_VALUE_MARKER"),
            ("email", "person@example.invalid"),
            ("credential", "PWD=TOP_SECRET_MARKER"),
            ("stackTrace", "SECRET_STACK_TRACE"),
            ("machineId", "MACHINE_IDENTIFIER_MARKER"),
            ("logs", "SECRET_LOG_MARKER"),
        ];
        for (field, marker) in marker_fields {
            let mut candidate = base.clone();
            candidate[field] = json!(marker);
            assert!(
                serde_json::from_value::<DiagnosticReport>(candidate).is_err(),
                "accepted {field}"
            );
        }

        let mut nested = base;
        nested["metrics"]["fileName"] = json!("CONFIDENTIAL_FILENAME_MARKER");
        assert!(serde_json::from_value::<DiagnosticReport>(nested).is_err());

        let serialized = serde_json::to_string(&report()).unwrap();
        for marker in [
            "SECRET_RAW_ERROR",
            "salary.csv",
            "SELECT *",
            "TOP_SECRET",
            "MACHINE_IDENTIFIER",
        ] {
            assert!(!serialized.contains(marker));
        }
    }

    #[test]
    fn contract_rejects_unbounded_or_inconsistent_codes_and_metrics() {
        let mut invalid = report();
        invalid.error_codes = vec![
            DiagnosticErrorCode::DatasetLoadFailed,
            DiagnosticErrorCode::DatasetProfileFailed,
            DiagnosticErrorCode::DatasetReviewFailed,
            DiagnosticErrorCode::TransformApplyFailed,
            DiagnosticErrorCode::LocalExportFailed,
            DiagnosticErrorCode::DatabaseExportFailed,
        ];
        assert!(invalid.validate().is_err());

        let mut invalid = report();
        invalid
            .error_codes
            .push(DiagnosticErrorCode::TransformApplyFailed);
        assert!(invalid.validate().is_err());

        let mut invalid = report();
        invalid.metrics.as_mut().unwrap().rows = DiagnosticRowBucket::NotApplicable;
        assert!(invalid.validate().is_err());

        let mut invalid = report();
        invalid.metrics.as_mut().unwrap().dataset_loaded = false;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn cancel_selection_writes_nothing_and_local_export_contains_only_the_contract() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("diagnostic.json");

        assert_eq!(write_report_if_selected(&report(), None).unwrap(), None);
        assert!(!destination.exists());

        assert_eq!(
            write_report_if_selected(&report(), Some(destination.clone())).unwrap(),
            Some(())
        );
        let content = fs::read_to_string(&destination).unwrap();
        let written: DiagnosticReport = serde_json::from_str(&content).unwrap();
        assert_eq!(written, report());
        assert!(!content.contains("path"));
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            1,
            "temporary files must be removed"
        );
    }

    #[test]
    fn invalid_version_or_schema_never_reaches_local_file() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join(PathBuf::from("diagnostic.json"));

        let mut invalid = report();
        invalid.schema_version = 2;
        assert!(write_report_if_selected(&invalid, Some(destination.clone())).is_err());
        assert!(!destination.exists());

        let mut invalid = report();
        invalid.app_version.push_str("-different");
        assert!(write_report_if_selected(&invalid, Some(destination.clone())).is_err());
        assert!(!destination.exists());
    }
}
