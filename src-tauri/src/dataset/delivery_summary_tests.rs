use super::*;
use ::zip::ZipArchive;
use std::{fs, io::Read};

fn sensitive_recipe() -> StoredTransformRecipe {
    StoredTransformRecipe {
        version: RECIPE_FILE_VERSION,
        name: "nombre de receta privado".to_owned(),
        saved_at: "2026-09-13T00:00:00Z".to_owned(),
        recipe: TransformRecipe {
            renames: vec![RecipeRename {
                from: "columna-sensible".to_owned(),
                to: "contacto".to_owned(),
            }],
            ..TransformRecipe::default()
        },
        source_schema: None,
        export_options: None,
    }
}

fn validation_result() -> QualityValidationResult {
    QualityValidationResult {
        passed: true,
        row_count: 2,
        total_rules: 1,
        failed_rules: 0,
        rules: vec![QualityRuleResult {
            column: "columna-sensible".to_owned(),
            kind: QualityRuleKind::Regex,
            max_invalid: None,
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
            pattern: Some("patron-privado".to_owned()),
            dtype: None,
            columns: None,
            operator: None,
            min_date: None,
            max_date: None,
            when: None,
            then: None,
            allow_additional: None,
            required_order: None,
            checked_count: 2,
            invalid_count: 0,
            invalid_pct: 0.0,
            passed: true,
        }],
    }
}

fn archive_member(archive: &mut ZipArchive<std::io::Cursor<Vec<u8>>>, name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("el miembro solicitado debe existir")
        .read_to_end(&mut bytes)
        .expect("el miembro debe poder leerse");
    bytes
}

fn verify_summary_and_manifest(destination: &Path, private_text: &[&str]) {
    let bytes = fs::read(destination).expect("el bundle debe poder leerse");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).expect("el ZIP debe abrirse");
    let summary_bytes = archive_member(&mut archive, BUNDLE_DELIVERY_SUMMARY_FILE);
    let summary = String::from_utf8(summary_bytes.clone()).expect("el resumen debe ser UTF-8");
    let manifest_bytes = archive_member(&mut archive, "manifest.json");
    let manifest: JsonValue =
        serde_json::from_slice(&manifest_bytes).expect("el manifest debe ser JSON");

    assert_eq!(manifest["version"], BUNDLE_MANIFEST_VERSION);
    assert_eq!(
        manifest["deliverySummaryFile"],
        BUNDLE_DELIVERY_SUMMARY_FILE
    );
    assert!(summary.contains("Filas: 2"));
    assert!(summary.contains("Columnas: 2"));
    assert!(summary.contains("Operaciones agregadas en la receta adjunta: 1"));
    assert!(summary.contains("Reglas con resultado incluido: 1 de 1"));
    assert!(summary.contains("Reglas aprobadas: 1"));
    assert!(summary.contains("columnia-bundle` v2"));
    assert!(summary.contains("La calidad se validó antes de aplicar la política de privacidad"));

    let files = manifest["files"]
        .as_array()
        .expect("el manifest debe enumerar archivos");
    let summary_entry = files
        .iter()
        .find(|file| file["path"] == BUNDLE_DELIVERY_SUMMARY_FILE)
        .expect("el manifest debe registrar el resumen");
    assert_eq!(summary_entry["bytes"], summary_bytes.len() as u64);
    assert_eq!(
        summary_entry["sha256"],
        hex::encode(Sha256::digest(&summary_bytes))
    );

    for member_name in [
        "dataset.csv",
        "dictionary.json",
        "quality-report.json",
        "recipe.json",
    ] {
        let member_bytes = archive_member(&mut archive, member_name);
        let expected_hash = hex::encode(Sha256::digest(&member_bytes));
        assert!(
            summary.contains(&expected_hash),
            "falta el hash de {member_name}"
        );
    }
    for value in private_text {
        assert!(
            !summary.contains(value),
            "el resumen no debe revelar {value}"
        );
    }
}

#[test]
fn materialized_bundle_includes_safe_aggregate_summary_and_hashes() {
    let frame = df![
        "columna-sensible" => &["persona-privada@example.com", "otra-persona@example.com"],
        "amount" => &[2_i64, 3_i64]
    ]
    .expect("el frame debe construirse");
    let validation = validation_result();
    let recipe = sensitive_recipe();
    let directory = tempfile::tempdir().expect("debe crearse el directorio temporal");
    let destination = directory.path().join("resultado.zip");

    export_frame_atomic_with_privacy_and_quality_and_recipe(
        &frame,
        &destination,
        ExportFormat::Bundle,
        PrivacyMode::None,
        Some(&validation),
        Some(&recipe),
        |_, _| {},
        || false,
    )
    .expect("el bundle materializado debe exportarse");

    verify_summary_and_manifest(
        &destination,
        &[
            "persona-privada@example.com",
            "nombre de receta privado",
            "columna-sensible",
            "patron-privado",
        ],
    );
}

#[test]
fn source_backed_bundle_includes_summary_without_source_path_or_values() {
    let source_directory = tempfile::tempdir().expect("debe crearse la fuente temporal");
    let source_path = source_directory.path().join("entrada-privada.csv");
    fs::write(
        &source_path,
        "columna-sensible,amount\npersona-privada@example.com,2\notra-persona@example.com,3\n",
    )
    .expect("la fuente CSV debe escribirse");
    let source_size = fs::metadata(&source_path)
        .expect("la fuente debe existir")
        .len();
    let destination_directory = tempfile::tempdir().expect("debe crearse la salida temporal");
    let destination = destination_directory
        .path()
        .join("resultado-source-backed.zip");
    let validation = validation_result();
    let recipe = sensitive_recipe();

    export_source_backed_bundle_atomic(
        &source_path,
        source_size,
        2,
        Some(&validation),
        Some(&recipe),
        &destination,
        |_, _| {},
        || false,
    )
    .expect("el bundle source-backed debe exportarse");

    verify_summary_and_manifest(
        &destination,
        &[
            "persona-privada@example.com",
            "nombre de receta privado",
            "columna-sensible",
            "patron-privado",
            source_path.to_string_lossy().as_ref(),
        ],
    );
}
