use super::*;
use polars::prelude::{DataFrame, IntoColumn, NamedFrom, Series};

fn workbook_profile() -> ImportProfile {
    ImportProfile {
        version: 1,
        format: "excel".to_owned(),
        sheet_name: Some("Datos".to_owned()),
        header_mode: Some(crate::dataset::SpreadsheetHeaderMode::FirstRow),
        date_convention: Some(crate::dataset::ImportDateConvention::Dmy),
        number_convention: Some(crate::dataset::ImportNumberConvention::CommaDecimalDotGrouping),
        schema: vec![
            crate::dataset::ImportProfileColumn {
                name: "id".to_owned(),
                data_type: "Int64".to_owned(),
            },
            crate::dataset::ImportProfileColumn {
                name: "amount".to_owned(),
                data_type: "Float64".to_owned(),
            },
        ],
    }
}

fn test_workspace(import_profile: Option<ImportProfile>) -> ProjectWorkspace {
    ProjectWorkspace {
        quality_rules: Vec::new(),
        recipe_draft: None,
        sql_history: Vec::new(),
        review_tab: None,
        preview_offset: None,
        active_phase: None,
        query_engine: None,
        analysis_sample_rows: None,
        performance_profile: None,
        export_format: None,
        privacy_mode: None,
        comparison_key_columns: Vec::new(),
        join_type: None,
        import_profile,
    }
}

fn active_test_dataset(directory: &Path) -> DatasetState {
    let source = directory.join("source.parquet");
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), [1_i64, 2]).into_column(),
            Series::new("amount".into(), [1.5_f64, 2.5]).into_column(),
        ],
    )
    .expect("el dataset de prueba debe ser válido");
    write_snapshot(&frame, &source).expect("se debe escribir el snapshot de prueba");
    let candidate = DatasetState::prepare_project_candidate(source, "reporte.xlsx".to_owned())
        .expect("se debe preparar el dataset de prueba");
    let state = DatasetState::default();
    state
        .activate_project_candidate(candidate)
        .expect("se debe activar el dataset de prueba");
    state
}

#[test]
fn import_profile_roundtrips_with_a_project_without_ephemeral_or_row_data() {
    let profile = workbook_profile();
    let profile_json = serde_json::to_string(&profile).unwrap();
    assert!(profile_json.contains("\"version\":1"));
    assert!(profile_json.contains("\"sheetName\":\"Datos\""));
    assert!(!profile_json.contains("selectionId"));
    assert!(!profile_json.contains("sourcePath"));
    assert!(!profile_json.contains("fileName"));
    assert!(!profile_json.contains("private-row-value"));

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("data");
    let store = ProjectStore::initialize(root.clone()).unwrap();
    let dataset = active_test_dataset(directory.path());
    let expected = test_workspace(Some(profile));
    let project = store
        .save(
            &dataset,
            None,
            "Importación periódica".to_owned(),
            expected.clone(),
        )
        .unwrap();
    drop(store);

    let reopened = ProjectStore::initialize(root).unwrap();
    let actual = reopened
        .open(&DatasetState::default(), project.id)
        .expect("el proyecto debe abrirse con su perfil");
    assert_eq!(actual.workspace, expected);
}

#[test]
fn schema_mismatch_lists_only_column_names_and_types() {
    let string_alias_profile = ImportProfile {
        version: 1,
        format: "csv".to_owned(),
        sheet_name: None,
        header_mode: None,
        date_convention: None,
        number_convention: None,
        schema: vec![crate::dataset::ImportProfileColumn {
            name: "id".to_owned(),
            data_type: "String".to_owned(),
        }],
    };
    let string_alias_frame = DataFrame::new(
        1,
        vec![Series::new("id".into(), ["private-row-value"]).into_column()],
    )
    .expect("el tipo str debe ser válido");
    assert!(crate::dataset::import_profile_schema_mismatch(
        &string_alias_profile,
        &string_alias_frame
    )
    .is_none());

    let profile = ImportProfile {
        version: 1,
        format: "csv".to_owned(),
        sheet_name: None,
        header_mode: None,
        date_convention: Some(crate::dataset::ImportDateConvention::Iso8601),
        number_convention: Some(crate::dataset::ImportNumberConvention::DotDecimalCommaGrouping),
        schema: vec![
            crate::dataset::ImportProfileColumn {
                name: "id".to_owned(),
                data_type: "String".to_owned(),
            },
            crate::dataset::ImportProfileColumn {
                name: "amount".to_owned(),
                data_type: "Int64".to_owned(),
            },
            crate::dataset::ImportProfileColumn {
                name: "period".to_owned(),
                data_type: "String".to_owned(),
            },
        ],
    };
    let candidate = DataFrame::new(
        1,
        vec![
            Series::new("id".into(), ["private-row-value"]).into_column(),
            Series::new("amount".into(), ["not-a-number"]).into_column(),
            Series::new("region".into(), ["private-region-value"]).into_column(),
        ],
    )
    .expect("el nuevo esquema debe ser válido");

    let mismatch = crate::dataset::import_profile_schema_mismatch(&profile, &candidate)
        .expect("los cambios deben requerir revisión");
    assert_eq!(mismatch.code, "importProfileSchemaMismatch");
    assert_eq!(mismatch.missing_columns, vec!["period"]);
    assert_eq!(mismatch.added_columns, vec!["region"]);
    assert_eq!(mismatch.changed_types.len(), 1);
    assert_eq!(mismatch.changed_types[0].column, "amount");
    assert_eq!(mismatch.changed_types[0].expected, "Int64");
    assert_eq!(mismatch.changed_types[0].actual, "String");
    let summary = serde_json::to_string(&mismatch).unwrap();
    assert!(!summary.contains("private-row-value"));
    assert!(!summary.contains("not-a-number"));
    assert!(!summary.contains("private-region-value"));
}

#[test]
fn v12_catalog_migrates_and_old_project_opens_without_an_import_profile() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("data-v12");
    let store = ProjectStore::initialize(root.clone()).unwrap();
    let dataset = active_test_dataset(directory.path());
    let project = store
        .save(
            &dataset,
            None,
            "Proyecto anterior".to_owned(),
            test_workspace(None),
        )
        .unwrap();
    drop(store);

    Connection::open(root.join("projects.sqlite3"))
        .unwrap()
        .execute_batch(
            "ALTER TABLE projects DROP COLUMN import_profile_json;
             PRAGMA user_version = 12;",
        )
        .unwrap();

    let migrated = ProjectStore::initialize(root).unwrap();
    let connection = migrated.connection().unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 13);
    let has_profile_column: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('projects') WHERE name = 'import_profile_json')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_profile_column);
    drop(connection);

    let opened = migrated
        .open(&DatasetState::default(), project.id)
        .expect("un proyecto v12 debe seguir siendo compatible");
    assert_eq!(opened.workspace.import_profile, None);
}
