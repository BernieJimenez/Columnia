use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use super::samples::{ensure_sample_dataset, list_sample_datasets};
use super::*;
use ::zip::ZipArchive;

fn temporary_csv(contents: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("el reloj del sistema debe ser válido")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "columnia-dataset-test-{}-{nonce}.csv",
        std::process::id()
    ));
    let mut file = File::create(&path).expect("se debe poder crear el CSV temporal");
    file.write_all(contents.as_bytes())
        .expect("se debe poder escribir el CSV temporal");
    path
}

#[test]
fn sample_dataset_catalog_is_static_and_paths_stay_native() {
    let samples = list_sample_datasets();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].id, "quality");
    assert_eq!(samples[0].format, "csv");
    assert_eq!(samples[1].id, "temporal");
    assert_eq!(samples[1].format, "tsv");
    let serialized = serde_json::to_string(&samples).unwrap();
    assert!(!serialized.contains("examples"));
    assert!(!serialized.contains("app_data"));
}

#[test]
fn sample_dataset_creation_is_allowlisted_and_reuses_a_regular_file() {
    let directory = tempfile::tempdir().unwrap();
    let app_data_dir = directory.path().join("app-data");
    let path = ensure_sample_dataset(&app_data_dir, "quality").unwrap();
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("clientes_calidad.csv")
    );
    assert!(fs::read_to_string(&path)
        .unwrap()
        .contains("cliente_id,nombre"));
    let second_path = ensure_sample_dataset(&app_data_dir, "quality").unwrap();
    assert_eq!(second_path, path);
    assert!(ensure_sample_dataset(&app_data_dir, "unknown").is_err());
}

fn complete_stored_recipe() -> StoredTransformRecipe {
    build_stored_recipe(
        TransformRecipe {
            renames: vec![RecipeRename {
                from: "old".to_owned(),
                to: "new".to_owned(),
            }],
            casts: vec![RecipeCast {
                column: "amount".to_owned(),
                target: RecipeCastTarget::Decimal,
            }],
            date_parses: vec![RecipeDateParse {
                column: "created".to_owned(),
                format: RecipeDateFormat::Iso8601,
                target: RecipeDateTarget::Datetime,
            }],
            filters: vec![RecipeFilter {
                column: "status".to_owned(),
                operator: RecipeFilterOperator::Eq,
                value: Some("active".to_owned()),
            }],
            calculated_column: Some(CalculatedColumnRecipe {
                name: "total".to_owned(),
                source: "amount".to_owned(),
                operation: CalculatedOperation::Multiply,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "2".to_owned(),
                }),
            }),
            find_replace: Some(FindReplaceRecipe {
                scope: FindReplaceScope::Column,
                column: Some("city".to_owned()),
                find: "SD".to_owned(),
                replace: "Santo Domingo".to_owned(),
                regex: false,
            }),
            keep_columns: Some(vec!["new".to_owned(), "total".to_owned()]),
            split_column: Some(SplitColumnRecipe {
                source: "full_name".to_owned(),
                delimiter: " ".to_owned(),
                names: vec!["first_name".to_owned(), "last_name".to_owned()],
                drop_source: true,
            }),
            merge_columns: Some(MergeColumnsRecipe {
                sources: vec!["city".to_owned(), "country".to_owned()],
                name: "location".to_owned(),
                separator: ", ".to_owned(),
                drop_sources: false,
            }),
            outlier_treatments: vec![OutlierTreatment {
                column: "amount".to_owned(),
                action: OutlierAction::Cap,
            }],
            group_summary: Some(GroupSummaryRecipe {
                group_by: vec!["country".to_owned()],
                aggregations: vec![SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Mean,
                }],
            }),
            contact_normalizations: vec![ContactNormalization {
                column: "email".to_owned(),
                kind: ContactKind::Email,
            }],
            text_extractions: vec![TextExtraction {
                source: "code".to_owned(),
                kind: ExtractionKind::Digits,
                name: "code_number".to_owned(),
                delimiter: None,
            }],
        },
        "Limpieza completa".to_owned(),
    )
    .expect("la receta de prueba debe ser válida")
}

#[test]
fn recipe_file_roundtrips_every_supported_operation_without_exposing_a_path() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("limpieza.json");
    let expected = complete_stored_recipe();

    save_recipe_atomic(&expected, &destination).expect("la receta debe guardarse");
    let loaded = load_recipe_file(&destination).expect("la receta debe volver a cargar");

    assert_eq!(loaded, expected);
    let public_json = serde_json::to_value(&loaded).expect("la respuesta debe serializarse");
    assert_eq!(public_json["version"], RECIPE_FILE_VERSION);
    assert!(public_json.get("path").is_none());
    assert!(!public_json
        .to_string()
        .contains(&directory.path().display().to_string()));
}

#[test]
fn recipe_save_atomically_replaces_the_destination_and_leaves_no_temporary_file() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("recipe.json");
    fs::write(&destination, "contenido anterior").expect("se debe preparar el destino");
    let document = complete_stored_recipe();

    save_recipe_atomic(&document, &destination).expect("la receta debe reemplazarse");

    assert_eq!(load_recipe_file(&destination).unwrap(), document);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn recipe_load_rejects_future_versions_corruption_unknown_fields_and_oversize() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("recipe.json");

    fs::write(
        &path,
        r#"{"version":2,"name":"Futura","savedAt":"2026-01-01T00:00:00Z","recipe":{}}"#,
    )
    .unwrap();
    assert!(load_recipe_file(&path).unwrap_err().contains("versión 2"));

    fs::write(&path, b"{not-json").unwrap();
    assert!(load_recipe_file(&path)
        .unwrap_err()
        .contains("JSON no es válida"));

    fs::write(
        &path,
        r#"{"version":1,"name":"Extra","savedAt":"2026-01-01T00:00:00Z","recipe":{},"sourcePath":"secret.csv"}"#,
    )
    .unwrap();
    assert!(load_recipe_file(&path)
        .unwrap_err()
        .contains("unknown field"));

    fs::write(
        &path,
        r#"{"version":1,"name":"Fecha mala","savedAt":"ayer","recipe":{}}"#,
    )
    .unwrap();
    assert!(load_recipe_file(&path).unwrap_err().contains("RFC 3339"));

    let mut excessive = complete_stored_recipe();
    excessive.recipe.filters = vec![
        RecipeFilter {
            column: "status".to_owned(),
            operator: RecipeFilterOperator::IsNull,
            value: None,
        };
        4
    ];
    fs::write(&path, serde_json::to_vec(&excessive).unwrap()).unwrap();
    assert!(load_recipe_file(&path).unwrap_err().contains("máximo 3"));

    fs::write(&path, vec![b' '; RECIPE_FILE_LIMIT_BYTES as usize + 1]).unwrap();
    assert!(load_recipe_file(&path)
        .unwrap_err()
        .contains("supera el límite"));
}

#[test]
fn recipe_semantic_budget_accepts_boundaries_and_rejects_large_apply_and_save_payloads() {
    let boundary_value = "x".repeat(MAX_RECIPE_TEXT_FIELD_CHARS);
    let boundary = TransformRecipe {
        keep_columns: Some(vec![boundary_value.clone(); 16]),
        ..Default::default()
    };
    assert_eq!(
        boundary
            .keep_columns
            .as_ref()
            .unwrap()
            .iter()
            .map(|value| value.chars().count())
            .sum::<usize>(),
        MAX_RECIPE_TOTAL_TEXT_CHARS
    );
    validate_recipe_structure(&boundary).expect("el límite exacto debe admitirse");

    let oversized_field = TransformRecipe {
        keep_columns: Some(vec!["x".repeat(MAX_RECIPE_TEXT_FIELD_CHARS + 1)]),
        ..Default::default()
    };
    let save_error = build_stored_recipe(oversized_field, "Receta grande".to_owned())
        .expect_err("guardar debe rechazar un campo desproporcionado");
    assert!(save_error.contains(&MAX_RECIPE_TEXT_FIELD_CHARS.to_string()));
    assert!(!save_error.contains(&"x".repeat(32)));

    let oversized_total = TransformRecipe {
        keep_columns: Some(vec![boundary_value; 17]),
        ..Default::default()
    };
    let path = temporary_csv("value\n1\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let mut dataset = loaded_dataset(path.clone(), frame);
    let apply_error = apply_recipe_to_dataset(&mut dataset, &oversized_total)
        .expect_err("aplicar debe rechazar el presupuesto total excedido");
    assert!(apply_error.contains(&MAX_RECIPE_TOTAL_TEXT_CHARS.to_string()));
    assert!(!apply_error.contains(&"x".repeat(32)));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

fn temporary_delimited(extension: &str, contents: &str) -> PathBuf {
    temporary_delimited_bytes(extension, contents.as_bytes())
}

fn loaded_dataset(path: PathBuf, frame: DataFrame) -> LoadedDataset {
    let history = HistoryManager::new(&frame).expect("el historial debe inicializarse");
    LoadedDataset {
        source_path: Some(path.clone()),
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dataset.csv")
            .to_owned(),
        file_size_bytes: fs::metadata(&path).map(|value| value.len()).unwrap_or(0),
        row_count: frame.height(),
        frame,
        source_backed: false,
        profile: None,
        history,
    }
}

fn temporary_delimited_bytes(extension: &str, contents: &[u8]) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("el reloj del sistema debe ser válido")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "columnia-delimited-test-{}-{nonce}.{extension}",
        std::process::id()
    ));
    let mut file = File::create(&path).expect("se debe crear el archivo temporal");
    file.write_all(contents)
        .expect("se debe escribir el archivo temporal");
    path
}

#[test]
fn loads_schema_and_rows_from_a_csv() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");

    let (frame, preview) = load_csv(&path).expect("el CSV debe cargar");

    assert_eq!(frame.height(), 2);
    assert_eq!(
        preview.file_name,
        path.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(preview.row_count, 2);
    assert_eq!(preview.column_count, 2);
    assert_eq!(preview.columns[0].name, "city");
    assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
    assert_eq!(preview.rows[0][0].as_deref(), Some("Santo Domingo"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_load_keeps_only_schema_while_preparing_the_preview() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");

    let (schema, preview, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");

    assert_eq!(schema.height(), 0);
    assert_eq!(schema.width(), 2);
    assert_eq!(row_count, 2);
    assert_eq!(preview.row_count, 2);
    assert_eq!(preview.rows[0][0].as_deref(), Some("Santo Domingo"));
    assert_eq!(preview.rows[1][1].as_deref(), Some("28"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn loads_json_variants_through_private_parquet_snapshots() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let variants = [
        ("json", r#"[{"id":1,"name":"Ana"},{"id":2,"name":"Luis"}]"#),
        (
            "jsonl",
            "{\"id\":1,\"name\":\"Ana\"}\n{\"id\":2,\"name\":\"Luis\"}\n",
        ),
        (
            "ndjson",
            "{\"id\":1,\"name\":\"Ana\"}\n{\"id\":2,\"name\":\"Luis\"}\n",
        ),
    ];

    for (extension, contents) in variants {
        assert!(should_defer_source_load(
            extension,
            SOURCE_BACKED_LOAD_THRESHOLD_BYTES
        ));
        let source = directory.path().join(format!("large.{extension}"));
        fs::write(&source, contents).expect("se debe escribir el JSON temporal");
        let snapshot = directory.path().join(format!("{extension}.parquet"));
        let (schema, preview, row_count) = source_backed_json_load(&source, &snapshot, || false)
            .expect("el JSON debe abrirse mediante snapshot");

        assert_eq!(schema.height(), 0);
        assert_eq!(schema.width(), 2);
        assert_eq!(row_count, 2);
        assert_eq!(preview.row_count, 2);
        assert_eq!(preview.rows[0][1].as_deref(), Some("Ana"));
        assert_eq!(preview.rows[1][1].as_deref(), Some("Luis"));
        let restored = read_parquet_frame(&snapshot).expect("el snapshot debe ser legible");
        assert_eq!(restored.height(), 2);
        assert_eq!(restored.width(), 2);
    }

    let recipe_source = directory.path().join("recipe.json");
    fs::write(
        &recipe_source,
        r#"[{"id":1,"name":"Ana"},{"id":2,"name":"Luis"}]"#,
    )
    .expect("se debe escribir el JSON de receta");
    let recipe_snapshot = directory.path().join("recipe.parquet");
    let (schema, _, row_count) =
        source_backed_json_load(&recipe_source, &recipe_snapshot, || false)
            .expect("el JSON de receta debe abrirse mediante snapshot");
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let file_size_bytes = fs::metadata(&recipe_source)
        .expect("la fuente de receta debe existir")
        .len();
    let mut dataset = LoadedDataset {
        source_path: Some(recipe_source.clone()),
        file_name: "recipe.json".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    dataset.history.source_snapshot_path = Some(recipe_snapshot.clone());
    let recipe = TransformRecipe {
        filters: vec![RecipeFilter {
            column: "id".to_owned(),
            operator: RecipeFilterOperator::Eq,
            value: Some("2".to_owned()),
        }],
        ..TransformRecipe::default()
    };
    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta JSON source-backed debe publicarse");
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar un snapshot Parquet");
    let output = read_parquet_frame(output_path).expect("la receta debe escribir Parquet");
    assert_eq!(result.removed_row_count, 1);
    assert_eq!(output.height(), 1);
    assert_eq!(
        output.column("name").unwrap().str().unwrap().get(0),
        Some("Luis")
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);

    let cancelled_source = directory.path().join("cancelled.json");
    fs::write(&cancelled_source, variants[0].1).expect("se debe escribir el JSON cancelado");
    let cancelled_snapshot = directory.path().join("cancelled.parquet");
    let cancellation = source_backed_json_load(&cancelled_source, &cancelled_snapshot, || true)
        .expect_err("la apertura cancelada debe detenerse antes de escribir");
    assert_eq!(cancellation, OPERATION_CANCELLED_MESSAGE);
    assert!(!cancelled_snapshot.exists());
}

#[test]
fn removes_empty_rows_from_source_backed_snapshot_with_reversible_history() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("empty-rows.json");
    fs::write(
        &source,
        r#"[{"id":1,"name":"Ana","_cambios":"Previo"},{"id":null,"name":"   ","_cambios":null},{"id":2,"name":"Luis","_cambios":null}]"#,
    )
    .expect("se debe escribir el JSON de filas vacías");
    let source_snapshot = directory.path().join("source.parquet");
    let (schema, _, row_count) = source_backed_json_load(&source, &source_snapshot, || false)
        .expect("el JSON debe abrirse mediante snapshot");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut history = HistoryManager::deferred().expect("el historial debe inicializarse");
    history.source_snapshot_path = Some(source_snapshot);
    let mut dataset = LoadedDataset {
        source_path: Some(source.clone()),
        file_name: "empty-rows.json".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let mutation = remove_empty_rows_source_backed(&mut dataset)
        .expect("la limpieza source-backed debe ejecutarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(mutation.affected_row_count, 1);
    assert_eq!(mutation.dataset.row_count, 2);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert!(dataset.history.snapshots_enabled);
    assert!(dataset.history.state().can_undo);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la limpieza debe conservar el snapshot actual");
    let current = read_parquet_frame(current_path).expect("el snapshot limpio debe ser legible");
    assert_eq!(current.height(), 2);
    assert_eq!(
        current.column("name").unwrap().str().unwrap().get(1),
        Some("Luis")
    );
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        Some("Previo; Eliminar filas completamente vacías")
    );

    let undo = undo_dataset(&mut dataset).expect("la limpieza debe poder deshacerse");
    assert_eq!(undo.dataset.row_count, 3);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    let redo = redo_dataset(&mut dataset).expect("la limpieza debe poder rehacerse");
    assert_eq!(redo.dataset.row_count, 2);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
}

#[test]
fn source_backed_cleanups_remove_duplicates_and_columns_with_reversible_history() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("cleanup.json");
    fs::write(
        &source,
        r#"[
            {"id":1,"name":"Ana","constant":"same","empty":null,"sparse":"keep","_cambios":"base-1"},
            {"id":1,"name":"Ana","constant":"same","empty":null,"sparse":"keep","_cambios":"base-1"},
            {"id":2,"name":"Luis","constant":"same","empty":null,"sparse":null,"_cambios":"base-2"},
            {"id":3,"name":"Marta","constant":"same","empty":null,"sparse":null,"_cambios":"base-3"},
            {"id":4,"name":"Pablo","constant":"same","empty":null,"sparse":null,"_cambios":"base-4"},
            {"id":5,"name":"Rosa","constant":"same","empty":null,"sparse":null,"_cambios":"base-5"}
        ]"#,
    )
    .expect("se debe escribir el JSON de limpieza");
    let source_snapshot = directory.path().join("source.parquet");
    let (schema, _, row_count) = source_backed_json_load(&source, &source_snapshot, || false)
        .expect("el JSON debe abrirse mediante snapshot");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut history = HistoryManager::deferred().expect("el historial debe inicializarse");
    history.source_snapshot_path = Some(source_snapshot);
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "cleanup.json".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let duplicates = remove_duplicates_source_backed(&mut dataset)
        .expect("los duplicados source-backed deben procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(duplicates.affected_row_count, 1);
    assert_eq!(duplicates.dataset.row_count, 5);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);

    let high_null = remove_columns_source_backed(
        &mut dataset,
        SourceBackedColumnCleanup::HighNull,
        "Eliminar columnas con alta nulidad",
    )
    .expect("las columnas con alta nulidad deben procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(high_null.removed_columns, vec!["sparse"]);

    let constants = remove_columns_source_backed(
        &mut dataset,
        SourceBackedColumnCleanup::Constant,
        "Eliminar columnas constantes",
    )
    .expect("las columnas constantes deben procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(constants.removed_columns, vec!["constant"]);

    let empty = remove_columns_source_backed(
        &mut dataset,
        SourceBackedColumnCleanup::Empty,
        "Eliminar columnas completamente vacías",
    )
    .expect("las columnas vacías deben procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(empty.removed_columns, vec!["empty"]);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la limpieza debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot final debe ser legible");
    assert_eq!(current.get_column_names(), &["id", "name", "_cambios"]);
    assert_eq!(current.height(), 5);
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        Some("base-1; Eliminar filas duplicadas; Eliminar columnas con alta nulidad; Eliminar columnas constantes; Eliminar columnas completamente vacías")
    );
    assert!(dataset.history.state().can_undo);

    let undo = undo_dataset(&mut dataset).expect("la última limpieza debe poder deshacerse");
    assert_eq!(undo.dataset.row_count, 5);
    assert!(dataset.source_backed);
    assert!(dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "empty"));
}

#[test]
fn source_backed_near_duplicate_cleanup_matches_eager_and_preserves_exact_repeats() {
    let path = temporary_csv(
        "name,city,amount\nAna,Santo Domingo,1\n ana , santo   domingo ,1\nAna,Santo Domingo,1\nLuis,Santiago,2\nluis,Santiago,2\nMarta,Santiago,3\n",
    );
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "near-duplicates.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let (expected, expected_removed) =
        remove_near_duplicate_rows(&source_frame).expect("la ruta eager debe procesarse");

    let mutation = remove_near_duplicates_source_backed(&mut dataset)
        .expect("la limpieza source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(mutation.affected_row_count, expected_removed);
    assert_eq!(mutation.dataset.row_count, expected.height());
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("la limpieza debe conservar el snapshot actual");
    let output = read_parquet_frame(output_path).expect("el resultado debe ser legible");
    assert!(output.equals_missing(&expected));
    assert_eq!(output.height(), 4);
    assert_eq!(
        output.column("name").unwrap().str().unwrap().get(2),
        Some("Luis")
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_safe_corrections_combine_trim_and_renames() {
    let path = temporary_csv("Año Venta,city\n1,\" Bogotá \"\n2,\" Santo Domingo \"\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "safe-corrections.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let (expected, expected_rows, expected_cells, expected_renames) =
        safe_corrected_frame(&source_frame).expect("la ruta eager debe procesarse");

    let result = source_backed_safe_corrections(&mut dataset)
        .expect("las correcciones source-backed deben procesarse")
        .expect("la fuente debe ser compatible");
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(result.dataset.row_count, expected.height());
    assert_eq!(result.affected_row_count, expected_rows);
    assert_eq!(result.changed_cell_count, expected_cells);
    assert_eq!(result.renames, expected_renames);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("las correcciones deben conservar el snapshot actual");
    let output = read_parquet_frame(output_path).expect("el resultado debe ser legible");
    assert!(output.equals_missing(&expected));
    assert_eq!(
        output.column("city").unwrap().str().unwrap().get(0),
        Some("Bogotá")
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_privacy_column_cleanup_keeps_audit_and_history() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("privacy-cleanup.json");
    fs::write(
        &source,
        r#"[
            {"customer_id":"a-1","email":"ana@example.com","amount":10,"_cambios":"base-1"},
            {"customer_id":"b-2","email":"luis@example.com","amount":20,"_cambios":"base-2"}
        ]"#,
    )
    .expect("se debe escribir el JSON de privacidad");
    let source_snapshot = directory.path().join("source.parquet");
    let (schema, _, row_count) = source_backed_json_load(&source, &source_snapshot, || false)
        .expect("el JSON debe abrirse mediante snapshot");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut history = HistoryManager::deferred().expect("el historial debe inicializarse");
    history.source_snapshot_path = Some(source_snapshot);
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "privacy-cleanup.json".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let identifiers = remove_columns_source_backed(
        &mut dataset,
        SourceBackedColumnCleanup::Identifier,
        "Retirar columnas identificadoras",
    )
    .expect("las columnas identificadoras deben procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(identifiers.removed_columns, vec!["customer_id"]);

    let personal = remove_columns_source_backed(
        &mut dataset,
        SourceBackedColumnCleanup::Personal,
        "Retirar datos personales detectados",
    )
    .expect("las columnas personales deben procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(personal.removed_columns, vec!["email"]);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la limpieza debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(current.get_column_names(), &["amount", "_cambios"]);
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        Some("base-1; Retirar columnas identificadoras; Retirar datos personales detectados")
    );
    assert!(dataset.history.state().can_undo);

    let undo = undo_dataset(&mut dataset).expect("el retiro personal debe poder deshacerse");
    assert_eq!(undo.dataset.row_count, 2);
    assert!(dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "email"));
}

#[test]
fn source_backed_personal_mask_counts_changes_without_materializing_rows() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("mask.json");
    fs::write(
        &source,
        r#"[
            {"email":"[REDACTED]","name":"Ana","amount":10,"_cambios":"base-1"},
            {"email":"luis@example.com","name":"Luis","amount":20,"_cambios":"base-2"}
        ]"#,
    )
    .expect("se debe escribir el JSON de máscara");
    let source_snapshot = directory.path().join("source.parquet");
    let (schema, _, row_count) = source_backed_json_load(&source, &source_snapshot, || false)
        .expect("el JSON debe abrirse mediante snapshot");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut history = HistoryManager::deferred().expect("el historial debe inicializarse");
    history.source_snapshot_path = Some(source_snapshot);
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "mask.json".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let result = mask_personal_values_source_backed(&mut dataset)
        .expect("la máscara source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(result.changed_cell_count, 3);
    assert_eq!(result.changed_column_count, 2);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la máscara debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.column("email").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        current.column("email").unwrap().str().unwrap().get(1),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        Some("base-1; Proteger valores personales detectados")
    );
    let undo = undo_dataset(&mut dataset).expect("la máscara debe poder deshacerse");
    assert_eq!(undo.dataset.row_count, 2);
    assert!(dataset.source_backed);
    let undone_path = dataset
        .source_path
        .as_deref()
        .expect("el undo debe conservar el snapshot actual");
    let undone = read_parquet_frame(undone_path).expect("el snapshot deshecho debe ser legible");
    assert_eq!(
        undone.column("email").unwrap().str().unwrap().get(1),
        Some("luis@example.com")
    );
}

#[test]
fn source_backed_schema_changes_normalize_names_and_enable_audit_reversibly() {
    let source = temporary_csv("Customer ID,customer-id,Name\na-1,1,Ana\nb-2,2,Luis\n");
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "headers.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let normalized = normalize_column_names_source_backed(&mut dataset)
        .expect("la normalización source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(normalized.renamed_column_count, 3);
    assert_eq!(normalized.renames[0].from, "Customer ID");
    assert_eq!(normalized.renames[0].to, "customer_id");
    assert_eq!(normalized.renames[1].to, "customer_id_2");

    let audit = enable_row_audit_source_backed(&mut dataset)
        .expect("la trazabilidad source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(audit.dataset.row_count, 2);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la trazabilidad debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.get_column_names(),
        &["customer_id", "customer_id_2", "name", "_cambios"]
    );
    assert_eq!(
        current.column("customer_id").unwrap().str().unwrap().get(0),
        Some("a-1")
    );
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        None
    );

    let undo = undo_dataset(&mut dataset).expect("la trazabilidad debe poder deshacerse");
    assert_eq!(undo.dataset.row_count, 2);
    assert!(dataset.source_backed);
    assert!(!dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios"));
    let redo = redo_dataset(&mut dataset).expect("la trazabilidad debe poder rehacerse");
    assert_eq!(redo.dataset.row_count, 2);
    assert!(dataset.source_backed);
    assert!(dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios"));
}

#[test]
fn source_backed_text_cleaning_streams_trim_sentinels_and_normalization() {
    let source =
        temporary_csv("name,notes,amount\n\"  Ana  \",N/A,1\n\" Bob \",\"  Café  \",2\nLuis,?,3\n");
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "text.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    enable_row_audit_source_backed(&mut dataset)
        .expect("la trazabilidad debe poder activarse antes de limpiar texto")
        .expect("la fuente debe ser compatible");

    let trimmed = source_backed_text_cleaning(
        &mut dataset,
        Some(&["name".to_owned(), "notes".to_owned()]),
        TextCleaningMode::Trim,
    )
    .expect("el recorte source-backed debe procesarse")
    .expect("la fuente debe ser compatible");
    assert_eq!(trimmed.affected_row_count, 2);
    assert_eq!(trimmed.changed_cell_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("el recorte debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.column("name").unwrap().str().unwrap().get(0),
        Some("Ana")
    );
    assert_eq!(
        current.column("notes").unwrap().str().unwrap().get(1),
        Some("Café")
    );
    assert_eq!(
        current.column("_cambios").unwrap().str().unwrap().get(0),
        Some("Recortar espacios")
    );

    let sentinels = source_backed_text_cleaning(&mut dataset, None, TextCleaningMode::Sentinels)
        .expect("los centinelas source-backed deben procesarse")
        .expect("la fuente debe seguir siendo compatible");
    assert_eq!(sentinels.affected_row_count, 2);
    assert_eq!(sentinels.changed_cell_count, 2);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("los centinelas deben conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(current.column("notes").unwrap().str().unwrap().get(0), None);
    assert_eq!(current.column("notes").unwrap().str().unwrap().get(2), None);

    let normalized = source_backed_text_cleaning(
        &mut dataset,
        Some(&["notes".to_owned()]),
        TextCleaningMode::Normalize {
            remove_accents: true,
        },
    )
    .expect("la normalización source-backed debe procesarse")
    .expect("la fuente debe seguir siendo compatible");
    assert_eq!(normalized.affected_row_count, 1);
    assert_eq!(normalized.changed_cell_count, 1);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la normalización debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.column("notes").unwrap().str().unwrap().get(1),
        Some("cafe")
    );
}

#[test]
fn source_backed_boolean_normalization_preserves_the_candidate_threshold() {
    let source = temporary_csv(
        "flag,note\nYES,a\nno,b\nSí,c\ntrue,d\nfalse,e\nyes,f\nno,g\nTRUE,h\nFalse,i\nmaybe,j\n",
    );
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "boolean.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let normalized = source_backed_text_cleaning(&mut dataset, None, TextCleaningMode::Booleans)
        .expect("la normalización booleana source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(normalized.affected_row_count, 7);
    assert_eq!(normalized.changed_cell_count, 7);
    assert_eq!(normalized.changed_columns[0].name, "flag");
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la normalización debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    let flag = current.column("flag").unwrap().str().unwrap();
    assert_eq!(flag.get(0), Some("true"));
    assert_eq!(flag.get(1), Some("false"));
    assert_eq!(flag.get(2), Some("true"));
    assert_eq!(flag.get(9), Some("maybe"));
}

#[test]
fn source_backed_invalid_type_cleanup_matches_eager_inference_without_rows_in_memory() {
    let source = temporary_csv(
        "created_at,amount,ratio,flag,note\n2024-01-01,1,1.5,yes,keep\n2024-01-02,2,2.5,no,keep\n2024-01-03,3,3.5,si,keep\n2024-01-04,4,4.5,true,keep\n2024-01-05,5,5.5,false,keep\n2024-01-06,6,6.5,yes,keep\n2024-01-07,7,7.5,no,keep\n2024-01-08,8,8.5,TRUE,keep\n2024-01-09,9,9.5,False,keep\nsin fecha,x,invalid,maybe,keep\n",
    );
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "invalid-types.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let result =
        source_backed_text_cleaning(&mut dataset, None, TextCleaningMode::NullifyInvalidTypes)
            .expect("la limpieza de tipos source-backed debe procesarse")
            .expect("la fuente debe ser compatible");
    assert_eq!(result.affected_row_count, 1);
    assert_eq!(result.changed_cell_count, 4);
    assert_eq!(result.changed_columns[0].name, "created_at");
    assert_eq!(result.changed_columns[1].name, "amount");
    assert_eq!(result.changed_columns[2].name, "ratio");
    assert_eq!(result.changed_columns[3].name, "flag");
    assert_eq!(dataset.frame.height(), 0);

    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la limpieza debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    let dates = current.column("created_at").unwrap().str().unwrap();
    assert_eq!(dates.get(0), Some("2024-01-01"));
    assert_eq!(dates.get(8), Some("2024-01-09"));
    assert_eq!(dates.get(9), None);
    assert_eq!(
        current.column("amount").unwrap().str().unwrap().get(9),
        None
    );
    assert_eq!(current.column("ratio").unwrap().str().unwrap().get(9), None);
    assert_eq!(current.column("flag").unwrap().str().unwrap().get(9), None);
    assert_eq!(
        current.column("note").unwrap().str().unwrap().get(0),
        Some("keep")
    );
}

#[test]
fn source_backed_encoding_fix_handles_safe_mojibake_without_rows_in_memory() {
    let source =
        temporary_csv("city,quote\nBogotÃ¡,â€œholaâ€”\nMÃ¡laga,â€™\nSanto Domingo,normal\n");
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "encoding.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let result = source_backed_text_cleaning(&mut dataset, None, TextCleaningMode::FixEncoding)
        .expect("la corrección source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(result.affected_row_count, 2);
    assert_eq!(result.changed_cell_count, 4);
    assert_eq!(dataset.frame.height(), 0);

    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la corrección debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    let city = current.column("city").unwrap().str().unwrap();
    let quote = current.column("quote").unwrap().str().unwrap();
    assert_eq!(city.get(0), Some("Bogotá"));
    assert_eq!(city.get(1), Some("Málaga"));
    assert_eq!(quote.get(0), Some("“hola—"));
    assert_eq!(quote.get(1), Some("’"));
    assert_eq!(quote.get(2), Some("normal"));
}

#[test]
fn source_backed_encoding_fix_falls_back_for_unsafe_values() {
    let source = temporary_csv("city\nBogotÃ¡ 😀\nSanto Domingo\n");
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "unsafe-encoding.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    assert!(
        source_backed_text_cleaning(&mut dataset, None, TextCleaningMode::FixEncoding)
            .expect("el fallback debe resolverse sin error")
            .is_none()
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
}

#[test]
fn source_backed_inferred_numeric_and_date_casts_keep_safe_columns_lazy() {
    let source = temporary_csv(
        "amount,when,code\n10,2024-01-02,001\n20,2024-02-03,002\n30,2024-03-04,003\n",
    );
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "inferred.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let numeric = source_backed_numeric_cast(&mut dataset)
        .expect("la conversión numérica source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(numeric.affected_row_count, 3);
    assert_eq!(numeric.changed_cell_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la conversión numérica debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(current.column("amount").unwrap().dtype(), &DataType::Int64);
    assert_eq!(current.column("code").unwrap().dtype(), &DataType::String);
    assert_eq!(
        current.column("code").unwrap().str().unwrap().get(0),
        Some("001")
    );

    let dates = source_backed_date_parsing(&mut dataset)
        .expect("la interpretación de fechas source-backed debe procesarse")
        .expect("la fuente debe seguir siendo compatible");
    assert_eq!(dates.affected_row_count, 3);
    assert_eq!(dates.changed_cell_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la interpretación debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert!(matches!(
        current.column("when").unwrap().dtype(),
        DataType::Datetime(_, None)
    ));
    assert_eq!(
        current.column("amount").unwrap().i64().unwrap().get(2),
        Some(30)
    );
}

#[test]
fn source_backed_imputation_matches_eager_replacements_without_rows_in_memory() {
    let source = temporary_csv("amount,category\n10,x\n,\n30,x\n");
    let (schema, _, row_count) = source_backed_load(&source, "csv", || false)
        .expect("la fuente debe inspeccionarse en disco");
    let file_size_bytes = fs::metadata(&source).expect("la fuente debe existir").len();
    let history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: Some(source),
        file_name: "imputation.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    source_backed_numeric_cast(&mut dataset)
        .expect("la conversión numérica previa debe procesarse")
        .expect("la fuente debe ser compatible");
    let categorical = source_backed_imputation(&mut dataset, true)
        .expect("la imputación categórica source-backed debe procesarse")
        .expect("la fuente debe ser compatible");
    assert_eq!(categorical.affected_row_count, 1);
    assert_eq!(categorical.changed_cell_count, 1);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la imputación debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.column("category").unwrap().str().unwrap().get(1),
        Some("Desconocido")
    );

    let numeric = source_backed_imputation(&mut dataset, false)
        .expect("la imputación conservadora source-backed debe procesarse")
        .expect("la fuente debe seguir siendo compatible");
    assert_eq!(numeric.affected_row_count, 1);
    assert_eq!(numeric.changed_cell_count, 1);
    assert_eq!(dataset.frame.height(), 0);
    let current_path = dataset
        .source_path
        .as_deref()
        .expect("la imputación numérica debe conservar el snapshot actual")
        .to_owned();
    let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
    assert_eq!(
        current.column("amount").unwrap().i64().unwrap().get(1),
        Some(10)
    );
    assert_eq!(
        current.column("category").unwrap().str().unwrap().get(1),
        Some("Desconocido")
    );
}

#[test]
fn source_backed_direct_outlier_modes_match_eager_without_rows_in_memory() {
    for action in [
        OutlierAction::Cap,
        OutlierAction::Impute,
        OutlierAction::Drop,
    ] {
        let path = temporary_csv("amount,group\n1,A\n2,A\n3,A\n4,A\n100,A\n");
        let (schema, _, row_count) = source_backed_load(&path, "csv", || false)
            .expect("la fuente debe inspeccionarse en disco");
        let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
        let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
        let mut dataset = LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "direct-outliers.csv".to_owned(),
            file_size_bytes,
            row_count,
            frame: schema,
            source_backed: true,
            profile: None,
            history,
        };

        source_backed_numeric_cast(&mut dataset)
            .expect("la conversión numérica previa debe procesarse")
            .expect("la fuente debe ser compatible");
        let current_path = dataset
            .source_path
            .as_deref()
            .expect("la conversión debe conservar un snapshot actual")
            .to_owned();
        let current = read_parquet_frame(&current_path).expect("el snapshot debe ser legible");
        let (expected, expected_rows, expected_cells) = match action {
            OutlierAction::Cap => {
                let (frame, rows, cells, _) =
                    apply_outlier_mode(&current, OutlierMode::Cap).unwrap();
                (frame, rows, cells)
            }
            OutlierAction::Impute => {
                let (frame, rows, cells, _) = impute_outlier_values_in_frame(&current).unwrap();
                (frame, rows, cells)
            }
            OutlierAction::Drop => {
                let (frame, rows, cells, _) =
                    apply_outlier_mode(&current, OutlierMode::Drop).unwrap();
                (frame, rows, cells)
            }
        };

        let label = match action {
            OutlierAction::Cap => "Limitar outliers con IQR",
            OutlierAction::Impute => "Imputación de outliers",
            OutlierAction::Drop => "Eliminar filas atípicas",
        };
        let result = source_backed_direct_outlier(&mut dataset, action, label)
            .expect("el tratamiento directo source-backed debe procesarse")
            .expect("la fuente debe seguir siendo compatible");
        assert!(dataset.source_backed);
        assert_eq!(result.dataset.row_count, expected.height());
        assert_eq!(result.affected_row_count, expected_rows);
        assert_eq!(result.changed_cell_count, expected_cells);
        assert_eq!(dataset.frame.height(), 0);
        let output_path = dataset
            .source_path
            .as_deref()
            .expect("el resultado debe conservar un snapshot actual");
        let output = read_parquet_frame(output_path).expect("el resultado debe ser legible");
        assert!(output.equals_missing(&expected));
        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }
}

#[test]
fn source_backed_project_snapshot_streams_to_parquet_without_materializing_state() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let state = DatasetState {
        current: Mutex::new(Some(LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "dataset.csv".to_owned(),
            file_size_bytes,
            row_count,
            frame: schema,
            source_backed: true,
            profile: None,
            history,
        })),
        ..DatasetState::default()
    };

    let snapshot = state
        .active_project_snapshot()
        .expect("el guardado debe crear un snapshot Parquet");
    let snapshot_path = snapshot
        .current_snapshot_path
        .as_deref()
        .expect("el dataset source-backed debe conservar su snapshot directo");
    let output = read_parquet_frame(snapshot_path).expect("el snapshot debe ser legible");
    assert_eq!(output.height(), row_count);
    assert_eq!(output.width(), 2);
    assert_eq!(
        output.column("city").unwrap().str().unwrap().get(0),
        Some("Santo Domingo")
    );
    assert_eq!(snapshot.frame.height(), 0);

    let second_snapshot = state
        .active_project_snapshot()
        .expect("un segundo guardado debe crear otro snapshot temporal");
    assert_ne!(
        snapshot.current_snapshot_path,
        second_snapshot.current_snapshot_path
    );

    let current = state
        .current
        .lock()
        .expect("la sesión debe seguir disponible");
    let dataset = current.as_ref().expect("el dataset debe seguir activo");
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    drop(current);
    drop(second_snapshot);
    drop(snapshot);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_projection_recipe_writes_parquet_without_materializing_rows() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "city".to_owned(),
            to: "location".to_owned(),
        }],
        keep_columns: Some(vec!["city".to_owned()]),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta de proyección debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicarse");

    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, source_frame.height());
    assert_eq!(dataset.frame.get_column_names(), ["location"]);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado source-backed debe conservar una fuente");
    assert_ne!(output_path, path.as_path());
    assert_eq!(
        output_path.extension().and_then(|value| value.to_str()),
        Some("parquet")
    );
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.dataset.column_count, 1);
    assert_eq!(result.dataset.row_count, source_frame.height());
    assert!(!dataset.history.state().can_undo);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_regex_replacement_matches_eager_and_counts_cells() {
    let path = temporary_csv("text,city\nAna-01,Santo Domingo\nLuis-02,Santiago\n,La Vega\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".to_owned()),
            find: r"([A-Za-z]+)-(\d+)".to_owned(),
            replace: "$2:$1".to_owned(),
            regex: true,
        }),
        ..TransformRecipe::default()
    };
    assert!(source_backed_projection_recipe_supported(
        &dataset.frame,
        &recipe
    ));
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta regex source-backed debe publicarse");

    assert_eq!(result.replaced_cell_count, 2);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el resultado Parquet debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(
        output.column("text").unwrap().str().unwrap().get(0),
        Some("01:Ana")
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_division_matches_eager_and_rejects_zero_before_publish() {
    let path = temporary_csv("left,right\n10,2\n9,3\n,4\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "ratio".to_owned(),
            source: "left".to_owned(),
            operation: CalculatedOperation::Divide,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value: "right".to_owned(),
            }),
        }),
        ..TransformRecipe::default()
    };
    assert!(source_backed_projection_recipe_supported(
        &dataset.frame,
        &recipe
    ));
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la división eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la división source-backed debe publicarse");

    assert_eq!(result.calculated_column_count, 1);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el resultado Parquet debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(
        output.column("ratio").unwrap().f64().unwrap().get(0),
        Some(5.0)
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");

    let invalid_path = temporary_csv("left,right\n10,0\n");
    let (invalid_schema, _, invalid_row_count) = source_backed_load(&invalid_path, "csv", || false)
        .expect("la fuente inválida debe inspeccionarse en disco");
    let invalid_history =
        HistoryManager::deferred().expect("el historial inválido debe inicializarse");
    let invalid_size = fs::metadata(&invalid_path)
        .expect("la fuente inválida debe existir")
        .len();
    let mut invalid_dataset = LoadedDataset {
        source_path: Some(invalid_path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes: invalid_size,
        row_count: invalid_row_count,
        frame: invalid_schema,
        source_backed: true,
        profile: None,
        history: invalid_history,
    };
    let error = apply_recipe_to_dataset(&mut invalid_dataset, &recipe)
        .expect_err("la división por cero debe abortar antes de publicar");
    assert!(error.contains("división por cero"));
    assert!(invalid_dataset.source_backed);
    assert_eq!(invalid_dataset.frame.height(), 0);
    assert_eq!(
        invalid_dataset.source_path.as_deref(),
        Some(invalid_path.as_path())
    );
    fs::remove_file(invalid_path).expect("se debe limpiar el CSV inválido");
}

#[test]
fn source_backed_date_parts_after_filters_match_eager() {
    let path = temporary_csv("day,amount\n31/12/2025,20\n01/01/2026,5\n15/02/2026,30\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "day".to_owned(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        filters: vec![RecipeFilter {
            column: "amount".to_owned(),
            operator: RecipeFilterOperator::Gt,
            value: Some("10".to_owned()),
        }],
        calculated_column: Some(CalculatedColumnRecipe {
            name: "year".to_owned(),
            source: "day".to_owned(),
            operation: CalculatedOperation::Year,
            operand: None,
        }),
        ..TransformRecipe::default()
    };
    assert!(source_backed_projection_recipe_supported(
        &dataset.frame,
        &recipe
    ));
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("las partes de fecha source-backed deben publicarse");

    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el resultado Parquet debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.removed_row_count, 1);
    assert_eq!(
        output.column("year").unwrap().i32().unwrap().get(0),
        Some(2025)
    );
    assert_eq!(
        output.column("year").unwrap().i32().unwrap().get(1),
        Some(2026)
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_date_range_filters_match_eager() {
    let path =
        temporary_csv("day,amount\n31/12/2025,20\n01/01/2026,5\n15/02/2026,30\n01/03/2026,40\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "day".to_owned(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        filters: vec![
            RecipeFilter {
                column: "day".to_owned(),
                operator: RecipeFilterOperator::Gte,
                value: Some("2026-01-01".to_owned()),
            },
            RecipeFilter {
                column: "day".to_owned(),
                operator: RecipeFilterOperator::Lt,
                value: Some("2026-03-01".to_owned()),
            },
        ],
        ..TransformRecipe::default()
    };
    assert!(source_backed_projection_recipe_supported(
        &dataset.frame,
        &recipe
    ));
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("el filtro de fechas eager debe ser válido")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("el filtro de fechas source-backed debe publicarse");

    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el resultado Parquet debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.removed_row_count, 2);
    assert_eq!(output.height(), 2);
    assert_eq!(output.column("day").unwrap().dtype(), &DataType::Date);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_date_equality_filters_match_eager() {
    let path =
        temporary_csv("day,amount\n31/12/2025,20\n01/01/2026,5\n15/02/2026,30\n01/03/2026,40\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "day".to_owned(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Datetime,
        }],
        filters: vec![
            RecipeFilter {
                column: "day".to_owned(),
                operator: RecipeFilterOperator::Eq,
                value: Some("2026-01-01".to_owned()),
            },
            RecipeFilter {
                column: "day".to_owned(),
                operator: RecipeFilterOperator::Neq,
                value: Some("2026-03-01".to_owned()),
            },
        ],
        ..TransformRecipe::default()
    };
    assert!(source_backed_projection_recipe_supported(
        &dataset.frame,
        &recipe
    ));
    let expected = apply_eager_recipe_to_frame(&source_frame, &recipe)
        .expect("la igualdad de fecha eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la igualdad de fecha source-backed debe publicarse");
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el resultado Parquet debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.removed_row_count, 3);
    assert_eq!(output.height(), 1);
    assert!(matches!(
        output.column("day").unwrap().dtype(),
        DataType::Datetime(_, None)
    ));
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_filters_execute_on_disk_and_match_the_eager_recipe() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\nLa Vega,25\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "city".to_owned(),
            to: "location".to_owned(),
        }],
        filters: vec![
            RecipeFilter {
                column: "temperature".to_owned(),
                operator: RecipeFilterOperator::Gte,
                value: Some("28".to_owned()),
            },
            RecipeFilter {
                column: "city".to_owned(),
                operator: RecipeFilterOperator::Contains,
                value: Some("dom".to_owned()),
            },
        ],
        keep_columns: Some(vec!["city".to_owned(), "temperature".to_owned()]),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicarse");

    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, 1);
    assert_eq!(
        dataset.frame.get_column_names(),
        ["location", "temperature"]
    );
    assert_eq!(result.removed_row_count, 2);
    assert_eq!(result.renamed_column_count, 1);
    assert_eq!(result.dropped_column_count, 0);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.dataset.row_count, 1);
    assert_eq!(result.dataset.rows[0][0].as_deref(), Some("Santo Domingo"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_group_summary_preserves_stable_groups_nulls_and_counters() {
    let path = temporary_csv("group,amount\nA,10\nB,5\nA,20\n,7\nB,\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "amount".to_owned(),
            target: RecipeCastTarget::Integer,
        }],
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["group".to_owned()],
            aggregations: vec![
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Sum,
                },
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Mean,
                },
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Count,
                },
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::CountUnique,
                },
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Min,
                },
                SummaryAggregation {
                    column: "amount".to_owned(),
                    operation: SummaryOperation::Max,
                },
            ],
        }),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("el resumen source-backed debe publicarse");

    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, 3);
    assert_eq!(result.group_count, 3);
    assert_eq!(result.aggregated_column_count, 6);
    assert_eq!(result.collapsed_row_count, 2);
    assert_eq!(result.removed_row_count, 0);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.dataset.rows[0][0].as_deref(), Some("A"));
    assert_eq!(result.dataset.rows[2][0], None);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_iqr_modes_match_eager_and_keep_separate_counts() {
    for (action, expected_adjusted, expected_removed, expected_rows) in [
        (OutlierAction::Cap, 1, 0, 5),
        (OutlierAction::Impute, 1, 0, 5),
        (OutlierAction::Drop, 0, 1, 4),
    ] {
        let path = temporary_csv("amount,group\n1,A\n2,A\n3,A\n4,A\n100,A\n");
        let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let (schema, _, row_count) = source_backed_load(&path, "csv", || false)
            .expect("la fuente debe inspeccionarse en disco");
        let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
        let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
        let mut dataset = LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "dataset.csv".to_owned(),
            file_size_bytes,
            row_count,
            frame: schema,
            source_backed: true,
            profile: None,
            history,
        };
        let recipe = TransformRecipe {
            casts: vec![RecipeCast {
                column: "amount".to_owned(),
                target: RecipeCastTarget::Integer,
            }],
            outlier_treatments: vec![OutlierTreatment {
                column: "amount".to_owned(),
                action,
            }],
            ..TransformRecipe::default()
        };
        let expected = apply_recipe_to_frame(&source_frame, &recipe)
            .expect("la receta eager debe ser válida")
            .0;

        let result = apply_recipe_to_dataset(&mut dataset, &recipe)
            .expect("el tratamiento IQR source-backed debe publicarse");

        assert_eq!(result.adjusted_outlier_cell_count, expected_adjusted);
        assert_eq!(result.outlier_removed_row_count, expected_removed);
        assert_eq!(result.outlier_column_count, 1);
        assert_eq!(result.dataset.row_count, expected_rows);
        let output_path = dataset
            .source_path
            .as_deref()
            .expect("el resultado debe conservar una fuente Parquet");
        let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
        assert!(output.equals_missing(&expected));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }
}

#[test]
fn source_backed_iqr_uses_filtered_baseline_and_separates_removed_rows() {
    let path = temporary_csv("amount,group\n1,A\n2,A\n3,A\n4,A\n100,A\n100,B\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "amount".to_owned(),
            target: RecipeCastTarget::Integer,
        }],
        filters: vec![RecipeFilter {
            column: "group".to_owned(),
            operator: RecipeFilterOperator::Eq,
            value: Some("A".to_owned()),
        }],
        outlier_treatments: vec![OutlierTreatment {
            column: "amount".to_owned(),
            action: OutlierAction::Drop,
        }],
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("el tratamiento IQR source-backed debe respetar filtros");

    assert_eq!(result.removed_row_count, 1);
    assert_eq!(result.outlier_removed_row_count, 1);
    assert_eq!(result.dataset.row_count, 4);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_cast_dates_and_calculations_match_the_eager_recipe() {
    let path =
        temporary_csv("amount,when,city\n10,2024-01-02,Santo Domingo\n20,2024-02-03,Santiago\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![
            RecipeRename {
                from: "amount".to_owned(),
                to: "revenue".to_owned(),
            },
            RecipeRename {
                from: "when".to_owned(),
                to: "event_date".to_owned(),
            },
        ],
        casts: vec![RecipeCast {
            column: "amount".to_owned(),
            target: RecipeCastTarget::Decimal,
        }],
        date_parses: vec![RecipeDateParse {
            column: "when".to_owned(),
            format: RecipeDateFormat::Ymd,
            target: RecipeDateTarget::Date,
        }],
        calculated_column: Some(CalculatedColumnRecipe {
            name: "total".to_owned(),
            source: "amount".to_owned(),
            operation: CalculatedOperation::Add,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "5".to_owned(),
            }),
        }),
        keep_columns: Some(vec![
            "amount".to_owned(),
            "when".to_owned(),
            "city".to_owned(),
        ]),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicarse");

    assert!(dataset.source_backed);
    assert_eq!(dataset.row_count, 2);
    assert_eq!(
        dataset.frame.get_column_names(),
        ["revenue", "event_date", "city", "total"]
    );
    assert_eq!(result.renamed_column_count, 2);
    assert_eq!(result.converted_column_count, 1);
    assert_eq!(result.parsed_date_column_count, 1);
    assert_eq!(result.calculated_column_count, 1);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_date_parts_match_the_eager_recipe_after_date_parse() {
    for (operation, name, expected_value) in [
        (CalculatedOperation::Year, "year", "2024"),
        (CalculatedOperation::Month, "month", "1"),
        (CalculatedOperation::Day, "day", "2"),
    ] {
        let path = temporary_csv("when\n2024-01-02\n");
        let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
        let (schema, _, row_count) = source_backed_load(&path, "csv", || false)
            .expect("la fuente debe inspeccionarse en disco");
        let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
        let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
        let mut dataset = LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "dataset.csv".to_owned(),
            file_size_bytes,
            row_count,
            frame: schema,
            source_backed: true,
            profile: None,
            history,
        };
        let recipe = TransformRecipe {
            date_parses: vec![RecipeDateParse {
                column: "when".to_owned(),
                format: RecipeDateFormat::Ymd,
                target: RecipeDateTarget::Date,
            }],
            calculated_column: Some(CalculatedColumnRecipe {
                name: name.to_owned(),
                source: "when".to_owned(),
                operation,
                operand: None,
            }),
            ..TransformRecipe::default()
        };
        let expected = apply_recipe_to_frame(&source_frame, &recipe)
            .expect("la receta eager debe ser válida")
            .0;

        let result = apply_recipe_to_dataset(&mut dataset, &recipe)
            .expect("la receta source-backed debe publicar partes de fecha");

        assert!(dataset.source_backed);
        assert_eq!(result.parsed_date_column_count, 1);
        assert_eq!(result.calculated_column_count, 1);
        assert_eq!(dataset.frame.get_column_names(), ["when", name]);
        let output_path = dataset
            .source_path
            .as_deref()
            .expect("el resultado debe conservar una fuente Parquet");
        let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
        assert!(output.equals_missing(&expected));
        assert_eq!(result.dataset.rows[0][1].as_deref(), Some(expected_value));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }
}

#[test]
fn source_backed_iso8601_matches_eager_for_naive_and_utc_values() {
    for target in [RecipeDateTarget::Datetime, RecipeDateTarget::Date] {
        let path =
            temporary_csv("when\n2025-12-31\n2025-12-31T23:15:30.125\n2025-12-31T23:15:30Z\n");
        let (source_frame, _) = load_csv(&path).expect("el CSV ISO debe cargar");
        let (schema, _, row_count) = source_backed_load(&path, "csv", || false)
            .expect("la fuente ISO debe inspeccionarse en disco");
        let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
        let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
        let mut dataset = LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "dataset.csv".to_owned(),
            file_size_bytes,
            row_count,
            frame: schema,
            source_backed: true,
            profile: None,
            history,
        };
        let recipe = TransformRecipe {
            date_parses: vec![RecipeDateParse {
                column: "when".to_owned(),
                format: RecipeDateFormat::Iso8601,
                target,
            }],
            ..TransformRecipe::default()
        };
        let expected = apply_recipe_to_frame(&source_frame, &recipe)
            .expect("la receta eager ISO debe ser válida")
            .0;

        let result = apply_recipe_to_dataset(&mut dataset, &recipe)
            .expect("la receta ISO source-backed debe publicarse");

        assert!(dataset.source_backed);
        assert_eq!(result.parsed_date_column_count, 1);
        let output_path = dataset
            .source_path
            .as_deref()
            .expect("el resultado debe conservar una fuente Parquet");
        let output = read_parquet_frame(output_path).expect("el Parquet ISO debe leerse");
        assert!(output.equals_missing(&expected));

        fs::remove_file(path).expect("se debe limpiar el CSV temporal");
    }
}

#[test]
fn source_backed_iso8601_validates_the_current_private_snapshot() {
    let path = temporary_csv("when\n2025-12-31T23:15:30+02:00\n2025-12-31T23:15:30Z\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV ISO debe cargar");
    let (schema, _, _) = source_backed_load(&path, "csv", || false)
        .expect("la fuente ISO debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let snapshot_path = history.directory.path().join("filtered.parquet");
    let filtered_frame = source_frame.slice(1, 1);
    let mut snapshot_frame = filtered_frame.clone();
    let mut snapshot_file = File::create(&snapshot_path).expect("el snapshot debe crearse");
    ParquetWriter::new(&mut snapshot_file)
        .finish(&mut snapshot_frame)
        .expect("el snapshot debe escribirse");
    drop(snapshot_file);
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count: filtered_frame.height(),
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    dataset.history.source_snapshot_path = Some(snapshot_path);
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".to_owned(),
            format: RecipeDateFormat::Iso8601,
            target: RecipeDateTarget::Datetime,
        }],
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&filtered_frame, &recipe)
        .expect("la receta eager ISO debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta ISO debe usar el snapshot vigente");

    assert!(dataset.source_backed);
    assert_eq!(result.parsed_date_column_count, 1);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet ISO debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(dataset.row_count, 1);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_iso8601_falls_back_for_non_utc_offsets() {
    let path = temporary_csv("when\n2025-12-31T23:15:30+02:00\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV con offset debe cargar");
    let (schema, _, row_count) = source_backed_load(&path, "csv", || false)
        .expect("la fuente con offset debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".to_owned(),
            format: RecipeDateFormat::Iso8601,
            target: RecipeDateTarget::Datetime,
        }],
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager con offset debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("el fallback eager ISO debe publicarse");

    assert!(!dataset.source_backed);
    assert_eq!(result.parsed_date_column_count, 1);
    assert!(dataset.frame.equals_missing(&expected));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_literal_replacement_matches_eager_order_and_counts_cells() {
    let path =
        temporary_csv("name,note,score\nO'Reilly,customer,1\nOmar,customer,2\nNope,customer,3\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "name".to_owned(),
            to: "customer".to_owned(),
        }],
        filters: vec![RecipeFilter {
            column: "score".to_owned(),
            operator: RecipeFilterOperator::Gte,
            value: Some("2".to_owned()),
        }],
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("name".to_owned()),
            find: "O".to_owned(),
            replace: "X".to_owned(),
            regex: false,
        }),
        keep_columns: Some(vec!["name".to_owned(), "note".to_owned()]),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicar el reemplazo");

    assert!(dataset.source_backed);
    assert_eq!(result.removed_row_count, 1);
    assert_eq!(result.replaced_cell_count, 1);
    assert_eq!(result.renamed_column_count, 1);
    assert_eq!(dataset.frame.get_column_names(), ["customer", "note"]);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(result.dataset.rows[0][0].as_deref(), Some("Xmar"));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_merge_matches_eager_order_nulls_empty_strings_and_casts() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let source_frame = DataFrame::new(
        3,
        vec![
            Series::new("first".into(), [Some("A"), None, None]).into_column(),
            Series::new("second".into(), [Some(""), Some("B"), None]).into_column(),
            Series::new("number".into(), [Some(12_i64), Some(34), Some(56)]).into_column(),
            Series::new("tag".into(), ["x", "y", "z"]).into_column(),
        ],
    )
    .expect("el frame Parquet debe ser válido");
    let mut parquet_frame = source_frame.clone();
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut parquet_frame)
        .expect("se debe escribir el Parquet temporal");
    let schema = read_parquet_schema_frame(&path).expect("se debe leer el esquema Parquet");
    let row_count = source_frame.height();
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "number".to_owned(),
            to: "count".to_owned(),
        }],
        casts: vec![RecipeCast {
            column: "number".to_owned(),
            target: RecipeCastTarget::String,
        }],
        keep_columns: Some(vec![
            "tag".to_owned(),
            "first".to_owned(),
            "second".to_owned(),
            "number".to_owned(),
        ]),
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["first".to_owned(), "second".to_owned(), "number".to_owned()],
            name: "joined".to_owned(),
            separator: "|".to_owned(),
            drop_sources: true,
        }),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicar la unión");

    assert!(dataset.source_backed);
    assert_eq!(
        (
            result.renamed_column_count,
            result.converted_column_count,
            result.merged_column_count,
            result.dropped_source_column_count
        ),
        (1, 1, 1, 3)
    );
    assert_eq!(dataset.frame.get_column_names(), ["tag", "joined"]);
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    assert_eq!(
        result
            .dataset
            .rows
            .iter()
            .map(|row| row[1].clone())
            .collect::<Vec<_>>(),
        vec![
            Some("A||12".to_owned()),
            Some("B|34".to_owned()),
            Some("56".to_owned()),
        ]
    );
}

#[test]
fn source_backed_split_matches_eager_remainder_nulls_empty_segments_and_drop_source() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let source_frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "path".into(),
                [
                    Some("uno🙂dos🙂tres🙂resto"),
                    Some("solo"),
                    None,
                    Some("a🙂"),
                ],
            )
            .into_column(),
            Series::new("tag".into(), ["x", "y", "z", "w"]).into_column(),
            Series::new("category".into(), ["a", "b", "c", "d"]).into_column(),
        ],
    )
    .expect("el frame Parquet debe ser válido");
    let mut parquet_frame = source_frame.clone();
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut parquet_frame)
        .expect("se debe escribir el Parquet temporal");
    let schema = read_parquet_schema_frame(&path).expect("se debe leer el esquema Parquet");
    let row_count = source_frame.height();
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.parquet".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "path".to_owned(),
            to: "route".to_owned(),
        }],
        keep_columns: Some(vec![
            "path".to_owned(),
            "tag".to_owned(),
            "category".to_owned(),
        ]),
        split_column: Some(SplitColumnRecipe {
            source: "path".to_owned(),
            delimiter: "🙂".to_owned(),
            names: vec!["first".to_owned(), "second".to_owned(), "third".to_owned()],
            drop_source: true,
        }),
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["tag".to_owned(), "category".to_owned()],
            name: "label".to_owned(),
            separator: ":".to_owned(),
            drop_sources: false,
        }),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicar la división");

    assert!(dataset.source_backed);
    assert_eq!(
        (
            result.split_column_count,
            result.merged_column_count,
            result.dropped_source_column_count
        ),
        (3, 1, 1)
    );
    assert_eq!(
        dataset.frame.get_column_names(),
        ["tag", "category", "first", "second", "third", "label"]
    );
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    let rows = result.dataset.rows;
    assert_eq!(rows[0][2].as_deref(), Some("uno"));
    assert_eq!(rows[0][3].as_deref(), Some("dos"));
    assert_eq!(rows[0][4].as_deref(), Some("tres🙂resto"));
    assert_eq!(rows[0][5].as_deref(), Some("x:a"));
    assert_eq!(rows[1][2].as_deref(), Some("solo"));
    assert_eq!(rows[1][3], None);
    assert_eq!(rows[2][2], None);
    assert_eq!(rows[3][3].as_deref(), Some(""));
    assert_eq!(rows[3][4], None);
}

#[test]
fn source_backed_text_extractions_match_eager_unicode_nulls_and_empty_segments() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let source_frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "text".into(),
                [
                    Some("  José Pérez 123🙂resto"),
                    Some("a🙂"),
                    Some("🙂"),
                    None,
                ],
            )
            .into_column(),
            Series::new(
                "arabic".into(),
                [Some("١٢ abc 45"), Some(""), None, Some("١٢")],
            )
            .into_column(),
        ],
    )
    .expect("el frame Parquet debe ser válido");
    let mut parquet_frame = source_frame.clone();
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut parquet_frame)
        .expect("se debe escribir el Parquet temporal");
    let schema = read_parquet_schema_frame(&path).expect("se debe leer el esquema Parquet");
    let row_count = source_frame.height();
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.parquet".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "text".to_owned(),
            to: "description".to_owned(),
        }],
        keep_columns: Some(vec!["text".to_owned(), "arabic".to_owned()]),
        text_extractions: vec![
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::FirstToken,
                name: "first".to_owned(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::LastToken,
                name: "last".to_owned(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::Letters,
                name: "letters".to_owned(),
                delimiter: None,
            },
            TextExtraction {
                source: "arabic".to_owned(),
                kind: ExtractionKind::Digits,
                name: "digits".to_owned(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::Before,
                name: "before".to_owned(),
                delimiter: Some("🙂".to_owned()),
            },
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::After,
                name: "after".to_owned(),
                delimiter: Some("🙂".to_owned()),
            },
            TextExtraction {
                source: "text".to_owned(),
                kind: ExtractionKind::Before,
                name: "missing".to_owned(),
                delimiter: Some("NO".to_owned()),
            },
        ],
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicar las extracciones");

    assert!(dataset.source_backed);
    assert_eq!(result.extracted_column_count, 7);
    assert_eq!(
        dataset.frame.get_column_names(),
        [
            "description",
            "arabic",
            "first",
            "last",
            "letters",
            "digits",
            "before",
            "after",
            "missing"
        ]
    );
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    let rows = result.dataset.rows;
    assert_eq!(rows[0][2].as_deref(), Some("José"));
    assert_eq!(rows[0][3].as_deref(), Some("123🙂resto"));
    assert_eq!(rows[0][4].as_deref(), Some("José"));
    assert_eq!(rows[0][5].as_deref(), Some("45"));
    assert_eq!(rows[0][6].as_deref(), Some("  José Pérez 123"));
    assert_eq!(rows[0][7].as_deref(), Some("resto"));
    assert_eq!(rows[0][8], None);
    assert_eq!(rows[1][6].as_deref(), Some("a"));
    assert_eq!(rows[1][7].as_deref(), Some(""));
    assert_eq!(rows[2][6].as_deref(), Some(""));
    assert_eq!(rows[2][7].as_deref(), Some(""));
    assert_eq!(rows[3][2], None);
    assert_eq!(rows[3][8], None);

    fs::remove_file(path).expect("se debe limpiar el Parquet temporal");
}

#[test]
fn source_backed_contact_normalizations_match_eager_and_feed_extractions() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let source_frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "email".into(),
                [
                    Some("\u{a0}A@B.COM\u{a0}"),
                    None,
                    Some("i\u{307}@EXAMPLE.COM"),
                    Some("ok@example.com"),
                ],
            )
            .into_column(),
            Series::new(
                "phone".into(),
                [
                    Some("\u{a0}+1 (809) 555-01\u{a0}"),
                    Some("809-12"),
                    None,
                    Some("+"),
                ],
            )
            .into_column(),
            Series::new(
                "address".into(),
                [
                    Some(" Calle\u{a0}Uno\nNorte "),
                    Some(" 12 "),
                    None,
                    Some("ok"),
                ],
            )
            .into_column(),
        ],
    )
    .expect("el frame Parquet debe ser válido");
    let mut parquet_frame = source_frame.clone();
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut parquet_frame)
        .expect("se debe escribir el Parquet temporal");
    let schema = read_parquet_schema_frame(&path).expect("se debe leer el esquema Parquet");
    let row_count = source_frame.height();
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.parquet".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "email".to_owned(),
            to: "mail".to_owned(),
        }],
        keep_columns: Some(vec![
            "email".to_owned(),
            "phone".to_owned(),
            "address".to_owned(),
        ]),
        contact_normalizations: vec![
            ContactNormalization {
                column: "email".to_owned(),
                kind: ContactKind::Email,
            },
            ContactNormalization {
                column: "phone".to_owned(),
                kind: ContactKind::Phone,
            },
            ContactNormalization {
                column: "address".to_owned(),
                kind: ContactKind::Address,
            },
        ],
        text_extractions: vec![TextExtraction {
            source: "email".to_owned(),
            kind: ExtractionKind::Before,
            name: "user".to_owned(),
            delimiter: Some("@".to_owned()),
        }],
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicar los contactos");

    assert!(dataset.source_backed);
    assert_eq!(
        (
            result.normalized_contact_cell_count,
            result.normalized_contact_column_count,
            result.extracted_column_count
        ),
        (6, 3, 1)
    );
    assert_eq!(
        dataset.frame.get_column_names(),
        ["mail", "phone", "address", "user"]
    );
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");
    assert!(output.equals_missing(&expected));
    let rows = result.dataset.rows;
    assert_eq!(rows[0][0].as_deref(), Some("a@b.com"));
    assert_eq!(rows[0][1].as_deref(), Some("+180955501"));
    assert_eq!(rows[0][2].as_deref(), Some("Calle Uno Norte"));
    assert_eq!(rows[0][3].as_deref(), Some("a"));
    assert_eq!(rows[1][0], None);
    assert_eq!(rows[1][1].as_deref(), Some("80912"));
    assert_eq!(rows[1][2].as_deref(), Some("12"));
    assert_eq!(rows[2][3].as_deref(), Some("i\u{307}"));
    assert_eq!(rows[3][0].as_deref(), Some("ok@example.com"));
    assert_eq!(rows[3][1].as_deref(), Some("+"));

    fs::remove_file(path).expect("se debe limpiar el Parquet temporal");
}

#[test]
fn source_backed_text_and_null_filters_keep_eager_semantics() {
    let path = temporary_csv("name,tag\nO'Reilly,\nOmar,customer\n");
    let (source_frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let recipe = TransformRecipe {
        filters: vec![
            RecipeFilter {
                column: "name".to_owned(),
                operator: RecipeFilterOperator::Eq,
                value: Some("O'Reilly".to_owned()),
            },
            RecipeFilter {
                column: "tag".to_owned(),
                operator: RecipeFilterOperator::IsNull,
                value: None,
            },
        ],
        keep_columns: Some(vec!["name".to_owned()]),
        ..TransformRecipe::default()
    };
    let expected = apply_recipe_to_frame(&source_frame, &recipe)
        .expect("la receta eager debe ser válida")
        .0;

    let result = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect("la receta source-backed debe publicarse");
    let output_path = dataset
        .source_path
        .as_deref()
        .expect("el resultado debe conservar una fuente Parquet");
    let output = read_parquet_frame(output_path).expect("el Parquet resultante debe leerse");

    assert!(output.equals_missing(&expected));
    assert_eq!(result.removed_row_count, 1);
    assert_eq!(result.dataset.rows[0][0].as_deref(), Some("O'Reilly"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn materializes_a_deferred_dataset_only_when_an_operation_requires_rows() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let history = HistoryManager::deferred().expect("el historial diferido debe inicializarse");
    let file_size_bytes = fs::metadata(&path).expect("la fuente debe existir").len();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    materialize_loaded_dataset(&mut dataset)
        .expect("la operación debe materializar la fuente sin cambiarla");

    assert!(!dataset.source_backed);
    assert_eq!(dataset.frame.height(), 2);
    assert_eq!(dataset.row_count, 2);
    assert_eq!(
        preview_value(dataset.frame.column("city").unwrap().get(0).unwrap()),
        Some("Santo Domingo".to_owned())
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn large_source_backed_materialization_requires_available_memory_budget() {
    let required = MATERIALIZATION_GUARD_THRESHOLD_BYTES
        .saturating_mul(MATERIALIZATION_ESTIMATE_MULTIPLIER)
        .saturating_add(MATERIALIZATION_RESERVE_BYTES);
    let error = materialization_budget_error(
        MATERIALIZATION_GUARD_THRESHOLD_BYTES,
        required.saturating_sub(1),
    )
    .expect("la materialización grande debe rechazar RAM insuficiente");

    assert!(error.contains("materializar una fuente o snapshot grande"));
    assert!(error.contains("RAM disponible"));
}

#[test]
fn materialization_budget_does_not_block_small_sources_or_exact_capacity() {
    assert!(materialization_budget_error(
        MATERIALIZATION_GUARD_THRESHOLD_BYTES.saturating_sub(1),
        1,
    )
    .is_none());

    let required = MATERIALIZATION_GUARD_THRESHOLD_BYTES
        .saturating_mul(MATERIALIZATION_ESTIMATE_MULTIPLIER)
        .saturating_add(MATERIALIZATION_RESERVE_BYTES);
    assert!(
        materialization_budget_error(MATERIALIZATION_GUARD_THRESHOLD_BYTES, required,).is_none()
    );
}

#[test]
fn source_backed_profile_matches_the_in_memory_profile_without_retaining_rows() {
    let path = temporary_csv(
        "city,segment,amount,when\nSanto Domingo,Pyme,10,2025-01-01\nSanto Domingo,Pyme,20,2025-01-02\nSantiago,Pyme,30,2025-01-03\nSantiago,Pyme,30,2025-01-03\nLa Romana,Empresa,40,2025-01-04\nLa Romana,Empresa,50,2025-01-05\n",
    );
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let expected = profile_dataset(&frame).expect("el perfil en memoria debe calcularse");
    let (_, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe inspeccionarse en disco");
    let mut updates = Vec::new();
    let actual = profile_source_backed_with_progress(
        &path,
        "csv",
        fs::metadata(&path).expect("la fuente debe existir").len(),
        row_count,
        |stage, percent| updates.push((stage, percent)),
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect("el perfil source-backed debe calcularse");

    assert_eq!(actual.row_count, expected.row_count);
    assert_eq!(actual.duplicate_row_count, expected.duplicate_row_count);
    assert_eq!(
        actual.near_duplicate_row_count,
        expected.near_duplicate_row_count
    );
    assert_eq!(actual.columns, expected.columns);
    assert_eq!(
        actual.categorical_group_summaries,
        expected.categorical_group_summaries
    );
    assert_eq!(actual.temporal_series, expected.temporal_series);
    assert!(actual.numeric_correlations.is_none());
    assert!(updates
        .iter()
        .any(|(stage, _)| *stage == "Analizando filas y columnas"));
    assert!(updates.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn materialized_history_snapshot_profiles_without_using_the_active_frame() {
    let frame = df![
        "city" => &["Santo Domingo", "Santiago", "Santiago"],
        "amount" => &[10_i64, 20, 20],
        "score" => &[1_i64, 2, 2]
    ]
    .expect("el dataset debe construirse");
    let expected = profile_dataset(&frame).expect("el perfil en memoria debe calcularse");
    let history = HistoryManager::new(&frame).expect("el historial durable debe inicializarse");
    let dataset = LoadedDataset {
        source_path: None,
        file_name: "materialized.parquet".to_owned(),
        file_size_bytes: 256,
        row_count: frame.height(),
        frame: frame.clone(),
        source_backed: false,
        profile: None,
        history,
    };
    let (snapshot_path, snapshot_size, row_count) = current_history_parquet_snapshot(&dataset)
        .expect("el cursor durable debe exponer un snapshot Parquet");
    let actual = profile_source_backed_with_progress(
        &snapshot_path,
        "parquet",
        snapshot_size,
        row_count,
        |_, _| {},
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect("el perfil del snapshot Parquet debe calcularse");

    assert_eq!(actual.row_count, expected.row_count);
    assert_eq!(actual.duplicate_row_count, expected.duplicate_row_count);
    assert_eq!(actual.columns, expected.columns);
    assert_eq!(
        actual.categorical_group_summaries,
        expected.categorical_group_summaries
    );
    assert_eq!(actual.temporal_series, expected.temporal_series);
    assert_eq!(actual.numeric_correlations, expected.numeric_correlations);
    assert_eq!(dataset.frame.height(), 3);
}

#[test]
fn source_backed_quality_validation_matches_the_in_memory_contract() {
    let path = temporary_csv(
        "status,code,amount,when,comment,country\nok,A,10,2024-01-01,ready,DO\nskip,B,20,2024-06-15,,US\nok,A,30,2025-01-01,valid,DO\n",
    );
    let frame = read_delimited_frame(&path, "csv").expect("el CSV debe leerse como texto");
    let mut non_empty = quality_rule("comment", QualityRuleKind::NonEmpty);
    non_empty.max_invalid = Some(1);
    let mut allowed = quality_rule("status", QualityRuleKind::AllowedValues);
    allowed.values = Some(vec!["ok".to_owned(), "skip".to_owned()]);
    let mut regex = quality_rule("code", QualityRuleKind::Regex);
    regex.pattern = Some("^[A-Z]$".to_owned());
    let mut date_range = quality_rule("when", QualityRuleKind::DateRange);
    date_range.min_date = Some("2024-01-01".to_owned());
    date_range.max_date = Some("2024-12-31".to_owned());
    date_range.max_invalid = Some(1);
    let mut conditional = quality_rule("comment", QualityRuleKind::Conditional);
    conditional.when = Some(QualityCondition {
        column: "status".to_owned(),
        operator: Some(QualityComparison::Eq),
        value: Some("ok".to_owned()),
    });
    conditional.then = Some(Box::new(quality_rule("comment", QualityRuleKind::NonEmpty)));
    let mut referential = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
    referential.columns = Some(vec!["country".to_owned()]);
    referential.reference_values = Some(vec!["DO".to_owned(), "US".to_owned()]);
    let mut dtype = quality_rule("status", QualityRuleKind::Dtype);
    dtype.dtype = Some("string".to_owned());
    let mut schema = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::SchemaContract);
    schema.columns = Some(
        ["status", "code", "amount", "when", "comment", "country"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    );
    schema.required_order = Some(
        schema
            .columns
            .clone()
            .expect("el esquema debe tener columnas"),
    );
    let mut row_count = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::RowCount);
    row_count.min = Some(3.0);
    row_count.max = Some(3.0);
    let unique = quality_rule("code", QualityRuleKind::Unique);
    let mut unique_together = quality_rule("status", QualityRuleKind::UniqueTogether);
    unique_together.columns = Some(vec!["status".to_owned(), "country".to_owned()]);
    let monotonic = quality_rule("code", QualityRuleKind::Monotonic);
    let mut aggregate = quality_rule("amount", QualityRuleKind::AggregateCheck);
    aggregate.aggregate = Some(QualityAggregate::Sum);
    aggregate.expected = Some(60.0);
    let mut drift = quality_rule("amount", QualityRuleKind::DistributionDrift);
    drift.baseline = Some(vec!["10".to_owned(), "20".to_owned(), "30".to_owned()]);
    let rules = vec![
        quality_rule("country", QualityRuleKind::NotNull),
        non_empty,
        allowed,
        regex,
        date_range,
        conditional,
        referential,
        dtype,
        schema,
        row_count,
        unique,
        unique_together,
        monotonic,
        aggregate,
        drift,
    ];
    let expected =
        evaluate_quality_rules(&frame, &rules).expect("la validación en memoria debe funcionar");
    let actual = evaluate_source_quality_rules_with_cancel(
        &path,
        "csv",
        fs::metadata(&path).expect("la fuente debe existir").len(),
        frame.height(),
        &rules,
        || false,
    )
    .expect("la validación source-backed debe funcionar");

    assert_eq!(actual.passed, expected.passed);
    assert_eq!(actual.row_count, expected.row_count);
    assert_eq!(actual.failed_rules, expected.failed_rules);
    assert_eq!(actual.rules.len(), expected.rules.len());
    for (actual_rule, expected_rule) in actual.rules.iter().zip(expected.rules.iter()) {
        assert_eq!(actual_rule.kind, expected_rule.kind);
        assert_eq!(actual_rule.checked_count, expected_rule.checked_count);
        assert_eq!(actual_rule.invalid_count, expected_rule.invalid_count);
        assert_eq!(actual_rule.passed, expected_rule.passed);
    }

    assert!(source_quality_rule_is_incremental(&quality_rule(
        "code",
        QualityRuleKind::Unique,
    )));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_parquet_export_streams_through_a_private_snapshot() {
    let source = temporary_csv("city,amount\nSanto Domingo,10\nSantiago,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.parquet");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_parquet_atomic(
        &source,
        expected_size,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación source-backed debe funcionar");
    let exported = read_parquet_frame(&destination)
        .expect("la salida Parquet debe poder leerse")
        .height();

    assert_eq!(result.format, "Parquet");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(exported, 2);
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_csv_export_streams_without_materializing_the_active_frame() {
    let source = temporary_csv("city,amount\nSanto Domingo,10\nSantiago,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.csv");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_csv_atomic(
        &source,
        expected_size,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación CSV source-backed debe funcionar");
    let exported = read_delimited_frame(&destination, "csv")
        .expect("la salida CSV debe poder leerse")
        .height();

    assert_eq!(result.format, "CSV");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(exported, 2);
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert!(source.is_file());
    assert!(destination.is_file());
}

#[test]
fn source_backed_xlsx_export_streams_rows_without_materializing_the_active_frame() {
    let source = temporary_csv("name,amount\nO'Brien,10\n=SUM(A1:A2),20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.xlsx");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_xlsx_atomic(
        &source,
        expected_size,
        2,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación Excel source-backed debe funcionar");

    let bytes = fs::read(&destination).expect("el XLSX debe existir");
    assert_eq!(&bytes[..2], b"PK");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut sheet = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet)
        .unwrap();
    assert!(sheet.contains("O&apos;Brien"));
    assert!(sheet.contains("=SUM(A1:A2)"));
    assert!(sheet.contains("t=\"inlineStr\""));
    let mut workbook = open_workbook_auto(&destination).expect("el XLSX debe poder reabrirse");
    let range = workbook
        .worksheet_range("dataset")
        .expect("la hoja dataset debe existir");
    assert_eq!(
        range.get((0, 0)).map(ToString::to_string).as_deref(),
        Some("name")
    );
    assert_eq!(
        range.get((2, 0)).map(ToString::to_string).as_deref(),
        Some("=SUM(A1:A2)")
    );
    assert_eq!(result.format, "Excel");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert!(source.is_file());
    assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_sqlite_export_streams_rows_without_materializing_the_active_frame() {
    let source = temporary_csv("name,amount\nO'Brien,10\n,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.sqlite");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_sqlite_atomic(
        &source,
        expected_size,
        2,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación SQLite source-backed debe funcionar");

    let connection = Connection::open(&destination).expect("la base SQLite debe abrirse");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM dataset", [], |row| row.get(0))
        .unwrap();
    let name: Option<String> = connection
        .query_row("SELECT name FROM dataset WHERE amount = '10'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(name.as_deref(), Some("O'Brien"));
    assert_eq!(result.format, "SQLite");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert!(source.is_file());
    assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_sql_export_streams_without_materializing_the_active_frame() {
    let source = temporary_csv("name,amount\nO'Brien,10\nSantiago,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.sql");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_sql_atomic(
        &source,
        expected_size,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación SQL source-backed debe funcionar");

    let script = fs::read_to_string(&destination).expect("la salida SQL debe poder leerse");
    assert_eq!(result.format, "SQL");
    assert_eq!(result.protected_column_count, 0);
    assert!(script.contains("INSERT INTO \"dataset\""));
    assert!(script.contains("'O''Brien'"));
    assert!(script.contains("COMMIT;"));
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert!(source.is_file());
    assert!(destination.is_file());
    assert!(!directory.path().join("dataset.parquet").exists());
}

#[test]
fn source_backed_global_quality_rules_count_duplicates_across_blocks() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 1;
    let mut contents = String::from("id,group,ordered,amount\n");
    for row_index in 0..row_count {
        let id = if row_index == LOCAL_QUERY_BLOCK_ROWS {
            "0".to_owned()
        } else {
            row_index.to_string()
        };
        let group = if row_index == LOCAL_QUERY_BLOCK_ROWS {
            "segment-0".to_owned()
        } else {
            format!("segment-{row_index}")
        };
        let ordered = if row_index == LOCAL_QUERY_BLOCK_ROWS {
            "0"
        } else {
            "a"
        };
        contents.push_str(&format!("{id},{group},{ordered},1\n"));
    }
    let path = temporary_csv(&contents);
    let frame = read_delimited_frame(&path, "csv").expect("el CSV debe leerse como texto");
    let unique = quality_rule("id", QualityRuleKind::Unique);
    let mut unique_together = quality_rule("id", QualityRuleKind::UniqueTogether);
    unique_together.columns = Some(vec!["id".to_owned(), "group".to_owned()]);
    let monotonic = quality_rule("ordered", QualityRuleKind::Monotonic);
    let mut aggregate = quality_rule("amount", QualityRuleKind::AggregateCheck);
    aggregate.aggregate = Some(QualityAggregate::Sum);
    aggregate.expected = Some(row_count as f64);
    let rules = vec![unique, unique_together, monotonic, aggregate];

    let expected =
        evaluate_quality_rules(&frame, &rules).expect("la validación en memoria debe funcionar");
    let actual = evaluate_source_quality_rules_with_cancel(
        &path,
        "csv",
        fs::metadata(&path).expect("la fuente debe existir").len(),
        row_count,
        &rules,
        || false,
    )
    .expect("la validación global source-backed debe funcionar");

    assert_eq!(actual.failed_rules, expected.failed_rules);
    assert_eq!(
        actual
            .rules
            .iter()
            .map(|rule| rule.invalid_count)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 0]
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_json_export_streams_without_leaving_private_artifacts() {
    let source = temporary_csv("city,amount\nSanto Domingo,10\nSantiago,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.json");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_json_atomic(
        &source,
        expected_size,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("la exportación JSON source-backed debe funcionar");
    let exported: JsonValue =
        serde_json::from_slice(&fs::read(&destination).expect("la salida JSON debe existir"))
            .expect("la salida JSON debe ser válida");

    assert_eq!(result.format, "JSON");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(exported.as_array().map(Vec::len), Some(2));
    assert_eq!(
        exported[0]["city"],
        JsonValue::String("Santo Domingo".to_owned())
    );
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_bundle_streams_dataset_and_builds_dictionary_without_rows_in_memory() {
    let source = temporary_csv("city,amount\nSanto Domingo,10\n,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported.zip");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut progress = Vec::new();
    let result = export_source_backed_bundle_atomic(
        &source,
        expected_size,
        2,
        None,
        None,
        &destination,
        |stage, percent| progress.push((stage, percent)),
        || false,
    )
    .expect("el Bundle source-backed debe publicarse");

    let bytes = fs::read(&destination).expect("el Bundle debe existir");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for name in ["dataset.csv", "dictionary.json", "manifest.json"] {
        assert!(archive.by_name(name).is_ok(), "falta {name} en el Bundle");
    }
    let mut dictionary = String::new();
    archive
        .by_name("dictionary.json")
        .unwrap()
        .read_to_string(&mut dictionary)
        .unwrap();
    let dictionary: JsonValue = serde_json::from_str(&dictionary).unwrap();
    assert_eq!(dictionary["columns"][0]["name"], "city");
    assert_eq!(dictionary["columns"][0]["nullCount"], 1);
    assert_eq!(dictionary["columns"][1]["name"], "amount");
    assert_eq!(dictionary["columns"][1]["nullCount"], 0);
    assert_eq!(result.format, "Paquete Columnia");
    assert_eq!(result.protected_column_count, 0);
    assert_eq!(progress.last(), Some(&("Exportación lista", 100)));
    assert!(source.is_file());
    assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_bundle_includes_validated_recipe_without_materializing_rows() {
    let source = temporary_csv("city,amount\nSanto Domingo,10\nSantiago,20\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let destination = directory.path().join("exported-with-recipe.zip");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let recipe = complete_stored_recipe();

    export_source_backed_bundle_atomic(
        &source,
        expected_size,
        2,
        None,
        Some(&recipe),
        &destination,
        |_, _| {},
        || false,
    )
    .expect("el Bundle source-backed debe conservar la receta");

    let bytes = fs::read(&destination).expect("el Bundle debe existir");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut recipe_json = String::new();
    archive
        .by_name("recipe.json")
        .expect("falta recipe.json en el Bundle source-backed")
        .read_to_string(&mut recipe_json)
        .unwrap();
    let recipe_value: JsonValue = serde_json::from_str(&recipe_json).unwrap();
    assert_eq!(recipe_value["name"], "Limpieza completa");

    let mut manifest_json = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest_json)
        .unwrap();
    let manifest: JsonValue = serde_json::from_str(&manifest_json).unwrap();
    assert_eq!(manifest["recipeFile"], "recipe.json");
    let expected_hash = format!("{:x}", Sha256::digest(recipe_json.as_bytes()));
    assert!(manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "recipe.json" && file["sha256"] == expected_hash));
    assert!(source.is_file());
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn reports_ordered_csv_loading_phases() {
    let path = temporary_csv("city\nSanto Domingo\n");
    let mut updates = Vec::new();

    load_csv_with_progress(
        &path,
        |stage, percent| updates.push((stage, percent)),
        || false,
    )
    .expect("el CSV debe cargar");

    assert_eq!(
        updates,
        vec![
            ("Validando archivo", 10),
            ("Leyendo y detectando columnas", 25),
            ("Preparando vista previa", 85),
            ("Preparando sesión", 95),
        ]
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn rejects_unsupported_dataset_files() {
    let path = std::env::temp_dir().join("columnia-invalid-dataset.bin");
    File::create(&path).expect("se debe poder crear el archivo temporal");

    let error = validate_dataset_file(&path)
        .expect_err("un archivo que no es un dataset compatible debe rechazarse");

    assert!(error.contains("admite CSV, TSV, TXT delimitado, JSON, Parquet y libros Excel/ODS"));
    fs::remove_file(path).expect("se debe limpiar el archivo temporal");
}

#[test]
fn canonicalizes_dataset_reads_and_write_parents() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let nested = directory.path().join("nested");
    fs::create_dir(&nested).expect("se debe crear la subcarpeta");
    let dataset = directory.path().join("datos.csv");
    fs::write(&dataset, "value\n1\n").expect("se debe crear el dataset");
    let non_canonical_dataset = nested.join("..").join("datos.csv");

    let (canonical_dataset, _, extension) =
        validate_dataset_file(&non_canonical_dataset).expect("la ruta equivalente debe validarse");
    let destination =
        canonicalize_write_destination(&nested.join("..").join("salida.csv"), "la exportación")
            .expect("la carpeta de salida debe canonicalizarse");

    assert_eq!(canonical_dataset, fs::canonicalize(dataset).unwrap());
    assert_eq!(extension, "csv");
    assert_eq!(
        destination,
        fs::canonicalize(directory.path())
            .unwrap()
            .join("salida.csv")
    );
}

#[test]
fn rejects_directories_as_read_sources_or_write_destinations() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");

    let read_error = canonicalize_existing_file(directory.path(), "el dataset seleccionado")
        .expect_err("un directorio no es un dataset");
    let write_error = canonicalize_write_destination(directory.path(), "la exportación")
        .expect_err("un directorio no es un archivo de destino");

    assert!(read_error.contains("archivo regular"));
    assert!(write_error.contains("archivo regular"));
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_for_reads_and_existing_destinations() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let target = directory.path().join("target.csv");
    let link = directory.path().join("link.csv");
    let dangling_link = directory.path().join("dangling.csv");
    fs::write(&target, "value\n1\n").expect("se debe crear el archivo real");
    symlink(&target, &link).expect("se debe crear el enlace simbólico");
    symlink(directory.path().join("missing.csv"), &dangling_link)
        .expect("se debe crear el enlace simbólico colgante");

    let read_error = canonicalize_existing_file(&link, "el dataset seleccionado")
        .expect_err("una lectura no debe seguir enlaces simbólicos");
    let write_error = canonicalize_write_destination(&link, "la exportación")
        .expect_err("una escritura no debe seguir enlaces simbólicos");
    let dangling_error = canonicalize_write_destination(&dangling_link, "la exportación")
        .expect_err("una escritura no debe aceptar enlaces simbólicos colgantes");

    assert!(read_error.contains("enlace simbólico"));
    assert!(write_error.contains("enlace simbólico"));
    assert!(dangling_error.contains("enlace simbólico"));
}

#[cfg(windows)]
#[test]
fn rejects_windows_reparse_points_including_dangling_links() {
    use std::io::ErrorKind;
    use std::os::windows::fs::symlink_file;

    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let target = directory.path().join("target.csv");
    let link = directory.path().join("link.csv");
    let dangling_link = directory.path().join("dangling.csv");
    fs::write(&target, "value\n1\n").expect("se debe crear el archivo real");

    match symlink_file(&target, &link) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("no se pudo crear el enlace simbólico de prueba: {error}"),
    }
    match symlink_file(directory.path().join("missing.csv"), &dangling_link) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => {
            panic!("no se pudo crear el enlace simbólico colgante de prueba: {error}")
        }
    }

    let read_error = canonicalize_existing_file(&link, "el dataset seleccionado")
        .expect_err("una lectura no debe seguir reparse points");
    let write_error = canonicalize_write_destination(&link, "la exportación")
        .expect_err("una escritura no debe seguir reparse points");
    let dangling_read_error = canonicalize_existing_file(&dangling_link, "el dataset seleccionado")
        .expect_err("una lectura no debe aceptar enlaces simbólicos colgantes");
    let dangling_write_error = canonicalize_write_destination(&dangling_link, "la exportación")
        .expect_err("una escritura no debe aceptar enlaces simbólicos colgantes");

    assert!(read_error.contains("punto de reanálisis"));
    assert!(write_error.contains("punto de reanálisis"));
    assert!(dangling_read_error.contains("punto de reanálisis"));
    assert!(dangling_write_error.contains("punto de reanálisis"));
}

#[test]
fn accepts_a_dataset_above_the_previous_500_mebibyte_threshold() {
    let path = temporary_csv("value\n");
    let previous_limit = 500_u64 * 1024 * 1024;
    File::options()
        .write(true)
        .open(&path)
        .expect("se debe poder abrir el CSV temporal")
        .set_len(previous_limit + 1)
        .expect("se debe poder crear un archivo disperso grande");

    let (_, size, extension) = validate_dataset_file(&path)
        .expect("el tamaño no debe impedir seleccionar un dataset compatible");

    assert_eq!(size, previous_limit + 1);
    assert_eq!(extension, "csv");
    fs::remove_file(path).expect("se debe eliminar el CSV temporal");
}

#[test]
fn returns_a_bounded_page_from_an_offset() {
    let path = temporary_csv("value\nfirst\nsecond\nthird\nfourth\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let page = dataset_page(&frame, 2, 2).expect("la página debe existir");

    assert_eq!(page.offset, 2);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0][0].as_deref(), Some("third"));
    assert_eq!(page.rows[1][0].as_deref(), Some("fourth"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn reads_only_the_schema_for_parquet_query_validation() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let mut frame =
        df!["id" => &[1_i64, 2], "name" => &["A", "B"]].expect("el frame Parquet debe ser válido");
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut frame)
        .expect("se debe escribir el Parquet temporal");

    let schema = read_parquet_schema_frame(&path).expect("se debe leer el esquema Parquet");

    assert_eq!(schema.height(), 0);
    assert_eq!(schema.column("id").unwrap().dtype(), &DataType::Int64);
    assert_eq!(schema.column("name").unwrap().dtype(), &DataType::String);
}

#[test]
fn reads_a_page_from_the_parquet_snapshot_with_slice_pushdown() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let mut frame = df![
        "id" => &[1_i64, 2, 3, 4],
        "name" => &["A", "B", "C", "D"]
    ]
    .expect("el frame Parquet debe ser válido");
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut frame)
        .expect("se debe escribir el Parquet temporal");

    let page = dataset_page_from_parquet(&path, frame.height(), 1, 2)
        .expect("la página debe leerse desde el snapshot");

    assert_eq!(page.offset, 1);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0][0].as_deref(), Some("2"));
    assert_eq!(page.rows[1][1].as_deref(), Some("C"));
}

#[test]
fn reads_a_page_from_an_unchanged_delimited_source() {
    let path = temporary_csv("value\nfirst\nsecond\nthird\nfourth\n");

    let page = dataset_page_from_source(&path, "csv", 4, 1, 2)
        .expect("la página debe leerse desde la fuente delimitada");

    assert_eq!(page.offset, 1);
    assert_eq!(
        page.rows,
        vec![
            vec![Some("second".to_owned())],
            vec![Some("third".to_owned())],
        ]
    );
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn rejects_oversized_pages() {
    let frame = DataFrame::empty();

    let error = dataset_page(&frame, 0, MAX_PAGE_SIZE + 1)
        .expect_err("una página excesiva debe rechazarse");

    assert!(error.contains("entre 1 y 200"));
}

#[test]
fn local_query_is_read_only_projected_and_bounded() {
    let frame = df![
        "city" => &["Santo Domingo", "Santiago", "La Vega"],
        "value" => &[10_i64, 20_i64, 30_i64]
    ]
    .unwrap();
    let result = execute_local_query(&frame, "SELECT city, value FROM dataset LIMIT 1 OFFSET 1")
        .expect("la consulta segura debe ejecutarse");
    assert_eq!(
        result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["city", "value"]
    );
    assert_eq!(result.row_count, 3);
    assert_eq!(result.offset, 1);
    assert_eq!(
        result.rows,
        vec![vec![Some("Santiago".to_owned()), Some("20".to_owned())]]
    );
    assert!(result.truncated);
    assert!(execute_local_query(&frame, "DELETE FROM dataset").is_err());
    assert!(execute_local_query(&frame, "SELECT * FROM dataset LIMIT 201").is_err());
    assert!(execute_local_query(&frame, "SELECT missing FROM dataset").is_err());
}

#[test]
fn parquet_backed_local_queries_match_materialized_results_across_blocks() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 3;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let groups = (0..row_count)
        .map(|index| {
            if index.is_multiple_of(2) {
                "even".to_owned()
            } else {
                "odd".to_owned()
            }
        })
        .collect::<Vec<_>>();
    let frame = DataFrame::new(
        row_count,
        vec![
            Column::new("id".into(), ids),
            Column::new("group".into(), groups),
        ],
    )
    .expect("el frame de consulta debe ser válido");
    {
        let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
        let mut snapshot = frame.clone();
        ParquetWriter::new(&mut file)
            .finish(&mut snapshot)
            .expect("se debe escribir el Parquet temporal");
    }

    for query in [
        "SELECT id, group FROM dataset WHERE id >= 16382 LIMIT 7",
        "SELECT group, COUNT(*) AS total FROM dataset GROUP BY group LIMIT 10",
    ] {
        let eager =
            execute_local_query(&frame, query).expect("la consulta materializada debe ejecutarse");
        let parquet =
            execute_local_query_from_parquet_with_cancel(&path, frame.height(), query, &|| false)
                .expect("la consulta respaldada por Parquet debe ejecutarse");
        assert_eq!(parquet, eager, "la consulta debe conservar su paridad");
    }
}

#[test]
fn parquet_backed_local_query_honors_cancellation_and_rejects_stale_row_count() {
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let path = directory.path().join("source.parquet");
    let mut frame = df!["id" => &[1_i64, 2, 3]].expect("el frame debe ser válido");
    let mut file = File::create(&path).expect("se debe crear el Parquet temporal");
    ParquetWriter::new(&mut file)
        .finish(&mut frame)
        .expect("se debe escribir el Parquet temporal");
    drop(file);

    let cancelled = execute_local_query_from_parquet_with_cancel(
        &path,
        frame.height(),
        "SELECT * FROM dataset",
        &|| true,
    )
    .expect_err("una consulta cancelada no debe leer el snapshot");
    assert_eq!(cancelled, OPERATION_CANCELLED_MESSAGE);

    let stale = execute_local_query_from_parquet_with_cancel(
        &path,
        frame.height() + 1,
        "SELECT * FROM dataset",
        &|| false,
    )
    .expect_err("un snapshot con conteo obsoleto debe fallar cerrado");
    assert!(stale.starts_with(LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX));

    let short = execute_local_query_from_parquet_with_cancel(
        &path,
        frame.height() - 1,
        "SELECT * FROM dataset",
        &|| false,
    )
    .expect_err("un snapshot con filas sobrantes debe fallar cerrado");
    assert!(short.starts_with(LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX));
}

#[test]
fn duckdb_query_uses_the_same_safe_contract_and_join_page() {
    let current = df![
        "id" => &[1_i64, 2, 3],
        "city" => &["Santo Domingo", "Santiago", "La Vega"]
    ]
    .unwrap();
    let compared = df!["id" => &[2_i64, 3], "segment" => &["B", "C"]].unwrap();
    let spec = prepare_duckdb_query(
        "SELECT id, segment FROM dataset LEFT JOIN compared ON dataset.id = compared.id LIMIT 2",
        &current,
        Some(&compared),
    )
    .expect("la consulta DuckDB debe validar el JOIN local");

    let result =
        crate::duckdb_query::execute_duckdb_query(&current, Some(&compared), &spec, || false)
            .expect("DuckDB debe ejecutar el JOIN local");

    assert_eq!(result.row_count, 3);
    assert_eq!(result.offset, 0);
    assert_eq!(result.rows[0], vec![Some("1".to_owned()), None]);
    assert_eq!(
        result.rows[1],
        vec![Some("2".to_owned()), Some("B".to_owned())]
    );
    assert!(result.truncated);
}

#[test]
fn duckdb_join_can_read_the_original_file_when_history_is_degraded() {
    let current_path = temporary_csv("id,name\n1,A\n2,B\n3,C\n");
    let current = df![
        "id" => &["1", "2", "3"],
        "name" => &["A", "B", "C"]
    ]
    .expect("el dataset activo debe construirse");
    let compared = df!["id" => &["2", "3"], "amount" => &[200_i64, 300]]
        .expect("el dataset comparado debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let mut dataset = loaded_dataset(current_path.clone(), current.clone());
    dataset.history = HistoryManager::with_limits(&current, HISTORY_MAX_ENTRIES, 0)
        .expect("el historial degradado debe inicializarse");
    let (source_path, source_format) = current_duckdb_file_source(&dataset)
        .expect("el archivo delimitado original debe ser una fuente DuckDB válida");
    let spec = prepare_duckdb_query(
        "SELECT id, amount FROM dataset LEFT JOIN compared ON dataset.id = compared.id LIMIT 2",
        &current,
        Some(&compared),
    )
    .expect("la consulta DuckDB debe validar el JOIN");

    let result = crate::duckdb_query::execute_duckdb_query_from_file_sources(
        &source_path,
        source_format,
        Some(&compared_path),
        &spec,
        || false,
    )
    .expect("DuckDB debe leer el archivo original y el snapshot comparado");

    assert_eq!(result.row_count, 3);
    assert_eq!(result.rows[0], vec![Some("1".to_owned()), None]);
    assert_eq!(
        result.rows[1],
        vec![Some("2".to_owned()), Some("200".to_owned())]
    );
    assert!(current_path.is_file());
    drop(compared_directory);
    fs::remove_file(current_path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn degraded_file_query_matches_the_materialized_query() {
    let current_path = temporary_csv("id,name\n1,A\n2,B\n3,C\n");
    let (current, _) = load_csv(&current_path).expect("el CSV debe cargar");
    let mut dataset = loaded_dataset(current_path.clone(), current.clone());
    dataset.history = HistoryManager::with_limits(&current, HISTORY_MAX_ENTRIES, 0)
        .expect("el historial degradado debe inicializarse");
    let (source_path, source_format) = current_duckdb_file_source(&dataset)
        .expect("el archivo original debe poder consultarse desde DuckDB");
    let query = "SELECT id, name FROM dataset LIMIT 2 OFFSET 1";
    let spec = prepare_duckdb_query(query, &current, None)
        .expect("la consulta compatible debe preparar su contrato DuckDB");
    let disk_result = crate::duckdb_query::execute_duckdb_query_from_file(
        &source_path,
        source_format,
        None,
        &spec,
        || false,
    )
    .expect("DuckDB debe consultar la fuente degradada");
    let materialized_result = execute_local_query(&current, query)
        .expect("la misma consulta debe ejecutarse sobre el frame");

    assert_eq!(disk_result.row_count, materialized_result.row_count);
    assert_eq!(disk_result.offset, materialized_result.offset);
    assert_eq!(disk_result.rows, materialized_result.rows);
    assert_eq!(
        disk_result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        materialized_result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(disk_result.truncated, materialized_result.truncated);
    assert!(!dataset.history.snapshots_enabled);
    fs::remove_file(current_path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn duckdb_join_preparation_does_not_apply_the_polars_input_row_limit() {
    let current = DataFrame::new(
        LOCAL_QUERY_JOIN_MAX_INPUT_ROWS + 1,
        vec![Column::full_null(
            "id".into(),
            LOCAL_QUERY_JOIN_MAX_INPUT_ROWS + 1,
            &DataType::Int64,
        )],
    )
    .expect("el esquema grande debe ser válido");
    let compared = df!["id" => &[1_i64]].unwrap();
    let query = "SELECT id FROM dataset JOIN compared ON dataset.id = compared.id LIMIT 1";

    assert!(parse_local_join_query_spec(query, &current, Some(&compared)).is_err());
    let spec = prepare_duckdb_query(query, &current, Some(&compared))
        .expect("DuckDB debe preparar el JOIN sin limitar la entrada por filas");
    assert_eq!(spec.limit, 1);
    assert!(spec.dataset_view_query.is_some());
}

#[test]
fn source_backed_full_join_uses_the_real_active_row_count_for_ordering() {
    let current = df!["id" => &[1_i64, 2]].unwrap();
    let compared = df!["id" => &[2_i64, 3], "segment" => &["B", "C"]].unwrap();
    let (current_directory, current_path) =
        persist_comparison_snapshot(&current).expect("el snapshot activo debe escribirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let current_schema = current.slice(0, 0);
    let compared_schema = compared.slice(0, 0);
    let spec = prepare_duckdb_query_with_row_count(
        "SELECT id, segment FROM dataset FULL JOIN compared ON dataset.id = compared.id LIMIT 10",
        &current_schema,
        Some(&compared_schema),
        current.height(),
    )
    .expect("la consulta DuckDB debe validar los esquemas source-backed");

    let result = crate::duckdb_query::execute_duckdb_query_from_parquet_sources(
        &current_path,
        Some(&compared_path),
        &spec,
        || false,
    )
    .expect("DuckDB debe ejecutar el FULL JOIN desde snapshots source-backed");

    assert_eq!(result.row_count, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), None],
            vec![Some("2".to_owned()), Some("B".to_owned())],
            vec![Some("3".to_owned()), Some("C".to_owned())],
        ]
    );
    assert!(spec
        .dataset_view_query
        .as_deref()
        .is_some_and(|query| query.contains("2 + r.")));
    drop(current_directory);
    drop(compared_directory);
}

#[test]
fn duckdb_join_reads_compared_snapshot_without_materializing_it() {
    let current = df!["id" => &[1_i64, 2, 3]].unwrap();
    let compared = df!["id" => &[2_i64, 3], "segment" => &["B", "C"]].unwrap();
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let spec = prepare_duckdb_query_with_row_count(
        "SELECT id, segment FROM dataset LEFT JOIN compared ON dataset.id = compared.id LIMIT 10",
        &current.slice(0, 0),
        Some(&compared.slice(0, 0)),
        current.height(),
    )
    .expect("la consulta DuckDB debe validar el esquema del JOIN");

    let result = crate::duckdb_query::execute_duckdb_query_from_frame_and_parquet(
        &current,
        &compared_path,
        &spec,
        || false,
    )
    .expect("DuckDB debe combinar el frame activo con el snapshot comparado");

    assert_eq!(result.row_count, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), None],
            vec![Some("2".to_owned()), Some("B".to_owned())],
            vec![Some("3".to_owned()), Some("C".to_owned())],
        ]
    );
    drop(compared_directory);
}

#[test]
fn source_backed_file_queries_support_all_join_types_without_a_snapshot() {
    let source = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let (compared_directory, compared_path) = persist_comparison_snapshot(
        &df![
            "id" => &["2", "3"],
            "segment" => &["B", "C"]
        ]
        .expect("la comparación debe construirse"),
    )
    .expect("el snapshot comparado debe escribirse");
    let (current_frame, _, row_count) =
        source_backed_load(&source, "csv", || false).expect("la fuente debe abrirse");
    assert_eq!(current_frame.height(), 0);

    for (join_keyword, expected_row_count, expected_rows) in [
        (
            "INNER JOIN",
            1,
            vec![vec![Some("2".to_owned()), Some("B".to_owned())]],
        ),
        (
            "LEFT JOIN",
            2,
            vec![
                vec![Some("1".to_owned()), None],
                vec![Some("2".to_owned()), Some("B".to_owned())],
            ],
        ),
        (
            "FULL JOIN",
            3,
            vec![
                vec![Some("1".to_owned()), None],
                vec![Some("2".to_owned()), Some("B".to_owned())],
                vec![Some("3".to_owned()), Some("C".to_owned())],
            ],
        ),
    ] {
        let query = format!(
            "SELECT id, segment FROM dataset {join_keyword} compared ON dataset.id = compared.id LIMIT 10"
        );
        let compared_schema = read_parquet_schema_frame(&compared_path)
            .expect("el esquema comparado debe poder leerse");
        let spec = prepare_duckdb_query_with_row_count(
            &query,
            &current_frame,
            Some(&compared_schema),
            row_count,
        )
        .expect("la consulta debe validarse");
        let (source_path, source_format) = current_duckdb_file_source(&LoadedDataset {
            source_path: Some(source.clone()),
            file_name: "source.csv".to_owned(),
            file_size_bytes: fs::metadata(&source).unwrap().len(),
            row_count,
            frame: current_frame.clone(),
            source_backed: true,
            profile: None,
            history: HistoryManager::deferred().unwrap(),
        })
        .expect("la fuente debe conservarse disponible");
        let result = crate::duckdb_query::execute_duckdb_query_from_file_sources(
            &source_path,
            source_format,
            Some(&compared_path),
            &spec,
            || false,
        )
        .expect("DuckDB debe ejecutar el JOIN desde la fuente original");
        assert_eq!(result.row_count, expected_row_count);
        assert_eq!(result.rows, expected_rows);
    }

    assert_eq!(current_frame.height(), 0);
    assert!(source.is_file());
    assert!(!source.with_file_name("source.parquet").exists());
    drop(compared_directory);
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn source_backed_join_materializes_only_the_result_for_all_join_types() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared_path = temporary_csv("id,segment\n2,B\n3,C\n");
    let (current_schema, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_format = crate::duckdb_query::DuckDbFileFormat::Delimited {
        delimiter: detect_delimiter(&current_path, "csv").expect("el delimitador debe detectarse"),
    };
    let output_directory = tempfile::tempdir().expect("se debe crear la salida temporal");

    for (join_type, expected_rows) in [
        (
            DatasetJoinType::Inner,
            vec![vec![
                Some("2".to_owned()),
                Some("Santiago".to_owned()),
                Some("B".to_owned()),
            ]],
        ),
        (
            DatasetJoinType::Left,
            vec![
                vec![Some("1".to_owned()), Some("Santo Domingo".to_owned()), None],
                vec![
                    Some("2".to_owned()),
                    Some("Santiago".to_owned()),
                    Some("B".to_owned()),
                ],
            ],
        ),
        (
            DatasetJoinType::Full,
            vec![
                vec![Some("1".to_owned()), Some("Santo Domingo".to_owned()), None],
                vec![
                    Some("2".to_owned()),
                    Some("Santiago".to_owned()),
                    Some("B".to_owned()),
                ],
                vec![Some("3".to_owned()), None, Some("C".to_owned())],
            ],
        ),
    ] {
        let (
            joined_schema,
            dataset_view_query,
            output_query,
            current_order_column,
            compared_order_column,
        ) = source_backed_join_plan(
            &current_schema,
            &compared_schema,
            &["id".to_owned()],
            join_type,
            current_row_count,
        )
        .expect("el plan source-backed debe validar el esquema");
        let output_path = output_directory
            .path()
            .join(format!("{}-join.parquet", join_type.label()));
        let row_count = crate::duckdb_query::materialize_file_sources_query_to_parquet(
            crate::duckdb_query::DuckDbFileSourcesQuery {
                current_path: &current_path,
                current_format,
                compared_path: &compared_path,
                compared_format,
                dataset_view_query: &dataset_view_query,
                query: &output_query,
                destination: &output_path,
                current_order_column: &current_order_column,
                compared_order_column: &compared_order_column,
                max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
            },
        )
        .expect("DuckDB debe publicar solo el resultado JOIN");
        let result = read_parquet_frame(&output_path).expect("el resultado Parquet debe leerse");
        let page = dataset_page(&result, 0, PREVIEW_ROW_LIMIT)
            .expect("la página del resultado debe poder leerse");
        assert_eq!(row_count, expected_rows.len());
        assert_eq!(result.height(), expected_rows.len());
        assert_eq!(page.rows, expected_rows);
        assert_eq!(
            joined_schema
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["id", "city", "segment"]
        );
        assert_eq!(current_schema.height(), 0);
    }

    assert_eq!(
        fs::read_to_string(&current_path).expect("la fuente activa debe permanecer intacta"),
        "id,city\n1,Santo Domingo\n2,Santiago\n"
    );
    assert_eq!(
        fs::read_to_string(&compared_path).expect("la fuente comparada debe permanecer intacta"),
        "id,segment\n2,B\n3,C\n"
    );
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn source_backed_join_rejects_an_oversized_result_before_writing_it() {
    let current_path = temporary_csv("id,city\n1,A\n1,B\n");
    let compared_path = temporary_csv("id,segment\n1,X\n1,Y\n");
    let (current_schema, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_format = crate::duckdb_query::DuckDbFileFormat::Delimited {
        delimiter: detect_delimiter(&current_path, "csv").expect("el delimitador debe detectarse"),
    };
    let (_, dataset_view_query, output_query, current_order_column, compared_order_column) =
        source_backed_join_plan(
            &current_schema,
            &compared_schema,
            &["id".to_owned()],
            DatasetJoinType::Inner,
            current_row_count,
        )
        .expect("el plan source-backed debe validar el esquema");
    let output_directory = tempfile::tempdir().expect("se debe crear la salida temporal");
    let output_path = output_directory.path().join("oversized.parquet");
    let error = crate::duckdb_query::materialize_file_sources_query_to_parquet(
        crate::duckdb_query::DuckDbFileSourcesQuery {
            current_path: &current_path,
            current_format,
            compared_path: &compared_path,
            compared_format,
            dataset_view_query: &dataset_view_query,
            query: &output_query,
            destination: &output_path,
            current_order_column: &current_order_column,
            compared_order_column: &compared_order_column,
            max_rows: Some(3),
        },
    )
    .expect_err("el resultado acotado debe rechazarse antes de escribir");

    assert!(error.contains("supera el límite local de 3"));
    assert!(!output_path.exists());
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn source_backed_join_publishes_a_reversible_parquet_cursor() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared_path = temporary_csv("id,segment\n2,B\n3,C\n");
    let (current_schema, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_size_bytes = fs::metadata(&current_path)
        .expect("la fuente activa debe conservar sus metadatos")
        .len();
    let mut dataset = LoadedDataset {
        source_path: Some(current_path.clone()),
        file_name: "current.csv".to_owned(),
        file_size_bytes: current_size_bytes,
        row_count: current_row_count,
        frame: current_schema.clone(),
        source_backed: true,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    };
    let context = source_backed_join_context(&dataset)
        .expect("el contexto source-backed debe conservarse sin materializar filas");
    let (
        _joined_schema,
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ) = source_backed_join_plan(
        &current_schema,
        &compared_schema,
        &["id".to_owned()],
        DatasetJoinType::Full,
        current_row_count,
    )
    .expect("el plan del JOIN debe validarse");
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .expect("la salida temporal debe prepararse");
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let output_row_count = crate::duckdb_query::materialize_file_sources_query_to_parquet(
        crate::duckdb_query::DuckDbFileSourcesQuery {
            current_path: &context.source_path,
            current_format: context.source_format,
            compared_path: &compared_path,
            compared_format,
            dataset_view_query: &dataset_view_query,
            query: &output_query,
            destination: &output_path,
            current_order_column: &current_order_column,
            compared_order_column: &compared_order_column,
            max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
        },
    )
    .expect("el JOIN debe escribir el resultado Parquet");
    let preview = publish_source_backed_result_output(
        &mut dataset,
        &context,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes: fs::metadata(&compared_path)
                .expect("la fuente comparada debe conservar sus metadatos")
                .len(),
            output_path: &output_path,
            output_row_count,
            file_name: "Join full · current.csv + compared.csv",
            label: "Unir datasets (full)",
        },
    )
    .expect("el cursor source-backed debe publicarse")
    .expect("el historial debe permitir publicar el cursor");

    assert_eq!(preview.row_count, 3);
    assert_eq!(dataset.row_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    assert!(dataset.source_backed);
    assert!(dataset.history.snapshots_enabled);
    assert_eq!(dataset.history.entries.len(), 2);
    assert!(dataset.history.state().can_undo);
    let restored = dataset
        .history
        .restore(0)
        .expect("el dataset original debe poder restaurarse");
    assert_eq!(restored.height(), 2);
    assert!(!output_path.exists());
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn snapshot_backed_join_uses_the_current_history_cursor_without_materializing_active_rows() {
    let compared_path = temporary_csv("id,segment\n2,B\n3,C\n");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_frame = df![
        "id" => &["1", "2"],
        "city" => &["Santo Domingo", "Santiago"]
    ]
    .expect("el dataset activo debe construirse");
    let current_size_bytes = 128;
    let history =
        HistoryManager::new(&current_frame).expect("el historial durable debe inicializarse");
    let mut dataset = LoadedDataset {
        source_path: None,
        file_name: "current.csv".to_owned(),
        file_size_bytes: current_size_bytes,
        row_count: current_frame.height(),
        frame: current_frame.clone(),
        source_backed: false,
        profile: None,
        history,
    };
    let context = snapshot_backed_join_context(&dataset)
        .expect("el cursor Parquet actual debe poder usarse como fuente");
    assert!(context.snapshot_only);
    assert!(matches!(
        context.source_format,
        crate::duckdb_query::DuckDbFileFormat::Parquet
    ));
    assert_eq!(context.schema.height(), 0);
    assert_eq!(context.schema.width(), current_frame.width());
    assert_eq!(context.row_count, current_frame.height());
    assert_eq!(dataset.frame.height(), 2);

    let (
        joined_schema,
        dataset_view_query,
        output_query,
        current_order_column,
        compared_order_column,
    ) = source_backed_join_plan(
        &context.schema,
        &compared_schema,
        &["id".to_owned()],
        DatasetJoinType::Full,
        context.row_count,
    )
    .expect("el plan del JOIN debe validarse");
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .expect("la salida temporal debe prepararse");
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let output_row_count = crate::duckdb_query::materialize_file_sources_query_to_parquet(
        crate::duckdb_query::DuckDbFileSourcesQuery {
            current_path: &context.source_path,
            current_format: context.source_format,
            compared_path: &compared_path,
            compared_format,
            dataset_view_query: &dataset_view_query,
            query: &output_query,
            destination: &output_path,
            current_order_column: &current_order_column,
            compared_order_column: &compared_order_column,
            max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
        },
    )
    .expect("el JOIN debe escribir el resultado Parquet");
    assert_eq!(
        joined_schema
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["id", "city", "segment"]
    );
    let preview = publish_source_backed_result_output(
        &mut dataset,
        &context,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes: fs::metadata(&compared_path)
                .expect("la fuente comparada debe conservar sus metadatos")
                .len(),
            output_path: &output_path,
            output_row_count,
            file_name: "Join full · current.csv + compared.csv",
            label: "Unir datasets (full)",
        },
    )
    .expect("el cursor del snapshot debe publicarse")
    .expect("el historial debe permitir publicar el cursor");

    assert_eq!(preview.row_count, 3);
    assert_eq!(dataset.row_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    assert!(dataset.source_backed);
    assert!(dataset.source_path.is_some());
    assert!(dataset.history.snapshots_enabled);
    assert_eq!(dataset.history.entries.len(), 2);
    assert!(dataset.history.state().can_undo);
    let audit = enable_row_audit_source_backed(&mut dataset)
        .expect("la trazabilidad debe poder publicarse sobre el cursor JOIN")
        .expect("la trazabilidad source-backed debe devolver una mutación");
    assert_eq!(audit.dataset.row_count, 3);
    materialize_loaded_dataset(&mut dataset).expect("el cursor JOIN debe materializar sus filas");
    assert_eq!(dataset.frame.height(), 3);
    assert!(dataset
        .frame
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "_cambios"));
    assert_eq!(
        dataset
            .history
            .restore(0)
            .expect("el dataset original debe poder restaurarse")
            .height(),
        2
    );
    assert!(!output_path.exists());
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn source_backed_consolidation_materializes_only_new_keys() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared_path = temporary_csv("id,city\n2,Santiago\n3,La Vega\n");
    let (current_schema, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_format = crate::duckdb_query::DuckDbFileFormat::Delimited {
        delimiter: detect_delimiter(&current_path, "csv").expect("el delimitador debe detectarse"),
    };
    let (dataset_view_query, output_query, current_order_column, compared_order_column) =
        source_backed_consolidation_plan(
            &current_schema,
            &compared_schema,
            &["id".to_owned()],
            current_row_count,
        )
        .expect("el plan de consolidación debe validar el esquema");
    validate_source_backed_consolidation(
        &current_path,
        current_format,
        &compared_path,
        compared_format,
        &current_schema,
        &["id".to_owned()],
    )
    .expect("las claves nuevas deben ser compatibles");
    let output_directory = tempfile::tempdir().expect("se debe crear la salida temporal");
    let output_path = output_directory.path().join("consolidated.parquet");
    let row_count = crate::duckdb_query::materialize_file_sources_query_to_parquet(
        crate::duckdb_query::DuckDbFileSourcesQuery {
            current_path: &current_path,
            current_format,
            compared_path: &compared_path,
            compared_format,
            dataset_view_query: &dataset_view_query,
            query: &output_query,
            destination: &output_path,
            current_order_column: &current_order_column,
            compared_order_column: &compared_order_column,
            max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
        },
    )
    .expect("DuckDB debe escribir solo el resultado consolidado");
    let result = read_parquet_frame(&output_path).expect("el resultado Parquet debe leerse");
    assert_eq!(row_count, 3);
    assert_eq!(result.height(), 3);
    assert_eq!(
        dataset_page(&result, 0, PREVIEW_ROW_LIMIT)
            .expect("la página consolidada debe poder leerse")
            .rows,
        vec![
            vec![Some("1".to_owned()), Some("Santo Domingo".to_owned())],
            vec![Some("2".to_owned()), Some("Santiago".to_owned())],
            vec![Some("3".to_owned()), Some("La Vega".to_owned())],
        ]
    );
    assert_eq!(current_schema.height(), 0);
    assert_eq!(
        fs::read_to_string(&current_path).expect("la fuente activa debe permanecer intacta"),
        "id,city\n1,Santo Domingo\n2,Santiago\n"
    );
    assert_eq!(
        fs::read_to_string(&compared_path).expect("la fuente comparada debe permanecer intacta"),
        "id,city\n2,Santiago\n3,La Vega\n"
    );
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn source_backed_consolidation_rejects_duplicate_and_conflicting_keys() {
    let duplicate_current_path = temporary_csv("id,city\n1,A\n1,A\n");
    let duplicate_compared_path = temporary_csv("id,city\n2,B\n");
    let (duplicate_current_schema, _, _) =
        source_backed_load(&duplicate_current_path, "csv", || false)
            .expect("la fuente duplicada debe abrirse source-backed");
    let (_, _duplicate_compared_schema) =
        source_backed_join_source(&duplicate_compared_path, "csv")
            .expect("el esquema comparado debe leerse")
            .expect("el CSV comparado debe ser source-backed");
    let current_format = crate::duckdb_query::DuckDbFileFormat::Delimited {
        delimiter: detect_delimiter(&duplicate_current_path, "csv")
            .expect("el delimitador debe detectarse"),
    };
    let compared_format = crate::duckdb_query::DuckDbFileFormat::Delimited {
        delimiter: detect_delimiter(&duplicate_compared_path, "csv")
            .expect("el delimitador debe detectarse"),
    };
    let error = validate_source_backed_consolidation(
        &duplicate_current_path,
        current_format,
        &duplicate_compared_path,
        compared_format,
        &duplicate_current_schema,
        &["id".to_owned()],
    )
    .expect_err("las claves duplicadas deben rechazarse");
    assert!(error.contains("conflictos o duplicados"));

    let conflict_current_path = temporary_csv("id,city\n1,A\n");
    let conflict_compared_path = temporary_csv("id,city\n1,B\n");
    let (conflict_current_schema, _, _) =
        source_backed_load(&conflict_current_path, "csv", || false)
            .expect("la fuente en conflicto debe abrirse source-backed");
    let (_, _conflict_compared_schema) = source_backed_join_source(&conflict_compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let conflict_error = validate_source_backed_consolidation(
        &conflict_current_path,
        current_format,
        &conflict_compared_path,
        compared_format,
        &conflict_current_schema,
        &["id".to_owned()],
    )
    .expect_err("las claves con payload distinto deben rechazarse");
    assert!(conflict_error.contains("conflictos o duplicados"));

    fs::remove_file(duplicate_current_path).expect("se debe limpiar la fuente duplicada");
    fs::remove_file(duplicate_compared_path).expect("se debe limpiar la comparación duplicada");
    fs::remove_file(conflict_current_path).expect("se debe limpiar la fuente en conflicto");
    fs::remove_file(conflict_compared_path).expect("se debe limpiar la comparación en conflicto");
}

#[test]
fn source_backed_consolidation_publishes_a_reversible_parquet_cursor() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared_path = temporary_csv("id,city\n2,Santiago\n3,La Vega\n");
    let (current_schema, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let (compared_format, compared_schema) = source_backed_join_source(&compared_path, "csv")
        .expect("el esquema comparado debe leerse")
        .expect("el CSV comparado debe ser source-backed");
    let current_size_bytes = fs::metadata(&current_path)
        .expect("la fuente activa debe conservar sus metadatos")
        .len();
    let mut dataset = LoadedDataset {
        source_path: Some(current_path.clone()),
        file_name: "current.csv".to_owned(),
        file_size_bytes: current_size_bytes,
        row_count: current_row_count,
        frame: current_schema.clone(),
        source_backed: true,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    };
    let context = source_backed_join_context(&dataset)
        .expect("el contexto source-backed debe conservarse sin materializar filas");
    let (dataset_view_query, output_query, current_order_column, compared_order_column) =
        source_backed_consolidation_plan(
            &current_schema,
            &compared_schema,
            &["id".to_owned()],
            current_row_count,
        )
        .expect("el plan de consolidación debe validarse");
    validate_source_backed_consolidation(
        &context.source_path,
        context.source_format,
        &compared_path,
        compared_format,
        &current_schema,
        &["id".to_owned()],
    )
    .expect("la consolidación debe validar las claves");
    let temporary = tempfile::NamedTempFile::with_suffix_in(".parquet", &context.history_directory)
        .expect("la salida temporal debe prepararse");
    let output_path = temporary.path().to_owned();
    drop(temporary);
    let output_row_count = crate::duckdb_query::materialize_file_sources_query_to_parquet(
        crate::duckdb_query::DuckDbFileSourcesQuery {
            current_path: &context.source_path,
            current_format: context.source_format,
            compared_path: &compared_path,
            compared_format,
            dataset_view_query: &dataset_view_query,
            query: &output_query,
            destination: &output_path,
            current_order_column: &current_order_column,
            compared_order_column: &compared_order_column,
            max_rows: Some(LOCAL_QUERY_JOIN_MAX_RESULT_ROWS),
        },
    )
    .expect("la consolidación debe escribir el resultado Parquet");
    let preview = publish_source_backed_result_output(
        &mut dataset,
        &context,
        SourceBackedResultOutput {
            compared_path: &compared_path,
            compared_size_bytes: fs::metadata(&compared_path)
                .expect("la comparación debe conservar sus metadatos")
                .len(),
            output_path: &output_path,
            output_row_count,
            file_name: "Consolidado · current.csv + compared.csv",
            label: "Consolidar datasets",
        },
    )
    .expect("el cursor de consolidación debe publicarse")
    .expect("el historial debe permitir publicar el cursor");

    assert_eq!(preview.row_count, 3);
    assert_eq!(
        dataset.file_name,
        "Consolidado · current.csv + compared.csv"
    );
    assert_eq!(dataset.row_count, 3);
    assert_eq!(dataset.frame.height(), 0);
    assert!(dataset.source_backed);
    assert!(dataset.history.snapshots_enabled);
    assert_eq!(dataset.history.entries.len(), 2);
    assert!(dataset.history.state().can_undo);
    let restored = dataset
        .history
        .restore(0)
        .expect("el dataset original debe poder restaurarse");
    assert_eq!(restored.height(), 2);
    assert!(!output_path.exists());
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
    fs::remove_file(compared_path).expect("se debe limpiar la fuente comparada");
}

#[test]
fn disk_backed_conflict_page_keeps_source_active_frame_deferred() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared = df![
        "id" => &["1", "2"],
        "city" => &["La Romana", "Santiago"]
    ]
    .expect("la comparación debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let (current_frame, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let current_size_bytes = fs::metadata(&current_path)
        .expect("la fuente activa debe conservar sus metadatos")
        .len();
    let state = DatasetState::default();
    *state
        .current
        .lock()
        .expect("el estado activo debe estar disponible") = Some(LoadedDataset {
        source_path: Some(current_path.clone()),
        file_name: "current.csv".to_owned(),
        file_size_bytes: current_size_bytes,
        row_count: current_row_count,
        frame: current_frame,
        source_backed: true,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    });

    let page = disk_backed_conflict_page(
        &state,
        &compared_path,
        compared.height(),
        &["id".to_owned()],
        0,
        MAX_CONFLICT_PREVIEW,
    )
    .expect("la página source-backed debe poder leerse")
    .expect("el dataset source-backed debe usar la ruta diferida");

    assert_eq!(page.offset, 0);
    assert!(!page.has_next);
    assert_eq!(page.conflicts.len(), 1);
    assert_eq!(page.conflicts[0].key, vec![Some("1".to_owned())]);
    assert_eq!(page.conflicts[0].cells.len(), 1);
    assert_eq!(page.conflicts[0].cells[0].column, "city");
    assert_eq!(
        page.conflicts[0].cells[0].current,
        Some("Santo Domingo".to_owned())
    );
    assert_eq!(
        page.conflicts[0].cells[0].compared,
        Some("La Romana".to_owned())
    );
    let active = state
        .current
        .lock()
        .expect("el estado activo debe seguir disponible");
    let dataset = active.as_ref().expect("el dataset activo debe conservarse");
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert!(current_path.is_file());
    drop(active);
    drop(compared_directory);
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
}

#[test]
fn disk_backed_conflict_page_uses_the_current_history_snapshot() {
    let compared = df![
        "id" => &["1", "2"],
        "city" => &["La Romana", "Santiago"]
    ]
    .expect("la comparación debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let current_frame = df![
        "id" => &["1", "2"],
        "city" => &["Santo Domingo", "Santiago"]
    ]
    .expect("el dataset activo debe construirse");
    let history =
        HistoryManager::new(&current_frame).expect("el historial durable debe inicializarse");
    let state = DatasetState::default();
    *state
        .current
        .lock()
        .expect("el estado activo debe estar disponible") = Some(LoadedDataset {
        source_path: None,
        file_name: "current.csv".to_owned(),
        file_size_bytes: 128,
        row_count: current_frame.height(),
        frame: current_frame,
        source_backed: false,
        profile: None,
        history,
    });

    let context = {
        let active = state
            .current
            .lock()
            .expect("el estado activo debe seguir disponible");
        snapshot_backed_join_context(active.as_ref().expect("el dataset debe existir"))
            .expect("el snapshot activo debe poder usarse como fuente")
    };
    assert!(context.snapshot_only);

    let page = disk_backed_conflict_page(
        &state,
        &compared_path,
        compared.height(),
        &["id".to_owned()],
        0,
        MAX_CONFLICT_PREVIEW,
    )
    .expect("la página snapshot-backed debe poder leerse")
    .expect("el dataset con historial debe usar la ruta diferida");

    assert_eq!(page.offset, 0);
    assert!(!page.has_next);
    assert_eq!(page.conflicts.len(), 1);
    assert_eq!(page.conflicts[0].key, vec![Some("1".to_owned())]);
    assert_eq!(page.conflicts[0].cells.len(), 1);
    assert_eq!(page.conflicts[0].cells[0].column, "city");
    assert_eq!(
        page.conflicts[0].cells[0].current,
        Some("Santo Domingo".to_owned())
    );
    assert_eq!(
        page.conflicts[0].cells[0].compared,
        Some("La Romana".to_owned())
    );
    let active = state
        .current
        .lock()
        .expect("el estado activo debe seguir disponible");
    let dataset = active.as_ref().expect("el dataset activo debe conservarse");
    assert!(!dataset.source_backed);
    assert_eq!(dataset.frame.height(), 2);
    drop(active);
    drop(compared_directory);
}

#[test]
fn snapshot_backed_conflict_resolution_publishes_a_reversible_cursor() {
    let compared = df![
        "id" => &["1", "2"],
        "city" => &["La Vega", "Santiago"],
        "total" => &["15", "25"]
    ]
    .expect("la comparación debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let current_frame = df![
        "id" => &["1", "2"],
        "city" => &["Santo Domingo", "Santiago"],
        "total" => &["10", "20"]
    ]
    .expect("el dataset activo debe construirse");
    let history =
        HistoryManager::new(&current_frame).expect("el historial durable debe inicializarse");
    let state = DatasetState::default();
    *state
        .current
        .lock()
        .expect("el estado activo debe estar disponible") = Some(LoadedDataset {
        source_path: None,
        file_name: "current.csv".to_owned(),
        file_size_bytes: 128,
        row_count: current_frame.height(),
        frame: current_frame,
        source_backed: false,
        profile: None,
        history,
    });
    let context = {
        let current = state
            .current
            .lock()
            .expect("el estado activo debe estar disponible");
        current_join_context(current.as_ref().expect("el dataset debe existir"))
            .expect("el contexto snapshot-backed debe conservarse")
    };
    assert!(context.snapshot_only);
    let compared_size_bytes = fs::metadata(&compared_path)
        .expect("el snapshot comparado debe conservar sus metadatos")
        .len();
    let compared_schema =
        read_parquet_schema_frame(&compared_path).expect("el esquema comparado debe poder leerse");
    let preview = resolve_source_backed_conflicts(
        &state,
        SourceBackedConflictResolutionRequest {
            context,
            compared_path: compared_path.clone(),
            compared_size_bytes,
            compared_row_count: compared.height(),
            compared_schema,
            compared_file_name: "compared.parquet".to_owned(),
            key_columns: vec!["id".to_owned()],
            decisions: vec![
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("city".to_owned()),
                    source: ConflictSource::Compared,
                },
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("total".to_owned()),
                    source: ConflictSource::Current,
                },
                ConflictResolution {
                    conflict_index: 1,
                    column: None,
                    source: ConflictSource::Compared,
                },
            ],
        },
    )
    .expect("la resolución snapshot-backed debe poder ejecutarse")
    .expect("la resolución debe publicarse con historial");

    assert_eq!(preview.row_count, 2);
    let active = state
        .current
        .lock()
        .expect("el estado activo debe seguir disponible");
    let dataset = active.as_ref().expect("el dataset activo debe conservarse");
    assert!(dataset.source_backed);
    assert!(dataset.source_path.is_some());
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.history.entries.len(), 2);
    let resolved = dataset
        .history
        .restore(1)
        .expect("el resultado resuelto debe poder restaurarse");
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(0),
        Some("La Vega")
    );
    assert_eq!(
        resolved.column("total").unwrap().str().unwrap().get(0),
        Some("10")
    );
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(1),
        Some("Santiago")
    );
    assert_eq!(
        resolved.column("total").unwrap().str().unwrap().get(1),
        Some("25")
    );
    drop(active);
    drop(compared_directory);
    assert!(!compared_path.exists());
}

#[test]
fn snapshot_backed_consolidation_publishes_only_new_keys_reversibly() {
    let compared = df![
        "id" => &["2", "3"],
        "city" => &["Santiago", "La Vega"]
    ]
    .expect("la comparación debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let current_frame = df![
        "id" => &["1", "2"],
        "city" => &["Santo Domingo", "Santiago"]
    ]
    .expect("el dataset activo debe construirse");
    let history =
        HistoryManager::new(&current_frame).expect("el historial durable debe inicializarse");
    let state = DatasetState::default();
    *state
        .current
        .lock()
        .expect("el estado activo debe estar disponible") = Some(LoadedDataset {
        source_path: None,
        file_name: "current.csv".to_owned(),
        file_size_bytes: 128,
        row_count: current_frame.height(),
        frame: current_frame,
        source_backed: false,
        profile: None,
        history,
    });
    let context = {
        let current = state
            .current
            .lock()
            .expect("el estado activo debe estar disponible");
        current_join_context(current.as_ref().expect("el dataset debe existir"))
            .expect("el contexto snapshot-backed debe conservarse")
    };
    assert!(context.snapshot_only);
    let compared_size_bytes = fs::metadata(&compared_path)
        .expect("el snapshot comparado debe conservar sus metadatos")
        .len();
    let compared_schema =
        read_parquet_schema_frame(&compared_path).expect("el esquema comparado debe poder leerse");
    let preview = consolidate_source_backed_dataset(
        &state,
        SourceBackedConsolidationRequest {
            context,
            compared_path: compared_path.clone(),
            compared_format: crate::duckdb_query::DuckDbFileFormat::Parquet,
            compared_schema,
            compared_file_name: "compared.parquet".to_owned(),
            compared_size_bytes,
            key_columns: vec!["id".to_owned()],
        },
    )
    .expect("la consolidación snapshot-backed debe poder ejecutarse")
    .expect("la consolidación debe publicarse con historial");

    assert_eq!(preview.row_count, 3);
    let active = state
        .current
        .lock()
        .expect("el estado activo debe seguir disponible");
    let dataset = active.as_ref().expect("el dataset activo debe conservarse");
    assert!(dataset.source_backed);
    assert!(dataset.source_path.is_some());
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.history.entries.len(), 2);
    let consolidated = dataset
        .history
        .restore(1)
        .expect("el resultado consolidado debe poder restaurarse");
    assert_eq!(consolidated.height(), 3);
    assert_eq!(
        dataset_page(&consolidated, 0, PREVIEW_ROW_LIMIT)
            .expect("la página consolidada debe poder leerse")
            .rows,
        vec![
            vec![Some("1".to_owned()), Some("Santo Domingo".to_owned())],
            vec![Some("2".to_owned()), Some("Santiago".to_owned())],
            vec![Some("3".to_owned()), Some("La Vega".to_owned())],
        ]
    );
    drop(active);
    drop(compared_directory);
    assert!(!compared_path.exists());
}

#[test]
fn source_backed_conflict_resolution_publishes_selected_values_reversibly() {
    let current_path = temporary_csv("id,city,total\n1,Santo Domingo,10\n2,Santiago,20\n");
    let compared = df![
        "id" => &["1", "2"],
        "city" => &["La Vega", "Santiago"],
        "total" => &["15", "25"]
    ]
    .expect("la comparación debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let (current_frame, _, current_row_count) = source_backed_load(&current_path, "csv", || false)
        .expect("la fuente activa debe abrirse source-backed");
    let current_size_bytes = fs::metadata(&current_path)
        .expect("la fuente activa debe conservar sus metadatos")
        .len();
    let state = DatasetState::default();
    *state
        .current
        .lock()
        .expect("el estado activo debe estar disponible") = Some(LoadedDataset {
        source_path: Some(current_path.clone()),
        file_name: "current.csv".to_owned(),
        file_size_bytes: current_size_bytes,
        row_count: current_row_count,
        frame: current_frame.clone(),
        source_backed: true,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    });
    let compared_size_bytes = fs::metadata(&compared_path)
        .expect("el snapshot comparado debe conservar sus metadatos")
        .len();
    *state
        .comparison
        .lock()
        .expect("la comparación debe estar disponible") = Some(PendingComparison {
        file_name: "compared.parquet".to_owned(),
        file_size_bytes: compared_size_bytes,
        row_count: compared.height(),
        _directory: compared_directory,
        snapshot_path: compared_path.clone(),
        key_columns: vec!["id".to_owned()],
    });
    let context = {
        let current = state
            .current
            .lock()
            .expect("el estado activo debe estar disponible");
        source_backed_join_context(current.as_ref().expect("el dataset debe existir"))
            .expect("el contexto source-backed debe conservarse")
    };
    let compared_schema =
        read_parquet_schema_frame(&compared_path).expect("el esquema comparado debe poder leerse");
    let preview = resolve_source_backed_conflicts(
        &state,
        SourceBackedConflictResolutionRequest {
            context,
            compared_path: compared_path.clone(),
            compared_size_bytes,
            compared_row_count: compared.height(),
            compared_schema,
            compared_file_name: "compared.parquet".to_owned(),
            key_columns: vec!["id".to_owned()],
            decisions: vec![
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("city".to_owned()),
                    source: ConflictSource::Compared,
                },
                ConflictResolution {
                    conflict_index: 0,
                    column: Some("total".to_owned()),
                    source: ConflictSource::Current,
                },
                ConflictResolution {
                    conflict_index: 1,
                    column: None,
                    source: ConflictSource::Compared,
                },
            ],
        },
    )
    .expect("la resolución source-backed debe poder ejecutarse")
    .expect("la resolución debe publicarse con historial");

    assert_eq!(preview.row_count, 2);
    let active = state
        .current
        .lock()
        .expect("el estado activo debe seguir disponible");
    let dataset = active.as_ref().expect("el dataset activo debe conservarse");
    assert_eq!(
        dataset.file_name,
        "Resuelto · current.csv + compared.parquet"
    );
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.history.entries.len(), 2);
    let resolved = dataset
        .history
        .restore(1)
        .expect("el resultado resuelto debe poder restaurarse");
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(0),
        Some("La Vega")
    );
    assert_eq!(
        resolved.column("total").unwrap().str().unwrap().get(0),
        Some("10")
    );
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(1),
        Some("Santiago")
    );
    assert_eq!(
        resolved.column("total").unwrap().str().unwrap().get(1),
        Some("25")
    );
    assert!(state
        .comparison
        .lock()
        .expect("la comparación debe seguir accesible")
        .is_none());
    drop(active);
    assert!(current_path.is_file());
    assert!(!compared_path.exists());
    fs::remove_file(current_path).expect("se debe limpiar la fuente activa");
}

#[test]
fn source_backed_conflict_resolution_defers_oversized_decisions_to_the_eager_path() {
    let decisions = (0..=SOURCE_BACKED_RESOLUTION_MAX_CONFLICTS)
        .map(|conflict_index| ConflictResolution {
            conflict_index,
            column: None,
            source: ConflictSource::Current,
        })
        .collect::<Vec<_>>();
    let result = validate_source_backed_conflict_decisions(
        Path::new("missing-current.parquet"),
        0,
        Path::new("missing-compared.parquet"),
        0,
        &[],
        &[],
        &decisions,
    )
    .expect("las decisiones fuera de presupuesto deben conservar fallback");
    assert!(result.is_none());
}

fn write_duckdb_join_benchmark_csv(path: &Path, target_bytes: u64) -> (usize, u64) {
    let file = File::create(path).expect("la fuente del benchmark debe poder crearse");
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    let payload = "x".repeat(256);
    writer
        .write_all(b"id,name,amount,payload\n")
        .expect("el encabezado del benchmark debe escribirse");
    let mut row_count = 0;
    let mut written_bytes = 0;
    while written_bytes < target_bytes {
        for _ in 0..8192 {
            row_count += 1;
            writeln!(
                writer,
                "id-{row_count},Synthetic name {row_count},123.45,{payload}"
            )
            .expect("las filas del benchmark deben escribirse");
        }
        writer
            .flush()
            .expect("el benchmark debe vaciar sus bloques");
        written_bytes = writer
            .get_ref()
            .metadata()
            .expect("la fuente del benchmark debe conservar sus metadatos")
            .len();
    }
    writer
        .flush()
        .expect("la fuente del benchmark debe sincronizarse");
    let written_bytes = fs::metadata(path)
        .expect("la fuente del benchmark debe existir")
        .len();
    (row_count, written_bytes)
}

#[test]
#[ignore = "benchmark opt-in de escala; ejecutado por perf:duckdb:join"]
fn duckdb_source_backed_join_handles_large_file() {
    let target_mib = std::env::var("COLUMNIA_DUCKDB_JOIN_TARGET_MIB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(512);
    let target_bytes = target_mib
        .checked_mul(1024 * 1024)
        .expect("el objetivo del benchmark debe caber en bytes");
    let directory = tempfile::tempdir().expect("se debe crear el directorio del benchmark");
    let current_path = directory.path().join("current.csv");
    let started = Instant::now();
    let (row_count, file_size_bytes) = write_duckdb_join_benchmark_csv(&current_path, target_bytes);
    assert!(file_size_bytes >= target_bytes);
    let (current_frame, _preview, loaded_row_count) =
        source_backed_load(&current_path, "csv", || false)
            .expect("la fuente grande debe abrirse source-backed");
    assert_eq!(current_frame.height(), 0);
    assert_eq!(loaded_row_count, row_count);
    let compared = df![
        "id" => &["id-1", "id-2", "id-3", "id-unmatched"],
        "segment" => &["A", "B", "C", "Z"]
    ]
    .expect("la comparación del benchmark debe construirse");
    let (compared_directory, compared_path) =
        persist_comparison_snapshot(&compared).expect("el snapshot comparado debe escribirse");
    let dataset = LoadedDataset {
        source_path: Some(current_path.clone()),
        file_name: "current.csv".to_owned(),
        file_size_bytes,
        row_count: loaded_row_count,
        frame: current_frame,
        source_backed: true,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    };
    let (source_path, source_format) = current_duckdb_file_source(&dataset)
        .expect("la fuente grande debe poder registrarse en DuckDB");
    let compared_schema = read_parquet_schema_frame(&compared_path)
        .expect("el esquema comparado debe leerse sin materializar sus filas");
    let mut join_results = Vec::new();
    for (join_keyword, expected_row_count) in [
        ("INNER JOIN", 3_usize),
        ("LEFT JOIN", row_count),
        ("FULL JOIN", row_count + 1),
    ] {
        let query = format!(
            "SELECT id, segment FROM dataset {join_keyword} compared ON dataset.id = compared.id LIMIT 3"
        );
        let spec = prepare_duckdb_query_with_row_count(
            &query,
            &dataset.frame,
            Some(&compared_schema),
            dataset.row_count,
        )
        .expect("el JOIN del benchmark debe validar sus esquemas");
        let result = crate::duckdb_query::execute_duckdb_query_from_file_sources(
            &source_path,
            source_format,
            Some(&compared_path),
            &spec,
            || false,
        )
        .expect("DuckDB debe consultar la fuente grande desde disco");

        assert_eq!(dataset.frame.height(), 0);
        assert_eq!(result.row_count, expected_row_count);
        assert_eq!(result.rows.len(), 3);
        assert_eq!(
            result.rows[0],
            vec![Some("id-1".to_owned()), Some("A".to_owned())]
        );
        assert_eq!(
            result.rows[2],
            vec![Some("id-3".to_owned()), Some("C".to_owned())]
        );
        join_results.push(serde_json::json!({
            "joinType": join_keyword,
            "resultRowCount": result.row_count,
            "pageRows": result.rows.len()
        }));
    }
    let source_backed_frame_rows = dataset.frame.height();
    let elapsed_ms = started.elapsed().as_millis();
    let current_root = directory.path().to_path_buf();
    drop(dataset);
    drop(compared_directory);
    drop(directory);
    let cleanup_verified = !current_root.exists();
    assert!(cleanup_verified);
    println!(
        "DUCKDB_JOIN_BENCHMARK:{}",
        serde_json::json!({
            "targetMiB": target_mib,
            "fileSizeBytes": file_size_bytes,
            "rowCount": row_count,
            "resultRowCount": row_count,
            "pageRows": 3,
            "joinResults": join_results,
            "sourceBackedFrameRows": source_backed_frame_rows,
            "elapsedMs": elapsed_ms,
            "cleanupVerified": cleanup_verified
        })
    );
}

#[test]
fn joins_route_to_duckdb_when_compared_snapshot_exists() {
    let query = "SELECT id FROM dataset JOIN compared ON dataset.id = compared.id LIMIT 1";

    assert!(should_route_join_to_duckdb(query, true));
    assert!(!should_route_join_to_duckdb(query, false));
    assert!(!should_route_join_to_duckdb(
        "SELECT id FROM dataset LIMIT 1",
        true,
    ));
}

#[test]
fn duckdb_query_preserves_active_row_order_without_leaking_internal_columns() {
    let current = df![
        "id" => &[3_i64, 1, 2],
        "city" => &["La Vega", "Santo Domingo", "Santiago"]
    ]
    .unwrap();
    let spec = prepare_duckdb_query("SELECT * FROM dataset LIMIT 2", &current, None)
        .expect("la consulta DuckDB debe validar el dataset activo");

    let result = crate::duckdb_query::execute_duckdb_query(&current, None, &spec, || false)
        .expect("DuckDB debe ejecutar la consulta paginada");

    assert_eq!(
        result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "city"]
    );
    assert_eq!(
        result.rows,
        vec![
            vec![Some("3".to_owned()), Some("La Vega".to_owned())],
            vec![Some("1".to_owned()), Some("Santo Domingo".to_owned())],
        ]
    );
}

#[test]
fn duckdb_query_keeps_full_join_keys_and_group_order() {
    let current = df![
        "id" => &[1_i64, 2],
        "region" => &["north", "south"]
    ]
    .unwrap();
    let compared = df!["id" => &[2_i64, 3], "segment" => &["B", "C"]].unwrap();
    let spec = prepare_duckdb_query(
        "SELECT id, COUNT(*) AS total FROM dataset FULL JOIN compared ON dataset.id = compared.id GROUP BY id LIMIT 10",
        &current,
        Some(&compared),
    )
    .expect("la consulta agregada DuckDB debe validar el JOIN local");

    let result =
        crate::duckdb_query::execute_duckdb_query(&current, Some(&compared), &spec, || false)
            .expect("DuckDB debe ejecutar la agregación del JOIN");

    assert_eq!(result.row_count, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), Some("1".to_owned())],
            vec![Some("2".to_owned()), Some("1".to_owned())],
            vec![Some("3".to_owned()), Some("1".to_owned())],
        ]
    );
    assert!(!result.truncated);
}

#[test]
fn local_query_joins_only_the_loaded_comparison_and_pages_results() {
    let current = df![
        "id" => &[1_i64, 2, 3],
        "city" => &["Santo Domingo", "Santiago", "La Vega"]
    ]
    .unwrap();
    let compared = df![
        "id" => &[2_i64, 3],
        "segment" => &["B", "C"]
    ]
    .unwrap();

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id, segment FROM dataset LEFT JOIN compared ON dataset.id = compared.id LIMIT 2",
    )
    .expect("el JOIN local debe usar la comparación cargada");

    assert_eq!(
        result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "segment"]
    );
    assert_eq!(result.row_count, 3);
    assert_eq!(result.offset, 0);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), None],
            vec![Some("2".to_owned()), Some("B".to_owned())]
        ]
    );
    assert!(result.truncated);
}

#[test]
fn local_query_joins_by_multiple_keys_without_cross_matching() {
    let current = df![
        "id" => &[1_i64, 2, 2, 3],
        "region" => &["north", "north", "south", "south"]
    ]
    .unwrap();
    let compared = df![
        "id" => &[2_i64, 2, 3],
        "region" => &["north", "south", "south"],
        "segment" => &["A", "B", "C"]
    ]
    .unwrap();

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id, region, segment FROM dataset LEFT JOIN compared ON dataset.id = compared.id AND dataset.region = compared.region LIMIT 10",
    )
    .expect("el JOIN compuesto local debe ejecutarse");

    assert_eq!(result.row_count, 4);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), Some("north".to_owned()), None],
            vec![
                Some("2".to_owned()),
                Some("north".to_owned()),
                Some("A".to_owned())
            ],
            vec![
                Some("2".to_owned()),
                Some("south".to_owned()),
                Some("B".to_owned())
            ],
            vec![
                Some("3".to_owned()),
                Some("south".to_owned()),
                Some("C".to_owned())
            ],
        ]
    );

    let filtered_grouped = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT region, COUNT(*) AS total FROM dataset LEFT JOIN compared ON dataset.id = compared.id AND dataset.region = compared.region WHERE segment IS NOT NULL GROUP BY region LIMIT 10",
    )
    .expect("la consulta posterior al JOIN debe conservar WHERE y GROUP BY");
    assert_eq!(
        filtered_grouped.rows,
        vec![
            vec![Some("north".to_owned()), Some("1".to_owned())],
            vec![Some("south".to_owned()), Some("2".to_owned())],
        ]
    );

    let duplicate = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id FROM dataset JOIN compared ON id = id AND dataset.id = compared.id LIMIT 1",
    )
    .expect_err("el JOIN no debe repetir columnas clave");
    assert!(duplicate.contains("no pueden repetirse"));

    let too_many_conditions = std::iter::repeat_n("id = id", LOCAL_QUERY_MAX_JOIN_COLUMNS + 1)
        .collect::<Vec<_>>()
        .join(" AND ");
    let too_many = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        &format!("SELECT id FROM dataset JOIN compared ON {too_many_conditions} LIMIT 1"),
    )
    .expect_err("el JOIN debe respetar el límite de pares de claves");
    assert!(too_many.contains("entre 1 y 8"));
}

#[test]
fn local_query_join_pages_across_current_blocks_without_changing_order() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 5;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let segments = ids
        .iter()
        .map(|id| format!("segment-{id}"))
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        row_count,
        vec![Series::new("id".into(), ids.clone()).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids).into_column(),
            Series::new("segment".into(), segments).into_column(),
        ],
    )
    .expect("el dataset comparado debe construirse");
    let offset = LOCAL_QUERY_BLOCK_ROWS - 2;

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        &format!(
            "SELECT id, segment FROM dataset INNER JOIN compared ON dataset.id = compared.id LIMIT 5 OFFSET {offset}"
        ),
    )
    .expect("el JOIN paginado debe atravesar el límite de bloque");

    assert_eq!(result.row_count, row_count);
    assert_eq!(result.offset, offset);
    assert_eq!(
        result.rows,
        (offset as i64..offset as i64 + 5)
            .map(|id| vec![Some(id.to_string()), Some(format!("segment-{id}"))])
            .collect::<Vec<_>>()
    );
    assert!(result.truncated);
}

#[test]
fn local_query_join_aggregates_across_current_blocks_without_materializing_all_rows() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 4;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let regions = ids
        .iter()
        .map(|id| {
            if *id < LOCAL_QUERY_BLOCK_ROWS as i64 {
                "north".to_owned()
            } else {
                "south".to_owned()
            }
        })
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids.clone()).into_column(),
            Series::new("region".into(), regions).into_column(),
        ],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids.clone()).into_column(),
            Series::new("value".into(), ids).into_column(),
        ],
    )
    .expect("el dataset comparado debe construirse");
    let query = "SELECT region, COUNT(*) AS total, SUM(value) AS total_value, AVG(value) AS average_value, MIN(value) AS minimum_value, MAX(value) AS maximum_value FROM dataset LEFT JOIN compared ON dataset.id = compared.id WHERE value IS NOT NULL GROUP BY region LIMIT 10";
    let result = execute_local_query_with_comparison(&current, Some(&compared), query)
        .expect("la agregación JOIN debe atravesar el límite de bloque");
    let north_sum = (0..LOCAL_QUERY_BLOCK_ROWS as i64).sum::<i64>();
    let south_start = LOCAL_QUERY_BLOCK_ROWS as i64;
    let south_sum = (south_start..south_start + 4).sum::<i64>();

    assert_eq!(result.row_count, 2);
    assert_eq!(
        result.rows,
        vec![
            vec![
                Some("north".to_owned()),
                Some(LOCAL_QUERY_BLOCK_ROWS.to_string()),
                Some(north_sum.to_string()),
                Some((north_sum as f64 / LOCAL_QUERY_BLOCK_ROWS as f64).to_string()),
                Some("0".to_owned()),
                Some((LOCAL_QUERY_BLOCK_ROWS as i64 - 1).to_string()),
            ],
            vec![
                Some("south".to_owned()),
                Some("4".to_owned()),
                Some(south_sum.to_string()),
                Some((south_sum as f64 / 4.0).to_string()),
                Some(south_start.to_string()),
                Some((south_start + 3).to_string()),
            ],
        ]
    );
    assert!(!result.truncated);

    let paged_query = query.replace("LIMIT 10", "LIMIT 1 OFFSET 1");
    let page = execute_local_query_with_comparison(&current, Some(&compared), &paged_query)
        .expect("la agregación JOIN debe conservar OFFSET/LIMIT");
    assert_eq!(page.row_count, 2);
    assert_eq!(page.offset, 1);
    assert_eq!(
        page.rows,
        vec![vec![
            Some("south".to_owned()),
            Some("4".to_owned()),
            Some(south_sum.to_string()),
            Some((south_sum as f64 / 4.0).to_string()),
            Some(south_start.to_string()),
            Some((south_start + 3).to_string()),
        ],]
    );
    assert!(!page.truncated);
}

#[test]
fn local_query_full_join_pages_current_and_right_only_blocks_in_order() {
    let current_row_count = LOCAL_QUERY_BLOCK_ROWS + 3;
    let current_ids = (0..current_row_count as i64).collect::<Vec<_>>();
    let mut compared_ids = current_ids.clone();
    compared_ids.extend([current_row_count as i64, current_row_count as i64 + 1]);
    let compared_segments = compared_ids
        .iter()
        .map(|id| format!("segment-{id}"))
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        current_row_count,
        vec![Series::new("id".into(), current_ids).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        compared_ids.len(),
        vec![
            Series::new("id".into(), compared_ids).into_column(),
            Series::new("segment".into(), compared_segments).into_column(),
        ],
    )
    .expect("el dataset comparado debe construirse");
    let offset = current_row_count - 2;

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        &format!(
            "SELECT id, segment FROM dataset FULL JOIN compared ON dataset.id = compared.id LIMIT 4 OFFSET {offset}"
        ),
    )
    .expect("el FULL JOIN paginado debe recorrer ambos lados");

    assert_eq!(result.row_count, current_row_count + 2);
    assert_eq!(result.offset, offset);
    assert_eq!(
        result.rows,
        vec![
            vec![
                Some((current_row_count - 2).to_string()),
                Some(format!("segment-{}", current_row_count - 2)),
            ],
            vec![
                Some((current_row_count - 1).to_string()),
                Some(format!("segment-{}", current_row_count - 1)),
            ],
            vec![
                Some(current_row_count.to_string()),
                Some(format!("segment-{current_row_count}")),
            ],
            vec![
                Some((current_row_count + 1).to_string()),
                Some(format!("segment-{}", current_row_count + 1)),
            ],
        ]
    );
    assert!(!result.truncated);
}

#[test]
fn full_join_visits_right_only_rows_in_bounded_blocks_and_keeps_null_keys_unmatched() {
    let current = DataFrame::new(
        2,
        vec![Series::new("id".into(), &[Some(1_i64), None]).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[Some(1_i64), None]).into_column(),
            Series::new("segment".into(), &[Some("matched"), Some("null-right")]).into_column(),
        ],
    )
    .expect("el dataset comparado debe construirse");
    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id, segment FROM dataset FULL JOIN compared ON dataset.id = compared.id LIMIT 10",
    )
    .expect("el FULL JOIN debe conservar las claves nulas como no emparejadas");

    assert_eq!(result.row_count, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), Some("matched".to_owned())],
            vec![None, None],
            vec![None, Some("null-right".to_owned())],
        ]
    );

    let mut compared_ids = (2_i64..=LOCAL_QUERY_BLOCK_ROWS as i64 + 2).collect::<Vec<_>>();
    compared_ids.push(LOCAL_QUERY_BLOCK_ROWS as i64 + 3);
    let compared_large = DataFrame::new(
        compared_ids.len(),
        vec![Series::new("id".into(), compared_ids.clone()).into_column()],
    )
    .expect("el dataset comparado grande debe construirse");
    let mut visited_heights = Vec::new();
    visit_unmatched_join_right_blocks_with_cancel(
        &current,
        &compared_large,
        &["id".to_owned()],
        &["id".to_owned()],
        &|| false,
        |block| {
            visited_heights.push(block.height());
            Ok(())
        },
    )
    .expect("el anti-join derecho debe emitirse por bloques");
    assert_eq!(visited_heights, vec![LOCAL_QUERY_BLOCK_ROWS, 2]);
}

#[test]
fn local_query_full_join_aggregates_right_only_rows() {
    let current_row_count = LOCAL_QUERY_BLOCK_ROWS + 4;
    let current_ids = (0..current_row_count as i64).collect::<Vec<_>>();
    let mut compared_ids = current_ids.clone();
    compared_ids.extend([current_row_count as i64, current_row_count as i64 + 1]);
    let values = compared_ids.clone();
    let current = DataFrame::new(
        current_row_count,
        vec![Series::new("id".into(), current_ids).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        compared_ids.len(),
        vec![
            Series::new("id".into(), compared_ids).into_column(),
            Series::new("value".into(), values).into_column(),
        ],
    )
    .expect("el dataset comparado debe construirse");

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT COUNT(*) AS total, SUM(value) AS total_value FROM dataset FULL JOIN compared ON dataset.id = compared.id LIMIT 10",
    )
    .expect("la agregación FULL JOIN debe incluir las filas derechas");
    let total = current_row_count + 2;
    let sum = (0..total as i64).sum::<i64>();

    assert_eq!(result.row_count, 1);
    assert_eq!(
        result.rows,
        vec![vec![Some(total.to_string()), Some(sum.to_string())]]
    );
    assert!(!result.truncated);
}

#[test]
fn local_query_full_join_preserves_duplicate_matches_and_filters_both_sides() {
    let current = df!["id" => &[1_i64, 2]].unwrap();
    let compared = df![
        "id" => &[1_i64, 1, 3],
        "segment" => &["A", "B", "C"]
    ]
    .unwrap();

    let result = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id, segment FROM dataset FULL JOIN compared ON dataset.id = compared.id WHERE segment IS NOT NULL LIMIT 10",
    )
    .expect("el FULL JOIN debe conservar duplicados y filtrar ambos lados");

    assert_eq!(result.row_count, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![Some("1".to_owned()), Some("A".to_owned())],
            vec![Some("1".to_owned()), Some("B".to_owned())],
            vec![Some("3".to_owned()), Some("C".to_owned())],
        ]
    );
}

#[test]
fn local_query_full_join_honors_cancellation_during_preflight_or_block_walk() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 1;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let current = DataFrame::new(
        row_count,
        vec![Series::new("id".into(), ids.clone()).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(row_count, vec![Series::new("id".into(), ids).into_column()])
        .expect("el dataset comparado debe construirse");
    let checks = AtomicU64::new(0);

    let error = execute_local_query_with_comparison_and_cancel(
        &current,
        Some(&compared),
        "SELECT id FROM dataset FULL JOIN compared ON dataset.id = compared.id LIMIT 1",
        &|| checks.fetch_add(1, Ordering::Relaxed) >= 12,
    )
    .expect_err("el FULL JOIN debe respetar la cancelación cooperativa");

    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
}

#[test]
fn local_query_join_requires_comparison_and_matching_key_types() {
    let current = df!["id" => &[1_i64], "value" => &[10_i64]].unwrap();
    let compared = df!["id" => &["1"], "segment" => &["A"]].unwrap();

    let missing = execute_local_query_with_comparison(
        &current,
        None,
        "SELECT * FROM dataset JOIN compared ON id = id LIMIT 1",
    )
    .expect_err("un JOIN sin comparación no debe leer una fuente arbitraria");
    assert!(missing.contains("No hay un dataset comparado cargado"));

    let incompatible = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT * FROM dataset JOIN compared ON id = id LIMIT 1",
    )
    .expect_err("las claves con tipos distintos deben rechazarse");
    assert!(incompatible.contains("tipos incompatibles"));

    assert!(execute_local_query_with_comparison(
        &current,
        Some(&current),
        "SELECT * FROM dataset RIGHT JOIN compared ON id = id LIMIT 1",
    )
    .is_err());
}

#[test]
fn local_query_join_rejects_many_to_many_cardinality_before_materializing() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 1_501;
    let current = DataFrame::new(
        row_count,
        vec![Series::new("id".into(), vec![1_i64; row_count]).into_column()],
    )
    .expect("el dataset activo debe construirse");
    let compared = DataFrame::new(
        row_count,
        vec![Series::new("id".into(), vec![1_i64; row_count]).into_column()],
    )
    .expect("el dataset comparado debe construirse");

    let error = execute_local_query_with_comparison(
        &current,
        Some(&compared),
        "SELECT id FROM dataset JOIN compared ON id = id LIMIT 1",
    )
    .expect_err("el fan-out del JOIN debe rechazarse antes de materializarse");

    assert!(error.contains("resultado estimado del JOIN"));
    assert!(error.contains(&LOCAL_QUERY_JOIN_MAX_RESULT_ROWS.to_string()));
}

#[test]
fn local_query_aggregate_rejects_matching_rows_over_materialization_budget() {
    let row_count = LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS + 1;
    let frame = DataFrame::new(
        row_count,
        vec![Series::new("id".into(), vec![1_i64; row_count]).into_column()],
    )
    .expect("el dataset grande debe construirse");

    let error = execute_local_query(&frame, "SELECT COUNT(*) AS total FROM dataset")
        .expect_err("una agregación que excede el presupuesto debe rechazarse");

    assert!(error.contains("limita las filas coincidentes"));
    assert!(error.contains(&LOCAL_QUERY_AGGREGATE_MAX_MATCHING_ROWS.to_string()));
}

#[test]
fn local_query_stops_at_cooperative_cancellation_point() {
    let frame = df!["id" => &[1_i64, 2, 3]].unwrap();
    let error = execute_local_query_with_cancel(&frame, "SELECT id FROM dataset LIMIT 1", &|| true)
        .expect_err("la consulta debe detenerse si se cancela antes de escanear");

    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
}

#[test]
fn local_query_filters_nulls_and_calculates_bounded_aggregates() {
    let frame = df![
        "city" => &[Some("Santo Domingo"), None, Some("Santiago"), Some("Santiago")],
        "value" => &[Some(10_i64), Some(20_i64), Some(30_i64), Some(40_i64)]
    ]
    .unwrap();

    let filtered = execute_local_query(
        &frame,
        "SELECT city, value FROM dataset WHERE city IS NOT NULL AND value >= 30 LIMIT 1",
    )
    .expect("el filtro local debe ejecutarse");
    assert_eq!(filtered.row_count, 2);
    assert_eq!(
        filtered.rows,
        vec![vec![Some("Santiago".to_owned()), Some("30".to_owned())]]
    );
    assert!(filtered.truncated);

    let aggregate = execute_local_query(
        &frame,
        "SELECT COUNT(*) AS total, AVG(value) AS average, MAX(value) AS highest FROM dataset WHERE value >= 20",
    )
    .expect("las agregaciones locales deben ejecutarse");
    assert_eq!(aggregate.row_count, 1);
    assert_eq!(
        aggregate
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["total", "average", "highest"]
    );
    assert_eq!(
        aggregate.rows,
        vec![vec![
            Some("3".to_owned()),
            Some("30".to_owned()),
            Some("40".to_owned())
        ]]
    );

    let grouped = execute_local_query(
        &frame,
        "SELECT city, COUNT(*) AS total, SUM(value) AS sum_value FROM dataset GROUP BY city LIMIT 10",
    )
    .expect("GROUP BY local debe ejecutarse");
    assert_eq!(grouped.row_count, 3);
    assert_eq!(
        grouped.rows[0],
        vec![
            Some("Santo Domingo".to_owned()),
            Some("1".to_owned()),
            Some("10".to_owned())
        ]
    );
    assert_eq!(
        grouped.rows[1],
        vec![None, Some("1".to_owned()), Some("20".to_owned())]
    );
    assert_eq!(
        grouped.rows[2],
        vec![
            Some("Santiago".to_owned()),
            Some("2".to_owned()),
            Some("70".to_owned())
        ]
    );

    assert!(
        execute_local_query(&frame, "SELECT city FROM dataset WHERE city = untrusted").is_err()
    );
    assert!(execute_local_query(&frame, "SELECT SUM(city) FROM dataset").is_err());
    assert!(execute_local_query(&frame, "SELECT city, COUNT(*) FROM dataset").is_err());
    assert!(execute_local_query(&frame, "SELECT city FROM dataset GROUP BY city").is_err());
}

#[test]
fn local_query_groups_by_multiple_columns_with_stable_null_keys() {
    let frame = df![
        "region" => &["north", "north", "north", "south", "south", "north"],
        "segment" => &[Some("a"), Some("b"), Some("a"), Some("a"), None, Some("b")],
        "value" => &[1_i64, 2, 3, 4, 5, 6]
    ]
    .unwrap();

    let grouped = execute_local_query(
        &frame,
        "SELECT region, segment, SUM(value) AS total FROM dataset GROUP BY region, segment LIMIT 10",
    )
    .expect("GROUP BY compuesto local debe ejecutarse");

    assert_eq!(grouped.row_count, 4);
    assert_eq!(
        grouped.rows,
        vec![
            vec![
                Some("north".to_owned()),
                Some("a".to_owned()),
                Some("4".to_owned())
            ],
            vec![
                Some("north".to_owned()),
                Some("b".to_owned()),
                Some("8".to_owned())
            ],
            vec![
                Some("south".to_owned()),
                Some("a".to_owned()),
                Some("4".to_owned())
            ],
            vec![Some("south".to_owned()), None, Some("5".to_owned())],
        ]
    );

    let duplicate = execute_local_query(
        &frame,
        "SELECT region, COUNT(*) AS total FROM dataset GROUP BY region, region LIMIT 10",
    )
    .expect_err("GROUP BY no debe aceptar claves duplicadas");
    assert!(duplicate.contains("está duplicada"));

    let too_many_groups = (0..=LOCAL_QUERY_MAX_GROUP_COLUMNS)
        .map(|index| format!("column_{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let too_many = execute_local_query(
        &frame,
        &format!("SELECT COUNT(*) AS total FROM dataset GROUP BY {too_many_groups} LIMIT 10"),
    )
    .expect_err("GROUP BY debe respetar el límite de columnas");
    assert!(too_many.contains("entre 1 y 8"));
}

#[test]
fn local_query_parallel_blocks_preserve_page_order_and_aggregate_totals() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS * 2 + 37;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let values = ids.iter().map(|value| value * 2).collect::<Vec<_>>();
    let buckets = ids.iter().map(|value| value % 2).collect::<Vec<_>>();
    let frame = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids).into_column(),
            Series::new("value".into(), values).into_column(),
            Series::new("bucket".into(), buckets).into_column(),
        ],
    )
    .expect("el dataset de prueba debe construirse");

    let offset = LOCAL_QUERY_BLOCK_ROWS - 2;
    let query = format!("SELECT id, value FROM dataset WHERE value >= 0 LIMIT 5 OFFSET {offset}");
    let first_page =
        execute_local_query(&frame, &query).expect("la página paralela debe ejecutarse");
    let second_page =
        execute_local_query(&frame, &query).expect("la página paralela debe ser determinista");

    assert_eq!(first_page, second_page);
    assert_eq!(first_page.row_count, row_count);
    assert_eq!(
        first_page.rows,
        (offset as i64..offset as i64 + 5)
            .map(|id| vec![Some(id.to_string()), Some((id * 2).to_string())])
            .collect::<Vec<_>>()
    );

    let aggregate = execute_local_query(
        &frame,
        "SELECT COUNT(*) AS total, SUM(value) AS total_value, AVG(value) AS average_value, MIN(value) AS minimum_value, MAX(value) AS maximum_value FROM dataset WHERE value >= 0",
    )
    .expect("la agregación paralela debe ejecutarse");
    let sum = (row_count as i64 * (row_count as i64 - 1)).to_string();
    let average = (row_count as f64 - 1.0).to_string();
    assert_eq!(
        aggregate.rows,
        vec![vec![
            Some(row_count.to_string()),
            Some(sum),
            Some(average),
            Some("0".to_owned()),
            Some(((row_count as i64 - 1) * 2).to_string()),
        ]]
    );

    let grouped = execute_local_query(
        &frame,
        "SELECT bucket, COUNT(*) AS total, SUM(value) AS total_value FROM dataset GROUP BY bucket LIMIT 10",
    )
    .expect("la agrupación por bloques debe ejecutarse");
    let even_count = (0..row_count as i64).filter(|id| id % 2 == 0).count();
    let even_sum = (0..row_count as i64)
        .filter(|id| id % 2 == 0)
        .map(|id| id * 2)
        .sum::<i64>();
    assert_eq!(
        grouped.rows,
        vec![
            vec![
                Some("0".to_owned()),
                Some(even_count.to_string()),
                Some(even_sum.to_string())
            ],
            vec![
                Some("1".to_owned()),
                Some((row_count - even_count).to_string()),
                Some(((row_count as i64 * (row_count as i64 - 1)) - even_sum).to_string())
            ],
        ]
    );
}

#[test]
fn profiles_nulls_uniques_and_numeric_statistics() {
    let path = temporary_csv(
        "city,temperature\nSanto Domingo,30\nSantiago,\nSantiago,28\nSantiago,28\n,25\n",
    );
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let city = &profile.columns[0];
    let temperature = &profile.columns[1];

    assert_eq!(profile.row_count, 5);
    assert_eq!(profile.duplicate_row_count, 1);
    assert_eq!(profile.duplicate_percentage, 20.0);
    assert_eq!(city.unique_count, 2);
    assert_eq!(city.null_count, 1);
    assert_eq!(city.mean, None);
    assert_eq!(city.empty_count, Some(0));
    assert_eq!(city.minimum_length, Some(8));
    assert_eq!(city.maximum_length, Some(13));
    assert!((city.average_length.unwrap() - 9.25).abs() < 0.001);
    assert_eq!(city.suggested_type, None);
    assert_eq!(temperature.null_count, 1);
    assert_eq!(temperature.completeness_percentage, 80.0);
    assert_eq!(temperature.minimum.as_deref(), Some("25"));
    assert_eq!(temperature.maximum.as_deref(), Some("30"));
    assert_eq!(temperature.mean, Some(27.75));
    assert_eq!(temperature.empty_count, Some(0));
    assert_eq!(temperature.suggested_type, Some("integer".to_owned()));
    assert_eq!(temperature.first_quartile, Some(27.25));
    assert_eq!(temperature.median, Some(28.0));
    assert_eq!(temperature.third_quartile, Some(28.5));
    assert!((temperature.standard_deviation.unwrap() - 2.061_552).abs() < 0.001);
    assert_eq!(temperature.outlier_count, Some(1));
    let histogram = temperature
        .histogram
        .as_ref()
        .expect("la columna numérica debe incluir histograma");
    assert_eq!(histogram.len(), NUMERIC_HISTOGRAM_BUCKETS);
    assert_eq!(
        histogram.iter().map(|bucket| bucket.count).sum::<usize>(),
        4
    );
    assert_eq!(histogram.first().map(|bucket| bucket.lower), Some(25.0));
    assert_eq!(histogram.last().map(|bucket| bucket.upper), Some(30.0));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn csv_preserves_lexical_values_and_does_not_profile_identifiers_as_numbers() {
    let path = temporary_csv(
        "identifier,amount,huge\n00123,1.00,184467440737095516160\n00456,2.50,184467440737095516161\n00789,3.00,184467440737095516162\n",
    );
    let (frame, preview) = load_csv(&path).expect("el CSV debe cargar sin inferencia destructiva");
    let profile = profile_dataset(&frame).expect("el perfil semántico debe calcularse");

    assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
    assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
    assert_eq!(preview.rows[0][1].as_deref(), Some("1.00"));
    assert_eq!(preview.rows[0][2].as_deref(), Some("184467440737095516160"));
    assert_eq!(profile.columns[0].suggested_type, None);
    assert_eq!(profile.columns[0].mean, None);
    assert_eq!(profile.columns[0].outlier_count, None);
    assert_eq!(
        profile.columns[1].suggested_type,
        Some("decimal".to_owned())
    );
    assert_eq!(profile.columns[1].minimum.as_deref(), Some("1"));
    assert!((profile.columns[1].mean.unwrap() - 2.166_666).abs() < 0.001);
    assert_eq!(profile.columns[2].suggested_type, None);
    assert_eq!(profile.columns[2].mean, None);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn profiles_bounded_numeric_correlations_without_exposing_cells() {
    let path = temporary_csv(
        "first,second,constant,identifier\n1,2,9,001\n2,4,9,002\n,8,9,003\n4,8,9,004\n",
    );
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let correlations = profile
        .numeric_correlations
        .as_ref()
        .expect("debe calcular correlaciones para dos columnas numéricas");
    assert_eq!(correlations.columns, vec!["first", "second", "constant"]);
    assert_eq!(correlations.sampled_row_count, 4);
    assert!(!correlations.truncated);
    let first_second = correlations
        .pairs
        .iter()
        .find(|pair| pair.first_column == "first" && pair.second_column == "second")
        .expect("debe incluir el par first-second");
    assert_eq!(first_second.sample_count, 3);
    assert!((first_second.coefficient.expect("debe ser definido") - 1.0).abs() < 1e-9);
    let first_constant = correlations
        .pairs
        .iter()
        .find(|pair| pair.first_column == "first" && pair.second_column == "constant")
        .expect("debe incluir el par first-constant");
    assert_eq!(first_constant.coefficient, None);
    assert!(profile
        .columns
        .iter()
        .all(|column| { column.name != "identifier" || column.mean.is_none() }));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn numeric_correlations_honor_the_requested_sample_limit() {
    let path = temporary_csv("first,second\n1,2\n2,4\n3,6\n4,8\n5,10\n6,12\n7,14\n8,16\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset_with_sample_rows(&frame, 2).expect("el perfil debe calcularse");
    let correlations = profile
        .numeric_correlations
        .as_ref()
        .expect("debe calcular correlaciones para dos columnas numéricas");
    assert_eq!(correlations.sampled_row_count, 2);
    assert_eq!(correlations.pairs[0].sample_count, 2);
    assert_eq!(correlations.pairs[0].coefficient, Some(1.0));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn validates_numeric_correlation_sample_bounds() {
    assert_eq!(
        validate_numeric_correlation_sample_rows(MIN_NUMERIC_CORRELATION_SAMPLE_ROWS),
        Ok(MIN_NUMERIC_CORRELATION_SAMPLE_ROWS)
    );
    assert_eq!(
        validate_numeric_correlation_sample_rows(MAX_NUMERIC_CORRELATION_SAMPLE_ROWS),
        Ok(MAX_NUMERIC_CORRELATION_SAMPLE_ROWS)
    );
    assert!(validate_numeric_correlation_sample_rows(0).is_err());
    assert!(
        validate_numeric_correlation_sample_rows(MAX_NUMERIC_CORRELATION_SAMPLE_ROWS + 1).is_err()
    );
}

#[test]
fn profiles_bounded_categorical_groups_and_keeps_private_columns_out() {
    let path = temporary_csv(
        "segment,email,status\nA,ana@example.com,ok\nA,beatriz@example.com,ok\nA,carlos@example.com,ok\nB,diana@example.com,ok\nB,elena@example.com,ok\nC,francisco@example.com,ok\n",
    );
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let summaries = profile
        .categorical_group_summaries
        .as_ref()
        .expect("debe resumir una columna categórica no sensible");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].column, "segment");
    assert_eq!(summaries[0].distinct_count, 3);
    assert_eq!(summaries[0].groups.len(), 2);
    assert_eq!(summaries[0].groups[0].label, "A");
    assert_eq!(summaries[0].groups[0].row_count, 3);
    assert!(!summaries[0].groups[0].is_other);
    assert_eq!(summaries[0].groups[1].label, "Resto");
    assert_eq!(summaries[0].groups[1].row_count, 3);
    assert!(summaries[0].groups[1].is_other);
    let serialized = serde_json::to_string(&profile).expect("el perfil debe serializar");
    assert!(!serialized.contains("ana@example.com"));
    assert!(!serialized.contains("francisco@example.com"));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn profiles_temporal_month_trend_with_empty_periods_and_bounded_payload() {
    let frame = DataFrame::new(
        5,
        vec![Series::new(
            "created_at".into(),
            [
                "2024-01-15",
                "2024-02-15",
                "2024-02-20",
                "2024-04-01",
                "2025-01-01",
            ],
        )
        .into_column()],
    )
    .expect("el frame temporal debe ser válido");

    let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
    let summary = profile
        .temporal_series
        .as_ref()
        .and_then(|summaries| summaries.first())
        .expect("debe calcular una tendencia para una fecha sugerida");

    assert_eq!(summary.column, "created_at");
    assert_eq!(summary.granularity, "month");
    assert_eq!(summary.parsed_row_count, 5);
    assert_eq!(summary.unparsed_row_count, 0);
    assert!(!summary.truncated);
    assert_eq!(summary.periods.len(), 13);
    assert_eq!(summary.periods[0].period, "2024-01");
    assert_eq!(summary.periods[0].row_count, 1);
    assert_eq!(summary.periods[1].period, "2024-02");
    assert_eq!(summary.periods[1].row_count, 2);
    assert_eq!(summary.periods[2].period, "2024-03");
    assert_eq!(summary.periods[2].row_count, 0);
    assert_eq!(
        summary.periods.last().map(|period| period.period.as_str()),
        Some("2025-01")
    );
    assert_eq!(
        summary
            .periods
            .iter()
            .map(|period| period.row_count)
            .sum::<usize>(),
        5
    );
    assert!(serde_json::to_string(&profile)
        .expect("el perfil debe serializar")
        .contains("2024-02"));
}

#[test]
fn profiles_temporal_day_trend_keeps_empty_days_for_short_spans() {
    let frame = DataFrame::new(
        4,
        vec![Series::new(
            "created_at".into(),
            ["2024-04-01", "2024-04-03", "2024-04-03", "2024-04-05"],
        )
        .into_column()],
    )
    .expect("el frame temporal debe ser válido");

    let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
    let summary = profile
        .temporal_series
        .as_ref()
        .and_then(|summaries| summaries.first())
        .expect("debe calcular una tendencia diaria para un rango corto");

    assert_eq!(summary.granularity, "day");
    assert_eq!(summary.periods.len(), 5);
    assert_eq!(summary.periods[0].period, "2024-04-01");
    assert_eq!(summary.periods[0].row_count, 1);
    assert_eq!(summary.periods[1].period, "2024-04-02");
    assert_eq!(summary.periods[1].row_count, 0);
    assert_eq!(summary.periods[2].period, "2024-04-03");
    assert_eq!(summary.periods[2].row_count, 2);
    assert_eq!(summary.periods[4].period, "2024-04-05");
    assert_eq!(
        summary
            .periods
            .iter()
            .map(|period| period.row_count)
            .sum::<usize>(),
        4
    );
    assert!(summary
        .periods
        .iter()
        .all(|period| period.period.chars().count() == 10));
}

#[test]
fn profiles_temporal_year_trend_keeps_recent_periods_with_fixed_limit() {
    let dates = (1970..=2029)
        .map(|year| format!("{year}-01-01"))
        .collect::<Vec<_>>();
    let frame = DataFrame::new(
        dates.len(),
        vec![Series::new("created_at".into(), dates).into_column()],
    )
    .expect("el frame temporal debe ser válido");

    let profile = profile_dataset(&frame).expect("el perfil temporal debe calcularse");
    let summary = profile
        .temporal_series
        .as_ref()
        .and_then(|summaries| summaries.first())
        .expect("debe calcular una tendencia anual");

    assert_eq!(summary.granularity, "year");
    assert_eq!(summary.parsed_row_count, 60);
    assert!(summary.truncated);
    assert_eq!(summary.periods.len(), MAX_TEMPORAL_PERIODS);
    assert_eq!(summary.periods[0].period, "Periodos anteriores");
    assert_eq!(summary.periods[0].row_count, 13);
    assert_eq!(
        summary.periods.last().map(|period| period.period.as_str()),
        Some("2029")
    );
    assert_eq!(
        summary
            .periods
            .iter()
            .map(|period| period.row_count)
            .sum::<usize>(),
        60
    );
}

#[test]
fn empty_numeric_dataset_does_not_attempt_to_sample_correlations() {
    let frame = DataFrame::new(
        0,
        vec![
            Series::new("first".into(), Vec::<i64>::new()).into_column(),
            Series::new("second".into(), Vec::<i64>::new()).into_column(),
        ],
    )
    .expect("el frame vacío debe ser válido");

    let profile = profile_dataset(&frame).expect("el perfil vacío debe calcularse");

    assert_eq!(profile.row_count, 0);
    assert_eq!(profile.numeric_correlations, None);
}

#[test]
fn reports_profile_progress_per_column() {
    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let mut updates = Vec::new();

    profile_dataset_with_progress(
        &frame,
        |stage, percent| {
            updates.push((stage, percent));
        },
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect("el perfil debe calcularse");

    assert_eq!(updates.first(), Some(&("Detectando filas duplicadas", 10)));
    assert_eq!(updates.last(), Some(&("Analizando columnas", 100)));
    assert!(updates
        .iter()
        .any(|(stage, _)| *stage == "Contando valores únicos"));
    assert!(updates
        .iter()
        .any(|(stage, _)| *stage == "Calculando estadísticas numéricas"));
    assert!(updates.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn stops_profile_at_a_cooperative_cancellation_point() {
    use std::sync::atomic::AtomicUsize;

    let path = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let checks = AtomicUsize::new(0);

    let error = profile_dataset_with_progress(
        &frame,
        |_, _| {},
        || {
            let count = checks.fetch_add(1, Ordering::SeqCst) + 1;
            count >= 2
        },
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect_err("el perfil debe detenerse al cancelar");

    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert_eq!(checks.load(Ordering::SeqCst), 2);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn invalidates_only_the_requested_operation_generation() {
    let state = DatasetState::default();
    let load_generation = state.begin_load();
    let profile_generation = state.begin_profile();
    let export_generation = state.begin_export();
    let query_generation = state.begin_query();
    state
        .cancel("profile")
        .expect("el perfil debe poder cancelarse");
    state
        .cancel("query")
        .expect("la consulta debe poder cancelarse");

    assert!(!state.load_was_cancelled(load_generation));
    assert!(state.profile_was_cancelled(profile_generation));
    assert!(!state.export_was_cancelled(export_generation));
    assert!(state.query_was_cancelled(query_generation));
    assert!(state.cancel("unknown").is_err());
}

#[test]
fn queues_and_consumes_a_native_drop_path_once() {
    let state = DatasetState::default();
    let path = PathBuf::from("C:/datos/ventas.csv");

    state.queue_dropped_path(path.clone());
    assert_eq!(
        state
            .take_dropped_path()
            .expect("la cola debe estar disponible"),
        Some(path)
    );
    assert_eq!(
        state
            .take_dropped_path()
            .expect("la cola debe quedar vacía"),
        None
    );
}

#[test]
fn exports_csv_by_atomically_replacing_the_destination() {
    let source = temporary_csv("city,temperature\nSanto Domingo,30\nSantiago,28\n");
    let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("resultado.csv");
    fs::write(&destination, "contenido anterior").expect("se debe preparar el destino");
    let mut updates = Vec::new();

    let result = export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Csv,
        |stage, percent| updates.push((stage, percent)),
        || false,
    )
    .expect("el CSV debe exportarse");

    let exported = fs::read_to_string(&destination).expect("se debe leer la exportación");
    assert!(exported.starts_with("city,temperature"));
    assert!(exported.contains("Santo Domingo,30"));
    assert_eq!(result.file_name, "resultado.csv");
    assert_eq!(result.format, "CSV");
    assert_eq!(updates.last(), Some(&("Exportación lista", 100)));
    fs::remove_file(source).expect("se debe limpiar el CSV temporal");
}

#[test]
fn csv_export_neutralizes_spreadsheet_formulas_and_parquet_preserves_values() {
    let dangerous = [
        "=SUM(A1:A2)",
        "+cmd",
        "-2+3",
        "@SUM(A1:A2)",
        "\tformula",
        "\rformula",
        "\nformula",
    ];
    let benign = ["text", "123", "  =not-a-prefix", "'already-text"];
    let values = dangerous
        .iter()
        .chain(benign.iter())
        .copied()
        .map(Some)
        .chain(std::iter::once(None))
        .collect::<Vec<_>>();
    let mut numbers = (0_i64..)
        .take(dangerous.len() + benign.len() + 1)
        .collect::<Vec<_>>();
    numbers[0] = -12;
    let frame = DataFrame::new(
        values.len(),
        vec![
            Series::new("text".into(), values).into_column(),
            Series::new("number".into(), numbers).into_column(),
        ],
    )
    .unwrap();

    let csv = frame_for_export(&frame, ExportFormat::Csv).expect("CSV debe protegerse");
    let exported_text = csv
        .column("text")
        .unwrap()
        .str()
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    for (index, value) in dangerous.iter().enumerate() {
        assert_eq!(exported_text[index], Some(format!("'{value}").as_str()));
    }
    for (offset, value) in benign.iter().enumerate() {
        assert_eq!(exported_text[dangerous.len() + offset], Some(*value));
    }
    assert_eq!(exported_text.last(), Some(&None));
    assert_eq!(
        csv.column("number").unwrap(),
        frame.column("number").unwrap()
    );
    assert_eq!(
        csv.column("number").unwrap().i64().unwrap().get(0),
        Some(-12)
    );

    let parquet = frame_for_export(&frame, ExportFormat::Parquet)
        .expect("Parquet debe conservar los valores originales");
    assert!(parquet.equals_missing(&frame));
}

#[test]
fn exports_a_valid_parquet_file() {
    let source = temporary_csv("value\n1\n2\n");
    let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("resultado.parquet");

    export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Parquet,
        |_, _| {},
        || false,
    )
    .expect("Parquet debe exportarse");

    let bytes = fs::read(destination).expect("se debe leer Parquet");
    assert!(bytes.starts_with(b"PAR1"));
    assert!(bytes.ends_with(b"PAR1"));
    fs::remove_file(source).expect("se debe limpiar el CSV temporal");
}

#[test]
fn exports_a_valid_json_array() {
    let source = temporary_csv("city,value\nSanto Domingo,30\nSantiago,28\n");
    let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("resultado.json");

    export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Json,
        |_, _| {},
        || false,
    )
    .expect("JSON debe exportarse");

    let value: JsonValue = serde_json::from_slice(&fs::read(&destination).unwrap())
        .expect("la salida debe ser JSON válido");
    let rows = value.as_array().expect("la salida debe ser un arreglo");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["city"], "Santo Domingo");
    assert_eq!(rows[1]["city"], "Santiago");
    fs::remove_file(source).expect("se debe limpiar el CSV temporal");
}

#[test]
fn exports_a_portable_sql_script_with_escaped_values_and_nulls() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("name".into(), &["O'Brien", "Ana"]).into_column(),
            Series::new("total".into(), &[Some(10_i64), None]).into_column(),
            Series::new("active".into(), &[true, false]).into_column(),
        ],
    )
    .expect("el frame tipado debe ser válido");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("resultado.sql");

    export_frame_atomic(&frame, &destination, ExportFormat::Sql, |_, _| {}, || false)
        .expect("SQL debe exportarse");

    let script = fs::read_to_string(&destination).expect("se debe leer SQL");
    assert!(script.contains("CREATE TABLE \"dataset\""));
    assert!(script.contains("\"name\" TEXT"));
    assert!(script.contains("\"total\" BIGINT"));
    assert!(script.contains("'O''Brien'"));
    assert!(script.contains("NULL"));
    assert!(script.contains("TRUE"));
    assert!(script.contains("BEGIN TRANSACTION;"));
    assert!(script.contains("COMMIT;"));
}

#[test]
fn exports_a_real_xlsx_with_safe_inline_strings() {
    let frame = df![
        "name" => &["A&B", "=SUM(A1:A2)"],
        "count" => &[1_i64, 2_i64]
    ]
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resultado.xlsx");
    export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Excel,
        |_, _| {},
        || false,
    )
    .expect("Excel debe publicarse");

    let bytes = fs::read(&destination).unwrap();
    assert_eq!(&bytes[..2], b"PK");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut sheet = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet)
        .unwrap();
    assert!(sheet.contains("A&amp;B"));
    assert!(sheet.contains("=SUM(A1:A2)"));
    assert!(sheet.contains("t=\"inlineStr\""));
    let mut workbook = open_workbook_auto(&destination).expect("Excel debe poder reabrirse");
    let range = workbook
        .worksheet_range("dataset")
        .expect("la hoja dataset debe existir");
    assert_eq!(
        range.get((0, 0)).map(ToString::to_string).as_deref(),
        Some("name")
    );
    assert_eq!(
        range.get((1, 0)).map(ToString::to_string).as_deref(),
        Some("A&B")
    );
}

#[test]
fn exports_a_typed_sqlite_database_atomically() {
    let frame = df![
        "name" => &[Some("Santo Domingo"), None],
        "count" => &[Some(2_i64), Some(3_i64)]
    ]
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resultado.sqlite");
    export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Sqlite,
        |_, _| {},
        || false,
    )
    .expect("SQLite debe publicarse");

    let connection = Connection::open(&destination).unwrap();
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM dataset", [], |row| row.get(0))
        .unwrap();
    let nullable: Option<String> = connection
        .query_row("SELECT name FROM dataset WHERE count = 3", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(nullable, None);
}

#[test]
fn exports_bundle_with_dataset_dictionary_quality_and_manifest_hashes() {
    let frame = df![
        "email" => &[Some("ana@example.com"), None],
        "count" => &[Some(2_i64), Some(3_i64)]
    ]
    .unwrap();
    let validation = QualityValidationResult {
        passed: true,
        row_count: 2,
        total_rules: 0,
        failed_rules: 0,
        rules: Vec::new(),
    };
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resultado.zip");

    export_frame_atomic_with_privacy_and_quality(
        &frame,
        &destination,
        ExportFormat::Bundle,
        PrivacyMode::Mask,
        Some(&validation),
        |_, _| {},
        || false,
    )
    .expect("el bundle debe publicarse");

    let bytes = fs::read(&destination).unwrap();
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for name in [
        "dataset.csv",
        "dictionary.json",
        "quality-report.json",
        "manifest.json",
    ] {
        assert!(archive.by_name(name).is_ok(), "falta {name} en el bundle");
    }
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let manifest: JsonValue = serde_json::from_str(&manifest).unwrap();
    assert_eq!(manifest["format"], "columnia-bundle");
    assert_eq!(manifest["datasetFile"], "dataset.csv");
    assert_eq!(manifest["rowCount"], 2);
    assert_eq!(manifest["columnCount"], 2);
    assert!(manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "quality-report.json"));
    let mut dataset = String::new();
    archive
        .by_name("dataset.csv")
        .unwrap()
        .read_to_string(&mut dataset)
        .unwrap();
    assert!(dataset.contains("[REDACTED]"));
    assert!(!dataset.contains("ana@example.com"));
}

#[test]
fn exports_validated_recipe_with_manifest_reference_and_hash() {
    let frame = df!["amount" => &[Some(2_i64), Some(3_i64)]].unwrap();
    let recipe = complete_stored_recipe();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resultado-con-receta.zip");

    export_frame_atomic_with_privacy_and_quality_and_recipe(
        &frame,
        &destination,
        ExportFormat::Bundle,
        PrivacyMode::None,
        None,
        Some(&recipe),
        |_, _| {},
        || false,
    )
    .expect("el bundle debe incluir la receta válida");

    let bytes = fs::read(&destination).unwrap();
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut recipe_json = String::new();
    archive
        .by_name("recipe.json")
        .expect("falta recipe.json en el bundle")
        .read_to_string(&mut recipe_json)
        .unwrap();
    let recipe_value: JsonValue = serde_json::from_str(&recipe_json).unwrap();
    assert_eq!(recipe_value["name"], "Limpieza completa");
    assert_eq!(recipe_value["version"], RECIPE_FILE_VERSION);

    let mut manifest_json = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest_json)
        .unwrap();
    let manifest: JsonValue = serde_json::from_str(&manifest_json).unwrap();
    assert_eq!(manifest["recipeFile"], "recipe.json");
    let expected_hash = format!("{:x}", Sha256::digest(recipe_json.as_bytes()));
    assert!(manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "recipe.json" && file["sha256"] == expected_hash));
}

#[test]
fn privacy_modes_mask_or_hash_detect_all_detected_columns_without_values() {
    let frame = df![
        "email" => &["ana@example.com"],
        "city" => &["Santo Domingo"],
        "identifier" => &[42_i64]
    ]
    .unwrap();

    let (masked, protected_columns) = privacy_safe_frame(&frame, PrivacyMode::Mask).unwrap();
    assert_eq!(protected_columns, vec!["email", "identifier"]);
    assert_eq!(
        masked.column("email").unwrap().str().unwrap().get(0),
        Some("[REDACTED]")
    );
    assert_eq!(
        masked.column("city").unwrap().str().unwrap().get(0),
        Some("Santo Domingo")
    );
    assert_eq!(
        masked.column("identifier").unwrap().str().unwrap().get(0),
        Some("[REDACTED]")
    );

    let (hashed, _) = privacy_safe_frame(&frame, PrivacyMode::Hash).unwrap();
    let hashed_value = hashed
        .column("email")
        .unwrap()
        .str()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(hashed_value.len(), 64);
    assert_ne!(hashed_value, "ana@example.com");
    let (unprotected, protected_columns) = privacy_safe_frame(&frame, PrivacyMode::None).unwrap();
    assert_eq!(unprotected, frame);
    assert!(protected_columns.is_empty());
}

#[test]
fn source_backed_privacy_snapshot_masks_and_hashes_without_materializing_rows() {
    let source =
        temporary_csv("email,identifier,city\nana@example.com,42,Santo Domingo\n,7,Santiago\n");
    let directory = tempfile::tempdir().expect("se debe crear el destino temporal");
    let expected_size = fs::metadata(&source)
        .expect("la fuente source-backed debe existir")
        .len();
    let masked_path = directory.path().join("masked.parquet");
    let masked_columns = source_backed_privacy_snapshot(
        &source,
        expected_size,
        &masked_path,
        PrivacyMode::Mask,
        || false,
    )
    .expect("la protección source-backed debe crear un snapshot");
    assert_eq!(masked_columns, vec!["email", "identifier"]);
    let masked = read_parquet_frame(&masked_path).expect("el snapshot protegido debe abrir");
    assert_eq!(masked.height(), 2);
    assert_eq!(
        masked.column("email").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert!(masked.column("email").unwrap().get(1).unwrap().is_null());
    assert_eq!(
        masked.column("identifier").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        masked.column("city").unwrap().str().unwrap().get(0),
        Some("Santo Domingo")
    );

    let hashed_path = directory.path().join("hashed.parquet");
    let hashed_columns = source_backed_privacy_snapshot(
        &source,
        expected_size,
        &hashed_path,
        PrivacyMode::Hash,
        || false,
    )
    .expect("el hash source-backed debe crear un snapshot");
    assert_eq!(hashed_columns, masked_columns);
    let hashed = read_parquet_frame(&hashed_path).expect("el snapshot hash debe abrir");
    let expected_email_hash = format!("{:x}", Sha256::digest(b"ana@example.com"));
    let expected_identifier_hash = format!("{:x}", Sha256::digest(b"42"));
    assert_eq!(
        hashed.column("email").unwrap().str().unwrap().get(0),
        Some(expected_email_hash.as_str())
    );
    assert!(hashed.column("email").unwrap().get(1).unwrap().is_null());
    assert_eq!(
        hashed.column("identifier").unwrap().str().unwrap().get(0),
        Some(expected_identifier_hash.as_str())
    );
    assert!(source.is_file());
    fs::remove_file(source).expect("se debe limpiar la fuente temporal");
}

#[test]
fn compares_multiset_rows_and_reports_schema_differences_before_consolidation() {
    let current_path = temporary_csv("id,city\n1,Santo Domingo\n2,Santiago\n");
    let compared_path = temporary_csv("id,city\n2,Santiago\n3,La Vega\n");
    let (current, _) = load_csv(&current_path).expect("el dataset activo debe cargar");
    let (compared, _) = load_csv(&compared_path).expect("el dataset comparado debe cargar");

    let result = compare_frames(&current, "activo.csv", &compared, "comparado.csv", &[])
        .expect("la comparación debe calcularse");
    assert_eq!(result.common_row_count, 1);
    assert_eq!(result.current_only_row_count, 1);
    assert_eq!(result.compared_only_row_count, 1);
    assert_eq!(result.shared_columns, vec!["id", "city"]);
    assert!(result.schema_compatible);
    assert!(result.key_columns.is_empty());
    assert!(result.can_consolidate);

    let mut consolidated = current.clone();
    consolidated
        .vstack_mut(&compared)
        .expect("los esquemas compatibles deben consolidarse");
    assert_eq!(consolidated.height(), 4);

    fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
    fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
}

#[test]
fn compares_full_row_multisets_by_partitioned_signatures() {
    let current_count = LOCAL_QUERY_BLOCK_ROWS * 2 + 3;
    let compared_count = LOCAL_QUERY_BLOCK_ROWS + 11;
    let current_ids = (0..current_count)
        .map(|index| (index % 11) as i64)
        .collect::<Vec<_>>();
    let compared_ids = (0..compared_count)
        .map(|index| ((index + 3) % 11) as i64)
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        current_count,
        vec![Series::new("id".into(), current_ids.clone()).into_column()],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        compared_count,
        vec![Series::new("id".into(), compared_ids.clone()).into_column()],
    )
    .expect("el frame comparado debe ser válido");

    let mut current_frequencies = HashMap::<i64, usize>::new();
    for id in current_ids {
        *current_frequencies.entry(id).or_default() += 1;
    }
    let mut compared_frequencies = HashMap::<i64, usize>::new();
    for id in compared_ids {
        *compared_frequencies.entry(id).or_default() += 1;
    }
    let expected_common = current_frequencies
        .iter()
        .map(|(id, count)| (*count).min(compared_frequencies.get(id).copied().unwrap_or(0)))
        .sum::<usize>();

    let comparison = compare_frames(&current, "activo.csv", &compared, "comparado.csv", &[])
        .expect("la comparación completa por cubetas debe calcularse");
    assert_eq!(comparison.common_row_count, expected_common);
    assert_eq!(
        comparison.current_only_row_count,
        current_count - expected_common
    );
    assert_eq!(
        comparison.compared_only_row_count,
        compared_count - expected_common
    );
}

#[test]
fn compares_explicit_keys_and_reports_conflicts_and_duplicate_keys() {
    let current_path = temporary_csv("id,city,total\n1,Santo Domingo,10\n2,Santiago,20\n");
    let compared_path = temporary_csv("id,city,total\n2,Santiago,25\n3,La Vega,30\n");
    let (current, _) = load_csv(&current_path).expect("el dataset activo debe cargar");
    let (compared, _) = load_csv(&compared_path).expect("el dataset comparado debe cargar");

    let result = compare_frames(
        &current,
        "activo.csv",
        &compared,
        "comparado.csv",
        &["id".to_owned()],
    )
    .expect("la comparación por clave debe calcularse");
    assert_eq!(result.key_columns, vec!["id"]);
    assert_eq!(result.matched_key_count, 1);
    assert_eq!(result.current_only_key_count, 1);
    assert_eq!(result.compared_only_key_count, 1);
    assert_eq!(result.conflicting_key_count, 1);
    assert_eq!(result.duplicate_key_count, 0);
    assert!(!result.can_consolidate);

    let duplicate_path =
        temporary_csv("id,city,total\n2,Santiago,20\n2,Santiago,20\n3,La Vega,30\n");
    let (duplicate, _) = load_csv(&duplicate_path).expect("el dataset duplicado debe cargar");
    let duplicate_result = compare_frames(
        &current,
        "activo.csv",
        &duplicate,
        "duplicado.csv",
        &["id".to_owned()],
    )
    .expect("la comparación duplicada debe calcularse");
    assert_eq!(duplicate_result.duplicate_key_count, 1);
    assert!(!duplicate_result.can_consolidate);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].key, vec![Some("2".to_owned())]);
    assert_eq!(result.conflicts[0].cells[0].column, "total");
    assert_eq!(result.conflicts[0].cells[0].current, Some("20".to_owned()));
    assert_eq!(result.conflicts[0].cells[0].compared, Some("25".to_owned()));

    fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
    fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
    fs::remove_file(duplicate_path).expect("se debe limpiar el CSV duplicado");
}

#[test]
fn comparison_snapshot_round_trips_and_cleans_up_with_its_owner() {
    let frame = df![
        "id" => &[1_i64, 2],
        "label" => &[Some("uno"), None::<&str>]
    ]
    .expect("el frame comparado debe ser válido");
    let (directory, path) =
        persist_comparison_snapshot(&frame).expect("el snapshot comparado debe persistirse");

    assert!(path.is_file());
    let restored = read_parquet_frame(&path).expect("el snapshot debe poder restaurarse");
    assert!(restored.equals_missing(&frame));

    drop(directory);
    assert!(!path.exists());
}

#[test]
fn compares_parquet_source_by_blocks_without_materializing_the_compared_frame() {
    let compared_row_count = LOCAL_QUERY_BLOCK_ROWS + 3;
    let mut compared_ids = (0..compared_row_count as i64).collect::<Vec<_>>();
    let mut compared_values = compared_ids
        .iter()
        .map(|id| format!("comparado-{id}"))
        .collect::<Vec<_>>();
    compared_ids[compared_row_count - 1] = 2;
    compared_values[compared_row_count - 1] = "comparado-duplicado".to_owned();
    let compared = DataFrame::new(
        compared_row_count,
        vec![
            Series::new("id".into(), compared_ids).into_column(),
            Series::new("value".into(), compared_values).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let boundary_id = LOCAL_QUERY_BLOCK_ROWS as i64;
    let current = df![
        "id" => &[2_i64, boundary_id, boundary_id + 1],
        "value" => &["activo-2", "activo-boundary", "activo-last"]
    ]
    .expect("el frame activo debe ser válido");
    let key_columns = vec!["id".to_owned()];
    let (source_directory, source_path) =
        persist_comparison_snapshot(&compared).expect("la fuente Parquet debe poder escribirse");
    let (snapshot_directory, snapshot_path) = persist_comparison_source_file(&source_path)
        .expect("la fuente Parquet debe poder copiarse sin materializarla");
    assert_eq!(
        fs::read(&snapshot_path).unwrap(),
        fs::read(&source_path).unwrap()
    );

    let expected = compare_frames(
        &current,
        "activo.parquet",
        &compared,
        "comparado.parquet",
        &key_columns,
    )
    .expect("la comparación materializada de referencia debe calcularse");
    let actual = compare_parquet_source(
        &current,
        "activo.parquet",
        &snapshot_path,
        "comparado.parquet",
        parquet_row_count(&snapshot_path).expect("el conteo Parquet debe calcularse"),
        &key_columns,
    )
    .expect("la comparación Parquet por bloques debe calcularse");

    assert_eq!(actual, expected);
    assert_eq!(actual.compared_row_count, compared_row_count);
    assert_eq!(actual.conflicts.len(), 2);

    drop(snapshot_directory);
    drop(source_directory);
    assert!(!snapshot_path.exists());
    assert!(!source_path.exists());
}

#[test]
fn compares_two_parquet_sources_without_materializing_either_frame() {
    let current = df![
        "id" => &[1_i64, 2],
        "value" => &["activo-1", "igual"]
    ]
    .expect("el frame activo debe ser válido");
    let compared = df![
        "id" => &[1_i64, 2],
        "value" => &["comparado-1", "igual"]
    ]
    .expect("el frame comparado debe ser válido");
    let (current_directory, current_path) =
        persist_comparison_snapshot(&current).expect("el snapshot activo debe poder escribirse");
    let (compared_directory, compared_path) = persist_comparison_snapshot(&compared)
        .expect("el snapshot comparado debe poder escribirse");

    let result = compare_parquet_sources(
        &current_path,
        "activo.parquet",
        current.height(),
        &compared_path,
        "comparado.parquet",
        compared.height(),
        &["id".to_owned()],
    )
    .expect("la comparación entre snapshots debe calcularse");

    assert_eq!(result.common_row_count, 1);
    assert_eq!(result.current_only_row_count, 1);
    assert_eq!(result.compared_only_row_count, 1);
    assert_eq!(result.matched_key_count, 2);
    assert_eq!(result.conflicting_key_count, 1);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].key, vec![Some("1".to_owned())]);
    assert_eq!(result.conflicts[0].cells[0].column, "value");

    drop(compared_directory);
    drop(current_directory);
    assert!(!current_path.exists());
    assert!(!compared_path.exists());
}

#[test]
fn compares_delimited_source_through_a_parquet_snapshot_without_changing_values() {
    let compared_path =
        temporary_csv("id,city,total\n1,Santo Domingo,010\n2,Santiago,020\n3,La Vega,030\n");
    let current_path = temporary_csv("id,city,total\n1,Santo Domingo,010\n2,Santiago,025\n");
    let (current, _) = load_csv(&current_path).expect("el dataset activo debe cargar");
    let (compared, _) = load_csv(&compared_path).expect("el dataset comparado debe cargar");
    let (directory, snapshot_path) =
        persist_delimited_comparison_source_file(&compared_path, "csv")
            .expect("el CSV debe convertirse al snapshot temporal");
    let restored = read_parquet_frame(&snapshot_path).expect("el snapshot debe ser legible");
    assert!(restored.equals_missing(&compared));

    let expected = compare_frames(
        &current,
        "activo.csv",
        &compared,
        "comparado.csv",
        &["id".to_owned()],
    )
    .expect("la comparación CSV de referencia debe calcularse");
    let actual = compare_parquet_source(
        &current,
        "activo.csv",
        &snapshot_path,
        "comparado.csv",
        parquet_row_count(&snapshot_path).expect("el snapshot debe contar sus filas"),
        &["id".to_owned()],
    )
    .expect("la comparación del snapshot CSV debe calcularse");
    assert_eq!(actual, expected);

    drop(directory);
    fs::remove_file(current_path).expect("se debe limpiar el CSV activo");
    fs::remove_file(compared_path).expect("se debe limpiar el CSV comparado");
}

#[test]
fn compares_json_source_through_a_parquet_snapshot_without_changing_values() {
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let compared_path = directory.path().join("compared.json");
    fs::write(
        &compared_path,
        r#"[{"id":1,"city":"Santo Domingo","total":10,"metadata":{"tier":"gold"},"tags":["a","b"]},{"id":2,"city":"Santiago","total":20,"metadata":{"tier":"silver"},"tags":["b"]},{"id":3,"city":"La Vega","total":30,"metadata":{"tier":"bronze"},"tags":[]}]"#,
    )
    .expect("se debe escribir el JSON comparado");
    let current = df![
        "id" => &[1_i64, 2],
        "city" => &["Santo Domingo", "Santiago"],
        "total" => &[10_i64, 25],
        "metadata" => &[r#"{"tier":"gold"}"#, r#"{"tier":"silver"}"#],
        "tags" => &[r#"["a","b"]"#, r#"["b"]"#]
    ]
    .expect("el dataset activo debe construirse");
    let compared = load_json_records(&compared_path).expect("el JSON debe cargar");
    let (snapshot_directory, snapshot_path) = persist_json_comparison_source_file(&compared_path)
        .expect("el JSON debe convertirse al snapshot temporal");
    let restored = read_parquet_frame(&snapshot_path).expect("el snapshot debe ser legible");
    let restored_in_source_order = restored
        .select(compared.get_column_names())
        .expect("el snapshot debe conservar todas las columnas");
    assert!(restored_in_source_order.equals_missing(&compared));

    let expected = compare_frames(
        &current,
        "activo.json",
        &compared,
        "comparado.json",
        &["id".to_owned()],
    )
    .expect("la comparación JSON de referencia debe calcularse");
    let actual = compare_parquet_source(
        &current,
        "activo.json",
        &snapshot_path,
        "comparado.json",
        parquet_row_count(&snapshot_path).expect("el snapshot debe contar sus filas"),
        &["id".to_owned()],
    )
    .expect("la comparación del snapshot JSON debe calcularse");
    assert_eq!(actual, expected);

    drop(snapshot_directory);
    assert!(!snapshot_path.exists());
}

#[test]
fn comparison_signatures_merge_fixed_blocks_without_changing_counts() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS * 2 + 3;
    let ids = (0..row_count)
        .map(|index| (index % 7) as i64)
        .collect::<Vec<_>>();
    let frame = DataFrame::new(row_count, vec![Series::new("id".into(), ids).into_column()])
        .expect("el frame grande de comparación debe ser válido");
    let columns = vec!["id".to_owned()];

    let signature_spill = spill_key_rows(&frame, &columns).expect("las firmas deben derramarse");
    let signatures =
        collect_spilled_signature_counts(&signature_spill).expect("las firmas deben fusionarse");
    let key_spill = spill_key_rows(&frame, &columns).expect("las claves deben derramarse");
    let keys = collect_spilled_key_rows(&key_spill).expect("las claves deben fusionarse");

    assert_eq!(signatures.len(), 7);
    assert_eq!(keys.len(), 7);
    assert_eq!(signatures.values().sum::<usize>(), row_count);
    assert_eq!(keys.values().map(Vec::len).sum::<usize>(), row_count);
}

#[test]
fn keyed_conflicts_keep_current_row_order_across_spilled_blocks() {
    let row_count = LOCAL_QUERY_BLOCK_ROWS + 2;
    let ids = (0..row_count as i64).collect::<Vec<_>>();
    let current_values = (0..row_count)
        .map(|index| format!("current-{index}"))
        .collect::<Vec<_>>();
    let compared_values = (0..row_count)
        .map(|index| {
            if index == 0 || index == row_count - 1 {
                format!("compared-{index}")
            } else {
                format!("current-{index}")
            }
        })
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids.clone()).into_column(),
            Series::new("value".into(), current_values).into_column(),
        ],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        row_count,
        vec![
            Series::new("id".into(), ids).into_column(),
            Series::new("value".into(), compared_values).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let key_columns = vec!["id".to_owned()];
    let comparison = compare_frames(
        &current,
        "activo.csv",
        &compared,
        "comparado.csv",
        &key_columns,
    )
    .expect("la comparación por bloques debe calcularse");

    assert_eq!(comparison.conflicting_key_count, 2);
    assert_eq!(comparison.conflicts[0].key, vec![Some("0".to_owned())]);
    assert_eq!(
        comparison.conflicts[1].key,
        vec![Some((row_count as i64 - 1).to_string())]
    );
    let (last_page, has_next) = collect_key_conflicts_page(
        &current,
        &compared,
        &key_columns,
        &["id".to_owned(), "value".to_owned()],
        1,
        1,
    )
    .expect("la segunda página debe conservar el orden de filas");
    assert!(!has_next);
    assert_eq!(last_page[0].conflict.key, comparison.conflicts[1].key);
}

#[test]
fn keyed_consolidation_only_appends_rows_with_new_keys() {
    let current = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[1_i64, 2]).into_column(),
            Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
        ],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[2_i64, 3]).into_column(),
            Series::new("city".into(), &["Santiago", "La Vega"]).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");

    let additions = rows_with_new_keys(&current, &compared, &["id".to_owned()])
        .expect("se deben seleccionar las claves nuevas");
    assert_eq!(additions.height(), 1);
    assert_eq!(
        additions.column("id").unwrap().i64().unwrap().get(0),
        Some(3)
    );
}

#[test]
fn resolves_key_conflicts_by_column_and_keeps_legacy_row_decisions() {
    let current = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[1_i64, 2]).into_column(),
            Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
            Series::new("total".into(), &[10_i64, 20]).into_column(),
        ],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[1_i64, 2]).into_column(),
            Series::new("city".into(), &["La Vega", "Santiago"]).into_column(),
            Series::new("total".into(), &[15_i64, 20]).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let key_columns = vec!["id".to_owned()];
    let shared_columns = vec!["id".to_owned(), "city".to_owned(), "total".to_owned()];
    let (conflicts, truncated) =
        collect_key_conflicts(&current, &compared, &key_columns, &shared_columns)
            .expect("los conflictos deben poder inspeccionarse");
    assert!(!truncated);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict.key, vec![Some("1".to_owned())]);
    assert_eq!(conflicts[0].conflict.cells.len(), 2);

    let resolved = resolved_conflict_frame(
        &current,
        &compared,
        &key_columns,
        &[
            ConflictResolution {
                conflict_index: 0,
                column: Some("city".to_owned()),
                source: ConflictSource::Compared,
            },
            ConflictResolution {
                conflict_index: 0,
                column: Some("total".to_owned()),
                source: ConflictSource::Current,
            },
        ],
    )
    .expect("la resolución debe construir un frame válido");
    assert_eq!(
        resolved.column("id").unwrap().i64().unwrap().get(0),
        Some(1)
    );
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(0),
        Some("La Vega")
    );
    assert_eq!(
        resolved.column("total").unwrap().i64().unwrap().get(0),
        Some(10)
    );
    assert_eq!(
        resolved.column("city").unwrap().str().unwrap().get(1),
        Some("Santiago")
    );
    let legacy_resolved = resolved_conflict_frame(
        &current,
        &compared,
        &key_columns,
        &[ConflictResolution {
            conflict_index: 0,
            column: None,
            source: ConflictSource::Compared,
        }],
    )
    .expect("la resolución legacy por fila debe seguir funcionando");
    assert_eq!(
        legacy_resolved
            .column("total")
            .unwrap()
            .i64()
            .unwrap()
            .get(0),
        Some(15)
    );
    assert!(resolved_conflict_frame(&current, &compared, &key_columns, &[]).is_err());
    assert!(resolved_conflict_frame(
        &current,
        &compared,
        &key_columns,
        &[
            ConflictResolution {
                conflict_index: 0,
                column: None,
                source: ConflictSource::Current,
            },
            ConflictResolution {
                conflict_index: 0,
                column: None,
                source: ConflictSource::Compared,
            },
        ],
    )
    .is_err());
}

#[test]
fn paginates_and_resolves_conflicts_beyond_visible_preview_limit() {
    let ids = (0_i64..51).collect::<Vec<_>>();
    let current_values = ids
        .iter()
        .map(|id| format!("activo-{id}"))
        .collect::<Vec<_>>();
    let compared_values = ids
        .iter()
        .map(|id| format!("comparado-{id}"))
        .collect::<Vec<_>>();
    let current = DataFrame::new(
        ids.len(),
        vec![
            Series::new("id".into(), ids.clone()).into_column(),
            Series::new("value".into(), current_values).into_column(),
        ],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        ids.len(),
        vec![
            Series::new("id".into(), ids).into_column(),
            Series::new("value".into(), compared_values).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let key_columns = vec!["id".to_owned()];
    let result = compare_frames(
        &current,
        "activo.csv",
        &compared,
        "comparado.csv",
        &key_columns,
    )
    .expect("la comparación debe calcularse");

    assert_eq!(result.conflicting_key_count, 51);
    assert_eq!(result.conflicts.len(), 50);
    assert_eq!(result.conflict_offset, 0);
    assert!(result.conflicts_truncated);
    let (last_page, has_next) = collect_key_conflicts_page(
        &current,
        &compared,
        &key_columns,
        &["id".to_owned(), "value".to_owned()],
        50,
        MAX_CONFLICT_PREVIEW,
    )
    .expect("la segunda página debe poder calcularse");
    assert_eq!(last_page.len(), 1);
    assert!(!has_next);
    assert_eq!(last_page[0].conflict.key, vec![Some("50".to_owned())]);

    let decisions = (0..51)
        .map(|conflict_index| ConflictResolution {
            conflict_index,
            column: Some("value".to_owned()),
            source: ConflictSource::Compared,
        })
        .collect::<Vec<_>>();
    let resolved = resolved_conflict_frame(&current, &compared, &key_columns, &decisions)
        .expect("la resolución completa debe aceptar todas las páginas");
    assert_eq!(
        resolved.column("value").unwrap().str().unwrap().get(0),
        Some("comparado-0")
    );
    assert_eq!(
        resolved.column("value").unwrap().str().unwrap().get(50),
        Some("comparado-50")
    );
}

#[test]
fn paginates_parquet_conflicts_by_blocks_and_preserves_cross_block_duplicates() {
    let compared_row_count = LOCAL_QUERY_BLOCK_ROWS + 3;
    let mut compared_ids = (0..compared_row_count as i64).collect::<Vec<_>>();
    let mut compared_values = compared_ids
        .iter()
        .map(|id| format!("comparado-{id}"))
        .collect::<Vec<_>>();
    compared_ids[compared_row_count - 1] = 2;
    compared_values[compared_row_count - 1] = "comparado-duplicado".to_owned();
    let compared = DataFrame::new(
        compared_row_count,
        vec![
            Series::new("id".into(), compared_ids).into_column(),
            Series::new("value".into(), compared_values).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let boundary_id = LOCAL_QUERY_BLOCK_ROWS as i64;
    let current = df![
        "id" => &[2_i64, boundary_id, boundary_id + 1],
        "value" => &["activo-2", "activo-boundary", "activo-last"]
    ]
    .expect("el frame activo debe ser válido");
    let key_columns = vec!["id".to_owned()];
    let shared_columns = vec!["id".to_owned(), "value".to_owned()];
    let (expected, expected_has_next) = collect_key_conflicts_page(
        &current,
        &compared,
        &key_columns,
        &shared_columns,
        0,
        MAX_CONFLICT_PREVIEW,
    )
    .expect("la página en memoria debe poder calcularse");
    let (directory, compared_path) = persist_comparison_snapshot(&compared)
        .expect("el snapshot comparado debe poder persistirse");
    let (actual, actual_has_next) = collect_key_conflicts_page_from_parquet(
        &current,
        &compared_path,
        compared_row_count,
        &key_columns,
        &shared_columns,
        0,
        MAX_CONFLICT_PREVIEW,
    )
    .expect("la página Parquet debe poder calcularse por bloques");

    assert_eq!(actual_has_next, expected_has_next);
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        assert_eq!(actual.current_row_index, expected.current_row_index);
        assert_eq!(actual.compared_row_index, expected.compared_row_index);
        assert_eq!(actual.conflict, expected.conflict);
    }
    assert_eq!(
        actual
            .iter()
            .map(|item| item.conflict.key.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![Some(boundary_id.to_string())],
            vec![Some((boundary_id + 1).to_string())],
        ]
    );
    let stale_snapshot = collect_key_conflicts_page_from_parquet(
        &current,
        &compared_path,
        compared_row_count + 1,
        &key_columns,
        &shared_columns,
        0,
        MAX_CONFLICT_PREVIEW,
    );
    assert!(matches!(
        stale_snapshot,
        Err(error) if error.starts_with(LOCAL_QUERY_SNAPSHOT_ERROR_PREFIX)
    ));

    drop(directory);
    assert!(!compared_path.exists());
}

#[test]
fn joins_frames_by_key_for_inner_left_and_full_relations() {
    let current = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[1_i64, 2]).into_column(),
            Series::new("city".into(), &["Santo Domingo", "Santiago"]).into_column(),
        ],
    )
    .expect("el frame activo debe ser válido");
    let compared = DataFrame::new(
        2,
        vec![
            Series::new("id".into(), &[2_i64, 3]).into_column(),
            Series::new("city".into(), &["Santiago", "La Vega"]).into_column(),
            Series::new("segment".into(), &["B", "C"]).into_column(),
        ],
    )
    .expect("el frame comparado debe ser válido");
    let key = ["id".to_owned()];

    let inner = join_frames(&current, &compared, &key, DatasetJoinType::Inner)
        .expect("la unión inner debe calcularse");
    let left = join_frames(&current, &compared, &key, DatasetJoinType::Left)
        .expect("la unión left debe calcularse");
    let full = join_frames(&current, &compared, &key, DatasetJoinType::Full)
        .expect("la unión full debe calcularse");

    assert_eq!(inner.height(), 1);
    assert_eq!(left.height(), 2);
    assert_eq!(full.height(), 3);
    assert!(inner
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "city_right"));
    assert!(left
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "segment"));
    assert!(full
        .get_column_names()
        .iter()
        .any(|name| name.as_str() == "segment"));
}

#[test]
fn cancellation_keeps_the_previous_export_untouched() {
    use std::cell::Cell;

    let source = temporary_csv("value\n1\n2\n");
    let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let destination = directory.path().join("resultado.csv");
    fs::write(&destination, "exportación anterior")
        .expect("se debe preparar la exportación anterior");
    let checks = Cell::new(0);

    let error = export_frame_atomic(
        &frame,
        &destination,
        ExportFormat::Csv,
        |_, _| {},
        || {
            checks.set(checks.get() + 1);
            checks.get() >= 2
        },
    )
    .expect_err("la exportación debe cancelarse antes de publicarse");

    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert_eq!(
        fs::read_to_string(destination).expect("el destino debe conservarse"),
        "exportación anterior"
    );
    fs::remove_file(source).expect("se debe limpiar el CSV temporal");
}

#[test]
fn counts_blank_text_and_unicode_characters() {
    let path = temporary_csv("text\n\"  \"\nCafé\nRepública\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let text = &profile.columns[0];

    assert_eq!(text.empty_count, Some(1));
    assert_eq!(text.minimum_length, Some(2));
    assert_eq!(text.maximum_length, Some(9));
    assert_eq!(text.average_length, Some(5.0));
    assert_eq!(text.suggested_type, None);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn suggests_dates_and_reports_values_that_do_not_match() {
    let path = temporary_csv(
        "date_added\n\"September 9, 2019\"\n\"September 10, 2019\"\n\"September 11, 2019\"\n\"September 12, 2019\"\n\"September 13, 2019\"\n\"September 14, 2019\"\n\"September 15, 2019\"\n\"September 16, 2019\"\n\"September 17, 2019\"\nunknown\n",
    );
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let date = &profile.columns[0];

    assert_eq!(date.suggested_type, Some("date".to_owned()));
    assert_eq!(date.type_match_percentage, Some(90.0));
    assert_eq!(date.invalid_type_count, Some(1));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn detects_numeric_outliers_with_the_iqr_rule() {
    let path = temporary_csv("value\n10\n11\n12\n13\n100\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    let value = &profile.columns[0];

    assert_eq!(value.first_quartile, Some(11.0));
    assert_eq!(value.median, Some(12.0));
    assert_eq!(value.third_quartile, Some(13.0));
    assert_eq!(value.outlier_count, Some(1));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn removes_only_additional_duplicate_rows_and_preserves_order() {
    let path = temporary_csv("city,value\nSanto Domingo,1\nSantiago,2\nSanto Domingo,1\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let (cleaned, affected_row_count) =
        remove_duplicate_rows(&frame).expect("los duplicados deben eliminarse");
    let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

    assert_eq!(affected_row_count, 1);
    assert_eq!(cleaned.height(), 2);
    assert_eq!(page.rows[0][0].as_deref(), Some("Santo Domingo"));
    assert_eq!(page.rows[1][0].as_deref(), Some("Santiago"));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn counts_normalized_near_duplicates_without_double_counting_exact_rows() {
    let frame = df![
        "city" => &["Santo Domingo", " santo   domingo ", "Santo Domingo", "Santiago"],
        "value" => &[1_i64, 1, 1, 2]
    ]
    .unwrap();

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");

    assert_eq!(profile.duplicate_row_count, 1);
    assert_eq!(profile.near_duplicate_row_count, 1);
}

#[test]
fn removes_near_duplicates_stably_without_removing_exact_repeats() {
    let frame = df![
        "city" => &[" Ana ", "Ana", " Ana ", "Luis", " luis "],
        "value" => &[1_i64, 1, 1, 2, 2]
    ]
    .unwrap();

    let (cleaned, affected_row_count) =
        remove_near_duplicate_rows(&frame).expect("los duplicados parecidos deben eliminarse");
    let cities = cleaned.column("city").unwrap().str().unwrap();

    assert_eq!(affected_row_count, 2);
    assert_eq!(cleaned.height(), 3);
    assert_eq!(cities.get(0), Some(" Ana "));
    assert_eq!(cities.get(1), Some(" Ana "));
    assert_eq!(cities.get(2), Some("Luis"));
}

#[test]
fn near_duplicate_removal_publishes_a_reversible_history_entry() {
    let path = temporary_csv("city,value\n Ana ,1\nAna,1\n Ana ,1\nLuis,2\n luis ,2\n");
    let (original, _) = load_csv(&path).expect("el CSV debe cargar");
    let (cleaned, affected_row_count) = remove_near_duplicate_rows(&original)
        .expect("los duplicados parecidos deben poder eliminarse");
    assert_eq!(affected_row_count, 2);

    let mut dataset = loaded_dataset(path.clone(), original);
    let preview = publish_candidate(&mut dataset, cleaned, "Eliminar filas duplicadas parecidas")
        .expect("la mutación debe publicarse");

    assert_eq!(preview.row_count, 3);
    assert_eq!(
        dataset.history.entries[1].label,
        "Eliminar filas duplicadas parecidas"
    );
    let undone = undo_dataset(&mut dataset).expect("la mutación debe poder deshacerse");
    assert_eq!(undone.dataset.row_count, 5);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn removes_only_rows_that_are_completely_empty() {
    let frame = df![
        "name" => &[Some("Ana"), Some(""), None, Some("  ")],
        "amount" => &[Some(10_i64), None, None, None]
    ]
    .unwrap();

    let (cleaned, affected_row_count) =
        remove_empty_rows_from_frame(&frame).expect("las filas vacías deben eliminarse");
    assert_eq!(affected_row_count, 3);
    assert_eq!(cleaned.height(), 1);
    assert_eq!(
        dataset_page(&cleaned, 0, 10).unwrap().rows[0][0].as_deref(),
        Some("Ana")
    );
}

#[test]
fn removes_constant_columns_but_preserves_a_usable_dataset() {
    let frame = df![
        "id" => &[1_i64, 2, 3],
        "constant" => &[Some("activo"), None, Some("activo")],
        "all_null" => &[None::<String>, None, None]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_constant_columns_from_frame(&frame)
        .expect("las columnas constantes deben poder eliminarse");
    assert_eq!(removed_columns, vec!["constant"]);
    assert_eq!(cleaned.get_column_names(), vec!["id", "all_null"]);
    assert_eq!(cleaned.height(), 3);

    let all_constant = df![
        "first" => &["same", "same"],
        "second" => &[1_i64, 1]
    ]
    .unwrap();
    let (kept, removed) = remove_constant_columns_from_frame(&all_constant)
        .expect("el dataset debe conservar una columna");
    assert_eq!(removed.len(), 1);
    assert_eq!(kept.width(), 1);
    assert_eq!(kept.height(), 2);
}

#[test]
fn removes_only_completely_empty_columns_and_keeps_one_column() {
    let frame = df![
        "id" => &[1_i64, 2],
        "empty" => &[None::<String>, None],
        "partial" => &[Some("ok"), None]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_empty_columns_from_frame(&frame)
        .expect("las columnas vacías deben poder eliminarse");
    assert_eq!(removed_columns, vec!["empty"]);
    assert_eq!(cleaned.get_column_names(), vec!["id", "partial"]);

    let all_empty = df![
        "first" => &[None::<String>, None],
        "second" => &[None::<i64>, None]
    ]
    .unwrap();
    let (kept, removed) =
        remove_empty_columns_from_frame(&all_empty).expect("el dataset debe conservar una columna");
    assert_eq!(removed.len(), 1);
    assert_eq!(kept.width(), 1);
    assert_eq!(kept.height(), 2);
}

#[test]
fn removes_detected_identifier_columns_but_keeps_other_personal_columns() {
    let frame = df![
        "customer_id" => &["a-1", "b-2"],
        "email" => &["ana@example.com", "luis@example.com"],
        "amount" => &[10_i64, 20]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&frame)
        .expect("las columnas identificadoras deben poder retirarse");
    assert_eq!(removed_columns, vec!["customer_id"]);
    assert_eq!(cleaned.get_column_names(), vec!["email", "amount"]);
    assert_eq!(cleaned.height(), 2);
}

#[test]
fn keeps_one_column_when_all_columns_are_identifiers() {
    let frame = df![
        "customer_id" => &["a-1", "b-2"],
        "order_id" => &["o-1", "o-2"]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_identifier_columns_from_frame(&frame)
        .expect("el dataset debe conservar una columna");
    assert_eq!(removed_columns, vec!["customer_id"]);
    assert_eq!(cleaned.get_column_names(), vec!["order_id"]);
    assert_eq!(cleaned.height(), 2);
}

#[test]
fn removes_personal_columns_by_category_without_touching_row_audit() {
    let frame = df![
        "email" => &["ana@example.com", "luis@example.com"],
        "phone" => &["555-0100", "555-0101"],
        "address" => &["Calle 1", "Calle 2"],
        "name" => &["Ana", "Luis"],
        "_cambios" => &[Some(""), Some("" )],
        "amount" => &[10_i64, 20]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_personal_columns_from_frame(&frame)
        .expect("las columnas personales deben poder retirarse");
    assert_eq!(removed_columns, vec!["email", "phone", "address", "name"]);
    assert_eq!(cleaned.get_column_names(), vec!["_cambios", "amount"]);
    assert_eq!(cleaned.height(), 2);
}

#[test]
fn masks_personal_values_without_touching_identifiers_or_row_audit() {
    let frame = df![
        "email" => &[Some("ana@example.com"), None::<&str>],
        "phone" => &["555-0100", "555-0101"],
        "address" => &["Calle 1", "Calle 2"],
        "name" => &["Ana", "Luis"],
        "customer_id" => &["a-1", "b-2"],
        "_cambios" => &[Some(""), Some("" )],
        "amount" => &[10_i64, 20]
    ]
    .unwrap();

    let (masked, changed_cell_count, changed_column_count) =
        mask_personal_values_from_frame(&frame)
            .expect("los valores personales deben poder protegerse");
    assert_eq!(changed_cell_count, 7);
    assert_eq!(changed_column_count, 4);
    assert_eq!(
        masked.column("email").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(masked.column("email").unwrap().str().unwrap().get(1), None);
    assert_eq!(
        masked.column("phone").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        masked.column("address").unwrap().str().unwrap().get(1),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        masked.column("name").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        masked.column("customer_id").unwrap().str().unwrap().get(0),
        Some("a-1")
    );
    assert_eq!(
        masked.column("_cambios").unwrap().str().unwrap().get(0),
        Some("")
    );
    assert_eq!(
        masked.column("amount").unwrap().i64().unwrap().get(0),
        Some(10)
    );
}

#[test]
fn masking_personal_values_is_idempotent_for_redacted_and_null_cells() {
    let frame = df![
        "email" => &[Some(REDACTED_VALUE), None::<&str>],
        "name" => &[Some("Luis"), None::<&str>],
        "amount" => &[1_i64, 2]
    ]
    .unwrap();

    let (masked, changed_cell_count, changed_column_count) =
        mask_personal_values_from_frame(&frame)
            .expect("la máscara debe poder repetirse sin cambiar lo ya protegido");
    assert_eq!(changed_cell_count, 1);
    assert_eq!(changed_column_count, 1);
    assert_eq!(
        masked.column("email").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
    assert_eq!(
        masked.column("name").unwrap().str().unwrap().get(0),
        Some(REDACTED_VALUE)
    );
}

#[test]
fn keeps_one_personal_column_when_all_usable_columns_are_personal() {
    let frame = df![
        "email" => &["ana@example.com", "luis@example.com"],
        "nombre" => &["Ana", "Luis"]
    ]
    .unwrap();

    let (cleaned, removed_columns) =
        remove_personal_columns_from_frame(&frame).expect("el dataset debe conservar una columna");
    assert_eq!(removed_columns, vec!["email"]);
    assert_eq!(cleaned.get_column_names(), vec!["nombre"]);
    assert_eq!(cleaned.width(), 1);
}

#[test]
fn removes_columns_at_or_above_eighty_percent_null_but_not_empty_or_below_threshold() {
    let frame = df![
        "id" => &[1_i64, 2, 3, 4, 5],
        "high" => &[Some("ok"), None, None, None, None],
        "below" => &[Some("a"), Some("b"), None, None, None],
        "empty" => &[None::<String>, None, None, None, None]
    ]
    .unwrap();

    let (cleaned, removed_columns) = remove_high_null_columns_from_frame(&frame)
        .expect("las columnas con alta nulidad deben poder eliminarse");
    assert_eq!(removed_columns, vec!["high"]);
    assert_eq!(cleaned.get_column_names(), vec!["id", "below", "empty"]);

    let all_high = df![
        "first" => &[Some("ok"), None, None, None, None],
        "second" => &[Some("ok"), None, None, None, None]
    ]
    .unwrap();
    let (kept, removed) = remove_high_null_columns_from_frame(&all_high)
        .expect("el dataset debe conservar una columna");
    assert_eq!(removed.len(), 1);
    assert_eq!(kept.width(), 1);
    assert_eq!(kept.height(), 5);
}

#[test]
fn normalizes_known_text_sentinels_to_null_without_touching_other_types() {
    let frame = df![
        "status" => &[Some("N/A"), Some("Normal"), None, Some("Sin datos"), Some("n/a")],
        "amount" => &[1_i64, 2, 3, 4, 5]
    ]
    .unwrap();

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    assert_eq!(profile.columns[0].sentinel_count, Some(3));
    assert_eq!(profile.columns[1].sentinel_count, None);

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        clean_text_columns(&frame, None, TextCleaningMode::Sentinels)
            .expect("los centinelas deben poder normalizarse");
    let values = cleaned
        .column("status")
        .expect("la columna debe conservarse")
        .str()
        .expect("la columna debe seguir siendo texto");

    assert_eq!(affected_rows, 3);
    assert_eq!(changed_cells, 3);
    assert_eq!(changed_columns[0].name, "status");
    assert_eq!(values.get(0), None);
    assert_eq!(values.get(1), Some("Normal"));
    assert_eq!(values.get(2), None);
    assert_eq!(values.get(3), None);
    assert_eq!(values.get(4), None);
    assert_eq!(
        cleaned.column("amount").unwrap().i64().unwrap().get(0),
        Some(1)
    );
}

#[test]
fn repairs_unambiguous_utf8_mojibake_without_touching_other_types() {
    let frame = df![
        "city" => &[Some("BogotÃ¡"), Some("â€™"), Some("Santo Domingo"), None::<&str>],
        "amount" => &[1_i64, 2, 3, 4]
    ]
    .unwrap();

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    assert_eq!(profile.columns[0].encoding_issue_count, Some(2));
    assert_eq!(profile.columns[1].encoding_issue_count, None);

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        clean_text_columns(&frame, None, TextCleaningMode::FixEncoding)
            .expect("la codificación debe poder corregirse");
    let city = cleaned.column("city").unwrap().str().unwrap();

    assert_eq!(affected_rows, 2);
    assert_eq!(changed_cells, 2);
    assert_eq!(changed_columns[0].name, "city");
    assert_eq!(city.get(0), Some("Bogotá"));
    assert_eq!(city.get(1), Some("’"));
    assert_eq!(city.get(2), Some("Santo Domingo"));
    assert_eq!(city.get(3), None);
    assert_eq!(
        cleaned.column("amount").unwrap().i64().unwrap().get(0),
        Some(1)
    );
}

#[test]
fn nullifies_invalid_values_for_a_confident_suggested_type() {
    let frame = df![
        "created_at" => &[
            Some("2024-01-01"), Some("2024-01-02"), Some("2024-01-03"),
            Some("2024-01-04"), Some("2024-01-05"), Some("2024-01-06"),
            Some("2024-01-07"), Some("2024-01-08"), Some("2024-01-09"),
            Some("sin fecha"),
        ],
        "note" => &[Some("keep"), Some("keep"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave"), Some("leave")]
    ]
    .unwrap();

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    assert_eq!(profile.columns[0].suggested_type, Some("date".to_owned()));
    assert_eq!(profile.columns[0].invalid_type_count, Some(1));

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        clean_text_columns(&frame, None, TextCleaningMode::NullifyInvalidTypes)
            .expect("los tipos incompatibles deben poder apartarse");
    let dates = cleaned.column("created_at").unwrap().str().unwrap();
    let notes = cleaned.column("note").unwrap().str().unwrap();

    assert_eq!(affected_rows, 1);
    assert_eq!(changed_cells, 1);
    assert_eq!(changed_columns[0].name, "created_at");
    assert_eq!(dates.get(0), Some("2024-01-01"));
    assert_eq!(dates.get(9), None);
    assert_eq!(notes.get(0), Some("keep"));
}

#[test]
fn imputes_repeated_text_and_lower_median_numeric_nuls_only() {
    let frame = df![
        "status" => &[Some("ok"), None, Some("ok"), Some("review")],
        "amount" => &[Some(10_i64), None, Some(20), Some(30)],
        "notes" => &[Some(""), None, Some("draft"), Some("ready")]
    ]
    .unwrap();

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        impute_missing_values_in_frame(&frame)
            .expect("los nulos imputables deben poder completarse");
    let status = cleaned.column("status").unwrap().str().unwrap();
    let amount = cleaned.column("amount").unwrap().i64().unwrap();
    let notes = cleaned.column("notes").unwrap().str().unwrap();

    assert_eq!(affected_rows, 1);
    assert_eq!(changed_cells, 2);
    assert_eq!(changed_columns.len(), 2);
    assert_eq!(status.get(1), Some("ok"));
    assert_eq!(amount.get(1), Some(20));
    assert_eq!(notes.get(1), None);
}

#[test]
fn normalizes_boolean_aliases_and_profiles_privacy_signals() {
    let frame = df![
        "active" => &[Some("YES"), Some("no"), Some("true"), Some("sí")],
        "email_address" => &[Some("a@example.com"), Some("b@example.com"), Some("c@example.com"), Some("d@example.com")],
        "customer_id" => &[Some("1"), Some("2"), Some("3"), Some("4")]
    ]
    .unwrap();

    let profile = profile_dataset(&frame).expect("el perfil debe calcularse");
    assert_eq!(
        profile.columns[0].suggested_type,
        Some("boolean".to_owned())
    );
    assert_eq!(profile.columns[0].type_match_percentage, Some(100.0));
    assert_eq!(profile.columns[1].privacy_signal, Some("email".to_owned()));
    assert_eq!(
        profile.columns[2].privacy_signal,
        Some("identifier".to_owned())
    );

    let (cleaned, rows, cells, columns) =
        clean_text_columns(&frame, None, TextCleaningMode::Booleans)
            .expect("los alias booleanos deben poder normalizarse");
    let active = cleaned.column("active").unwrap().str().unwrap();
    assert_eq!(active.get(0), Some("true"));
    assert_eq!(active.get(1), Some("false"));
    assert_eq!(active.get(2), Some("true"));
    assert_eq!(active.get(3), Some("true"));
    assert_eq!(rows, 3);
    assert_eq!(cells, 3);
    assert_eq!(columns[0].name, "active");
    assert_eq!(columns[0].changed_cell_count, 3);
}

#[test]
fn activates_row_audit_and_appends_future_change_labels() {
    let path = temporary_csv("value\n1\n2\n");
    let (original, _) = load_csv(&path).expect("el CSV debe cargar");
    let mut dataset = loaded_dataset(path.clone(), original);
    let (audited, added) = add_audit_column_to_frame(&dataset.frame)
        .expect("la columna de auditoría debe poder añadirse");
    assert!(added);
    publish_candidate(&mut dataset, audited, "Activar trazabilidad por fila")
        .expect("la activación debe publicarse");
    assert_eq!(
        dataset
            .frame
            .column("_cambios")
            .unwrap()
            .str()
            .unwrap()
            .get(0),
        None
    );

    let mut changed = dataset.frame.clone();
    changed
        .replace("value", Column::new("value".into(), ["3", "2"]))
        .expect("el cambio de prueba debe aplicarse");
    publish_candidate(&mut dataset, changed, "Normalizar booleanos")
        .expect("el cambio posterior debe publicarse");
    let audit = dataset.frame.column("_cambios").unwrap().str().unwrap();
    assert_eq!(audit.get(0), Some("Normalizar booleanos"));
    assert_eq!(audit.get(1), Some("Normalizar booleanos"));

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn normalizes_column_names_and_resolves_collisions_deterministically() {
    let path = temporary_csv("Año Venta,ano-venta,2025 Total,/\n1,2,3,4\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let (names, renames) = normalized_column_names(&frame);

    assert_eq!(
        names,
        vec!["ano_venta", "ano_venta_2", "col_2025_total", "unnamed"]
    );
    assert_eq!(renames.len(), 4);
    assert_eq!(renames[0].from, "Año Venta");
    assert_eq!(renames[0].to, "ano_venta");
    assert_eq!(renames[1].to, "ano_venta_2");

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn trims_text_without_changing_internal_spaces_case_accents_or_nulls() {
    let path = temporary_csv("city,note\n\" Bogotá \",\"A  B\"\nLima,\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let (cleaned, rows, cells, columns) = clean_text_columns(&frame, None, TextCleaningMode::Trim)
        .expect("los espacios deben limpiarse");
    let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

    assert_eq!(page.rows[0][0].as_deref(), Some("Bogotá"));
    assert_eq!(page.rows[0][1].as_deref(), Some("A  B"));
    assert_eq!(page.rows[1][1], None);
    assert_eq!(rows, 1);
    assert_eq!(cells, 1);
    assert_eq!(columns[0].name, "city");
    assert_eq!(columns[0].changed_cell_count, 1);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn normalizes_selected_text_and_preserves_unselected_columns() {
    let path = temporary_csv("city,code\n\"  BOGOTÁ\tNORTE  \",\" Ab C \"\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let selected = vec!["city".to_owned()];

    let (cleaned, rows, cells, columns) = clean_text_columns(
        &frame,
        Some(&selected),
        TextCleaningMode::Normalize {
            remove_accents: true,
        },
    )
    .expect("el texto seleccionado debe normalizarse");
    let page = dataset_page(&cleaned, 0, 50).expect("la vista previa debe generarse");

    assert_eq!(page.rows[0][0].as_deref(), Some("bogota norte"));
    assert_eq!(page.rows[0][1].as_deref(), Some(" Ab C "));
    assert_eq!(rows, 1);
    assert_eq!(cells, 1);
    assert_eq!(columns.len(), 1);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn undo_and_redo_restore_disk_backed_revisions() {
    let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
    let (original, _) = load_csv(&path).expect("el CSV debe cargar");
    let (cleaned, _) = remove_duplicate_rows(&original).expect("los duplicados deben eliminarse");
    let mut dataset = loaded_dataset(path.clone(), original.clone());
    publish_candidate(&mut dataset, cleaned, "Eliminar filas duplicadas").unwrap();
    assert!(dataset.source_path.is_none());
    dataset.profile = Some(profile_dataset(&original).expect("el perfil debe existir"));

    let undone = undo_dataset(&mut dataset).expect("el cambio debe deshacerse");
    assert_eq!(undone.dataset.row_count, 3);
    assert!(!undone.history.can_undo);
    assert!(undone.history.can_redo);
    assert!(dataset.profile.is_none());

    let redone = redo_dataset(&mut dataset).expect("el cambio debe rehacerse");
    assert_eq!(redone.dataset.row_count, 2);
    assert!(redone.history.can_undo);
    assert!(!redone.history.can_redo);
    assert!(redo_dataset(&mut dataset).is_err());

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn source_backed_undo_and_redo_restore_schema_and_page_without_materializing_rows() {
    let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
    let (schema, _, row_count) =
        source_backed_load(&path, "csv", || false).expect("la fuente debe abrirse diferida");
    let file_size_bytes = fs::metadata(&path).unwrap().len();
    let history = HistoryManager::deferred().unwrap();
    let mut dataset = LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "dataset.csv".to_owned(),
        file_size_bytes,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };

    let mutation = remove_duplicates_source_backed(&mut dataset)
        .unwrap()
        .expect("la deduplicación source-backed debe publicarse");
    assert_eq!(mutation.affected_row_count, 1);
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, 2);
    assert!(dataset.history.snapshots_enabled);

    let undone = undo_dataset(&mut dataset).expect("el cambio source-backed debe deshacerse");
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, 3);
    assert_eq!(undone.dataset.rows[0][0].as_deref(), Some("Santo Domingo"));
    assert!(undone.history.can_redo);
    assert!(current_duckdb_file_source(&dataset).is_some());

    let redone = redo_dataset(&mut dataset).expect("el cambio source-backed debe rehacerse");
    assert!(dataset.source_backed);
    assert_eq!(dataset.frame.height(), 0);
    assert_eq!(dataset.row_count, 2);
    assert_eq!(redone.dataset.rows[1][0].as_deref(), Some("Santiago"));
    assert!(!redone.history.can_redo);

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn restored_project_keeps_original_file_name_across_changes_and_history() {
    let path = temporary_csv("city\nSanto Domingo\nSantiago\nSantiago\n");
    let (original, _) = load_csv(&path).expect("el CSV debe cargar");
    let (cleaned, _) = remove_duplicate_rows(&original).unwrap();
    let mut dataset = loaded_dataset(path.clone(), original);
    dataset.source_path = None;
    dataset.file_name = "ventas originales.xlsx".to_owned();
    fs::remove_file(&path).expect("el snapshot persistente puede eliminarse del catálogo");

    let changed = publish_candidate(&mut dataset, cleaned, "Eliminar duplicados").unwrap();
    let undone = undo_dataset(&mut dataset).unwrap();
    let redone = redo_dataset(&mut dataset).unwrap();

    assert_eq!(changed.file_name, "ventas originales.xlsx");
    assert_eq!(undone.dataset.file_name, "ventas originales.xlsx");
    assert_eq!(redone.dataset.file_name, "ventas originales.xlsx");
}

#[test]
fn history_supports_multiple_steps_and_truncates_redo_only_on_real_branch() {
    let path = temporary_csv("value\n1\n");
    let (original, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), original);
    for (value, label) in [(2_i64, "Paso dos"), (3, "Paso tres")] {
        let candidate =
            DataFrame::new(1, vec![Series::new("value".into(), [value]).into()]).unwrap();
        publish_candidate(&mut dataset, candidate, label).unwrap();
    }
    assert_eq!(dataset.history.state().entry_count, 3);
    undo_dataset(&mut dataset).unwrap();
    assert!(dataset.history.state().can_redo);

    // Una receta sin cambios no crea una etapa ni elimina la rama de rehacer.
    let result = apply_recipe_to_dataset(&mut dataset, &TransformRecipe::default()).unwrap();
    assert!(!result.changed);
    assert_eq!(dataset.history.state().entry_count, 3);
    assert!(dataset.history.state().can_redo);

    let branch = DataFrame::new(1, vec![Series::new("value".into(), [4_i64]).into()]).unwrap();
    publish_candidate(&mut dataset, branch, "Rama nueva").unwrap();
    let state = dataset.history.state();
    assert_eq!(state.entry_count, 3);
    assert_eq!(state.current_index, 2);
    assert!(!state.can_redo);
    assert_eq!(state.entries[2].label, "Rama nueva");
    fs::remove_file(path).unwrap();
}

#[test]
fn history_evicts_old_snapshots_by_count_and_disables_an_oversize_snapshot() {
    let original = DataFrame::new(1, vec![Series::new("value".into(), [0_i64]).into()]).unwrap();
    let mut limited = HistoryManager::with_limits(&original, 3, u64::MAX).unwrap();
    for value in 1_i64..=4 {
        let frame = DataFrame::new(1, vec![Series::new("value".into(), [value]).into()]).unwrap();
        limited.record(&frame, &format!("Paso {value}")).unwrap();
    }
    let state = limited.state();
    assert_eq!(state.entry_count, 3);
    assert_eq!(state.current_index, 2);
    assert_eq!(state.entries[0].label, "Paso 2");

    let exact_snapshot_budget = HistoryManager::with_limits(&original, 12, u64::MAX)
        .unwrap()
        .disk_bytes();
    let mut budgeted = HistoryManager::with_limits(&original, 12, exact_snapshot_budget).unwrap();
    budgeted
        .record(&original, "Dentro del presupuesto")
        .unwrap();
    let state = budgeted.state();
    assert!(state.snapshots_enabled);
    assert_eq!(state.entry_count, 1);
    assert_eq!(state.entries[0].label, "Dentro del presupuesto");
    assert!(state.disk_bytes <= exact_snapshot_budget);

    let disabled = HistoryManager::with_limits(&original, 12, 0).unwrap();
    let state = disabled.state();
    assert!(!state.snapshots_enabled);
    assert!(!state.can_undo);
    assert_eq!(state.entry_count, 1);
    assert_eq!(state.disk_bytes, 0);
    assert!(state.degraded_reason.unwrap().contains("supera el límite"));
}

#[test]
fn corrupt_restore_and_snapshot_io_failure_leave_dataset_and_cursor_unchanged() {
    let path = temporary_csv("value\n1\n");
    let (original, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), original.clone());
    let changed = DataFrame::new(1, vec![Series::new("value".into(), [2_i64]).into()]).unwrap();
    publish_candidate(&mut dataset, changed, "Cambio").unwrap();
    let before = dataset.frame.clone();
    let cursor = dataset.history.cursor;
    fs::write(&dataset.history.entries[0].path, b"not parquet").unwrap();
    assert!(undo_dataset(&mut dataset).is_err());
    assert_eq!(dataset.history.cursor, cursor);
    assert!(dataset.frame.equals_missing(&before));

    let fresh_path = temporary_csv("value\n1\n");
    let (fresh, _) = load_csv(&fresh_path).unwrap();
    let mut io_failure = loaded_dataset(fresh_path.clone(), fresh.clone());
    fs::remove_dir_all(io_failure.history.directory.path()).unwrap();
    let candidate = DataFrame::new(1, vec![Series::new("value".into(), [9_i64]).into()]).unwrap();
    assert!(publish_candidate(&mut io_failure, candidate, "No publicable").is_err());
    assert!(io_failure.frame.equals_missing(&fresh));
    assert_eq!(io_failure.history.cursor, 0);
    fs::remove_file(path).unwrap();
    fs::remove_file(fresh_path).unwrap();
}

#[test]
fn replacing_a_loaded_dataset_removes_the_previous_snapshot_directory() {
    let first_path = temporary_csv("value\n1\n");
    let second_path = temporary_csv("value\n2\n");
    let (first, _) = load_csv(&first_path).unwrap();
    let (second, _) = load_csv(&second_path).unwrap();
    let mut current = Some(loaded_dataset(first_path.clone(), first));
    let old_directory = current
        .as_ref()
        .unwrap()
        .history
        .directory
        .path()
        .to_path_buf();
    assert!(old_directory.exists());
    current = Some(loaded_dataset(second_path.clone(), second));
    assert!(!old_directory.exists());
    drop(current);
    fs::remove_file(first_path).unwrap();
    fs::remove_file(second_path).unwrap();
}

#[test]
fn applies_safe_corrections_in_one_candidate_frame() {
    let path = temporary_csv("Año Venta,city\n1,\" Bogotá \"\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");

    let (corrected, rows, cells, renames) =
        safe_corrected_frame(&frame).expect("las correcciones deben aplicarse");
    let page = dataset_page(&corrected, 0, 50).expect("la vista previa debe generarse");

    assert_eq!(
        corrected
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["ano_venta", "city"]
    );
    assert_eq!(page.rows[0][1].as_deref(), Some("Bogotá"));
    assert_eq!(rows, 1);
    assert_eq!(cells, 1);
    assert_eq!(renames.len(), 1);
    assert_eq!(renames[0].to, "ano_venta");

    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn loads_parquet_preserving_schema_nulls_and_unicode() {
    let source = temporary_csv("city,value\nSanto Domingo,10\nBogotá,\n");
    let (frame, _) = load_csv(&source).expect("el CSV debe cargar");
    let directory = tempfile::tempdir().expect("se debe crear la carpeta temporal");
    let parquet = directory.path().join("dataset.parquet");
    export_frame_atomic(&frame, &parquet, ExportFormat::Parquet, |_, _| {}, || false)
        .expect("el Parquet debe escribirse");
    let mut updates = Vec::new();

    let (loaded, preview) = load_dataset_with_progress(
        &parquet,
        |stage, percent| updates.push((stage, percent)),
        || false,
    )
    .expect("el Parquet debe cargarse");

    assert_eq!(loaded.dtypes(), frame.dtypes());
    assert_eq!(preview.file_name, "dataset.parquet");
    assert_eq!(preview.row_count, 2);
    assert_eq!(preview.rows[1][0].as_deref(), Some("Bogotá"));
    assert_eq!(preview.rows[1][1], None);
    assert_eq!(updates.first(), Some(&("Validando archivo", 10)));
    assert_eq!(updates.last(), Some(&("Preparando sesión", 95)));

    fs::remove_file(source).expect("se debe limpiar el CSV temporal");
}

#[test]
fn loads_tsv_with_tabs_and_preserves_lexical_values() {
    let path = temporary_delimited(
        "tsv",
        "id\tdescription\tamount\n00123\t\"Santo Domingo, RD\"\t1.00\n18446744073709551616\tBogotá\t\n",
    );

    let (frame, preview) =
        load_dataset_with_progress(&path, |_, _| {}, || false).expect("el TSV debe cargar");

    assert!(frame
        .dtypes()
        .iter()
        .all(|kind| *kind == polars::prelude::DataType::String));
    assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
    assert_eq!(preview.rows[0][1].as_deref(), Some("Santo Domingo, RD"));
    assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
    assert_eq!(preview.rows[1][0].as_deref(), Some("18446744073709551616"));
    assert_eq!(preview.rows[1][2], None);

    fs::remove_file(path).expect("se debe limpiar el TSV temporal");
}

#[test]
fn detects_semicolon_csv_and_ignores_delimiters_inside_quotes() {
    let path = temporary_delimited(
        "csv",
        "id;place;amount\n00123;\"Santo Domingo, RD\";1.00\n00007;Santiago;2.50\n",
    );

    let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect("el CSV con punto y coma debe cargar");

    assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
    assert_eq!(preview.column_count, 3);
    assert_eq!(preview.rows[0][0].as_deref(), Some("00123"));
    assert_eq!(preview.rows[0][1].as_deref(), Some("Santo Domingo, RD"));
    assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn accepts_utf8_bom_without_including_it_in_the_header() {
    let path = temporary_delimited("csv", "\u{feff}id;city\n001;Santo Domingo\n002;Santiago\n");

    let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect("el CSV UTF-8 con BOM debe cargar");

    assert_eq!(frame.get_column_names()[0].as_str(), "id");
    assert_eq!(preview.rows[0][0].as_deref(), Some("001"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn detects_pipe_delimited_txt_without_numeric_inference() {
    let path = temporary_delimited(
        "txt",
        "code|description|amount\n0001|alpha|1.00\n0002|beta|2.50\n",
    );

    let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect("el TXT delimitado debe cargar");

    assert!(frame.dtypes().iter().all(|kind| *kind == DataType::String));
    assert_eq!(preview.column_count, 3);
    assert_eq!(preview.rows[0][0].as_deref(), Some("0001"));
    assert_eq!(preview.rows[0][2].as_deref(), Some("1.00"));
    fs::remove_file(path).expect("se debe limpiar el TXT temporal");
}

#[test]
fn rejects_invalid_utf8_instead_of_replacing_characters() {
    let path = temporary_delimited_bytes("csv", b"id,city\n001,Santo Domingo\n002,Bogot\xe1\n");

    let error = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect_err("los bytes que no son UTF-8 deben rechazarse");

    assert!(error.contains("UTF-8 válido"));
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn converts_spreadsheet_range_with_unique_headers_and_safe_types() {
    let mut range = Range::<Data>::new((0, 0), (2, 3));
    range.set_value((0, 0), Data::String("id".to_owned()));
    range.set_value((0, 1), Data::String("id".to_owned()));
    range.set_value((0, 2), Data::Empty);
    range.set_value((0, 3), Data::String("active".to_owned()));
    range.set_value((1, 0), Data::Int(1));
    range.set_value((2, 0), Data::Int(2));
    range.set_value((1, 1), Data::String("001".to_owned()));
    range.set_value((2, 1), Data::Error(calamine::CellErrorType::Div0));
    range.set_value((1, 2), Data::Float(1.5));
    range.set_value((2, 2), Data::Empty);
    range.set_value((1, 3), Data::Bool(true));
    range.set_value((2, 3), Data::Bool(false));

    let frame = spreadsheet_range_to_frame(&range, SpreadsheetHeaderMode::FirstRow)
        .expect("la hoja debe convertirse");
    assert_eq!(frame.height(), 2);
    assert_eq!(
        frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["id", "id_2", "column_3", "active"]
    );
    assert_eq!(frame.dtypes()[0], polars::prelude::DataType::Int64);
    assert_eq!(frame.dtypes()[1], polars::prelude::DataType::String);
    assert_eq!(frame.dtypes()[2], polars::prelude::DataType::Float64);
    assert_eq!(frame.dtypes()[3], polars::prelude::DataType::Boolean);
    let page = dataset_page(&frame, 0, 50).expect("debe generarse la vista previa");
    assert_eq!(page.rows[0][1].as_deref(), Some("001"));
    assert_eq!(page.rows[1][1].as_deref(), Some("#DIV/0!"));
}

#[test]
fn generates_spreadsheet_headers_without_consuming_the_first_row() {
    let mut range = Range::<Data>::new((0, 0), (1, 1));
    range.set_value((0, 0), Data::String("001".to_owned()));
    range.set_value((0, 1), Data::String("Santo Domingo".to_owned()));
    range.set_value((1, 0), Data::String("002".to_owned()));
    range.set_value((1, 1), Data::String("Santiago".to_owned()));

    let frame = spreadsheet_range_to_frame(&range, SpreadsheetHeaderMode::Generated)
        .expect("la hoja debe usar encabezados generados");

    assert_eq!(frame.height(), 2);
    assert_eq!(
        frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["column_1", "column_2"]
    );
    let page = dataset_page(&frame, 0, 50).expect("debe conservar la primera fila");
    assert_eq!(page.rows[0][0].as_deref(), Some("001"));
    assert_eq!(page.rows[0][1].as_deref(), Some("Santo Domingo"));
}

#[test]
fn snapshots_xlsx_in_streaming_blocks_without_changing_typed_values() {
    let compared = df![
        "id" => &[1_i64, 2, 3],
        "name" => &["Ana", "Luis", "María"],
        "amount" => &[10.5_f64, 20.0, 30.25],
        "active" => &[true, false, true]
    ]
    .expect("el frame Excel debe construirse");
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("compared.xlsx");
    let mut output = File::create(&source).expect("se debe crear el libro Excel");
    write_xlsx(&compared, &mut output, |_| {}, || false).expect("se debe escribir el libro");
    output
        .sync_all()
        .expect("se debe sincronizar el libro Excel");

    let expected = load_compare_frame(&source, "xlsx").expect("Excel debe cargar");
    let current = df![
        "id" => &[1.0_f64, 2.0, 3.0],
        "name" => &["Ana", "Luis", "María"],
        "amount" => &[10.5_f64, 19.0, 30.25],
        "active" => &[true, false, true]
    ]
    .expect("el frame activo debe construirse");
    let (snapshot_directory, snapshot_path, row_count) =
        persist_spreadsheet_comparison_source_file(&source, "xlsx")
            .expect("Excel debe convertirse al snapshot por bloques");
    assert_eq!(row_count, expected.height());
    let restored = read_parquet_frame(&snapshot_path).expect("el snapshot debe ser legible");
    assert!(restored.equals_missing(&expected));

    let expected_comparison = compare_frames(
        &current,
        "activo.xlsx",
        &expected,
        "comparado.xlsx",
        &["id".to_owned()],
    )
    .expect("la comparación Excel de referencia debe calcularse");
    let actual_comparison = compare_parquet_source(
        &current,
        "activo.xlsx",
        &snapshot_path,
        "comparado.xlsx",
        row_count,
        &["id".to_owned()],
    )
    .expect("la comparación del snapshot Excel debe calcularse");
    assert_eq!(actual_comparison, expected_comparison);

    drop(snapshot_directory);
    assert!(!snapshot_path.exists());
}

#[test]
fn loads_xlsx_through_a_source_backed_parquet_snapshot() {
    assert!(should_defer_source_load(
        "xlsx",
        SOURCE_BACKED_LOAD_THRESHOLD_BYTES
    ));
    assert!(should_defer_source_load(
        "xlsb",
        SOURCE_BACKED_LOAD_THRESHOLD_BYTES
    ));
    assert!(!should_defer_source_load(
        "xls",
        SOURCE_BACKED_LOAD_THRESHOLD_BYTES
    ));
    let expected = df![
        "id" => &[1_i64, 2, 3],
        "name" => &["Ana", "Luis", "María"],
        "amount" => &[10.5_f64, 20.0, 30.25],
        "active" => &[true, false, true]
    ]
    .expect("el frame Excel debe construirse");
    let directory = tempfile::tempdir().expect("se debe crear el directorio temporal");
    let source = directory.path().join("large.xlsx");
    let mut output = File::create(&source).expect("se debe crear el libro Excel");
    write_xlsx(&expected, &mut output, |_| {}, || false).expect("se debe escribir el libro");
    output
        .sync_all()
        .expect("se debe sincronizar el libro Excel");
    let expected_size = fs::metadata(&source).expect("la fuente debe existir").len();
    let mut history = HistoryManager::deferred().expect("el historial debe inicializarse");
    let cancelled_snapshot = history.directory.path().join("cancelled.parquet");
    let cancellation = source_backed_spreadsheet_load(
        &source,
        "dataset",
        SpreadsheetHeaderMode::FirstRow,
        &cancelled_snapshot,
        || true,
    )
    .expect_err("la apertura cancelada debe detenerse antes de escribir");
    assert_eq!(cancellation, OPERATION_CANCELLED_MESSAGE);
    assert!(!cancelled_snapshot.exists());
    let snapshot_path = history.directory.path().join("source.parquet");
    let (schema, preview, row_count) = source_backed_spreadsheet_load(
        &source,
        "dataset",
        SpreadsheetHeaderMode::FirstRow,
        &snapshot_path,
        || false,
    )
    .expect("el libro debe convertirse por bloques");

    assert_eq!(schema.height(), 0);
    assert_eq!(row_count, expected.height());
    assert_eq!(preview.file_name, "large.xlsx");
    assert_eq!(preview.rows[1][1].as_deref(), Some("Luis"));
    let restored = read_parquet_frame(&snapshot_path).expect("el snapshot debe ser legible");
    assert!(restored.equals_missing(&expected));
    let snapshot_size = fs::metadata(&snapshot_path)
        .expect("el snapshot debe tener metadatos")
        .len();
    let profile = profile_source_backed_with_progress(
        &snapshot_path,
        "parquet",
        snapshot_size,
        row_count,
        |_, _| {},
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect("el perfil debe recorrer el snapshot source-backed");
    assert_eq!(profile.row_count, row_count);

    history.source_snapshot_path = Some(snapshot_path);
    let mut dataset = LoadedDataset {
        source_path: Some(source.clone()),
        file_name: "large.xlsx".to_owned(),
        file_size_bytes: expected_size,
        row_count,
        frame: schema,
        source_backed: true,
        profile: None,
        history,
    };
    let (query_path, query_format) = current_duckdb_file_source(&dataset)
        .expect("el snapshot Excel debe quedar disponible para DuckDB");
    assert_eq!(dataset_extension(&query_path).unwrap(), "parquet");
    assert!(matches!(
        query_format,
        crate::duckdb_query::DuckDbFileFormat::Parquet
    ));
    materialize_loaded_dataset(&mut dataset).expect("el snapshot debe materializarse");
    assert!(!dataset.source_backed);
    assert!(dataset.frame.equals_missing(&expected));
    assert_eq!(fs::metadata(&source).unwrap().len(), expected_size);
    fs::remove_file(source).expect("se debe limpiar el libro Excel");
}

#[test]
fn loads_json_record_array_with_union_of_fields_and_nested_values() {
    let path = temporary_delimited(
        "json",
        r#"[
            {"id": 1, "active": true, "meta": {"city": "Santo Domingo"}},
            {"id": 2, "amount": 1.5, "active": null, "meta": ["a", "b"]}
        ]"#,
    );

    let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect("el arreglo JSON debe cargar");

    assert_eq!(frame.height(), 2);
    assert_eq!(
        frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["active", "id", "meta", "amount"]
    );
    assert_eq!(frame.dtypes()[0], polars::prelude::DataType::Boolean);
    assert_eq!(frame.dtypes()[1], polars::prelude::DataType::Int64);
    assert_eq!(frame.dtypes()[2], polars::prelude::DataType::String);
    assert_eq!(frame.dtypes()[3], polars::prelude::DataType::Float64);
    assert_eq!(
        preview.rows[0][2].as_deref(),
        Some(r#"{"city":"Santo Domingo"}"#)
    );
    assert_eq!(preview.rows[0][3], None);

    fs::remove_file(path).expect("se debe limpiar el JSON temporal");
}

#[test]
fn loads_json_lines_and_rejects_non_object_records() {
    let valid = temporary_delimited(
        "jsonl",
        "{\"code\":\"001\",\"value\":10}\n{\"code\":\"002\",\"value\":20}\n",
    );
    let (_, preview) =
        load_dataset_with_progress(&valid, |_, _| {}, || false).expect("JSON Lines debe cargar");
    assert_eq!(preview.rows[0][0].as_deref(), Some("001"));
    fs::remove_file(valid).expect("se debe limpiar JSON Lines");

    let invalid = temporary_delimited("json", "[{\"id\":1}, 2]");
    let error = load_dataset_with_progress(&invalid, |_, _| {}, || false)
        .expect_err("los registros escalares deben rechazarse");
    assert!(error.contains("registro JSON 2 no es un objeto"));
    fs::remove_file(invalid).expect("se debe limpiar el JSON inválido");
}

#[test]
fn preserves_json_integers_larger_than_u64_as_text() {
    let path = temporary_delimited(
        "json",
        "[{\"identifier\":184467440737095516160},{\"identifier\":1}]",
    );

    let (frame, preview) = load_dataset_with_progress(&path, |_, _| {}, || false)
        .expect("el entero JSON grande debe preservarse");

    assert_eq!(frame.dtypes()[0], polars::prelude::DataType::String);
    assert_eq!(preview.rows[0][0].as_deref(), Some("184467440737095516160"));
    assert_eq!(preview.rows[1][0].as_deref(), Some("1"));
    fs::remove_file(path).expect("se debe limpiar el JSON temporal");
}

#[test]
fn structural_recipe_applies_swapped_renames_strict_casts_and_dates_in_order() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("amount".into(), [Some("10.50"), None]).into_column(),
            Series::new("count".into(), [Some("42"), Some("-7")]).into_column(),
            Series::new("enabled".into(), [Some("TRUE"), Some("false")]).into_column(),
            Series::new("day".into(), [Some("31/12/2025"), Some("01/01/2026")]).into_column(),
            Series::new("left".into(), ["L1", "L2"]).into_column(),
            Series::new("right".into(), ["R1", "R2"]).into_column(),
        ],
    )
    .expect("el frame debe ser válido");
    let recipe = TransformRecipe {
        renames: vec![
            RecipeRename {
                from: "left".into(),
                to: "right".into(),
            },
            RecipeRename {
                from: "right".into(),
                to: "left".into(),
            },
            RecipeRename {
                from: "count".into(),
                to: "units".into(),
            },
        ],
        casts: vec![
            RecipeCast {
                column: "amount".into(),
                target: RecipeCastTarget::Decimal,
            },
            // References the pre-rename name deliberately.
            RecipeCast {
                column: "count".into(),
                target: RecipeCastTarget::Integer,
            },
            RecipeCast {
                column: "enabled".into(),
                target: RecipeCastTarget::Boolean,
            },
        ],
        date_parses: vec![RecipeDateParse {
            column: "day".into(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        ..Default::default()
    };

    assert!(!lazy_recipe_supported(&frame, &recipe));
    let (
        result,
        renamed,
        converted,
        dates,
        removed,
        calculated,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
    ) = apply_recipe_to_frame(&frame, &recipe).expect("la receta debe ser atómica y válida");
    assert_eq!((renamed, converted, dates), (3, 3, 1));
    assert_eq!((removed, calculated), (0, 0));
    assert_eq!(
        result.column("units").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    assert_eq!(
        result.column("amount").unwrap().dtype(),
        &polars::prelude::DataType::Float64
    );
    assert_eq!(
        result.column("enabled").unwrap().dtype(),
        &polars::prelude::DataType::Boolean
    );
    assert_eq!(
        result.column("day").unwrap().dtype(),
        &polars::prelude::DataType::Date
    );
    assert_eq!(
        dataset_page(&result, 0, 2).unwrap().rows[0][4].as_deref(),
        Some("L1")
    );
    assert_eq!(
        dataset_page(&result, 0, 2).unwrap().rows[0][5].as_deref(),
        Some("R1")
    );
}

#[test]
fn lazy_recipe_parses_fixed_date_formats_with_streaming_plan() {
    let frame = DataFrame::new(
        3,
        vec![Series::new(
            "when".into(),
            [Some("31/12/2025"), Some("01/01/2026"), None::<&str>],
        )
        .into_column()],
    )
    .expect("el frame de fechas debe ser válido");
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_lazy_recipe_to_frame(&frame, &recipe)
        .expect("el parseo lazy de fechas debe completarse");

    assert_eq!(outcome.3, 1);
    assert_eq!(outcome.0.column("when").unwrap().dtype(), &DataType::Date);
    assert!(matches!(
        outcome.0.column("when").unwrap().get(0),
        Ok(AnyValue::Date(_))
    ));
    assert!(matches!(
        outcome.0.column("when").unwrap().get(2),
        Ok(AnyValue::Null)
    ));
}

#[test]
fn lazy_recipe_filters_parsed_dates_before_extracting_year() {
    let frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "when".into(),
                [
                    Some("31/12/2025"),
                    Some("01/01/2026"),
                    Some("15/02/2026"),
                    None::<&str>,
                ],
            )
            .into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
        ],
    )
    .expect("el frame filtrable de fechas debe ser válido");
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        filters: vec![
            RecipeFilter {
                column: "when".into(),
                operator: RecipeFilterOperator::Gte,
                value: Some("2026-01-01".into()),
            },
            RecipeFilter {
                column: "when".into(),
                operator: RecipeFilterOperator::Lt,
                value: Some("2026-03-01".into()),
            },
        ],
        calculated_column: Some(CalculatedColumnRecipe {
            name: "year".into(),
            source: "when".into(),
            operation: CalculatedOperation::Year,
            operand: None,
        }),
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("el filtro temporal y la extracción deben compartir el plan lazy");

    assert_eq!((outcome.3, outcome.4, outcome.5), (1, 2, 1));
    assert_eq!(outcome.0.height(), 2);
    assert_eq!(outcome.0.column("when").unwrap().dtype(), &DataType::Date);
    assert_eq!(outcome.0.column("year").unwrap().dtype(), &DataType::Int32);
    let rows = dataset_page(&outcome.0, 0, 10)
        .expect("la página filtrada debe ser válida")
        .rows;
    assert_eq!(rows[0][0].as_deref(), Some("2026-01-01"));
    assert_eq!(rows[0][2].as_deref(), Some("2026"));
    assert_eq!(rows[1][0].as_deref(), Some("2026-02-15"));
    assert_eq!(rows[1][2].as_deref(), Some("2026"));
}

#[test]
fn lazy_recipe_parses_iso8601_dates_and_utc_values_without_offset_materialization() {
    let frame = DataFrame::new(
        4,
        vec![Series::new(
            "when".into(),
            [
                Some("2025-12-31"),
                Some("2025-12-31T23:15:30.125"),
                Some("2025-12-31T23:15:30Z"),
                None::<&str>,
            ],
        )
        .into_column()],
    )
    .expect("el frame ISO debe ser válido");
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Iso8601,
            target: RecipeDateTarget::Datetime,
        }],
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("los valores ISO sin offset y UTC deben usar streaming");
    assert_eq!(outcome.3, 1);
    let column = outcome.0.column("when").unwrap();
    assert_eq!(
        column.dtype(),
        &DataType::Datetime(TimeUnit::Milliseconds, None)
    );
    assert!(matches!(
        column.get(0),
        Ok(AnyValue::Datetime(value, TimeUnit::Milliseconds, None))
            if value == 1_767_139_200_000
    ));
    assert!(matches!(
        column.get(1),
        Ok(AnyValue::Datetime(value, TimeUnit::Milliseconds, None))
            if value == 1_767_222_930_125
    ));
    assert!(matches!(
        column.get(2),
        Ok(AnyValue::Datetime(value, TimeUnit::Milliseconds, None))
            if value == 1_767_222_930_000
    ));
    assert!(matches!(column.get(3), Ok(AnyValue::Null)));

    let date_recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Iso8601,
            target: RecipeDateTarget::Date,
        }],
        ..Default::default()
    };
    let date_outcome = apply_recipe_to_frame(&frame, &date_recipe)
        .expect("el objetivo Date ISO también debe usar streaming");
    assert_eq!(
        date_outcome.0.column("when").unwrap().dtype(),
        &DataType::Date
    );
    assert!(matches!(
        date_outcome.0.column("when").unwrap().get(1),
        Ok(AnyValue::Date(value)) if value == 20_453
    ));
}

#[test]
fn lazy_recipe_keeps_non_utc_iso_offsets_on_the_strict_eager_path() {
    let frame = DataFrame::new(
        1,
        vec![Series::new("when".into(), [Some("2025-12-31T23:15:30+02:00")]).into_column()],
    )
    .expect("el frame con offset debe ser válido");
    let recipe = TransformRecipe {
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Iso8601,
            target: RecipeDateTarget::Datetime,
        }],
        ..Default::default()
    };

    assert!(!lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("el fallback eager debe conservar la conversión UTC");
    assert!(matches!(
        outcome.0.column("when").unwrap().get(0),
        Ok(AnyValue::DatetimeOwned(value, TimeUnit::Milliseconds, None))
            if value == 1_767_215_730_000
    ));
}

#[test]
fn lazy_recipe_combines_date_parsing_and_casts_on_separate_columns() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("when".into(), [Some("31/12/2025"), Some("01/01/2026")]).into_column(),
            Series::new("amount".into(), [Some("5"), Some("12")]).into_column(),
        ],
    )
    .expect("el frame mixto debe ser válido");
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "amount".into(),
            target: RecipeCastTarget::Decimal,
        }],
        date_parses: vec![RecipeDateParse {
            column: "when".into(),
            format: RecipeDateFormat::Dmy,
            target: RecipeDateTarget::Date,
        }],
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("las etapas independientes deben compartir el plan lazy");

    assert_eq!(outcome.2, 1);
    assert_eq!(outcome.3, 1);
    assert_eq!(outcome.0.column("when").unwrap().dtype(), &DataType::Date);
    assert_eq!(
        outcome.0.column("amount").unwrap().dtype(),
        &DataType::Float64
    );
}

#[test]
fn lazy_recipe_applies_isolated_outlier_treatments_with_exact_counts() {
    let frame = DataFrame::new(
        6,
        vec![Series::new(
            "value".into(),
            [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
        )
        .into_column()],
    )
    .expect("el frame numérico debe ser válido");

    let cap = TransformRecipe {
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Cap,
        }],
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &cap));
    let capped = apply_recipe_to_frame(&frame, &cap).expect("el cap lazy debe completarse");
    assert_eq!((capped.12, capped.13, capped.14), (1, 0, 1));
    assert!(matches!(
        capped.0.column("value").unwrap().get(4),
        Ok(AnyValue::Float64(7.0))
    ));

    let impute = TransformRecipe {
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Impute,
        }],
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &impute));
    let imputed =
        apply_recipe_to_frame(&frame, &impute).expect("la imputación lazy debe completarse");
    assert_eq!((imputed.12, imputed.13, imputed.14), (1, 0, 1));
    assert!(matches!(
        imputed.0.column("value").unwrap().get(4),
        Ok(AnyValue::Int64(3))
    ));

    let drop = TransformRecipe {
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Drop,
        }],
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &drop));
    let dropped = apply_recipe_to_frame(&frame, &drop).expect("el drop lazy debe completarse");
    assert_eq!(
        (dropped.4, dropped.12, dropped.13, dropped.14),
        (1, 0, 1, 1)
    );
    assert_eq!(dropped.0.height(), 5);
    assert!(matches!(
        dropped.0.column("value").unwrap().get(4),
        Ok(AnyValue::Null)
    ));
}

#[test]
fn lazy_recipe_keeps_outlier_dependencies_when_projection_precedes_iqr() {
    let frame = df![
        "value" => [1_i64, 2, 3, 4, 100, 1000],
        "group" => ["keep", "keep", "keep", "keep", "keep", "discard"]
    ]
    .expect("el frame filtrable debe ser válido");
    let recipe = TransformRecipe {
        filters: vec![RecipeFilter {
            column: "group".into(),
            operator: RecipeFilterOperator::Eq,
            value: Some("keep".into()),
        }],
        keep_columns: Some(vec!["group".into(), "value".into()]),
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Cap,
        }],
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("la proyección que conserva la dependencia debe seguir en lazy");

    assert_eq!(
        (outcome.4, outcome.12, outcome.13, outcome.14),
        (1, 1, 0, 1)
    );
    assert_eq!(
        outcome
            .0
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["group", "value"]
    );
    assert_eq!(outcome.0.height(), 5);
    assert!(matches!(
        outcome.0.column("value").unwrap().get(4),
        Ok(AnyValue::Float64(7.0))
    ));

    let mut dropping_dependency = recipe;
    dropping_dependency.keep_columns = Some(vec!["group".into()]);
    assert!(!lazy_recipe_supported(&frame, &dropping_dependency));
    assert!(apply_recipe_to_frame(&frame, &dropping_dependency).is_err());
}

#[test]
fn lazy_recipe_calculates_outlier_thresholds_after_filters() {
    let frame = df![
        "value" => [1_i64, 2, 3, 4, 100, 1000],
        "group" => ["keep", "keep", "keep", "keep", "keep", "discard"]
    ]
    .expect("el frame filtrable debe ser válido");
    let recipe = TransformRecipe {
        filters: vec![RecipeFilter {
            column: "group".into(),
            operator: RecipeFilterOperator::Eq,
            value: Some("keep".into()),
        }],
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Cap,
        }],
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe)
        .expect("el IQR posterior al filtro debe completarse en lazy");

    assert_eq!(outcome.4, 1);
    assert_eq!(outcome.12, 1);
    assert_eq!(outcome.13, 0);
    assert_eq!(outcome.0.height(), 5);
    assert_eq!(
        dataset_page(&outcome.0, 0, 10).unwrap().rows[4][0].as_deref(),
        Some("7.0")
    );

    let drop_recipe = TransformRecipe {
        filters: recipe.filters.clone(),
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Drop,
        }],
        ..Default::default()
    };
    let dropped = apply_recipe_to_frame(&frame, &drop_recipe)
        .expect("el drop posterior al filtro debe conservar el conteo real");
    assert_eq!(dropped.4, 2);
    assert_eq!(dropped.12, 0);
    assert_eq!(dropped.13, 1);
    assert_eq!(dropped.0.height(), 4);
}

#[test]
fn lazy_recipe_combines_split_and_merge_columns() {
    let frame = df![
        "full_name" => ["Ada Lovelace", "Grace Hopper"],
        "team" => ["Math", "Navy"]
    ]
    .expect("el frame de texto debe ser válido");
    let recipe = TransformRecipe {
        split_column: Some(SplitColumnRecipe {
            source: "full_name".into(),
            delimiter: " ".into(),
            names: vec!["first".into(), "last".into()],
            drop_source: false,
        }),
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["full_name".into(), "team".into()],
            name: "label".into(),
            separator: " — ".into(),
            drop_sources: false,
        }),
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome =
        apply_recipe_to_frame(&frame, &recipe).expect("split y merge deben compartir el plan lazy");

    assert_eq!(outcome.9, 2);
    assert_eq!(outcome.10, 1);
    let page = dataset_page(&outcome.0, 0, 10).expect("la página debe ser válida");
    assert_eq!(page.rows[0][2].as_deref(), Some("Ada"));
    assert_eq!(page.rows[0][3].as_deref(), Some("Lovelace"));
    assert_eq!(page.rows[0][4].as_deref(), Some("Ada Lovelace — Math"));
}

#[test]
fn structural_recipe_supports_all_explicit_date_formats_and_datetime_targets() {
    for (value, format) in [
        ("2025-12-31", RecipeDateFormat::Ymd),
        ("31/12/2025", RecipeDateFormat::Dmy),
        ("12/31/2025", RecipeDateFormat::Mdy),
        ("2025-12-31T23:15:30Z", RecipeDateFormat::Iso8601),
    ] {
        let column = Series::new("when".into(), [value]).into_column();
        let converted = strict_date_column(&column, format, RecipeDateTarget::Datetime)
            .expect("el formato explícito debe aceptarse");
        assert!(matches!(
            converted.dtype(),
            polars::prelude::DataType::Datetime(_, _)
        ));
    }
}

#[test]
fn legacy_date_cleaning_skips_ambiguous_columns_instead_of_creating_nulls() {
    let frame = df![
        "safe" => &["2025-01-02", "2025-01-03", "2025-01-04", "2025-01-05"],
        "ambiguous" => &["01/02/2025", "02/03/2025", "2025/04/05", "May 6, 2025"]
    ]
    .expect("la fixture de fechas debe construirse");

    let (cleaned, changed_rows, changed_cells, changed_columns) =
        parse_inferred_date_columns(&frame).expect("el parseo conservador debe completarse");

    assert!(matches!(
        cleaned.column("safe").unwrap().dtype(),
        polars::prelude::DataType::Datetime(_, _)
    ));
    assert_eq!(
        cleaned.column("ambiguous").unwrap().dtype(),
        &DataType::String
    );
    assert_eq!(changed_rows, 4);
    assert_eq!(changed_cells, 4);
    assert_eq!(changed_columns.len(), 1);
    assert_eq!(changed_columns[0].name, "safe");
}

#[test]
fn legacy_numeric_cast_skips_identifiers_and_leading_zero_codes() {
    let frame = df![
        "amount" => &["10.5", "11.5", "12.5"],
        "code" => &["001", "002", "003"],
        "customer_id" => &["100", "101", "102"]
    ]
    .expect("la fixture numérica debe construirse");

    let (cast, changed_rows, changed_cells, changed_columns) =
        cast_inferred_numeric_columns(&frame).expect("el cast seguro debe completarse");

    assert_eq!(cast.column("amount").unwrap().dtype(), &DataType::Float64);
    assert_eq!(cast.column("code").unwrap().dtype(), &DataType::String);
    assert_eq!(
        cast.column("customer_id").unwrap().dtype(),
        &DataType::String
    );
    assert_eq!(changed_rows, 3);
    assert_eq!(changed_cells, 3);
    assert_eq!(changed_columns.len(), 1);
    assert_eq!(changed_columns[0].name, "amount");
}

#[test]
fn structural_recipe_rolls_back_fully_on_invalid_value_and_does_not_create_undo() {
    let path = temporary_csv("count\n1\nnot-an-integer\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let original = frame.clone();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "count".into(),
            to: "units".into(),
        }],
        casts: vec![RecipeCast {
            column: "count".into(),
            target: RecipeCastTarget::Integer,
        }],
        date_parses: vec![],
        ..Default::default()
    };

    let error = apply_recipe_to_dataset(&mut dataset, &recipe)
        .expect_err("un valor inválido debe abortar toda la receta");
    assert!(error.contains("fila 2"));
    assert!(dataset.frame.equals_missing(&original));
    assert!(!dataset.history.state().can_undo);
    assert!(!dataset.history.state().can_redo);

    let calculation_error = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            renames: vec![RecipeRename {
                from: "count".into(),
                to: "units".into(),
            }],
            calculated_column: Some(CalculatedColumnRecipe {
                name: "ratio".into(),
                source: "count".into(),
                operation: CalculatedOperation::Divide,
                operand: Some(CalculatedOperand {
                    kind: CalculatedOperandKind::Literal,
                    value: "0".into(),
                }),
            }),
            ..Default::default()
        },
    )
    .expect_err("el cálculo inválido debe abortar también el renombrado");
    assert!(
        calculation_error.contains("División por cero") || calculation_error.contains("número")
    );
    assert!(dataset.frame.equals_missing(&original));
    assert!(!dataset.history.state().can_undo);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");

    let metadata_error = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            renames: vec![RecipeRename {
                from: "count".into(),
                to: "units".into(),
            }],
            casts: vec![],
            date_parses: vec![],
            ..Default::default()
        },
    )
    .expect_err("un fallo al preparar la respuesta también debe abortar");
    assert!(metadata_error.contains("metadatos"));
    assert!(dataset.frame.equals_missing(&original));
    assert!(!dataset.history.state().can_undo);
}

#[test]
fn empty_or_already_satisfied_recipe_is_a_noop_without_history() {
    let path = temporary_csv("value\n1\n");
    let (frame, _) = load_csv(&path).expect("el CSV debe cargar");
    let mut dataset = loaded_dataset(path.clone(), frame);
    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            renames: vec![RecipeRename {
                from: "value".into(),
                to: "value".into(),
            }],
            casts: vec![],
            date_parses: vec![],
            ..Default::default()
        },
    )
    .expect("una receta ya satisfecha debe ser válida");
    assert!(!result.changed);
    assert!(!dataset.history.state().can_undo);
    fs::remove_file(path).expect("se debe limpiar el CSV temporal");
}

#[test]
fn structural_recipe_rejects_cross_step_conflicts_and_final_name_collisions() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("a".into(), ["2025-01-01"]).into_column(),
            Series::new("b".into(), ["value"]).into_column(),
        ],
    )
    .unwrap();
    let conflict = TransformRecipe {
        casts: vec![RecipeCast {
            column: "a".into(),
            target: RecipeCastTarget::String,
        }],
        date_parses: vec![RecipeDateParse {
            column: "a".into(),
            format: RecipeDateFormat::Ymd,
            target: RecipeDateTarget::Date,
        }],
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &conflict)
        .err()
        .unwrap()
        .contains("misma receta"));

    let collision = TransformRecipe {
        renames: vec![RecipeRename {
            from: "a".into(),
            to: "b".into(),
        }],
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &collision)
        .err()
        .unwrap()
        .contains("duplicados"));
}

#[test]
fn recipe_filters_use_stable_and_null_safe_semantics() {
    let frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "city".into(),
                [
                    Some("Santo Domingo"),
                    Some("Santiago"),
                    None,
                    Some("santo cielo"),
                ],
            )
            .into_column(),
            Series::new(
                "amount".into(),
                [Some("10"), Some("20"), Some("30"), Some("40")],
            )
            .into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        filters: vec![
            RecipeFilter {
                column: "city".into(),
                operator: RecipeFilterOperator::Contains,
                value: Some("SANTO".into()),
            },
            RecipeFilter {
                column: "amount".into(),
                operator: RecipeFilterOperator::Gte,
                value: Some("20".into()),
            },
        ],
        ..Default::default()
    };
    let (result, _, _, _, removed, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(removed, 3);
    assert_eq!(
        dataset_page(&result, 0, 10).unwrap().rows[0][0].as_deref(),
        Some("santo cielo")
    );

    let null_filter = TransformRecipe {
        filters: vec![RecipeFilter {
            column: "city".into(),
            operator: RecipeFilterOperator::IsNull,
            value: None,
        }],
        ..Default::default()
    };
    assert_eq!(
        apply_recipe_to_frame(&frame, &null_filter)
            .unwrap()
            .0
            .height(),
        1
    );
}

#[test]
fn lazy_recipe_casts_filters_and_calculates_in_one_plan() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("amount".into(), ["5", "12", "20"]).into_column(),
            Series::new("segment".into(), ["discard", "keep", "keep"]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "amount".into(),
            target: RecipeCastTarget::Decimal,
        }],
        filters: vec![RecipeFilter {
            column: "amount".into(),
            operator: RecipeFilterOperator::Gte,
            value: Some("10".into()),
        }],
        calculated_column: Some(CalculatedColumnRecipe {
            name: "total".into(),
            source: "amount".into(),
            operation: CalculatedOperation::Multiply,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "2".into(),
            }),
        }),
        ..Default::default()
    };

    let (result, _, converted, _, removed, calculated, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).expect("la receta simple debe usar lazy");
    assert_eq!(converted, 1);
    assert_eq!(removed, 1);
    assert_eq!(calculated, 1);
    assert_eq!(result.column("amount").unwrap().dtype(), &DataType::Float64);
    assert_eq!(
        dataset_page(&result, 0, 10).unwrap().rows[0][2].as_deref(),
        Some("24.0")
    );
    assert_eq!(
        dataset_page(&result, 0, 10).unwrap().rows[1][2].as_deref(),
        Some("40.0")
    );
}

#[test]
fn lazy_group_summary_preserves_stable_groups_nulls_and_counts() {
    let frame = DataFrame::new(
        5,
        vec![
            Series::new(
                "group".into(),
                [Some("B"), None, Some("B"), None, Some("A")],
            )
            .into_column(),
            Series::new(
                "value".into(),
                [Some("2"), None, Some("4"), Some("8"), None],
            )
            .into_column(),
            Series::new(
                "label".into(),
                [Some("z"), Some("x"), Some("a"), Some("x"), None],
            )
            .into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "value".into(),
            target: RecipeCastTarget::Integer,
        }],
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["group".into()],
            aggregations: vec![
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Sum,
                },
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Mean,
                },
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Count,
                },
                SummaryAggregation {
                    column: "label".into(),
                    operation: SummaryOperation::CountUnique,
                },
                SummaryAggregation {
                    column: "label".into(),
                    operation: SummaryOperation::Min,
                },
            ],
        }),
        ..Default::default()
    };
    let (
        result,
        _,
        cast_count,
        _,
        removed,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        groups,
        aggregations,
        collapsed,
        _,
        _,
        _,
    ) = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(
        (cast_count, removed, groups, aggregations, collapsed),
        (1, 0, 3, 5, 2)
    );
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("B"));
    assert_eq!(rows[1][0], None);
    assert_eq!(rows[0][1].as_deref(), Some("6"));
    assert_eq!(rows[1][1].as_deref(), Some("8"));
    assert_eq!(rows[2][1].as_deref(), Some("0"));
    assert_eq!(rows[1][3].as_deref(), Some("2"));
    assert_eq!(rows[1][4].as_deref(), Some("1"));
    assert_eq!(rows[2][5], None);
}

#[test]
fn lazy_group_summary_applies_literal_replacement_before_grouping() {
    let frame = DataFrame::new(
        4,
        vec![
            Series::new("group".into(), ["A", "A", "B", "B"]).into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
            Series::new("label".into(), ["x", "x", "x", "y"]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("group".into()),
            find: "A".into(),
            replace: "B".into(),
            regex: false,
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["group".into()],
            aggregations: vec![
                SummaryAggregation {
                    column: "value".into(),
                    operation: SummaryOperation::Sum,
                },
                SummaryAggregation {
                    column: "label".into(),
                    operation: SummaryOperation::CountUnique,
                },
            ],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((outcome.6, outcome.15, outcome.17), (2, 1, 3));
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("B"));
    assert_eq!(rows[0][1].as_deref(), Some("10"));
    assert_eq!(rows[0][2].as_deref(), Some("2"));
}

#[test]
fn lazy_group_summary_filters_before_grouping_and_validates_surviving_rows() {
    let frame = DataFrame::new(
        4,
        vec![
            Series::new("group".into(), ["A", "A", "B", "B"]).into_column(),
            Series::new("keep".into(), ["yes", "no", "yes", "no"]).into_column(),
            Series::new("value".into(), [i64::MAX, 1, 2, 20]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        filters: vec![RecipeFilter {
            column: "keep".into(),
            operator: RecipeFilterOperator::Eq,
            value: Some("yes".into()),
        }],
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["group".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((outcome.4, outcome.15, outcome.17), (2, 2, 0));
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("A"));
    assert_eq!(rows[0][1].as_deref(), Some("9223372036854775807"));
    assert_eq!(rows[1][0].as_deref(), Some("B"));
    assert_eq!(rows[1][1].as_deref(), Some("2"));
}

#[test]
fn lazy_group_summary_uses_normalized_contacts_before_grouping() {
    let frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "email".into(),
                [
                    Some(" A@EXAMPLE.COM "),
                    Some("a@example.com"),
                    None,
                    Some("b@example.com"),
                ],
            )
            .into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        contact_normalizations: vec![ContactNormalization {
            column: "email".into(),
            kind: ContactKind::Email,
        }],
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["email".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(
        (outcome.15, outcome.17, outcome.18, outcome.19),
        (3, 1, 1, 1)
    );
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("a@example.com"));
    assert_eq!(rows[0][1].as_deref(), Some("3"));
    assert_eq!(rows[1][0], None);
    assert_eq!(rows[1][1].as_deref(), Some("3"));
    assert_eq!(rows[2][0].as_deref(), Some("b@example.com"));
    assert_eq!(rows[2][1].as_deref(), Some("4"));
}

#[test]
fn lazy_group_summary_uses_text_extractions_before_grouping() {
    let frame = DataFrame::new(
        5,
        vec![
            Series::new(
                "text".into(),
                [
                    Some("A 123|east"),
                    Some("A 456|east"),
                    Some("B 123|west"),
                    None,
                    Some("B 999|west"),
                ],
            )
            .into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4, 5]).into_column(),
        ],
    )
    .unwrap();

    for (kind, delimiter, expected_groups) in [
        (ExtractionKind::FirstToken, None, 3),
        (ExtractionKind::LastToken, None, 5),
        (ExtractionKind::Digits, None, 4),
        (ExtractionKind::Letters, None, 3),
        (ExtractionKind::Before, Some("|"), 5),
        (ExtractionKind::After, Some("|"), 3),
    ] {
        let recipe = TransformRecipe {
            text_extractions: vec![TextExtraction {
                source: "text".into(),
                kind,
                name: "key".into(),
                delimiter: delimiter.map(str::to_owned),
            }],
            group_summary: Some(GroupSummaryRecipe {
                group_by: vec!["key".into()],
                aggregations: vec![
                    SummaryAggregation {
                        column: "value".into(),
                        operation: SummaryOperation::Sum,
                    },
                    SummaryAggregation {
                        column: "key".into(),
                        operation: SummaryOperation::CountUnique,
                    },
                ],
            }),
            ..Default::default()
        };
        assert!(lazy_recipe_supported(&frame, &recipe));
        let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
        assert_eq!(
            (outcome.15, outcome.16, outcome.17, outcome.20),
            (expected_groups, 2, 5 - expected_groups, 1)
        );
    }
}

#[test]
fn lazy_group_summary_uses_calculated_columns_before_grouping() {
    let numeric_frame = DataFrame::new(
        4,
        vec![
            Series::new("region".into(), ["A", "A", "B", "B"]).into_column(),
            Series::new("amount".into(), [10_i64, 20, 5, 15]).into_column(),
            Series::new("adjustment".into(), [1_i64, 2, 5, 0]).into_column(),
        ],
    )
    .unwrap();
    let numeric_recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "total".into(),
            source: "amount".into(),
            operation: CalculatedOperation::Add,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value: "adjustment".into(),
            }),
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["region".into()],
            aggregations: vec![SummaryAggregation {
                column: "total".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&numeric_frame, &numeric_recipe));
    let numeric_outcome = apply_recipe_to_frame(&numeric_frame, &numeric_recipe).unwrap();
    assert_eq!(
        (
            numeric_outcome.5,
            numeric_outcome.15,
            numeric_outcome.16,
            numeric_outcome.17
        ),
        (1, 2, 1, 2)
    );
    let numeric_rows = dataset_page(&numeric_outcome.0, 0, 10).unwrap().rows;
    assert_eq!(numeric_rows[0][0].as_deref(), Some("A"));
    assert_eq!(numeric_rows[0][1].as_deref(), Some("33.0"));
    assert_eq!(numeric_rows[1][0].as_deref(), Some("B"));
    assert_eq!(numeric_rows[1][1].as_deref(), Some("25.0"));

    let text_frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "city".into(),
                [Some("Santo"), Some("Santo"), Some("Santiago"), None],
            )
            .into_column(),
            Series::new("code".into(), [Some("1"), Some("2"), Some("1"), Some("3")]).into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
        ],
    )
    .unwrap();
    let text_recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "label".into(),
            source: "city".into(),
            operation: CalculatedOperation::Concat,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value: "code".into(),
            }),
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["label".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&text_frame, &text_recipe));
    let text_outcome = apply_recipe_to_frame(&text_frame, &text_recipe).unwrap();
    assert_eq!(
        (text_outcome.5, text_outcome.15, text_outcome.17),
        (1, 4, 0)
    );
}

#[test]
fn lazy_group_summary_uses_split_and_merge_columns_before_grouping() {
    let split_frame = DataFrame::new(
        5,
        vec![
            Series::new(
                "location".into(),
                [
                    Some("north|A"),
                    Some("north|B"),
                    Some("south|A"),
                    None,
                    Some("south|B"),
                ],
            )
            .into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4, 5]).into_column(),
        ],
    )
    .unwrap();
    let split_recipe = TransformRecipe {
        split_column: Some(SplitColumnRecipe {
            source: "location".into(),
            delimiter: "|".into(),
            names: vec!["region".into(), "branch".into()],
            drop_source: true,
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["region".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&split_frame, &split_recipe));
    let split_outcome = apply_recipe_to_frame(&split_frame, &split_recipe).unwrap();
    assert_eq!(
        (
            split_outcome.9,
            split_outcome.11,
            split_outcome.15,
            split_outcome.16,
            split_outcome.17
        ),
        (2, 1, 3, 1, 2)
    );
    let split_rows = dataset_page(&split_outcome.0, 0, 10).unwrap().rows;
    assert_eq!(split_rows[0][0].as_deref(), Some("north"));
    assert_eq!(split_rows[0][1].as_deref(), Some("3"));
    assert_eq!(split_rows[1][0].as_deref(), Some("south"));
    assert_eq!(split_rows[1][1].as_deref(), Some("8"));
    assert_eq!(split_rows[2][0], None);
    assert_eq!(split_rows[2][1].as_deref(), Some("4"));

    let eager_outcome = apply_eager_recipe_to_frame(&split_frame, &split_recipe).unwrap();
    assert_eq!(
        (
            eager_outcome.9,
            eager_outcome.11,
            eager_outcome.15,
            eager_outcome.16,
            eager_outcome.17
        ),
        (2, 1, 3, 1, 2)
    );

    let merge_frame = DataFrame::new(
        4,
        vec![
            Series::new("first".into(), [Some("A"), Some("A"), None, None]).into_column(),
            Series::new("last".into(), [Some("x"), Some("x"), Some("z"), None]).into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
        ],
    )
    .unwrap();
    let merge_recipe = TransformRecipe {
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["first".into(), "last".into()],
            name: "full_name".into(),
            separator: " ".into(),
            drop_sources: true,
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["full_name".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&merge_frame, &merge_recipe));
    let merge_outcome = apply_recipe_to_frame(&merge_frame, &merge_recipe).unwrap();
    assert_eq!(
        (
            merge_outcome.10,
            merge_outcome.11,
            merge_outcome.15,
            merge_outcome.16,
            merge_outcome.17
        ),
        (1, 2, 3, 1, 1)
    );
    let merge_rows = dataset_page(&merge_outcome.0, 0, 10).unwrap().rows;
    assert_eq!(merge_rows[0][0].as_deref(), Some("A x"));
    assert_eq!(merge_rows[0][1].as_deref(), Some("3"));
    assert_eq!(merge_rows[1][0].as_deref(), Some("z"));
    assert_eq!(merge_rows[1][1].as_deref(), Some("3"));
    assert_eq!(merge_rows[2][0], None);
    assert_eq!(merge_rows[2][1].as_deref(), Some("4"));
}

#[test]
fn lazy_group_summary_uses_date_parts_before_grouping() {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let days_since_epoch = |year: i32, month: u32, day: u32| {
        (NaiveDate::from_ymd_opt(year, month, day).unwrap() - epoch).num_days() as i32
    };
    let millis = |value: &str| {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
            .timestamp_millis()
    };
    let frame = DataFrame::new(
        4,
        vec![
            Series::new(
                "when_date".into(),
                [
                    Some(days_since_epoch(2024, 1, 2)),
                    Some(days_since_epoch(2024, 2, 3)),
                    Some(days_since_epoch(2025, 1, 2)),
                    None,
                ],
            )
            .cast(&DataType::Date)
            .unwrap()
            .into_column(),
            Series::new(
                "when_datetime".into(),
                [
                    Some(millis("2024-01-02 00:00:00")),
                    Some(millis("2024-02-03 00:00:00")),
                    Some(millis("2025-01-02 00:00:00")),
                    None,
                ],
            )
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
            .unwrap()
            .into_column(),
            Series::new("value".into(), [1_i64, 2, 3, 4]).into_column(),
        ],
    )
    .unwrap();

    let year_recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "year".into(),
            source: "when_date".into(),
            operation: CalculatedOperation::Year,
            operand: None,
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["year".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &year_recipe));
    let year_outcome = apply_recipe_to_frame(&frame, &year_recipe).unwrap();
    assert_eq!(
        (
            year_outcome.5,
            year_outcome.15,
            year_outcome.16,
            year_outcome.17
        ),
        (1, 3, 1, 1)
    );
    let year_rows = dataset_page(&year_outcome.0, 0, 10).unwrap().rows;
    assert_eq!(year_rows[0][0].as_deref(), Some("2024"));
    assert_eq!(year_rows[0][1].as_deref(), Some("3"));
    assert_eq!(year_rows[1][0].as_deref(), Some("2025"));
    assert_eq!(year_rows[1][1].as_deref(), Some("3"));
    assert_eq!(year_rows[2][0], None);
    assert_eq!(year_rows[2][1].as_deref(), Some("4"));

    let month_recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "month".into(),
            source: "when_datetime".into(),
            operation: CalculatedOperation::Month,
            operand: None,
        }),
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["month".into()],
            aggregations: vec![SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            }],
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &month_recipe));
    let month_outcome = apply_recipe_to_frame(&frame, &month_recipe).unwrap();
    assert_eq!(
        (month_outcome.5, month_outcome.15, month_outcome.17),
        (1, 3, 1)
    );
    let month_rows = dataset_page(&month_outcome.0, 0, 10).unwrap().rows;
    assert_eq!(month_rows[0][0].as_deref(), Some("1"));
    assert_eq!(month_rows[0][1].as_deref(), Some("4"));
    assert_eq!(month_rows[1][0].as_deref(), Some("2"));
    assert_eq!(month_rows[1][1].as_deref(), Some("2"));
    assert_eq!(month_rows[2][0], None);
    assert_eq!(month_rows[2][1].as_deref(), Some("4"));
}

#[test]
fn lazy_contact_normalization_preserves_nulls_and_counts_changes() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("email".into(), [Some(" İ@EXAMPLE.COM "), None]).into_column(),
            Series::new("phone".into(), [" +1 (809) 555-01 ", "1+2"]).into_column(),
            Series::new("address".into(), ["  Calle\u{a0}Uno\u{2003}Norte ", "ok"]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        contact_normalizations: vec![
            ContactNormalization {
                column: "email".into(),
                kind: ContactKind::Email,
            },
            ContactNormalization {
                column: "phone".into(),
                kind: ContactKind::Phone,
            },
            ContactNormalization {
                column: "address".into(),
                kind: ContactKind::Address,
            },
        ],
        ..Default::default()
    };
    let (result, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, changed, columns, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((changed, columns), (4, 3));
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("i\u{307}@example.com"));
    assert_eq!(rows[0][1].as_deref(), Some("+180955501"));
    assert_eq!(rows[1][1].as_deref(), Some("12"));
    assert_eq!(rows[0][2].as_deref(), Some("Calle Uno Norte"));
    assert_eq!(rows[1][2].as_deref(), Some("ok"));
    assert_eq!(rows[1][0], None);
}

#[test]
fn lazy_text_extraction_preserves_unicode_tokens_runs_and_missing_matches() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("text".into(), [Some("  José Pérez 123🙂resto"), None]).into_column(),
            Series::new("arabic".into(), [Some("١٢ abc 45"), None]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        text_extractions: vec![
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::FirstToken,
                name: "first".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::LastToken,
                name: "last".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Letters,
                name: "letters".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "arabic".into(),
                kind: ExtractionKind::Digits,
                name: "digits".into(),
                delimiter: None,
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Before,
                name: "before".into(),
                delimiter: Some("🙂".into()),
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::After,
                name: "after".into(),
                delimiter: Some("🙂".into()),
            },
            TextExtraction {
                source: "text".into(),
                kind: ExtractionKind::Before,
                name: "missing".into(),
                delimiter: Some("NO".into()),
            },
        ],
        ..Default::default()
    };
    let (result, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, extracted) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(extracted, 7);
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][2].as_deref(), Some("José"));
    assert_eq!(rows[0][4].as_deref(), Some("José"));
    assert_eq!(rows[0][5].as_deref(), Some("45"));
    assert_eq!(rows[0][7].as_deref(), Some("resto"));
    assert_eq!(rows[0][8], None);
    assert_eq!(rows[1][8], None);
}

#[test]
fn lazy_calculated_division_and_concat_preserve_nulls_and_validate_results() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("left".into(), [Some("10"), None, Some("20")]).into_column(),
            Series::new("right".into(), [Some("A"), Some("B"), Some("C")]).into_column(),
        ],
    )
    .unwrap();
    let concat = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "label".into(),
            source: "left".into(),
            operation: CalculatedOperation::Concat,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value: "right".into(),
            }),
        }),
        ..Default::default()
    };
    let (result, _, _, _, _, calculated, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &concat).unwrap();
    assert_eq!(calculated, 1);
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][2].as_deref(), Some("10A"));
    assert_eq!(rows[1][2], None);

    let divide = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "ratio".into(),
            source: "left".into(),
            operation: CalculatedOperation::Divide,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "2".into(),
            }),
        }),
        ..Default::default()
    };
    let result = apply_recipe_to_frame(&frame, &divide).unwrap().0;
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][2].as_deref(), Some("5.0"));
    assert_eq!(rows[1][2], None);
    assert_eq!(rows[2][2].as_deref(), Some("10.0"));
}

#[test]
fn recipe_filters_validate_the_same_input_independent_of_order() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("group".into(), ["keep", "discard"]).into_column(),
            Series::new("amount".into(), ["10", "invalid"]).into_column(),
        ],
    )
    .unwrap();
    let selective = RecipeFilter {
        column: "group".into(),
        operator: RecipeFilterOperator::Eq,
        value: Some("keep".into()),
    };
    let numeric = RecipeFilter {
        column: "amount".into(),
        operator: RecipeFilterOperator::Gt,
        value: Some("5".into()),
    };

    for filters in [
        vec![selective.clone(), numeric.clone()],
        vec![numeric.clone(), selective.clone()],
    ] {
        let error = apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                filters,
                ..Default::default()
            },
        )
        .err()
        .expect("el valor inválido debe rechazarse sin importar el orden");
        assert!(error.contains("fila 2"));
    }
}

#[test]
fn recipe_calculation_remaps_column_operands_and_preserves_nulls() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("price".into(), [Some("10"), None, Some("5")]).into_column(),
            Series::new("quantity".into(), [Some("2"), Some("3"), Some("4")]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "quantity".into(),
            to: "units".into(),
        }],
        calculated_column: Some(CalculatedColumnRecipe {
            name: "total".into(),
            source: "price".into(),
            operation: CalculatedOperation::Multiply,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Column,
                value: "quantity".into(),
            }),
        }),
        ..Default::default()
    };
    let (result, _, _, _, _, calculated, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(calculated, 1);
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][2].as_deref(), Some("20.0"));
    assert_eq!(rows[1][2], None);
    assert_eq!(rows[2][2].as_deref(), Some("20.0"));
}

#[test]
fn recipe_rejects_division_by_zero_precision_loss_and_too_many_filters() {
    let frame = DataFrame::new(
        1,
        vec![Series::new("value".into(), ["9007199254740993"]).into_column()],
    )
    .unwrap();
    let divide = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "result".into(),
            source: "value".into(),
            operation: CalculatedOperation::Divide,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "0".into(),
            }),
        }),
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &divide)
        .err()
        .unwrap()
        .contains("precisión"));

    let safe = DataFrame::new(1, vec![Series::new("value".into(), ["1"]).into_column()]).unwrap();
    assert!(apply_recipe_to_frame(&safe, &divide)
        .err()
        .unwrap()
        .contains("División por cero"));
    let too_many = TransformRecipe {
        filters: (0..4)
            .map(|_| RecipeFilter {
                column: "value".into(),
                operator: RecipeFilterOperator::Eq,
                value: Some("1".into()),
            })
            .collect(),
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&safe, &too_many)
        .err()
        .unwrap()
        .contains("máximo tres"));
}

#[test]
fn calculated_datetime_parts_support_values_before_unix_epoch() {
    let datetime = Series::new("when".into(), [Some(-1_i64)])
        .cast(&polars::prelude::DataType::Datetime(
            TimeUnit::Milliseconds,
            None,
        ))
        .unwrap()
        .into_column();
    let frame = DataFrame::new(1, vec![datetime]).unwrap();
    let recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "year".into(),
            source: "when".into(),
            operation: CalculatedOperation::Year,
            operand: None,
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&frame, &recipe));
    let result = apply_recipe_to_frame(&frame, &recipe).unwrap().0;
    assert_eq!(
        dataset_page(&result, 0, 1).unwrap().rows[0][1].as_deref(),
        Some("1969")
    );

    for (operation, expected) in [
        (CalculatedOperation::Month, "12"),
        (CalculatedOperation::Day, "31"),
    ] {
        let recipe = TransformRecipe {
            calculated_column: Some(CalculatedColumnRecipe {
                name: "part".into(),
                source: "when".into(),
                operation,
                operand: None,
            }),
            ..Default::default()
        };
        assert!(lazy_recipe_supported(&frame, &recipe));
        let result = apply_recipe_to_frame(&frame, &recipe).unwrap().0;
        assert_eq!(
            dataset_page(&result, 0, 1).unwrap().rows[0][1].as_deref(),
            Some(expected)
        );
    }

    let date = Series::new("when".into(), [Some(0_i32), None, Some(31)])
        .cast(&polars::prelude::DataType::Date)
        .unwrap()
        .into_column();
    let date_frame = DataFrame::new(3, vec![date]).unwrap();
    let date_recipe = TransformRecipe {
        calculated_column: Some(CalculatedColumnRecipe {
            name: "day".into(),
            source: "when".into(),
            operation: CalculatedOperation::Day,
            operand: None,
        }),
        ..Default::default()
    };
    assert!(lazy_recipe_supported(&date_frame, &date_recipe));
    let result = apply_recipe_to_frame(&date_frame, &date_recipe).unwrap().0;
    let rows = dataset_page(&result, 0, 3).unwrap().rows;
    assert_eq!(rows[0][1].as_deref(), Some("1"));
    assert_eq!(rows[1][1], None);
    assert_eq!(rows[2][1].as_deref(), Some("1"));

    let extreme = Series::new("when".into(), [Some(i64::MAX)])
        .cast(&polars::prelude::DataType::Datetime(
            TimeUnit::Milliseconds,
            None,
        ))
        .unwrap()
        .into_column();
    let extreme_frame = DataFrame::new(1, vec![extreme]).unwrap();
    assert!(apply_recipe_to_frame(&extreme_frame, &recipe)
        .err()
        .unwrap()
        .contains("fuera del rango"));
}

#[test]
fn find_replace_is_literal_unicode_null_safe_and_counts_cells() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("text".into(), [Some("á-á"), Some("á"), None]).into_column(),
            Series::new("number".into(), [1_i64, 2, 3]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".into()),
            find: "á".into(),
            replace: "🙂".into(),
            regex: false,
        }),
        ..Default::default()
    };
    let (result, _, _, _, _, _, replaced, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(replaced, 2);
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("🙂-🙂"));
    assert_eq!(rows[2][0], None);
    assert_eq!(
        result.column("number").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    let identical = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".into()),
            find: "á".into(),
            replace: "á".into(),
            regex: false,
        }),
        ..Default::default()
    };
    assert_eq!(apply_recipe_to_frame(&frame, &identical).unwrap().6, 0);
}

#[test]
fn eager_find_replace_regex_supports_captures_and_counts_changed_cells() {
    let frame = DataFrame::new(
        3,
        vec![Series::new("text".into(), [Some("Ana-01"), Some("Luis-02"), None]).into_column()],
    )
    .unwrap();
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".into()),
            find: r"([A-Za-z]+)-(\d+)".into(),
            replace: "$2:$1".into(),
            regex: true,
        }),
        ..Default::default()
    };

    let outcome = apply_eager_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(outcome.6, 2);
    assert_eq!(
        dataset_page(&outcome.0, 0, 10).unwrap().rows,
        vec![
            vec![Some("01:Ana".into())],
            vec![Some("02:Luis".into())],
            vec![None],
        ]
    );
}

#[test]
fn lazy_find_replace_regex_supports_captures_and_counts_changed_cells() {
    let frame = DataFrame::new(
        3,
        vec![Series::new("text".into(), [Some("Ana-01"), Some("Luis-02"), None]).into_column()],
    )
    .unwrap();
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".into()),
            find: r"([A-Za-z]+)-(\d+)".into(),
            replace: "$2:$1".into(),
            regex: true,
        }),
        ..Default::default()
    };

    assert!(lazy_recipe_supported(&frame, &recipe));
    let outcome = apply_lazy_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(outcome.6, 2);
    assert_eq!(
        dataset_page(&outcome.0, 0, 10).unwrap().rows,
        vec![
            vec![Some("01:Ana".into())],
            vec![Some("02:Luis".into())],
            vec![None],
        ]
    );
}

#[test]
fn invalid_find_replace_regex_is_rejected_before_eager_or_lazy_execution() {
    let frame = DataFrame::new(
        1,
        vec![Series::new("text".into(), [Some("Ana")]).into_column()],
    )
    .unwrap();
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("text".into()),
            find: "[".into(),
            replace: "x".into(),
            regex: true,
        }),
        ..Default::default()
    };

    assert!(apply_eager_recipe_to_frame(&frame, &recipe)
        .err()
        .expect("el patrón eager debe fallar")
        .contains("expresión regular válida"));
    assert!(apply_lazy_recipe_to_frame(&frame, &recipe)
        .err()
        .expect("el patrón lazy debe fallar")
        .contains("expresión regular válida"));
}

#[test]
fn find_replace_all_text_columns_skips_physical_non_text_and_remaps_rename() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("first".into(), ["x value"]).into_column(),
            Series::new("second".into(), ["x"]).into_column(),
            Series::new("count".into(), [10_i64]).into_column(),
        ],
    )
    .unwrap();
    let all = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::AllTextColumns,
            column: None,
            find: "x".into(),
            replace: "y".into(),
            regex: false,
        }),
        ..Default::default()
    };
    assert_eq!(apply_recipe_to_frame(&frame, &all).unwrap().6, 2);

    let renamed = TransformRecipe {
        renames: vec![RecipeRename {
            from: "first".into(),
            to: "title".into(),
        }],
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("first".into()),
            find: "x".into(),
            replace: "z".into(),
            regex: false,
        }),
        ..Default::default()
    };
    let result = apply_recipe_to_frame(&frame, &renamed).unwrap().0;
    assert_eq!(
        dataset_page(&result, 0, 1).unwrap().rows[0][0].as_deref(),
        Some("z value")
    );
}

#[test]
fn lazy_find_replace_counts_after_string_cast_and_preserves_nulls() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("code".into(), [12_i64, 20, 30]).into_column(),
            Series::new("note".into(), [Some("x"), None, Some("z")]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "code".into(),
            target: RecipeCastTarget::String,
        }],
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("code".into()),
            find: "2".into(),
            replace: "X".into(),
            regex: false,
        }),
        ..Default::default()
    };

    let (result, _, converted, _, _, _, replaced, _, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(converted, 1);
    assert_eq!(replaced, 2);
    assert_eq!(result.column("code").unwrap().dtype(), &DataType::String);
    assert_eq!(
        dataset_page(&result, 0, 3).unwrap().rows,
        vec![
            vec![Some("1X".into()), Some("x".into())],
            vec![Some("X0".into()), None],
            vec![Some("30".into()), Some("z".into())],
        ]
    );
}

#[test]
fn keep_columns_remaps_reorders_and_reports_drops() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("a".into(), ["A"]).into_column(),
            Series::new("b".into(), ["B"]).into_column(),
            Series::new("c".into(), ["C"]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        renames: vec![RecipeRename {
            from: "a".into(),
            to: "alpha".into(),
        }],
        keep_columns: Some(vec!["c".into(), "a".into()]),
        ..Default::default()
    };
    let (result, _, _, _, _, _, _, dropped, _, _, _, _, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(dropped, 1);
    assert_eq!(
        result
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["c", "alpha"]
    );
}

#[test]
fn keep_columns_rejects_empty_duplicate_missing_and_dropped_calculation_source() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("a".into(), ["1"]).into_column(),
            Series::new("b".into(), ["2"]).into_column(),
        ],
    )
    .unwrap();
    for keep in [vec![], vec!["a".into(), "a".into()], vec!["missing".into()]] {
        assert!(apply_recipe_to_frame(
            &frame,
            &TransformRecipe {
                keep_columns: Some(keep),
                ..Default::default()
            }
        )
        .is_err());
    }
    let calculation = TransformRecipe {
        keep_columns: Some(vec!["b".into()]),
        calculated_column: Some(CalculatedColumnRecipe {
            name: "result".into(),
            source: "a".into(),
            operation: CalculatedOperation::Add,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "1".into(),
            }),
        }),
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &calculation)
        .err()
        .unwrap()
        .contains("descartada"));

    let renamed_success = TransformRecipe {
        renames: vec![RecipeRename {
            from: "a".into(),
            to: "alpha".into(),
        }],
        keep_columns: Some(vec!["a".into()]),
        calculated_column: Some(CalculatedColumnRecipe {
            name: "result".into(),
            source: "a".into(),
            operation: CalculatedOperation::Add,
            operand: Some(CalculatedOperand {
                kind: CalculatedOperandKind::Literal,
                value: "1".into(),
            }),
        }),
        ..Default::default()
    };
    let result = apply_recipe_to_frame(&frame, &renamed_success).unwrap().0;
    assert_eq!(
        result
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "result"]
    );
}

#[test]
fn reorder_only_keep_columns_publishes_one_undo_revision() {
    let path = temporary_csv("a,b\nA,B\n");
    let (frame, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            keep_columns: Some(vec!["b".into(), "a".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(result.dropped_column_count, 0);
    assert_eq!(
        dataset
            .frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "a"]
    );
    assert!(dataset.history.state().can_undo);
    undo_dataset(&mut dataset).unwrap();
    assert_eq!(
        dataset
            .frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn split_is_literal_unicode_uses_remainder_and_preserves_missing_null_and_empty() {
    let frame = DataFrame::new(
        4,
        vec![Series::new(
            "path".into(),
            [
                Some("uno🙂dos🙂tres🙂resto"),
                Some("solo"),
                None,
                Some("a🙂"),
            ],
        )
        .into_column()],
    )
    .unwrap();
    let recipe = TransformRecipe {
        split_column: Some(SplitColumnRecipe {
            source: "path".into(),
            delimiter: "🙂".into(),
            names: vec!["first".into(), "second".into(), "third".into()],
            drop_source: false,
        }),
        ..Default::default()
    };
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!(outcome.9, 3);
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[0][3].as_deref(), Some("tres🙂resto"));
    assert_eq!(rows[1][1].as_deref(), Some("solo"));
    assert_eq!(rows[1][2], None);
    assert_eq!(rows[2][1], None);
    assert_eq!(rows[3][2].as_deref(), Some(""));
    assert_eq!(rows[3][3], None);
}

#[test]
fn merge_preserves_source_order_nulls_empty_strings_and_separator() {
    let frame = DataFrame::new(
        3,
        vec![
            Series::new("a".into(), [Some("A"), None, None]).into_column(),
            Series::new("b".into(), [Some(""), Some("B"), None]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["a".into(), "b".into()],
            name: "joined".into(),
            separator: "🙂".into(),
            drop_sources: true,
        }),
        ..Default::default()
    };
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((outcome.10, outcome.11), (1, 2));
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("A🙂"));
    assert_eq!(rows[1][0].as_deref(), Some("B"));
    assert_eq!(rows[2][0], None);
}

#[test]
fn lazy_merge_accepts_numeric_source_cast_to_text() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("number".into(), [12_i64, 34]).into_column(),
            Series::new("label".into(), ["A", "B"]).into_column(),
        ],
    )
    .unwrap();
    let recipe = TransformRecipe {
        casts: vec![RecipeCast {
            column: "number".into(),
            target: RecipeCastTarget::String,
        }],
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["number".into(), "label".into()],
            name: "joined".into(),
            separator: ":".into(),
            drop_sources: true,
        }),
        ..Default::default()
    };

    let (result, _, converted, _, _, _, _, _, _, _, merged, dropped, _, _, _, _, _, _, _, _, _) =
        apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((converted, merged, dropped), (1, 1, 2));
    assert_eq!(
        dataset_page(&result, 0, 2).unwrap().rows,
        vec![vec![Some("12:A".into())], vec![Some("34:B".into())],]
    );
}

#[test]
fn split_merge_remap_renames_and_validate_keep_and_drop_dependencies() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("full".into(), ["A-B"]).into_column(),
            Series::new("other".into(), ["C"]).into_column(),
        ],
    )
    .unwrap();
    let success = TransformRecipe {
        renames: vec![RecipeRename {
            from: "full".into(),
            to: "renamed".into(),
        }],
        keep_columns: Some(vec!["full".into(), "other".into()]),
        split_column: Some(SplitColumnRecipe {
            source: "full".into(),
            delimiter: "-".into(),
            names: vec!["left".into(), "right".into()],
            drop_source: false,
        }),
        merge_columns: Some(MergeColumnsRecipe {
            sources: vec!["full".into(), "other".into()],
            name: "joined".into(),
            separator: ":".into(),
            drop_sources: false,
        }),
        ..Default::default()
    };
    let result = apply_recipe_to_frame(&frame, &success).unwrap().0;
    assert_eq!(
        dataset_page(&result, 0, 1).unwrap().rows[0][4].as_deref(),
        Some("A-B:C")
    );

    let dropped = TransformRecipe {
        keep_columns: Some(vec!["other".into()]),
        split_column: success.split_column.clone(),
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &dropped)
        .err()
        .unwrap()
        .contains("keepColumns"));
    let conflict = TransformRecipe {
        split_column: Some(SplitColumnRecipe {
            drop_source: true,
            ..success.split_column.unwrap()
        }),
        merge_columns: success.merge_columns,
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &conflict)
        .err()
        .unwrap()
        .contains("descartaría"));
}

#[test]
fn invalid_split_collision_rolls_back_combined_recipe_without_undo() {
    let path = temporary_csv("full,existing\nA-B,x\n");
    let (frame, _) = load_csv(&path).unwrap();
    let original = frame.clone();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let recipe = TransformRecipe {
        find_replace: Some(FindReplaceRecipe {
            scope: FindReplaceScope::Column,
            column: Some("full".into()),
            find: "A".into(),
            replace: "Z".into(),
            regex: false,
        }),
        split_column: Some(SplitColumnRecipe {
            source: "full".into(),
            delimiter: "-".into(),
            names: vec!["existing".into(), "new".into()],
            drop_source: false,
        }),
        ..Default::default()
    };
    assert!(apply_recipe_to_dataset(&mut dataset, &recipe).is_err());
    assert!(dataset.frame.equals_missing(&original));
    assert!(!dataset.history.state().can_undo);
    fs::remove_file(path).unwrap();
}

#[test]
fn split_and_merge_observe_casts_but_reject_non_text_physical_columns() {
    let frame = DataFrame::new(
        1,
        vec![
            Series::new("number".into(), [12_i64]).into_column(),
            Series::new("text".into(), ["3-4"]).into_column(),
        ],
    )
    .unwrap();
    let cast_to_text = TransformRecipe {
        casts: vec![RecipeCast {
            column: "number".into(),
            target: RecipeCastTarget::String,
        }],
        split_column: Some(SplitColumnRecipe {
            source: "number".into(),
            delimiter: "2".into(),
            names: vec!["one".into(), "two".into()],
            drop_source: false,
        }),
        ..Default::default()
    };
    assert_eq!(apply_recipe_to_frame(&frame, &cast_to_text).unwrap().9, 2);

    let cast_away = TransformRecipe {
        casts: vec![RecipeCast {
            column: "text".into(),
            target: RecipeCastTarget::Integer,
        }],
        split_column: Some(SplitColumnRecipe {
            source: "text".into(),
            delimiter: "-".into(),
            names: vec!["one".into(), "two".into()],
            drop_source: false,
        }),
        ..Default::default()
    };
    assert!(apply_recipe_to_frame(&frame, &cast_away).is_err());
}

#[test]
fn split_and_merge_commit_as_one_undo_revision_with_metadata() {
    let path = temporary_csv("full,other\nA-B,C\n");
    let (frame, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            split_column: Some(SplitColumnRecipe {
                source: "full".into(),
                delimiter: "-".into(),
                names: vec!["left".into(), "right".into()],
                drop_source: true,
            }),
            merge_columns: Some(MergeColumnsRecipe {
                sources: vec!["left".into(), "other".into()],
                name: "joined".into(),
                separator: ":".into(),
                drop_sources: false,
            }),
            ..Default::default()
        },
    );
    // References produced by split are intentionally outside the contract.
    assert!(result.is_err());
    assert!(!dataset.history.state().can_undo);

    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            split_column: Some(SplitColumnRecipe {
                source: "full".into(),
                delimiter: "-".into(),
                names: vec!["left".into(), "right".into()],
                drop_source: true,
            }),
            merge_columns: None,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.split_column_count,
            result.dropped_source_column_count
        ),
        (2, 1)
    );
    assert!(dataset.history.state().can_undo);
    undo_dataset(&mut dataset).unwrap();
    assert_eq!(
        dataset
            .frame
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["full", "other"]
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn iqr_cap_uses_linear_quantiles_preserves_nulls_and_strict_boundaries() {
    let frame = DataFrame::new(
        6,
        vec![Series::new(
            "value".into(),
            [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
        )
        .into_column()],
    )
    .unwrap();
    let recipe = TransformRecipe {
        outlier_treatments: vec![OutlierTreatment {
            column: "value".into(),
            action: OutlierAction::Cap,
        }],
        ..Default::default()
    };
    let outcome = apply_recipe_to_frame(&frame, &recipe).unwrap();
    assert_eq!((outcome.12, outcome.13, outcome.14), (1, 0, 1));
    assert_eq!(
        outcome.0.column("value").unwrap().dtype(),
        &polars::prelude::DataType::Float64
    );
    let rows = dataset_page(&outcome.0, 0, 10).unwrap().rows;
    assert_eq!(rows[4][0].as_deref(), Some("7.0"));
    assert_eq!(rows[5][0], None);

    let boundary = DataFrame::new(
        5,
        vec![Series::new("value".into(), [1_i64, 2, 3, 4, 7]).into_column()],
    )
    .unwrap();
    let no_op = apply_recipe_to_frame(&boundary, &recipe).unwrap();
    assert_eq!(no_op.12, 0);
    assert_eq!(
        no_op.0.column("value").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    let zero_iqr = DataFrame::new(
        4,
        vec![Series::new("value".into(), [5_i64; 4]).into_column()],
    )
    .unwrap();
    assert_eq!(apply_recipe_to_frame(&zero_iqr, &recipe).unwrap().12, 0);

    let imputed = apply_recipe_to_frame(
        &frame,
        &TransformRecipe {
            outlier_treatments: vec![OutlierTreatment {
                column: "value".into(),
                action: OutlierAction::Impute,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((imputed.12, imputed.13, imputed.14), (1, 0, 1));
    assert_eq!(
        imputed.0.column("value").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    assert_eq!(
        imputed.0.column("value").unwrap().i64().unwrap().get(4),
        Some(3)
    );
    assert_eq!(
        imputed.0.column("value").unwrap().i64().unwrap().get(5),
        None
    );
}

#[test]
fn direct_outlier_modes_report_affected_rows_and_preserve_nulls() {
    let frame = DataFrame::new(
        6,
        vec![Series::new(
            "amount".into(),
            [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
        )
        .into_column()],
    )
    .unwrap();

    let (capped, cap_rows, cap_cells, cap_columns) =
        apply_outlier_mode(&frame, OutlierMode::Cap).unwrap();
    assert_eq!((cap_rows, cap_cells, cap_columns.len()), (1, 1, 1));
    assert_eq!(capped.column("amount").unwrap().dtype(), &DataType::Float64);
    assert_eq!(
        capped.column("amount").unwrap().f64().unwrap().get(4),
        Some(7.0)
    );
    assert_eq!(capped.column("amount").unwrap().f64().unwrap().get(5), None);

    let (dropped, drop_rows, drop_cells, drop_columns) =
        apply_outlier_mode(&frame, OutlierMode::Drop).unwrap();
    assert_eq!((drop_rows, drop_cells, drop_columns.len()), (1, 1, 1));
    assert_eq!(dropped.height(), 5);
}

#[test]
fn direct_outlier_imputation_preserves_numeric_types_and_audit_column() {
    let frame = DataFrame::new(
        6,
        vec![
            Series::new(
                "amount".into(),
                [Some(1_i64), Some(2), Some(3), Some(4), Some(100), None],
            )
            .into_column(),
            Series::new(
                "ratio".into(),
                [
                    Some(1.0),
                    Some(2.0),
                    Some(3.0),
                    Some(4.0),
                    Some(100.0),
                    None,
                ],
            )
            .into_column(),
            Series::new(
                "_cambios".into(),
                [Some(""), Some(""), Some(""), Some(""), Some("manual"), None],
            )
            .into_column(),
        ],
    )
    .unwrap();

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        impute_outlier_values_in_frame(&frame).unwrap();
    assert_eq!(affected_rows, 1);
    assert_eq!(changed_cells, 2);
    assert_eq!(changed_columns.len(), 2);
    assert_eq!(
        cleaned.column("amount").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    assert_eq!(
        cleaned.column("ratio").unwrap().dtype(),
        &polars::prelude::DataType::Float64
    );
    assert_eq!(
        cleaned.column("amount").unwrap().i64().unwrap().get(4),
        Some(3)
    );
    assert_eq!(
        cleaned.column("ratio").unwrap().f64().unwrap().get(4),
        Some(3.0)
    );
    assert_eq!(
        cleaned.column("_cambios").unwrap().str().unwrap().get(4),
        Some("manual")
    );
    assert_eq!(
        cleaned.column("amount").unwrap().i64().unwrap().get(5),
        None
    );
}

#[test]
fn categorical_imputation_uses_desconocido_without_touching_row_audit() {
    let frame = DataFrame::new(
        5,
        vec![
            Series::new(
                "status".into(),
                [Some("active"), None, Some("inactive"), None, None],
            )
            .into_column(),
            Series::new(
                "_cambios".into(),
                [Some("manual"), None, Some(""), None, None],
            )
            .into_column(),
        ],
    )
    .unwrap();

    let (cleaned, affected_rows, changed_cells, changed_columns) =
        impute_categorical_values_in_frame(&frame).unwrap();
    assert_eq!(affected_rows, 3);
    assert_eq!(changed_cells, 3);
    assert_eq!(changed_columns.len(), 1);
    assert_eq!(changed_columns[0].name, "status");
    assert_eq!(changed_columns[0].changed_cell_count, 3);
    assert_eq!(
        cleaned.column("status").unwrap().str().unwrap().get(1),
        Some("Desconocido")
    );
    assert_eq!(
        cleaned.column("_cambios").unwrap().str().unwrap().get(1),
        None
    );
}

#[test]
fn iqr_drop_treatments_are_order_independent_and_share_one_baseline() {
    let frame = DataFrame::new(
        6,
        vec![
            Series::new("a".into(), [1.0, 2.0, 3.0, 4.0, 100.0, 3.0]).into_column(),
            Series::new("b".into(), [1.0, 2.0, 3.0, 4.0, 3.0, 100.0]).into_column(),
        ],
    )
    .unwrap();
    let treatments = vec![
        OutlierTreatment {
            column: "a".into(),
            action: OutlierAction::Drop,
        },
        OutlierTreatment {
            column: "b".into(),
            action: OutlierAction::Drop,
        },
    ];
    let first = apply_recipe_to_frame(
        &frame,
        &TransformRecipe {
            outlier_treatments: treatments.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let second = apply_recipe_to_frame(
        &frame,
        &TransformRecipe {
            outlier_treatments: treatments.into_iter().rev().collect(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((first.13, first.14), (2, 2));
    assert!(first.0.equals_missing(&second.0));

    let mixed = apply_recipe_to_frame(
        &frame,
        &TransformRecipe {
            outlier_treatments: vec![
                OutlierTreatment {
                    column: "a".into(),
                    action: OutlierAction::Cap,
                },
                OutlierTreatment {
                    column: "b".into(),
                    action: OutlierAction::Drop,
                },
            ],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((mixed.12, mixed.13), (1, 1));
}

#[test]
fn iqr_validates_minimum_duplicates_limits_nonfinite_and_precision() {
    let small = DataFrame::new(
        3,
        vec![Series::new("x".into(), [1_i64, 2, 100]).into_column()],
    )
    .unwrap();
    let treatment = OutlierTreatment {
        column: "x".into(),
        action: OutlierAction::Cap,
    };
    assert!(apply_recipe_to_frame(
        &small,
        &TransformRecipe {
            outlier_treatments: vec![treatment.clone()],
            ..Default::default()
        }
    )
    .is_err());
    assert!(apply_recipe_to_frame(
        &small,
        &TransformRecipe {
            outlier_treatments: vec![treatment.clone(), treatment.clone()],
            ..Default::default()
        }
    )
    .is_err());
    assert!(apply_recipe_to_frame(
        &small,
        &TransformRecipe {
            outlier_treatments: vec![treatment.clone(); 17],
            ..Default::default()
        }
    )
    .is_err());

    let nonfinite = DataFrame::new(
        4,
        vec![Series::new("x".into(), [1.0, 2.0, 3.0, f64::INFINITY]).into_column()],
    )
    .unwrap();
    assert!(apply_recipe_to_frame(
        &nonfinite,
        &TransformRecipe {
            outlier_treatments: vec![treatment.clone()],
            ..Default::default()
        }
    )
    .err()
    .unwrap()
    .contains("infinito"));
    let precision = DataFrame::new(
        4,
        vec![Series::new("x".into(), [1_i64, 2, 3, 9_007_199_254_740_993]).into_column()],
    )
    .unwrap();
    assert!(apply_recipe_to_frame(
        &precision,
        &TransformRecipe {
            outlier_treatments: vec![treatment],
            ..Default::default()
        }
    )
    .err()
    .unwrap()
    .contains("precisión"));
    let overflow = DataFrame::new(
        4,
        vec![Series::new("x".into(), [-1e308, -5e307, 5e307, 1e308]).into_column()],
    )
    .unwrap();
    assert!(apply_recipe_to_frame(
        &overflow,
        &TransformRecipe {
            outlier_treatments: vec![OutlierTreatment {
                column: "x".into(),
                action: OutlierAction::Drop
            }],
            ..Default::default()
        }
    )
    .err()
    .unwrap()
    .contains("rango numérico"));
}

#[test]
fn iqr_remaps_rename_observes_cast_and_keep_and_commits_one_undo() {
    let path = temporary_csv("value,other\n1,a\n2,b\n3,c\n4,d\n100,e\n");
    let (frame, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            renames: vec![RecipeRename {
                from: "value".into(),
                to: "amount".into(),
            }],
            casts: vec![RecipeCast {
                column: "value".into(),
                target: RecipeCastTarget::Integer,
            }],
            keep_columns: Some(vec!["value".into()]),
            outlier_treatments: vec![OutlierTreatment {
                column: "value".into(),
                action: OutlierAction::Cap,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.adjusted_outlier_cell_count,
            result.outlier_column_count
        ),
        (1, 1)
    );
    assert!(dataset.history.state().can_undo);
    undo_dataset(&mut dataset).unwrap();
    assert_eq!(dataset.frame.width(), 2);
    fs::remove_file(path).unwrap();
}

#[test]
fn group_summary_is_stable_supports_null_keys_and_count_semantics() {
    let frame = DataFrame::new(
        5,
        vec![
            Series::new(
                "group".into(),
                [Some("B"), None, Some("B"), None, Some("A")],
            )
            .into_column(),
            Series::new("value".into(), [Some(2_i64), None, Some(4), Some(8), None]).into_column(),
            Series::new(
                "label".into(),
                [Some("z"), Some("x"), Some("a"), Some("x"), None],
            )
            .into_column(),
        ],
    )
    .unwrap();
    let summary = GroupSummaryRecipe {
        group_by: vec!["group".into()],
        aggregations: vec![
            SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Sum,
            },
            SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Mean,
            },
            SummaryAggregation {
                column: "value".into(),
                operation: SummaryOperation::Count,
            },
            SummaryAggregation {
                column: "label".into(),
                operation: SummaryOperation::CountUnique,
            },
            SummaryAggregation {
                column: "label".into(),
                operation: SummaryOperation::Min,
            },
        ],
    };
    let (result, groups, aggregations, collapsed) =
        apply_group_summary(frame, &summary, &HashMap::new()).unwrap();
    assert_eq!((groups, aggregations, collapsed), (3, 5, 2));
    assert_eq!(
        result
            .get_column_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "group",
            "value_sum",
            "value_mean",
            "value_count",
            "label_count_unique",
            "label_min"
        ]
    );
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("B"));
    assert_eq!(rows[1][0], None);
    assert_eq!(rows[0][1].as_deref(), Some("6"));
    assert_eq!(rows[1][1].as_deref(), Some("8"));
    assert_eq!(rows[2][1].as_deref(), Some("0"));
    assert_eq!(rows[1][3].as_deref(), Some("2"));
    assert_eq!(rows[1][4].as_deref(), Some("1"));
    assert_eq!(rows[2][5], None);
}

#[test]
fn group_summary_min_max_preserve_types_and_handle_negative_and_all_null_groups() {
    let dates = Series::new("date".into(), [Some(0_i32), Some(1), None, None])
        .cast(&polars::prelude::DataType::Date)
        .unwrap()
        .into_column();
    let datetimes = Series::new("time".into(), [Some(-1_i64), Some(1), None, None])
        .cast(&polars::prelude::DataType::Datetime(
            TimeUnit::Milliseconds,
            None,
        ))
        .unwrap()
        .into_column();
    let frame = DataFrame::new(
        4,
        vec![
            Series::new("g".into(), ["x", "x", "n", "n"]).into_column(),
            Series::new("number".into(), [Some(-2_i64), Some(-10), None, None]).into_column(),
            dates,
            datetimes,
        ],
    )
    .unwrap();
    let summary = GroupSummaryRecipe {
        group_by: vec!["g".into()],
        aggregations: vec![
            SummaryAggregation {
                column: "number".into(),
                operation: SummaryOperation::Min,
            },
            SummaryAggregation {
                column: "number".into(),
                operation: SummaryOperation::Max,
            },
            SummaryAggregation {
                column: "date".into(),
                operation: SummaryOperation::Min,
            },
            SummaryAggregation {
                column: "time".into(),
                operation: SummaryOperation::Max,
            },
        ],
    };
    let result = apply_group_summary(frame, &summary, &HashMap::new())
        .unwrap()
        .0;
    assert_eq!(
        result.column("number_min").unwrap().dtype(),
        &polars::prelude::DataType::Int64
    );
    assert_eq!(
        result.column("date_min").unwrap().dtype(),
        &polars::prelude::DataType::Date
    );
    assert!(matches!(
        result.column("time_max").unwrap().dtype(),
        polars::prelude::DataType::Datetime(_, _)
    ));
    let rows = dataset_page(&result, 0, 10).unwrap().rows;
    assert_eq!(rows[0][1].as_deref(), Some("-10"));
    assert_eq!(rows[0][2].as_deref(), Some("-2"));
    assert_eq!(rows[1][1], None);
    assert_eq!(rows[1][3], None);
}

#[test]
fn group_summary_rejects_overflow_precision_nonfinite_duplicates_and_missing_dependencies() {
    let overflow = DataFrame::new(
        2,
        vec![
            Series::new("g".into(), ["x", "x"]).into_column(),
            Series::new("v".into(), [i64::MAX, 1]).into_column(),
        ],
    )
    .unwrap();
    let sum = GroupSummaryRecipe {
        group_by: vec!["g".into()],
        aggregations: vec![SummaryAggregation {
            column: "v".into(),
            operation: SummaryOperation::Sum,
        }],
    };
    assert!(apply_group_summary(overflow, &sum, &HashMap::new())
        .err()
        .unwrap()
        .contains("desbordó"));
    let zeros = DataFrame::new(
        3,
        vec![
            Series::new("g".into(), ["x", "x", "x"]).into_column(),
            Series::new("v".into(), [-0.0, 0.0, f64::NAN]).into_column(),
        ],
    )
    .unwrap();
    let unique = GroupSummaryRecipe {
        group_by: vec!["g".into()],
        aggregations: vec![SummaryAggregation {
            column: "v".into(),
            operation: SummaryOperation::CountUnique,
        }],
    };
    assert!(apply_group_summary(zeros, &unique, &HashMap::new()).is_err());
    let signed_zero = DataFrame::new(
        3,
        vec![
            Series::new("g".into(), ["x", "x", "x"]).into_column(),
            Series::new("v".into(), [Some(-0.0), Some(0.0), None]).into_column(),
        ],
    )
    .unwrap();
    let result = apply_group_summary(signed_zero, &unique, &HashMap::new())
        .unwrap()
        .0;
    assert_eq!(
        dataset_page(&result, 0, 1).unwrap().rows[0][1].as_deref(),
        Some("1")
    );
    let precision = DataFrame::new(
        2,
        vec![
            Series::new("g".into(), ["x", "x"]).into_column(),
            Series::new("v".into(), [9_007_199_254_740_993_i64, 1]).into_column(),
        ],
    )
    .unwrap();
    let mean = GroupSummaryRecipe {
        group_by: vec!["g".into()],
        aggregations: vec![SummaryAggregation {
            column: "v".into(),
            operation: SummaryOperation::Mean,
        }],
    };
    assert!(apply_group_summary(precision, &mean, &HashMap::new())
        .err()
        .unwrap()
        .contains("precisión"));
}

#[test]
fn group_summary_runs_after_outliers_and_commits_one_undo_revision() {
    let path = temporary_csv("g,v\na,1\na,2\na,3\na,4\na,100\n");
    let (frame, _) = load_csv(&path).unwrap();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let result = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            casts: vec![RecipeCast {
                column: "v".into(),
                target: RecipeCastTarget::Integer,
            }],
            outlier_treatments: vec![OutlierTreatment {
                column: "v".into(),
                action: OutlierAction::Drop,
            }],
            group_summary: Some(GroupSummaryRecipe {
                group_by: vec!["g".into()],
                aggregations: vec![SummaryAggregation {
                    column: "v".into(),
                    operation: SummaryOperation::Sum,
                }],
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.group_count,
            result.aggregated_column_count,
            result.collapsed_row_count
        ),
        (1, 1, 3)
    );
    assert_eq!(
        dataset_page(&dataset.frame, 0, 1).unwrap().rows[0][1].as_deref(),
        Some("10")
    );
    assert!(dataset.history.state().can_undo);
    undo_dataset(&mut dataset).unwrap();
    assert_eq!(dataset.frame.height(), 5);
    fs::remove_file(path).unwrap();
}

#[test]
fn contacts_normalize_unicode_phone_and_address_and_count_changed_cells() {
    let frame = DataFrame::new(
        2,
        vec![
            Series::new("email".into(), [Some(" İ@EXAMPLE.COM "), None]).into_column(),
            Series::new("phone".into(), [" +1 (809) 555-01 ", "1+2"]).into_column(),
            Series::new("address".into(), ["  Calle\u{a0}Uno\u{2003}Norte ", "ok"]).into_column(),
        ],
    )
    .unwrap();
    let mut candidate = frame;
    let (cells, columns) = apply_contact_normalizations(
        &mut candidate,
        &[
            ContactNormalization {
                column: "email".into(),
                kind: ContactKind::Email,
            },
            ContactNormalization {
                column: "phone".into(),
                kind: ContactKind::Phone,
            },
            ContactNormalization {
                column: "address".into(),
                kind: ContactKind::Address,
            },
        ],
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!((cells, columns), (4, 3));
    let rows = dataset_page(&candidate, 0, 10).unwrap().rows;
    assert_eq!(rows[0][0].as_deref(), Some("i\u{307}@example.com"));
    assert_eq!(rows[0][1].as_deref(), Some("+180955501"));
    assert_eq!(rows[1][1].as_deref(), Some("12"));
    assert_eq!(rows[0][2].as_deref(), Some("Calle Uno Norte"));
    assert_eq!(rows[1][2].as_deref(), Some("ok"));
}

#[test]
fn text_extractions_cover_unicode_tokens_runs_delimiters_and_all_null() {
    let mut frame = DataFrame::new(
        2,
        vec![
            Series::new("text".into(), [Some("  José Pérez 123🙂resto"), None]).into_column(),
            Series::new("arabic".into(), [Some("١٢ abc 45"), None]).into_column(),
        ],
    )
    .unwrap();
    let extractions = vec![
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::FirstToken,
            name: "first".into(),
            delimiter: None,
        },
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::LastToken,
            name: "last".into(),
            delimiter: None,
        },
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::Letters,
            name: "letters".into(),
            delimiter: None,
        },
        TextExtraction {
            source: "arabic".into(),
            kind: ExtractionKind::Digits,
            name: "digits".into(),
            delimiter: None,
        },
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::Before,
            name: "before".into(),
            delimiter: Some("🙂".into()),
        },
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::After,
            name: "after".into(),
            delimiter: Some("🙂".into()),
        },
        TextExtraction {
            source: "text".into(),
            kind: ExtractionKind::Before,
            name: "missing".into(),
            delimiter: Some("NO".into()),
        },
    ];
    assert_eq!(
        apply_text_extractions(&mut frame, &extractions, &HashMap::new()).unwrap(),
        7
    );
    let rows = dataset_page(&frame, 0, 10).unwrap().rows;
    assert_eq!(rows[0][2].as_deref(), Some("José"));
    assert_eq!(rows[0][4].as_deref(), Some("José"));
    assert_eq!(rows[0][5].as_deref(), Some("45"));
    assert_eq!(rows[0][7].as_deref(), Some("resto"));
    assert_eq!(rows[0][8], None);
    assert_eq!(rows[1][8], None);
    assert_eq!(
        frame.column("missing").unwrap().dtype(),
        &polars::prelude::DataType::String
    );
}

#[test]
fn contacts_and_extractions_validate_remap_keep_group_and_rollback() {
    let path = temporary_csv("contact,other\n A@B.COM ,x\n");
    let (frame, _) = load_csv(&path).unwrap();
    let original = frame.clone();
    let mut dataset = loaded_dataset(path.clone(), frame);
    let success = apply_recipe_to_dataset(
        &mut dataset,
        &TransformRecipe {
            renames: vec![RecipeRename {
                from: "contact".into(),
                to: "email".into(),
            }],
            keep_columns: Some(vec!["contact".into()]),
            contact_normalizations: vec![ContactNormalization {
                column: "contact".into(),
                kind: ContactKind::Email,
            }],
            text_extractions: vec![TextExtraction {
                source: "contact".into(),
                kind: ExtractionKind::Before,
                name: "user".into(),
                delimiter: Some("@".into()),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (
            success.normalized_contact_cell_count,
            success.normalized_contact_column_count,
            success.extracted_column_count
        ),
        (1, 1, 1)
    );
    assert!(dataset.history.state().can_undo);
    undo_dataset(&mut dataset).unwrap();
    assert!(dataset.frame.equals_missing(&original));

    let grouped = TransformRecipe {
        text_extractions: vec![TextExtraction {
            source: "contact".into(),
            kind: ExtractionKind::FirstToken,
            name: "new".into(),
            delimiter: None,
        }],
        group_summary: Some(GroupSummaryRecipe {
            group_by: vec!["contact".into()],
            aggregations: vec![SummaryAggregation {
                column: "other".into(),
                operation: SummaryOperation::Count,
            }],
        }),
        ..Default::default()
    };
    let grouped = apply_recipe_to_dataset(&mut dataset, &grouped).unwrap();
    assert_eq!(
        (
            grouped.group_count,
            grouped.aggregated_column_count,
            grouped.extracted_column_count
        ),
        (1, 1, 1)
    );
    assert!(dataset.history.state().can_undo);
    fs::remove_file(path).unwrap();
}

fn quality_rule(column: &str, kind: QualityRuleKind) -> QualityRule {
    QualityRule {
        column: column.to_owned(),
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
    }
}

#[test]
fn quality_rules_apply_explicit_null_duplicate_empty_and_range_semantics() {
    let frame = df![
        "text" => &[Some("a"), Some("  "), None, Some("a"), Some("b")],
        "integer" => &[Some(1_i64), Some(2), None, Some(4), Some(5)],
        "decimal" => &[Some(1.0_f64), Some(f64::NAN), Some(f64::INFINITY), None, Some(5.0)]
    ]
    .unwrap();
    let mut non_empty = quality_rule("text", QualityRuleKind::NonEmpty);
    non_empty.max_invalid = Some(2);
    let mut unique = quality_rule("text", QualityRuleKind::Unique);
    unique.max_invalid = Some(2);
    let mut integer_range = quality_rule("integer", QualityRuleKind::NumericRange);
    integer_range.min = Some(1.0);
    integer_range.max = Some(4.0);
    integer_range.max_invalid = Some(2);
    let mut float_range = quality_rule("decimal", QualityRuleKind::NumericRange);
    float_range.min = Some(0.0);
    float_range.max_invalid = Some(3);

    let result = evaluate_quality_rules(
        &frame,
        &[
            quality_rule("text", QualityRuleKind::NotNull),
            non_empty,
            unique,
            integer_range,
            float_range,
        ],
    )
    .unwrap();

    assert_eq!(result.row_count, 5);
    assert_eq!(result.total_rules, 5);
    assert_eq!(result.rules[0].invalid_count, 1);
    assert!(!result.rules[0].passed);
    assert_eq!(result.rules[1].invalid_count, 2);
    assert_eq!(result.rules[2].invalid_count, 2);
    assert_eq!(result.rules[3].invalid_count, 2);
    assert_eq!(result.rules[4].invalid_count, 3);
    assert!(result.rules[1..].iter().all(|rule| rule.passed));
    assert_eq!(result.failed_rules, 1);
    assert!(!result.passed);
}

#[test]
fn quality_rules_apply_the_first_advanced_v3_slice() {
    let frame = df![
        "status" => &[Some("ok"), Some("bad"), None, Some("ok")],
        "email" => &[Some("a@example.com"), Some("invalid"), Some("b@example.com"), Some("c@example.com")],
        "number" => &[1_i64, 2, 3, 4],
        "key_a" => &["a", "a", "a", "b"],
        "key_b" => &[1_i64, 1, 2, 1]
    ]
    .unwrap();

    let mut allowed_values = quality_rule("status", QualityRuleKind::AllowedValues);
    allowed_values.values = Some(vec!["ok".to_owned(), "pending".to_owned()]);
    allowed_values.max_invalid = Some(2);

    let mut regex = quality_rule("email", QualityRuleKind::Regex);
    regex.pattern = Some(r"^[^@]+@[^@]+$".to_owned());
    regex.max_invalid = Some(1);

    let mut dtype = quality_rule("number", QualityRuleKind::Dtype);
    dtype.dtype = Some("integer".to_owned());

    let mut unique_together = quality_rule("key_a", QualityRuleKind::UniqueTogether);
    unique_together.columns = Some(vec!["key_a".to_owned(), "key_b".to_owned()]);
    unique_together.max_invalid = Some(1);

    let mut row_count = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::RowCount);
    row_count.min = Some(4.0);
    row_count.max = Some(4.0);

    let result = evaluate_quality_rules(
        &frame,
        &[allowed_values, regex, dtype, unique_together, row_count],
    )
    .unwrap();

    assert!(result.passed);
    assert_eq!(
        result
            .rules
            .iter()
            .map(|rule| rule.invalid_count)
            .collect::<Vec<_>>(),
        vec![2, 1, 0, 1, 0]
    );
    assert_eq!(result.rules[4].checked_count, 1);
    assert_eq!(result.rules[4].invalid_pct, 0.0);
}

#[test]
fn advanced_quality_rules_reject_invalid_parameters() {
    let frame = df!["text" => &["ok"], "number" => &[1_i64]].unwrap();

    let mut bad_regex = quality_rule("text", QualityRuleKind::Regex);
    bad_regex.pattern = Some("[".to_owned());
    assert!(evaluate_quality_rules(&frame, &[bad_regex]).is_err());

    let mut bad_values = quality_rule("number", QualityRuleKind::AllowedValues);
    bad_values.values = Some(vec!["1".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[bad_values]).is_err());

    let mut missing_key = quality_rule("text", QualityRuleKind::UniqueTogether);
    missing_key.columns = Some(vec!["text".to_owned(), "missing".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[missing_key]).is_err());

    let missing_row_bounds = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::RowCount);
    assert!(evaluate_quality_rules(&frame, &[missing_row_bounds]).is_err());

    let mut bad_dtype = quality_rule("text", QualityRuleKind::Dtype);
    bad_dtype.dtype = Some("money".to_owned());
    assert!(evaluate_quality_rules(&frame, &[bad_dtype]).is_err());

    let mut misplaced_operator = quality_rule("text", QualityRuleKind::NotNull);
    misplaced_operator.operator = Some(QualityComparison::Eq);
    assert!(evaluate_quality_rules(&frame, &[misplaced_operator]).is_err());
}

#[test]
fn quality_rules_apply_simple_and_composite_referential_integrity() {
    let frame = df![
        "country" => &[Some("DO"), Some("US"), Some("MX"), None],
        "code" => &[Some(1_i64), Some(2), Some(3), Some(4)]
    ]
    .unwrap();
    let mut simple = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
    simple.columns = Some(vec!["country".to_owned()]);
    simple.reference_values = Some(vec!["DO".to_owned(), "US".to_owned()]);
    simple.max_invalid = Some(2);

    let mut composite = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
    composite.columns = Some(vec!["country".to_owned(), "code".to_owned()]);
    composite.reference_values = Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()]);
    composite.max_invalid = Some(2);

    let result = evaluate_quality_rules(&frame, &[simple, composite]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].invalid_count, 2);
    assert_eq!(result.rules[1].invalid_count, 2);
    assert_eq!(
        result.rules[1].reference_values,
        Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()])
    );
}

#[test]
fn referential_integrity_rejects_missing_duplicate_or_unsupported_keys() {
    let date = Series::new("date".into(), [Some(0_i32)])
        .cast(&DataType::Date)
        .unwrap()
        .into_column();
    let frame = DataFrame::new(
        1,
        vec![Series::new("country".into(), ["DO"]).into_column(), date],
    )
    .unwrap();
    let mut missing_values = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
    missing_values.columns = Some(vec!["country".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[missing_values]).is_err());

    let mut duplicate_columns = quality_rule("country", QualityRuleKind::ReferentialIntegrity);
    duplicate_columns.columns = Some(vec!["country".to_owned(), "country".to_owned()]);
    duplicate_columns.reference_values = Some(vec![r#"["DO","DO"]"#.to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[duplicate_columns]).is_err());

    let mut unsupported = quality_rule("date", QualityRuleKind::ReferentialIntegrity);
    unsupported.columns = Some(vec!["date".to_owned()]);
    unsupported.reference_values = Some(vec!["2024-01-01".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[unsupported]).is_err());
}

#[test]
fn quality_rules_apply_non_strict_increasing_and_decreasing_sequences() {
    let frame = df![
        "value" => &[Some(1_i64), Some(2), Some(2), Some(1), Some(3), None, Some(2)]
    ]
    .unwrap();
    let mut increasing = quality_rule("value", QualityRuleKind::Monotonic);
    increasing.direction = Some(QualityMonotonicDirection::Increasing);
    increasing.max_invalid = Some(1);

    let mut decreasing = quality_rule("value", QualityRuleKind::Monotonic);
    decreasing.direction = Some(QualityMonotonicDirection::Decreasing);
    decreasing.max_invalid = Some(2);

    let result = evaluate_quality_rules(&frame, &[increasing, decreasing]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].checked_count, 7);
    assert_eq!(result.rules[0].invalid_count, 1);
    assert_eq!(result.rules[1].invalid_count, 2);
    assert_eq!(
        result.rules[0].direction,
        Some(QualityMonotonicDirection::Increasing)
    );
}

#[test]
fn monotonic_rejects_extra_parameters_and_migrates_direction_aliases() {
    let frame = df!["value" => &[1_i64, 2]].unwrap();
    let mut extra = quality_rule("value", QualityRuleKind::Monotonic);
    extra.direction = Some(QualityMonotonicDirection::Increasing);
    extra.values = Some(vec!["1".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[extra]).is_err());

    let result = migrate_quality_rules_document(serde_json::json!([
        {"kind": "monotonic", "column": "value", "direction": "desc"},
        {"kind": "monotonic", "column": "value", "order": "sideways"}
    ]))
    .unwrap();
    assert_eq!(result.converted_rules.len(), 1);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(
        result.converted_rules[0].direction,
        Some(QualityMonotonicDirection::Decreasing)
    );
}

#[test]
fn quality_rules_apply_aggregate_checks_and_reconciliation() {
    let frame = df![
        "amount" => &[Some(1_i64), Some(2), Some(3), None],
        "ledger" => &[Some(0_i64), Some(3), Some(3), None]
    ]
    .unwrap();
    let mut sum = quality_rule("amount", QualityRuleKind::AggregateCheck);
    sum.expected = Some(6.0);
    sum.aggregate = Some(QualityAggregate::Sum);

    let mut count = quality_rule("amount", QualityRuleKind::AggregateCheck);
    count.expected = Some(3.0);
    count.aggregate = Some(QualityAggregate::Count);

    let mut minimum = quality_rule("amount", QualityRuleKind::AggregateCheck);
    minimum.expected = Some(1.0);
    minimum.aggregate = Some(QualityAggregate::Min);

    let mut maximum = quality_rule("amount", QualityRuleKind::AggregateCheck);
    maximum.expected = Some(3.0);
    maximum.aggregate = Some(QualityAggregate::Max);

    let mut reconciliation = quality_rule("amount", QualityRuleKind::AggregateReconciliation);
    reconciliation.columns = Some(vec!["amount".to_owned(), "ledger".to_owned()]);
    reconciliation.tolerance_abs = Some(0.01);

    let result =
        evaluate_quality_rules(&frame, &[sum, count, minimum, maximum, reconciliation]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].checked_count, 4);
    assert_eq!(result.rules[1].checked_count, 4);
    assert_eq!(result.rules[4].invalid_count, 0);
    assert_eq!(result.rules[4].aggregate, None);
}

#[test]
fn aggregate_rules_reject_invalid_payloads_and_migrate_legacy_fields() {
    let frame = df!["amount" => &[1_i64, 2], "ledger" => &[1_i64, 2]].unwrap();
    let mut extra = quality_rule("amount", QualityRuleKind::AggregateCheck);
    extra.expected = Some(3.0);
    extra.values = Some(vec!["3".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[extra]).is_err());

    let result = migrate_quality_rules_document(serde_json::json!([
        {
            "kind": "aggregate_check",
            "column": "amount",
            "aggregate": "total",
            "expected": 3,
            "toleranceAbs": 0.5
        },
        {
            "kind": "aggregate_reconciliation",
            "column": "amount",
            "columns": ["amount", "ledger"],
            "toleranceRel": 0.01
        },
        {
            "kind": "aggregate_check",
            "column": "amount",
            "aggregate": "sideways",
            "expected": 3
        }
    ]))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 2);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(
        result.converted_rules[0].aggregate,
        Some(QualityAggregate::Sum)
    );
    assert_eq!(result.converted_rules[0].tolerance_abs, Some(0.5));
    assert_eq!(
        result.converted_rules[1].columns,
        Some(vec!["amount".to_owned(), "ledger".to_owned()])
    );
    assert_eq!(result.converted_rules[1].tolerance_rel, Some(0.01));
}

#[test]
fn quality_rules_apply_distribution_drift_to_numeric_means() {
    let frame = df!["amount" => &[Some(1_i64), Some(2), Some(3), None]].unwrap();
    let mut passing = quality_rule("amount", QualityRuleKind::DistributionDrift);
    passing.baseline = Some(vec!["1".to_owned(), "3".to_owned()]);
    passing.threshold = Some(0.0);

    let mut failing = quality_rule("amount", QualityRuleKind::DistributionDrift);
    failing.baseline = Some(vec!["4".to_owned(), "6".to_owned()]);
    failing.threshold = Some(2.0);

    let result = evaluate_quality_rules(&frame, &[passing, failing]).unwrap();

    assert_eq!(result.rules[0].checked_count, 4);
    assert_eq!(result.rules[0].invalid_count, 0);
    assert_eq!(result.rules[1].checked_count, 4);
    assert_eq!(result.rules[1].invalid_count, 1);
    assert!(!result.rules[1].passed);
    assert_eq!(result.failed_rules, 1);
}

#[test]
fn distribution_drift_rejects_invalid_baselines_and_migrates_aliases() {
    let frame = df!["amount" => &[1_i64, 2]].unwrap();
    let mut invalid_baseline = quality_rule("amount", QualityRuleKind::DistributionDrift);
    invalid_baseline.baseline = Some(vec!["not-a-number".to_owned()]);
    assert!(evaluate_quality_rules(&frame, &[invalid_baseline]).is_err());

    let mut invalid_threshold = quality_rule("amount", QualityRuleKind::DistributionDrift);
    invalid_threshold.baseline = Some(vec!["1".to_owned()]);
    invalid_threshold.threshold = Some(-1.0);
    assert!(evaluate_quality_rules(&frame, &[invalid_threshold]).is_err());

    let result = migrate_quality_rules_document(serde_json::json!([
        {
            "kind": "drift",
            "column": "amount",
            "baseline": [1, 3],
            "threshold": 0.5
        },
        {
            "kind": "distribution_drift",
            "column": "amount",
            "referenceValues": [2, 2],
            "toleranceAbs": 0.1
        },
        {
            "kind": "distribution_drift",
            "column": "amount",
            "baseline": []
        }
    ]))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 2);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(
        result.converted_rules[0].kind,
        QualityRuleKind::DistributionDrift
    );
    assert_eq!(
        result.converted_rules[0].baseline,
        Some(vec!["1".to_owned(), "3".to_owned()])
    );
    assert_eq!(result.converted_rules[1].tolerance_abs, Some(0.1));
}

#[test]
fn quality_rules_document_roundtrips_canonical_and_legacy_v1() {
    let directory = tempfile::tempdir().unwrap();
    let canonical_path = directory.path().join("quality.json");
    let legacy_path = directory.path().join("legacy.json");
    let first_rule = quality_rule("amount", QualityRuleKind::NotNull);
    let first_document = build_quality_rules_document(vec![first_rule.clone()]).unwrap();

    save_quality_rules_atomic(&first_document, &canonical_path).unwrap();
    let loaded = load_quality_rules_for_automation(&canonical_path).unwrap();
    assert_eq!(loaded, vec![first_rule]);
    let imported = load_quality_migration_file(&canonical_path).unwrap();
    assert_eq!(imported.source_format, "columnia");
    assert_eq!(imported.source_version.as_deref(), Some("1"));
    assert_eq!(imported.omitted_rules, 0);
    assert_eq!(imported.report.total_items, 1);
    assert_eq!(imported.report.converted_items, 1);
    assert_eq!(imported.report.omitted_items, 0);
    assert_eq!(imported.report.warning_count, 0);
    assert_eq!(imported.report.manual_actions.len(), 1);
    assert!(imported.report.artifact_sha256.as_deref().is_some_and(
        |hash| hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit())
    ));

    let mut replacement = quality_rule("amount", QualityRuleKind::NumericRange);
    replacement.min = Some(0.0);
    let replacement_document = build_quality_rules_document(vec![replacement.clone()]).unwrap();
    save_quality_rules_atomic(&replacement_document, &canonical_path).unwrap();
    assert_eq!(
        load_quality_rules_for_automation(&canonical_path).unwrap(),
        vec![replacement]
    );

    fs::write(
        &legacy_path,
        serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "rules": [quality_rule("amount", QualityRuleKind::NotNull)]
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        load_quality_rules_for_automation(&legacy_path)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn quality_rules_document_rejects_future_or_ambiguous_contracts() {
    let canonical_future = serde_json::json!({
        "format": QUALITY_RULES_DOCUMENT_FORMAT,
        "version": 2,
        "rules": []
    });
    assert!(migrate_quality_rules_document(canonical_future)
        .unwrap_err()
        .contains("no es compatible"));

    let wrong_format = serde_json::json!({
        "format": "another-quality-format",
        "version": 1,
        "rules": []
    });
    assert!(migrate_quality_rules_document(wrong_format)
        .unwrap_err()
        .contains("formato"));

    let unknown_field = serde_json::json!({
        "format": QUALITY_RULES_DOCUMENT_FORMAT,
        "version": 1,
        "rules": [],
        "extra": true
    });
    assert!(migrate_quality_rules_document(unknown_field).is_err());

    let legacy_future = serde_json::json!({
        "version": 4,
        "rules": []
    });
    assert!(migrate_quality_rules_document(legacy_future)
        .unwrap_err()
        .contains("legacy 4"));

    let directory = tempfile::tempdir().unwrap();
    let legacy_future = directory.path().join("legacy-future.json");
    fs::write(&legacy_future, br#"{"version":2,"rules":[]}"#).unwrap();
    assert!(load_quality_rules_for_automation(&legacy_future).is_err());
}

#[test]
fn migrates_supported_quality_rules_and_omits_unsupported_semantics() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "rules": [
            {"kind": "allowed_values", "column": "status", "values": ["ok"], "max_invalid": 1},
            {"type": "regex", "column": "email", "pattern": "^.+@.+$", "maxInvalidPct": 5},
            {"kind": "column_compare", "column": "left", "other_column": "right", "operator": "lte"},
            {"kind": "not_null", "column": "id", "severity": "warning", "max_invalid": 0},
            {"kind": "unique_together", "column": "a", "columns": ["a", "b"]}
        ]
    }))
    .expect("el documento de migración debe ser válido");

    assert_eq!(result.source_version.as_deref(), Some("3"));
    assert_eq!(result.converted_rules.len(), 4);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(result.converted_rules[0].max_invalid, Some(1));
    assert_eq!(result.converted_rules[1].max_invalid_pct, Some(5.0));
    assert_eq!(
        result.converted_rules[2].kind,
        QualityRuleKind::ColumnCompare
    );
    assert_eq!(
        result.converted_rules[2].columns,
        Some(vec!["left".to_owned(), "right".to_owned()])
    );
    assert_eq!(
        result.converted_rules[2].operator,
        Some(QualityComparison::Lte)
    );
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.severity == "omitted" && warning.source_kind == "not_null"));
}

#[test]
fn migration_preserves_range_alias_and_numeric_tolerance_strings() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "rules": [
            {
                "kind": "range",
                "column": "age",
                "min": "0",
                "max": "120",
                "maxInvalid": "2"
            },
            {
                "kind": "aggregate_check",
                "column": "amount",
                "aggregate": "sum",
                "expected": "100.5",
                "toleranceAbs": "0.5",
                "toleranceRel": "0.01",
                "max_invalid": 0
            }
        ]
    }))
    .expect("los aliases numéricos de legacy deben conservarse");

    assert_eq!(result.converted_rules.len(), 2);
    assert_eq!(
        result.converted_rules[0].kind,
        QualityRuleKind::NumericRange
    );
    assert_eq!(result.converted_rules[0].min, Some(0.0));
    assert_eq!(result.converted_rules[0].max, Some(120.0));
    assert_eq!(result.converted_rules[0].max_invalid, Some(2));
    assert_eq!(result.converted_rules[1].expected, Some(100.5));
    assert_eq!(result.converted_rules[1].tolerance_abs, Some(0.5));
    assert_eq!(result.converted_rules[1].tolerance_rel, Some(0.01));
    assert_eq!(result.omitted_rules, 0);
}

#[test]
fn migration_preserves_v3_aliases_and_scalar_allowed_values() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "quality_rules": [
            {
                "type": "allowed_values",
                "column": "status",
                "values": ["ok", 1, true],
                "max_invalid": 0
            },
            {
                "kind": "numeric_range",
                "column": "amount",
                "min_value": "1",
                "maxValue": "3",
                "maxInvalidPct": "5"
            },
            {
                "kind": "dtype",
                "column": "amount",
                "expectedType": "integer",
                "max_invalid": 0
            },
            {
                "kind": "column_compare",
                "column": "left",
                "otherColumn": "right",
                "operator": "lte",
                "max_invalid": 0
            }
        ]
    }))
    .expect("los aliases v3 representables deben migrarse");

    assert_eq!(result.converted_rules.len(), 4);
    assert_eq!(
        result.converted_rules[0].values,
        Some(vec!["ok".to_owned(), "1".to_owned(), "true".to_owned()])
    );
    assert_eq!(result.converted_rules[1].min, Some(1.0));
    assert_eq!(result.converted_rules[1].max, Some(3.0));
    assert_eq!(result.converted_rules[1].max_invalid_pct, Some(5.0));
    assert_eq!(result.converted_rules[2].dtype.as_deref(), Some("integer"));
    assert_eq!(
        result.converted_rules[3].columns,
        Some(vec!["left".to_owned(), "right".to_owned()])
    );
}

#[test]
fn migration_maps_blocking_alias_and_omits_unsafe_policy_metadata() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "rules": [
            {
                "kind": "not_null",
                "column": "id",
                "severity": "error",
                "onMissing": "fail",
                "nullPolicy": "invalid",
                "max_invalid": 0
            },
            {
                "kind": "unique",
                "column": "id",
                "blocking": false,
                "max_invalid": 0
            },
            {
                "kind": "not_null",
                "column": "id",
                "nullable": true,
                "max_invalid": 0
            },
            {
                "kind": "not_null",
                "column": "id",
                "referenceRevision": "rev-12",
                "max_invalid": 0
            },
            {
                "kind": "not_null",
                "column": "id",
                "severity": "warning",
                "blocking": true,
                "max_invalid": 0
            }
        ]
    }))
    .expect("las políticas deben aislarse por regla");

    assert_eq!(result.converted_rules.len(), 1);
    assert_eq!(result.converted_rules[0].kind, QualityRuleKind::NotNull);
    assert_eq!(result.omitted_rules, 4);
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("no bloqueante")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("nullable/allowNulls=true")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("referencia externa")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("contradictorias")));
}

#[test]
fn migration_accepts_condition_alias_but_keeps_nested_policies_blocking() {
    let result = migrate_quality_rules_document(serde_json::json!([{
        "kind": "conditional",
        "column": "amount",
        "condition": {"column": "status", "op": "eq", "val": "active"},
        "then": {"kind": "numeric_range", "minValue": 0, "maxValue": 10},
        "max_invalid": 0
    }]))
    .expect("la condición compatible debe migrarse");

    assert_eq!(result.converted_rules.len(), 1);
    let rule = &result.converted_rules[0];
    assert_eq!(
        rule.when.as_ref().map(|condition| condition.operator),
        Some(Some(QualityComparison::Eq))
    );
    assert_eq!(rule.then.as_deref().and_then(|then| then.min), Some(0.0));
    assert_eq!(rule.then.as_deref().and_then(|then| then.max), Some(10.0));
}

#[test]
fn migration_omits_negative_tolerance_instead_of_returning_invalid_rule() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "rules": [
            {
                "kind": "aggregate_check",
                "column": "amount",
                "expected": 100,
                "toleranceAbs": -0.5,
                "max_invalid": 0
            },
            {
                "kind": "distribution_drift",
                "column": "amount",
                "baseline": [100],
                "threshold": -1,
                "max_invalid": 0
            }
        ]
    }))
    .expect("una tolerancia negativa no debe invalidar todo el documento");

    assert_eq!(result.converted_rules.len(), 0);
    assert_eq!(result.omitted_rules, 2);
    assert!(result
        .warnings
        .iter()
        .all(|warning| warning.severity == "omitted"));
    assert!(result.warnings.iter().any(|warning| warning
        .message
        .contains("toleranceAbs no puede ser negativo")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("threshold no puede ser negativo")));
}

#[test]
fn migrates_simple_and_composite_referential_integrity_values() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "rules": [
            {
                "kind": "referential_integrity",
                "column": "country",
                "reference_values": ["DO", "US", 3, true],
                "max_invalid": 0
            },
            {
                "kind": "referential",
                "columns": ["country", "code"],
                "reference": [["DO", 1], ["US", 2]],
                "max_invalid": 0
            },
            {
                "kind": "referential_integrity",
                "column": "country",
                "reference_values": [null],
                "max_invalid": 0
            }
        ]
    }))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 2);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(
        result.converted_rules[0].reference_values,
        Some(vec![
            "DO".to_owned(),
            "US".to_owned(),
            "3".to_owned(),
            "true".to_owned()
        ])
    );
    assert_eq!(result.converted_rules[1].column, "country");
    assert_eq!(
        result.converted_rules[1].reference_values,
        Some(vec![r#"["DO",1]"#.to_owned(), r#"["US",2]"#.to_owned()])
    );
}

#[test]
fn quality_rules_compare_columns_with_nulls_and_tolerance() {
    let frame = df![
        "start" => &[Some(1_i64), Some(2), Some(3), None],
        "end" => &[Some(1_i64), Some(1), Some(4), Some(4)]
    ]
    .unwrap();
    let mut compare = quality_rule("start", QualityRuleKind::ColumnCompare);
    compare.columns = Some(vec!["start".to_owned(), "end".to_owned()]);
    compare.operator = Some(QualityComparison::Lte);
    compare.max_invalid = Some(2);

    let result = evaluate_quality_rules(&frame, &[compare]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].checked_count, 4);
    assert_eq!(result.rules[0].invalid_count, 2);
    assert_eq!(result.rules[0].operator, Some(QualityComparison::Lte));
}

#[test]
fn quality_rules_apply_conditional_row_rules_and_skip_null_conditions() {
    let frame = df![
        "status" => &[Some("ok"), Some("skip"), Some("ok"), Some("ok")],
        "amount" => &[Some(10_i64), None, Some(20), None]
    ]
    .unwrap();
    let mut conditional = quality_rule("amount", QualityRuleKind::Conditional);
    conditional.when = Some(QualityCondition {
        column: "status".to_owned(),
        operator: Some(QualityComparison::Eq),
        value: Some("ok".to_owned()),
    });
    conditional.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));

    let result = evaluate_quality_rules(&frame, &[conditional.clone()]).unwrap();

    assert!(!result.passed);
    assert_eq!(result.rules[0].checked_count, 4);
    assert_eq!(result.rules[0].invalid_count, 1);
    assert_eq!(result.rules[0].when, conditional.when);
    assert_eq!(result.rules[0].then, conditional.then);

    let mut allowed = conditional;
    allowed.max_invalid = Some(1);
    let result = evaluate_quality_rules(&frame, &[allowed]).unwrap();
    assert!(result.passed);
}

#[test]
fn quality_rules_match_numeric_conditions_across_integer_widths() {
    let frame = df![
        "status" => &["active", "inactive", "active"],
        "code" => &[1_i32, 2, 1],
        "amount" => &[Some(10_i64), None, Some(20)]
    ]
    .unwrap();
    let mut allowed = quality_rule("status", QualityRuleKind::AllowedValues);
    allowed.values = Some(vec!["active".to_owned(), "inactive".to_owned()]);
    allowed.max_invalid = Some(0);

    let mut conditional = quality_rule("amount", QualityRuleKind::Conditional);
    conditional.when = Some(QualityCondition {
        column: "code".to_owned(),
        operator: Some(QualityComparison::Eq),
        value: Some("1".to_owned()),
    });
    conditional.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));
    conditional.max_invalid = Some(0);

    let result = evaluate_quality_rules(&frame, &[allowed, conditional]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].invalid_count, 0);
    assert_eq!(result.rules[1].invalid_count, 0);
    assert_eq!(result.rules[1].checked_count, 3);
    assert_eq!(result.failed_rules, 0);
}

#[test]
fn conditional_rules_reject_missing_or_global_then_checks() {
    let frame = df!["status" => &["ok"], "amount" => &[1_i64]].unwrap();
    let mut missing_when = quality_rule("amount", QualityRuleKind::Conditional);
    missing_when.then = Some(Box::new(quality_rule("amount", QualityRuleKind::NotNull)));
    assert!(evaluate_quality_rules(&frame, &[missing_when]).is_err());

    let mut global_then = quality_rule("amount", QualityRuleKind::Conditional);
    global_then.when = Some(QualityCondition {
        column: "status".to_owned(),
        operator: Some(QualityComparison::Eq),
        value: Some("ok".to_owned()),
    });
    global_then.then = Some(Box::new(quality_rule("amount", QualityRuleKind::Unique)));
    assert!(evaluate_quality_rules(&frame, &[global_then]).is_err());
}

#[test]
fn migrates_conditional_rules_with_a_safe_nested_check() {
    let result = migrate_quality_rules_document(serde_json::json!([{
        "kind": "conditional",
        "column": "status",
        "when": {"column": "status", "operator": "eq", "value": "active"},
        "then": {"kind": "allowed_values", "values": ["active"]},
        "max_invalid": 0
    }]))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 1);
    let rule = &result.converted_rules[0];
    assert_eq!(rule.kind, QualityRuleKind::Conditional);
    assert_eq!(
        rule.when.as_ref().map(|when| when.column.as_str()),
        Some("status")
    );
    assert_eq!(
        rule.then.as_deref().map(|then| then.kind),
        Some(QualityRuleKind::AllowedValues)
    );
    assert_eq!(
        rule.then.as_deref().and_then(|then| then.values.clone()),
        Some(vec!["active".to_owned()])
    );
}

#[test]
fn migration_omits_nonrepresentable_nested_policy_and_malformed_policy() {
    let result = migrate_quality_rules_document(serde_json::json!({
        "version": 3,
        "rules": [
            {
                "kind": "conditional",
                "column": "amount",
                "when": {"column": "status", "value": "active"},
                "then": {
                    "kind": "not_null",
                    "severity": "warning"
                },
                "max_invalid": 0
            },
            {
                "kind": "not_null",
                "column": "status",
                "severity": true,
                "max_invalid": 0
            }
        ]
    }))
    .expect("las políticas no representables deben aislarse por regla");

    assert_eq!(result.converted_rules.len(), 0);
    assert_eq!(result.omitted_rules, 2);
    assert!(result
        .warnings
        .iter()
        .all(|warning| warning.severity == "omitted"));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("La severidad de la subregla then")));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("severity' debe ser texto")));
}

#[test]
fn quality_rules_validate_schema_required_additional_and_order_columns() {
    let frame = df![
        "status" => &["ok", "pending"],
        "amount" => &[1_i64, 2],
        "extra" => &[true, false]
    ]
    .unwrap();
    let mut schema = quality_rule(QUALITY_DATASET_COLUMN, QualityRuleKind::SchemaContract);
    schema.columns = Some(vec!["status".to_owned(), "amount".to_owned()]);
    schema.allow_additional = Some(false);

    let result = evaluate_quality_rules(&frame, &[schema.clone()]).unwrap();
    assert!(!result.passed);
    assert_eq!(result.rules[0].checked_count, 1);
    assert_eq!(result.rules[0].invalid_count, 1);

    schema.allow_additional = Some(true);
    schema.required_order = Some(vec![
        "status".to_owned(),
        "amount".to_owned(),
        "extra".to_owned(),
    ]);
    let result = evaluate_quality_rules(&frame, &[schema.clone()]).unwrap();
    assert!(result.passed);

    schema.required_order = Some(vec!["amount".to_owned(), "status".to_owned()]);
    let result = evaluate_quality_rules(&frame, &[schema]).unwrap();
    assert!(!result.passed);
    assert_eq!(result.rules[0].invalid_count, 1);
}

#[test]
fn migrates_schema_contract_aliases_and_defaults_additional_columns() {
    let result = migrate_quality_rules_document(serde_json::json!([
        {
            "kind": "schema",
            "requiredColumns": ["status", "amount"],
            "allowAdditional": false,
            "required_order": ["status", "amount"],
            "max_invalid": 0
        },
        {
            "kind": "schema_contract",
            "columns": ["status", "status"],
            "max_invalid": 0
        }
    ]))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 1);
    assert_eq!(result.omitted_rules, 1);
    let rule = &result.converted_rules[0];
    assert_eq!(rule.kind, QualityRuleKind::SchemaContract);
    assert_eq!(rule.column, QUALITY_DATASET_COLUMN);
    assert_eq!(rule.allow_additional, Some(false));
    assert_eq!(
        rule.required_order,
        Some(vec!["status".to_owned(), "amount".to_owned()])
    );
    assert_eq!(
        rule.columns,
        Some(vec!["status".to_owned(), "amount".to_owned()])
    );
}

#[test]
fn quality_rules_validate_date_ranges_and_count_unparseable_values() {
    let frame = df![
        "date" => &[
            Some("2024-01-01"),
            Some("2024-06-15"),
            Some("2025-01-01"),
            Some("not-a-date"),
            None
        ]
    ]
    .unwrap();
    let mut date_range = quality_rule("date", QualityRuleKind::DateRange);
    date_range.min_date = Some("2024-01-01".to_owned());
    date_range.max_date = Some("2024-12-31".to_owned());
    date_range.max_invalid = Some(3);

    let result = evaluate_quality_rules(&frame, &[date_range]).unwrap();

    assert!(result.passed);
    assert_eq!(result.rules[0].checked_count, 5);
    assert_eq!(result.rules[0].invalid_count, 3);
    assert_eq!(result.rules[0].min_date.as_deref(), Some("2024-01-01"));
    assert_eq!(result.rules[0].max_date.as_deref(), Some("2024-12-31"));
}

#[test]
fn migration_defaults_tolerance_clamps_percentages_and_isolates_bad_rules() {
    let result = migrate_quality_rules_document(serde_json::json!([
        {"kind": "not_null", "column": "id"},
        {"kind": "allowed_values", "column": "status", "values": "not-a-list"},
        {"kind": "row_count", "min_value": 1, "maxInvalidPct": 120}
    ]))
    .expect("el documento de migración debe ser válido");

    assert_eq!(result.converted_rules.len(), 2);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(result.converted_rules[0].max_invalid, Some(0));
    assert_eq!(result.converted_rules[1].max_invalid_pct, Some(100.0));
    assert!(result.warnings.iter().any(|warning| {
        warning.severity == "warning" && warning.message.contains("no traía tolerancia")
    }));
    assert!(result
        .warnings
        .iter()
        .any(|warning| { warning.severity == "warning" && warning.message.contains("ajustada") }));
    assert!(result.warnings.iter().any(|warning| {
        warning.severity == "omitted" && warning.message.contains("debe ser una lista")
    }));
}

#[test]
fn migrates_date_range_bounds_and_rejects_invalid_dates() {
    let result = migrate_quality_rules_document(serde_json::json!([
        {
            "kind": "date_range",
            "column": "created_at",
            "min_value": "2024-01-01",
            "max_value": "2024-12-31",
            "max_invalid": 0
        },
        {
            "kind": "date_range",
            "column": "created_at",
            "min_value": "not-a-date",
            "max_invalid": 0
        }
    ]))
    .unwrap();

    assert_eq!(result.converted_rules.len(), 1);
    assert_eq!(result.omitted_rules, 1);
    assert_eq!(
        result.converted_rules[0].min_date.as_deref(),
        Some("2024-01-01")
    );
    assert_eq!(
        result.converted_rules[0].max_date.as_deref(),
        Some("2024-12-31")
    );
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.message.contains("límites de fecha válidos")));
}

#[test]
fn quality_tolerances_are_inclusive_and_both_must_pass() {
    let frame = df!["value" => &[Some(1_i64), None, Some(3), Some(4)]].unwrap();
    let mut boundary = quality_rule("value", QualityRuleKind::NotNull);
    boundary.max_invalid = Some(1);
    boundary.max_invalid_pct = Some(25.0);
    let result = evaluate_quality_rules(&frame, &[boundary.clone()]).unwrap();
    assert!(result.passed);
    assert_eq!(result.rules[0].invalid_pct, 25.0);

    boundary.max_invalid_pct = Some(24.999);
    let result = evaluate_quality_rules(&frame, &[boundary]).unwrap();
    assert!(!result.passed);
}

#[test]
fn quality_rule_definitions_reject_invalid_contracts_and_types() {
    let frame = df!["text" => &["a"], "number" => &[1_i64]].unwrap();
    let mut missing_tolerance = quality_rule("text", QualityRuleKind::NotNull);
    missing_tolerance.max_invalid = None;
    assert!(evaluate_quality_rules(&frame, &[missing_tolerance]).is_err());

    let mut bad_pct = quality_rule("text", QualityRuleKind::NotNull);
    bad_pct.max_invalid_pct = Some(100.1);
    assert!(evaluate_quality_rules(&frame, &[bad_pct]).is_err());

    let mut bad_bounds = quality_rule("number", QualityRuleKind::NumericRange);
    bad_bounds.min = Some(2.0);
    bad_bounds.max = Some(1.0);
    assert!(evaluate_quality_rules(&frame, &[bad_bounds]).is_err());

    let no_bounds = quality_rule("number", QualityRuleKind::NumericRange);
    assert!(evaluate_quality_rules(&frame, &[no_bounds]).is_err());

    let mut non_finite = quality_rule("number", QualityRuleKind::NumericRange);
    non_finite.min = Some(f64::NAN);
    assert!(evaluate_quality_rules(&frame, &[non_finite]).is_err());
    assert!(
        evaluate_quality_rules(&frame, &[quality_rule("missing", QualityRuleKind::NotNull)])
            .is_err()
    );
    assert!(
        evaluate_quality_rules(&frame, &[quality_rule("number", QualityRuleKind::NonEmpty)])
            .is_err()
    );
    assert!(evaluate_quality_rules(
        &frame,
        &[quality_rule("text", QualityRuleKind::NumericRange)]
    )
    .is_err());
}

#[test]
fn integer_ranges_compare_large_values_exactly_and_reject_unsafe_bounds() {
    let frame = df![
        "number" => &[9_007_199_254_740_992_i64, 9_007_199_254_740_993_i64, i64::MAX, i64::MIN]
    ]
    .unwrap();
    let mut rule = quality_rule("number", QualityRuleKind::NumericRange);
    rule.max = Some(MAX_SAFE_INTEGER);
    rule.max_invalid = Some(4);
    let result = evaluate_quality_rules(&frame, &[rule]).unwrap();
    assert_eq!(result.rules[0].invalid_count, 3);
    assert!(result.rules[0].passed);

    let mut unsafe_bound = quality_rule("number", QualityRuleKind::NumericRange);
    unsafe_bound.max = Some(9_007_199_254_740_992.0);
    assert!(evaluate_quality_rules(&frame, &[unsafe_bound]).is_err());
}

#[test]
fn quality_contract_limits_rules_and_denies_unknown_or_negative_fields() {
    let frame = df!["value" => &[1_i64]].unwrap();
    let rules = vec![quality_rule("value", QualityRuleKind::NotNull); MAX_QUALITY_RULES];
    assert!(evaluate_quality_rules(&frame, &rules).is_ok());
    let too_many = vec![quality_rule("value", QualityRuleKind::NotNull); MAX_QUALITY_RULES + 1];
    assert!(evaluate_quality_rules(&frame, &too_many).is_err());

    assert!(serde_json::from_str::<QualityRule>(
        r#"{"column":"value","kind":"not_null","maxInvalid":0,"extra":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<QualityRule>(
        r#"{"column":"value","kind":"not_null","maxInvalid":-1}"#
    )
    .is_err());
}

#[test]
fn quality_semantic_budget_accepts_boundaries_and_rejects_validate_and_export_payloads() {
    let boundary_column = "c".repeat(MAX_QUALITY_COLUMN_CHARS);
    let boundary_rules = vec![
        quality_rule(&boundary_column, QualityRuleKind::NotNull);
        MAX_QUALITY_TOTAL_TEXT_CHARS / MAX_QUALITY_COLUMN_CHARS
    ];
    validate_quality_rules_payload(&boundary_rules)
        .expect("el presupuesto exacto de calidad debe admitirse");

    let oversized_column = "c".repeat(MAX_QUALITY_COLUMN_CHARS + 1);
    let field_error = validate_quality_rules_payload(&[quality_rule(
        &oversized_column,
        QualityRuleKind::NotNull,
    )])
    .expect_err("un nombre desproporcionado debe rechazarse");
    assert!(field_error.contains(&MAX_QUALITY_COLUMN_CHARS.to_string()));
    assert!(!field_error.contains(&"c".repeat(32)));

    let oversized_total = vec![
        quality_rule(&boundary_column, QualityRuleKind::NotNull);
        MAX_QUALITY_TOTAL_TEXT_CHARS / MAX_QUALITY_COLUMN_CHARS + 1
    ];
    let frame = df!["value" => &[1_i64]].unwrap();
    let validation_error = evaluate_quality_rules(&frame, &oversized_total)
        .expect_err("validar debe rechazar el total excedido antes de consultar columnas");
    assert!(validation_error.contains(&MAX_QUALITY_TOTAL_TEXT_CHARS.to_string()));
    let export_error =
        enforce_export_quality_with_cancel(&frame, &oversized_total, false, || false)
            .expect_err("exportar debe aplicar el mismo presupuesto");
    assert_eq!(export_error, validation_error);
    assert!(!export_error.contains(&"c".repeat(32)));
}

#[test]
fn quality_result_exposes_counts_without_samples_or_cell_values() {
    let frame = df!["secret" => &[Some("private-value"), None]].unwrap();
    let result =
        evaluate_quality_rules(&frame, &[quality_rule("secret", QualityRuleKind::NotNull)])
            .unwrap();
    let json = serde_json::to_value(result).unwrap();
    let encoded = json.to_string();
    assert!(!encoded.contains("private-value"));
    assert!(!encoded.contains("sample"));
    assert_eq!(json["rules"][0]["checkedCount"], 2);
    assert_eq!(json["rules"][0]["invalidCount"], 1);
}

#[test]
fn export_quality_gate_blocks_before_destination_and_requires_explicit_bypass() {
    use std::cell::Cell;

    let frame = df!["value" => &[Some(1_i64), None]].unwrap();
    assert!(enforce_export_quality_with_cancel(&frame, &[], false, || false).is_err());
    assert_eq!(
        enforce_export_quality_with_cancel(&frame, &[], true, || false).unwrap(),
        None
    );

    let failing = quality_rule("value", QualityRuleKind::NotNull);
    assert!(enforce_export_quality_with_cancel(&frame, &[failing], false, || false).is_err());

    let mut passing = quality_rule("value", QualityRuleKind::NotNull);
    passing.max_invalid = Some(1);
    assert!(
        enforce_export_quality_with_cancel(&frame, &[passing], false, || false)
            .unwrap()
            .is_some()
    );

    let checks = Cell::new(0_usize);
    let cancelled = enforce_export_quality_with_cancel(
        &frame,
        &[quality_rule("value", QualityRuleKind::NotNull)],
        false,
        || {
            checks.set(checks.get() + 1);
            checks.get() >= 2
        },
    );
    assert_eq!(cancelled.unwrap_err(), OPERATION_CANCELLED_MESSAGE);
}

#[test]
fn automation_source_backed_transform_and_quality_validation_stream_the_source() {
    let input = temporary_csv("value,kind\n1,keep\n2,drop\n3,keep\n");
    let output = input.with_file_name("automation-source-backed.csv");
    let recipe = StoredTransformRecipe {
        version: 1,
        name: "Filtro source-backed".to_owned(),
        saved_at: "2026-01-01T00:00:00Z".to_owned(),
        recipe: TransformRecipe {
            filters: vec![RecipeFilter {
                column: "kind".to_owned(),
                operator: RecipeFilterOperator::Eq,
                value: Some("keep".to_owned()),
            }],
            ..TransformRecipe::default()
        },
        export_options: None,
    };

    let transformed = transform_source_backed_for_automation(
        &input,
        None,
        None,
        &recipe,
        &output,
        ExportFormat::Csv,
    )
    .expect("la automatización debe poder transformar la fuente source-backed");
    assert!(transformed.changed);
    assert_eq!(transformed.input_row_count, 3);
    assert_eq!(transformed.output_row_count, 2);
    assert_eq!(transformed.input_column_count, 2);
    assert_eq!(transformed.output_column_count, 2);
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "value,kind\n1,keep\n3,keep\n"
    );

    let mut quality = quality_rule("value", QualityRuleKind::NotNull);
    quality.max_invalid = Some(0);
    let result =
        evaluate_source_backed_quality_rules_for_automation(&input, None, None, &[quality])
            .expect("la validación source-backed debe poder evaluar el contrato");
    assert!(result.passed);
    assert_eq!(result.row_count, 3);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}

#[test]
fn source_backed_project_import_profiles_and_snapshots_without_materializing_rows() {
    let input = temporary_csv("value,kind\n1,keep\n2,drop\n3,keep\n");
    let (state, preview) = DatasetState::for_source_backed_project_import(&input, None, None)
        .expect("el proyecto debe poder iniciarse desde una fuente source-backed");
    assert_eq!(preview.row_count, 3);

    let profile = state
        .cache_project_import_profile()
        .expect("el perfil del proyecto debe calcularse por bloques");
    assert_eq!(profile.row_count, 3);
    assert_eq!(state.dimensions_for_automation().unwrap(), (3, 2));

    let active = state
        .active_project_snapshot()
        .expect("el snapshot durable debe generarse desde la fuente");
    assert_eq!(active.row_count, 3);
    assert_eq!(active.column_count, 2);
    assert!(active.current_snapshot_path.is_some());

    let _ = fs::remove_file(input);
}
