//! «Script T-SQL» and «Paquete BULK INSERT» (2026-10-31) by both export paths.

use super::tests::test_source_format;
use super::*;
use ::zip::ZipArchive;
use std::io::Read;

fn sample_frame() -> DataFrame {
    df!(
        "id" => [1i64, 2],
        "cliente" => ["O'Brien Ñandú", "=SUMA(A1)"],
        "activo" => [true, false],
        "precio" => [1.5f64, 20.25],
    )
    .expect("frame de prueba")
}

fn zip_entries(path: &Path) -> Vec<(String, String)> {
    let mut archive = ZipArchive::new(File::open(path).expect("zip")).expect("zip válido");
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).expect("entrada");
            let mut text = String::new();
            entry.read_to_string(&mut text).expect("texto UTF-8");
            (entry.name().to_owned(), text)
        })
        .collect()
}

fn source_csv(directory: &Path, contents: &str) -> (PathBuf, u64) {
    let path = directory.join("origen.csv");
    fs::write(&path, contents).expect("CSV de origen");
    let size = fs::metadata(&path).expect("tamaño").len();
    (path, size)
}

#[test]
fn the_tsql_script_names_the_table_after_the_file_and_keeps_every_value() {
    let directory = tempfile::tempdir().expect("carpeta");
    let destination = directory.path().join("ventas limpias.sql");
    let result = export_frame_atomic(
        &sample_frame(),
        &destination,
        ExportFormat::SqlServerScript,
        |_, _| {},
        || false,
    )
    .expect("el script T-SQL se exporta");
    assert_eq!(result.format, "Script T-SQL");
    let script = fs::read_to_string(&destination).expect("script");
    assert!(script.contains("CREATE TABLE [dbo].[ventas_limpias] ("));
    assert!(script.contains("[id] BIGINT NULL,"));
    assert!(script.contains("[activo] BIT NULL,"));
    assert!(script.contains("[precio] FLOAT NULL"));
    assert!(script.contains("(1, N'O''Brien Ñandú', 1, 1.5)"));
    // No spreadsheet protection in a database script.
    assert!(script.contains("N'=SUMA(A1)'"));
    assert!(!script.contains("DROP TABLE"));
}

#[test]
fn the_tsql_script_refuses_values_sql_server_cannot_store() {
    let directory = tempfile::tempdir().expect("carpeta");
    let frame = df!("medida" => [1.0f64, f64::NAN]).expect("frame");
    let error = export_frame_atomic(
        &frame,
        &directory.path().join("medidas.sql"),
        ExportFormat::SqlServerScript,
        |_, _| {},
        || false,
    )
    .expect_err("NaN no cabe en SQL Server");
    assert!(error.contains("no finitos"));
    assert!(
        !directory.path().join("medidas.sql").exists(),
        "sin archivo a medias"
    );
}

#[test]
fn the_bulk_package_holds_the_csv_the_load_script_and_the_instructions() {
    let directory = tempfile::tempdir().expect("carpeta");
    let destination = directory.path().join("ventas.zip");
    let result = export_frame_atomic(
        &sample_frame(),
        &destination,
        ExportFormat::SqlServerBulk,
        |_, _| {},
        || false,
    )
    .expect("el paquete se exporta");
    assert_eq!(result.format, "Paquete BULK INSERT");
    let entries = zip_entries(&destination);
    let names = entries
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["ventas.csv", "cargar.sql", "LEEME.txt"]);
    let csv = &entries[0].1;
    assert!(csv.starts_with("id,cliente,activo,precio\n"));
    assert!(csv.contains("1,O'Brien Ñandú,1,1.5\n"));
    assert!(
        csv.contains("2,=SUMA(A1),0,20.25\n"),
        "sin protección de fórmulas: {csv}"
    );
    let load = &entries[1].1;
    assert!(load.contains("BULK INSERT [dbo].[ventas]\nFROM N'C:\\Columnia\\ventas.csv'"));
    assert!(load.contains("2 AS filas_esperadas"));
}

#[test]
fn large_files_stream_the_tsql_script_through_duckdb() {
    let directory = tempfile::tempdir().expect("carpeta");
    let (source, size) = source_csv(directory.path(), "nombre,importe\nO'Brien,10\n=1+1,20\n");
    let destination = directory.path().join("clientes.sql");
    let result = export_source_backed_tsql_atomic(
        &source,
        size,
        test_source_format(&source),
        &destination,
        None,
        |_, _| {},
        || false,
    )
    .expect("el script T-SQL de un archivo grande se exporta");
    assert_eq!(result.format, "Script T-SQL");
    let script = fs::read_to_string(&destination).expect("script");
    assert!(script.contains("CREATE TABLE [dbo].[clientes] ("));
    assert!(script.contains("N'O''Brien'"));
    assert!(script.contains("N'=1+1'"));
    assert!(script.contains("COMMIT TRANSACTION;"));
    assert!(script.ends_with("-- 2 filas.\n"));
}

/// AUD-034: with the profile's copy of the unchanged file, the script and the
/// package (with their text sizes taken from the longest value) are the ones
/// the file itself gives.
#[test]
fn the_profile_copy_gives_the_same_tsql_script_and_bulk_package_as_the_file() {
    let directory = tempfile::tempdir().expect("carpeta");
    let (source, size) = source_csv(
        directory.path(),
        "nombre,importe,nota\nO'Brien Ñandú,10,corta\n=1+1,20,una nota bastante más larga\n,,\n",
    );
    let format = crate::duckdb_query::DuckDbFileFormat::Delimited { delimiter: b',' };
    // The cache is keyed by the path the dataset and the export both resolve.
    let (canonical, _, _) = validate_dataset_file(&source).expect("fuente válida");
    clear_profile_snapshot_of(&canonical);
    let export = |name: &str| {
        let script = directory.path().join(format!("{name}.sql"));
        export_source_backed_tsql_atomic(
            &source,
            size,
            test_source_format(&source),
            &script,
            None,
            |_, _| {},
            || false,
        )
        .expect("script");
        let package = directory.path().join(format!("{name}.zip"));
        export_source_backed_sqlserver_bulk_atomic(
            &source,
            size,
            test_source_format(&source),
            3,
            &package,
            None,
            |_, _| {},
            || false,
        )
        .expect("paquete");
        (
            fs::read_to_string(script).expect("script"),
            zip_entries(&package),
        )
    };
    let without_copy = export("sin-copia");

    profile_source_backed_with_progress(
        &canonical,
        format,
        size,
        3,
        |_, _| {},
        || false,
        MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
    )
    .expect("perfil en disco");
    assert!(reusable_profile_copy(&canonical, format).is_some());
    let with_copy = export("con-copia");
    clear_profile_snapshot_of(&canonical);

    // Only the table name (the file name) differs between the two runs.
    let rename = |text: &str, name: &str| text.replace(name, "tabla");
    assert_eq!(
        rename(&with_copy.0, "con_copia"),
        rename(&without_copy.0, "sin_copia")
    );
    assert_eq!(with_copy.1.len(), without_copy.1.len());
    for (with, without) in with_copy.1.iter().zip(&without_copy.1) {
        assert_eq!(
            rename(&rename(&with.1, "con_copia"), "con-copia"),
            rename(&rename(&without.1, "sin_copia"), "sin-copia")
        );
    }
}

#[test]
fn large_files_stream_the_bulk_package_through_duckdb() {
    let directory = tempfile::tempdir().expect("carpeta");
    let (source, size) = source_csv(directory.path(), "nombre,importe\nO'Brien,10\n=1+1,20\n");
    let destination = directory.path().join("clientes.zip");
    export_source_backed_sqlserver_bulk_atomic(
        &source,
        size,
        test_source_format(&source),
        2,
        &destination,
        None,
        |_, _| {},
        || false,
    )
    .expect("el paquete de un archivo grande se exporta");
    let entries = zip_entries(&destination);
    assert_eq!(entries[0].0, "clientes.csv");
    assert!(entries[0].1.starts_with("nombre,importe\n"));
    assert!(
        entries[0].1.contains("=1+1,20"),
        "sin protección de fórmulas: {}",
        entries[0].1
    );
    assert!(entries[1].1.contains("2 AS filas_esperadas"));
    assert_eq!(entries[2].0, "LEEME.txt");
    // Nothing left behind next to the destination.
    let leftovers = fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".columnia-")
        })
        .count();
    assert_eq!(leftovers, 0);
}

fn sqlserver_options(json: &str) -> SqlServerScriptOptions {
    serde_json::from_str(json).expect("opciones de SQL Server")
}

fn export_with_options(
    frame: &DataFrame,
    destination: &Path,
    format: ExportFormat,
    options: &SqlServerScriptOptions,
) -> Result<ExportResult, String> {
    export_frame_atomic_with_privacy_and_quality_and_recipe(
        frame,
        destination,
        format,
        PrivacyMode::None,
        None,
        None,
        Some(options),
        |_, _| {},
        || false,
    )
}

#[test]
fn text_columns_are_sized_from_the_data_unless_max_is_chosen() {
    let directory = tempfile::tempdir().expect("carpeta");
    let destination = directory.path().join("ventas.sql");
    export_frame_atomic(
        &sample_frame(),
        &destination,
        ExportFormat::SqlServerScript,
        |_, _| {},
        || false,
    )
    .expect("script");
    let script = fs::read_to_string(&destination).expect("script");
    // «O'Brien Ñandú» is 15 UTF-8 bytes: a quarter more fits in 50.
    assert!(script.contains("[cliente] NVARCHAR(50) NULL,"), "{script}");

    export_with_options(
        &sample_frame(),
        &destination,
        ExportFormat::SqlServerScript,
        &sqlserver_options(r#"{"textSize":"max"}"#),
    )
    .expect("script sin límite");
    let script = fs::read_to_string(&destination).expect("script");
    assert!(script.contains("[cliente] NVARCHAR(MAX) NULL,"));
}

#[test]
fn the_chosen_table_and_mode_reach_the_in_memory_exports() {
    let directory = tempfile::tempdir().expect("carpeta");
    let options = sqlserver_options(
        r#"{"schema":"ventas","table":"Clientes Finales","mode":"upsert","keyColumns":["id"]}"#,
    );
    let script_path = directory.path().join("cualquiera.sql");
    export_with_options(
        &sample_frame(),
        &script_path,
        ExportFormat::SqlServerScript,
        &options,
    )
    .expect("script por clave");
    let script = fs::read_to_string(&script_path).expect("script");
    assert!(script.contains("IF OBJECT_ID(N'[ventas].[Clientes Finales]', N'U') IS NULL"));
    assert!(script.contains("CREATE TABLE #columnia_carga ("));
    assert!(script.contains("INNER JOIN #columnia_carga AS s ON t.[id] = s.[id];"));

    let package_path = directory.path().join("cualquiera.zip");
    export_with_options(
        &sample_frame(),
        &package_path,
        ExportFormat::SqlServerBulk,
        &sqlserver_options(r#"{"table":"Clientes Finales","mode":"append"}"#),
    )
    .expect("paquete para añadir");
    let entries = zip_entries(&package_path);
    assert_eq!(entries[0].0, "Clientes_Finales.csv");
    assert!(entries[1]
        .1
        .contains("BULK INSERT #columnia_carga\nFROM N'C:\\Columnia\\Clientes_Finales.csv'"));
    assert!(entries[1]
        .1
        .contains("INSERT INTO [dbo].[Clientes Finales]"));
    assert!(entries[2]
        .1
        .contains("Añade las filas a la tabla [dbo].[Clientes Finales]"));
}

#[test]
fn an_unknown_key_column_stops_the_export_without_a_file() {
    let directory = tempfile::tempdir().expect("carpeta");
    let destination = directory.path().join("ventas.sql");
    let error = export_with_options(
        &sample_frame(),
        &destination,
        ExportFormat::SqlServerScript,
        &sqlserver_options(r#"{"mode":"upsert","keyColumns":["codigo"]}"#),
    )
    .expect_err("la clave no existe");
    assert!(error.contains("«codigo»"));
    assert!(!destination.exists());
}

#[test]
fn large_files_measure_text_and_honour_the_options() {
    let directory = tempfile::tempdir().expect("carpeta");
    let (source, size) = source_csv(
        directory.path(),
        "nombre,importe\nO'Brien Ñandú,10\nAna,20\n",
    );
    let destination = directory.path().join("clientes.sql");
    let options = sqlserver_options(
        r#"{"schema":"ventas","table":"clientes_finales","mode":"upsert","keyColumns":["nombre"]}"#,
    );
    export_source_backed_tsql_atomic(
        &source,
        size,
        test_source_format(&source),
        &destination,
        Some(&options),
        |_, _| {},
        || false,
    )
    .expect("script por clave de un archivo grande");
    let script = fs::read_to_string(&destination).expect("script");
    assert!(
        script.contains("[nombre] NVARCHAR(50) COLLATE DATABASE_DEFAULT NULL,"),
        "{script}"
    );
    assert!(script
        .contains("UPDATE t\nSET t.[importe] = s.[importe]\nFROM [ventas].[clientes_finales]"));
    assert!(script.ends_with("-- 2 filas.\n"));

    let package = directory.path().join("clientes.zip");
    export_source_backed_sqlserver_bulk_atomic(
        &source,
        size,
        test_source_format(&source),
        2,
        &package,
        Some(&sqlserver_options(r#"{"textSize":"max"}"#)),
        |_, _| {},
        || false,
    )
    .expect("paquete de un archivo grande");
    let entries = zip_entries(&package);
    assert!(entries[1].1.contains("[nombre] NVARCHAR(MAX) NULL,"));
}

/// Writes both exports of a sample to `COLUMNIA_TSQL_OUT` to try them
/// against a real SQL Server.
#[test]
#[ignore = "escribe archivos de ejemplo en COLUMNIA_TSQL_OUT para probarlos en SQL Server"]
fn writes_sqlserver_samples_for_a_real_server() {
    let directory = PathBuf::from(std::env::var("COLUMNIA_TSQL_OUT").expect("COLUMNIA_TSQL_OUT"));
    let frame = df!(
        "id" => [Some(1i64), Some(2), None],
        "cliente" => [Some("O'Brien Ñandú, \"el viejo\""), Some(""), None],
        "activo" => [Some(true), Some(false), None],
        "precio" => [Some(1.5f64), Some(1e-7), Some(123456789.125)],
    )
    .expect("frame");
    export_frame_atomic(
        &frame,
        &directory.join("prueba_columnia.sql"),
        ExportFormat::SqlServerScript,
        |_, _| {},
        || false,
    )
    .expect("script");
    export_frame_atomic(
        &frame,
        &directory.join("prueba_columnia.zip"),
        ExportFormat::SqlServerBulk,
        |_, _| {},
        || false,
    )
    .expect("paquete");
    // Into an existing [dbo].[prueba_columnia]: id 2 is updated, id 4 is new.
    let changes = df!(
        "id" => [2i64, 4],
        "cliente" => ["Cambiado ñ", "Nuevo"],
        "activo" => [true, true],
        "precio" => [9.5f64, 4.0],
    )
    .expect("frame de cambios");
    let target = r#""table":"prueba_columnia""#;
    for (name, format, options) in [
        (
            "prueba_columnia_anadir.sql",
            ExportFormat::SqlServerScript,
            format!(r#"{{{target},"mode":"append"}}"#),
        ),
        (
            "prueba_columnia_clave.sql",
            ExportFormat::SqlServerScript,
            format!(r#"{{{target},"mode":"upsert","keyColumns":["id"]}}"#),
        ),
        (
            "prueba_columnia_anadir.zip",
            ExportFormat::SqlServerBulk,
            format!(r#"{{{target},"mode":"append"}}"#),
        ),
        (
            "prueba_columnia_clave.zip",
            ExportFormat::SqlServerBulk,
            format!(r#"{{{target},"mode":"upsert","keyColumns":["id"]}}"#),
        ),
    ] {
        export_with_options(
            &changes,
            &directory.join(name),
            format,
            &sqlserver_options(&options),
        )
        .expect(name);
    }
}
