//! AUD-076: cancellation and failure halfway through the long writes on disk.
//! The stage of the Preparar proposal for a large file (REN-14) and the two
//! exports to SQL Server (script and BULK INSERT package) are stopped, or
//! fail, after they have started to write; what must remain is the dataset as
//! it was and a folder without temporary files or a half-written destination.

use super::tests::test_source_format;
use super::*;
use std::collections::BTreeSet;
use std::sync::atomic::Ordering as AtomicOrdering;

/// The names inside `directory`, to compare the folder before and after.
fn names_in(directory: &Path) -> BTreeSet<String> {
    fs::read_dir(directory)
        .expect("la carpeta debe poder listarse")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

// The stage of the proposal on disk.

/// Rows of the source of the stage: four blocks of `SOURCE_STAGE_BLOCK_ROWS`
/// and a few more.
const STAGE_ROWS: usize = 4 * SOURCE_STAGE_BLOCK_ROWS + 100;

/// A Parquet source with a text column that types as numbers (`num`) and a
/// column of unique text the proposal leaves as it is (`payload`), most of the
/// bytes of the file.
fn stage_dataset(directory: &Path) -> LoadedDataset {
    let frame = df![
        "num" => (0..STAGE_ROWS).map(|row| row.to_string()).collect::<Vec<_>>(),
        "payload" => (0..STAGE_ROWS)
            .map(|row| format!("{:016x}-fila-{row}", (row as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
            .collect::<Vec<_>>(),
    ]
    .expect("la tabla debe crearse");
    let path = directory.join("fuente.parquet");
    let mut written = frame;
    ParquetWriter::new(File::create(&path).unwrap())
        .finish(&mut written)
        .unwrap();
    let (schema, _, row_count) =
        source_backed_load(&path, "parquet", || false).expect("la fuente debe inspeccionarse");
    LoadedDataset {
        source_path: Some(path.clone()),
        file_name: "fuente.parquet".to_owned(),
        file_size_bytes: fs::metadata(&path).unwrap().len(),
        row_count,
        frame: schema,
        source_backed: true,
        delimited_header_mode: None,
        profile: None,
        history: HistoryManager::deferred().expect("el historial diferido debe inicializarse"),
    }
}

/// What the person sees of the dataset: path, size, rows, columns and history.
fn dataset_fingerprint(dataset: &LoadedDataset) -> String {
    format!(
        "{:?}|{}|{}|{:?}|{}|{}|{:?}|{}|{}|{:?}|{}|{:?}",
        dataset.source_path,
        dataset.file_size_bytes,
        dataset.row_count,
        dataset.frame.schema(),
        dataset.source_backed,
        dataset.profile.is_none(),
        dataset
            .history
            .entries
            .iter()
            .map(|entry| (&entry.path, entry.bytes))
            .collect::<Vec<_>>(),
        dataset.history.cursor,
        dataset.history.next_id,
        dataset.history.source_snapshot_path,
        dataset.history.current_label,
        dataset.history.snapshots_enabled,
    )
}

/// The plan of the tests: type `num` as numbers, which only the stage does.
fn apply_stage_plan(
    dataset: &mut LoadedDataset,
    cancellation: &PrepareCancellation,
    report: &mut dyn FnMut(&'static str, u8),
) -> Result<Option<SafeCorrectionsResult>, String> {
    source_backed_safe_corrections_with_cancellation(
        dataset,
        false,
        false,
        false,
        false,
        Some(&["num".to_owned()]),
        None,
        None,
        cancellation,
        report,
    )
}

fn is_stage_step(step: &str) -> bool {
    step.starts_with("Convirtiendo")
}

#[test]
fn cancelling_the_disk_stage_after_the_first_block_leaves_dataset_and_folder_as_they_were() {
    let directory = tempfile::tempdir().expect("carpeta");
    let mut dataset = stage_dataset(directory.path());
    let history_directory = dataset.history.directory.path().to_owned();
    let before = dataset_fingerprint(&dataset);
    let source_before = names_in(directory.path());
    let history_before = names_in(&history_directory);

    // The progress of the stage is reported when a block from the second on
    // starts, once the one before is written: the cancellation is raised then
    // and the stage sees it at the check of the next block.
    let (cancellation, raise) = PrepareCancellation::raised_by_test();
    let mut stage_percents = Vec::new();
    let error = apply_stage_plan(&mut dataset, &cancellation, &mut |step, percent| {
        if is_stage_step(step) {
            stage_percents.push(percent);
            if percent > 80 {
                raise.store(true, AtomicOrdering::Release);
            }
        }
    })
    .expect_err("la etapa debe cancelarse");

    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    // It wrote the first blocks and stopped: the second block was reported,
    // the third (and the rest) never started.
    assert_eq!(stage_percents, vec![80, 83], "{stage_percents:?}");
    assert_eq!(dataset_fingerprint(&dataset), before);
    assert_eq!(names_in(directory.path()), source_before);
    assert_eq!(
        names_in(&history_directory),
        history_before,
        "la carpeta del historial no debe guardar la salida de DuckDB ni la de la etapa"
    );

    // Nothing was spoiled: the same plan, not cancelled, applies.
    let result = apply_stage_plan(
        &mut dataset,
        &PrepareCancellation::disabled(),
        &mut |_, _| {},
    )
    .expect("el plan debe aplicarse tras cancelar")
    .expect("el archivo debe procesarse en disco");
    assert_eq!(result.typed_column_count, 1);
    assert_eq!(dataset.row_count, STAGE_ROWS);
    assert_ne!(dataset_fingerprint(&dataset), before);
}

#[test]
fn a_read_error_halfway_through_the_disk_stage_leaves_dataset_and_folder_as_they_were() {
    let directory = tempfile::tempdir().expect("carpeta");
    let mut dataset = stage_dataset(directory.path());
    let history_directory = dataset.history.directory.path().to_owned();
    let before = dataset_fingerprint(&dataset);
    let source_before = names_in(directory.path());
    let history_before = names_in(&history_directory);

    // Nothing in the stage fails by itself once the check pass has decided
    // the columns, so the read of the file of DuckDB fails halfway, as a bad
    // sector would: when the stage is about to start, a span of the text
    // column of the last row group of that file (the file of the stage, not
    // the source) is overwritten. `num` is read whole by the check pass and
    // is not touched; the write pass reads the damaged part in the last
    // block, after the earlier ones are written.
    let stage_input = || {
        fs::read_dir(&history_directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "parquet")
            })
            .expect("la salida de DuckDB debe estar en la carpeta del historial")
    };
    let mut stage_percents = Vec::new();
    let mut damaged = false;
    let error = apply_stage_plan(
        &mut dataset,
        &PrepareCancellation::disabled(),
        &mut |step, percent| {
            if !is_stage_step(step) {
                return;
            }
            stage_percents.push(percent);
            if !damaged {
                damaged = true;
                let path = stage_input();
                let mut bytes = fs::read(&path).unwrap();
                let end = bytes.len() - 200 * 1024;
                bytes[end - 64 * 1024..end].fill(0xFF);
                fs::write(&path, bytes).unwrap();
            }
        },
    )
    .expect_err("la etapa debe fallar al leer el bloque dañado");

    assert!(damaged, "la etapa no llegó a empezar");
    assert!(error.contains("Parquet"), "{error}");
    // It was halfway: blocks after the first were reported and written.
    assert!(
        stage_percents.iter().any(|percent| *percent > 80),
        "{stage_percents:?}"
    );
    assert_eq!(dataset_fingerprint(&dataset), before);
    assert_eq!(names_in(directory.path()), source_before);
    assert_eq!(
        names_in(&history_directory),
        history_before,
        "{error}: no deben quedar la salida de DuckDB ni la de la etapa"
    );

    // The source was not damaged: the plan applies afterwards.
    let result = apply_stage_plan(
        &mut dataset,
        &PrepareCancellation::disabled(),
        &mut |_, _| {},
    )
    .expect("el plan debe aplicarse tras el fallo")
    .expect("el archivo debe procesarse en disco");
    assert_eq!(result.typed_column_count, 1);
}

// The exports to SQL Server of a large file.

const EXPORT_ROWS: usize = 120_000;

/// A Parquet source of `EXPORT_ROWS` rows with a number, a text and a double;
/// with `nan_at_the_end` the last double is NaN, which SQL Server cannot store.
fn export_source(directory: &Path, nan_at_the_end: bool) -> (PathBuf, u64) {
    let mut frame = df![
        "id" => (0..EXPORT_ROWS as i64).collect::<Vec<_>>(),
        "nombre" => (0..EXPORT_ROWS).map(|row| format!("cliente {row}")).collect::<Vec<_>>(),
        "valor" => (0..EXPORT_ROWS)
            .map(|row| if nan_at_the_end && row == EXPORT_ROWS - 1 { f64::NAN } else { row as f64 / 4.0 })
            .collect::<Vec<_>>(),
    ]
    .expect("la tabla debe crearse");
    let path = directory.join("origen.parquet");
    ParquetWriter::new(File::create(&path).unwrap())
        .finish(&mut frame)
        .unwrap();
    let size = fs::metadata(&path).unwrap().len();
    (path, size)
}

/// True once a file of `folder` whose name starts with `prefix` holds at least
/// `bytes`: the export has begun to write it. The state stays true while the
/// file exists, so the cancellation is seen whether the export is still
/// writing or has just finished. (The size is read from the file: on Windows
/// the entry of the folder does not show it while the file is open.)
fn once_a_file_holds(
    folder: &Path,
    prefix: &'static str,
    bytes: u64,
) -> impl Fn() -> bool + Clone + Send + 'static {
    let folder = folder.to_owned();
    move || {
        fs::read_dir(&folder).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry.file_name().to_string_lossy().starts_with(prefix)
                    && fs::metadata(entry.path()).is_ok_and(|metadata| metadata.len() >= bytes)
            })
        })
    }
}

fn tsql<C>(
    source: &Path,
    size: u64,
    destination: &Path,
    options: Option<&SqlServerScriptOptions>,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    C: Fn() -> bool + Clone + Send + 'static,
{
    export_source_backed_tsql_atomic(
        source,
        size,
        test_source_format(source),
        destination,
        options,
        |_, _| {},
        is_cancelled,
    )
}

fn bulk<F, C>(
    source: &Path,
    size: u64,
    destination: &Path,
    options: Option<&SqlServerScriptOptions>,
    report: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(&'static str, u8),
    C: Fn() -> bool + Clone + Send + 'static,
{
    export_source_backed_sqlserver_bulk_atomic(
        source,
        size,
        test_source_format(source),
        EXPORT_ROWS,
        destination,
        options,
        report,
        is_cancelled,
    )
}

/// The folder holds what it held before (so no `.columnia-*`, no `.tmp*` of
/// the publication and no destination that was not there) and a destination
/// that existed keeps its text.
fn assert_export_left_nothing(folder: &Path, before: &BTreeSet<String>, destination: &Path) {
    let after = names_in(folder);
    assert_eq!(&after, before, "la carpeta de destino cambió");
    assert!(
        after.iter().all(|name| !name.starts_with(".columnia-")),
        "{after:?}"
    );
    if before.contains("previo.sql") || before.contains("previo.zip") {
        assert_eq!(
            fs::read_to_string(destination).expect("el destino previo debe seguir"),
            "contenido previo"
        );
    } else {
        assert!(!destination.exists(), "destino a medias");
    }
}

#[test]
fn cancelling_the_tsql_export_of_a_large_file_leaves_no_temporary_and_no_destination() {
    let source_directory = tempfile::tempdir().expect("carpeta de origen");
    let (source, size) = export_source(source_directory.path(), false);
    let folder = tempfile::tempdir().expect("carpeta de destino");
    let destination = folder.path().join("clientes.sql");
    let before = names_in(folder.path());

    // The script of DuckDB is written to `.columnia-export-*.sql`; the export
    // is cancelled once it holds 256 KiB.
    let error = tsql(
        &source,
        size,
        &destination,
        None,
        once_a_file_holds(folder.path(), ".columnia-export-", 256 * 1024),
    )
    .expect_err("la exportación debe cancelarse");
    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert_export_left_nothing(folder.path(), &before, &destination);

    // The same export, not cancelled, finishes: the cancellation was the cause.
    tsql(&source, size, &destination, None, || false).expect("la exportación sin cancelar");
    assert!(fs::read_to_string(&destination)
        .unwrap()
        .ends_with(&format!("-- {EXPORT_ROWS} filas.\n")));
}

#[test]
fn a_value_sql_server_refuses_late_in_the_tsql_export_leaves_the_destination_as_it_was() {
    let source_directory = tempfile::tempdir().expect("carpeta de origen");
    let (source, size) = export_source(source_directory.path(), true);
    let folder = tempfile::tempdir().expect("carpeta de destino");
    // A script from an earlier export is already there.
    let destination = folder.path().join("previo.sql");
    fs::write(&destination, "contenido previo").unwrap();
    let before = names_in(folder.path());

    // The NaN is in the last row: the whole script but one row is written to
    // the temporary file when the export fails.
    let error =
        tsql(&source, size, &destination, None, || false).expect_err("NaN no cabe en SQL Server");
    assert!(error.contains("no finitos"), "{error}");
    assert_export_left_nothing(folder.path(), &before, &destination);
}

#[test]
fn cancelling_the_bulk_package_while_duckdb_writes_the_csv_leaves_nothing() {
    let source_directory = tempfile::tempdir().expect("carpeta de origen");
    let (source, size) = export_source(source_directory.path(), false);
    let folder = tempfile::tempdir().expect("carpeta de destino");
    let destination = folder.path().join("clientes.zip");
    let before = names_in(folder.path());

    let error = bulk(
        &source,
        size,
        &destination,
        None,
        |_, _| {},
        once_a_file_holds(folder.path(), ".columnia-bulk-", 256 * 1024),
    )
    .expect_err("el paquete debe cancelarse");
    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert_export_left_nothing(folder.path(), &before, &destination);

    bulk(&source, size, &destination, None, |_, _| {}, || false).expect("el paquete sin cancelar");
    assert!(destination.exists());
}

#[test]
fn cancelling_the_bulk_package_while_the_zip_is_assembled_keeps_the_previous_destination() {
    let source_directory = tempfile::tempdir().expect("carpeta de origen");
    let (source, size) = export_source(source_directory.path(), false);
    let folder = tempfile::tempdir().expect("carpeta de destino");
    let destination = folder.path().join("previo.zip");
    fs::write(&destination, "contenido previo").unwrap();
    let before = names_in(folder.path());

    // The CSV is complete; the ZIP being assembled is the `.tmp*` file next
    // to the destination, empty until the package begins to be written.
    let mut steps = Vec::new();
    let error = bulk(
        &source,
        size,
        &destination,
        None,
        |step, _| steps.push(step),
        once_a_file_holds(folder.path(), ".tmp", 1),
    )
    .expect_err("el paquete debe cancelarse");
    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert!(steps.contains(&"Preparando cargar.sql"), "{steps:?}");
    assert_export_left_nothing(folder.path(), &before, &destination);
}

#[test]
fn a_package_that_fails_after_the_csv_is_written_leaves_nothing() {
    let source_directory = tempfile::tempdir().expect("carpeta de origen");
    let (source, size) = export_source(source_directory.path(), false);
    let folder = tempfile::tempdir().expect("carpeta de destino");
    let destination = folder.path().join("clientes.zip");
    let before = names_in(folder.path());

    // The key column is checked against the columns when `cargar.sql` is
    // written, after the CSV (all the rows) is in the ZIP.
    let options: SqlServerScriptOptions =
        serde_json::from_str(r#"{"mode":"upsert","keyColumns":["codigo"]}"#)
            .expect("opciones de SQL Server");
    let mut steps = Vec::new();
    let error = bulk(
        &source,
        size,
        &destination,
        Some(&options),
        |step, _| steps.push(step),
        || false,
    )
    .expect_err("la clave no existe");
    assert!(error.contains("«codigo»"), "{error}");
    assert!(steps.contains(&"Preparando cargar.sql"), "{steps:?}");
    assert_export_left_nothing(folder.path(), &before, &destination);
}
