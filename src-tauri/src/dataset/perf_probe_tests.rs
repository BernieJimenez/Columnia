//! Opt-in timing probe for the interactive path (load → profile → prepare →
//! history → export) over a synthetic, realistic CSV. It asserts nothing about
//! durations; it prints them so a change can be compared before and after.
//!
//! `COLUMNIA_PROBE_ROWS` sets the row count (default 300 000).
//! Run with `COLUMNIA_TEST_HARNESS_MANIFEST=1` (Windows) and
//! `cargo test --release --lib perf_probe -- --ignored --nocapture`.

use super::*;
use std::time::Instant;

fn probe_csv(path: &Path, rows: usize) {
    let mut writer = BufWriter::new(File::create(path).expect("csv del probe"));
    writeln!(
        writer,
        "Id,Cliente,Categoría,Monto,Cantidad,Fecha,Email,Activo,Notas,Código"
    )
    .unwrap();
    let categories = ["Hogar", "hogar ", "Oficina", "Jardín", "N/A", ""];
    let names = [
        "Ana Pérez",
        " ana pérez",
        "Luis Gómez",
        "María  López",
        "José Ruiz",
    ];
    for index in 0..rows {
        let amount = if index % 17 == 0 {
            "N/A".to_owned()
        } else if index % 23 == 0 {
            String::new()
        } else {
            format!("{}.{:02}", (index * 37) % 5000, index % 100)
        };
        let quantity = if index % 29 == 0 {
            String::new()
        } else {
            ((index * 7) % 40).to_string()
        };
        let date = if index % 31 == 0 {
            String::new()
        } else {
            format!(
                "{:04}-{:02}-{:02}",
                2020 + index % 5,
                1 + index % 12,
                1 + index % 28
            )
        };
        let active = if index % 2 == 0 { "sí" } else { "no" };
        // Every 50th row repeats the previous one to create duplicates.
        let id = if index % 50 == 49 { index - 1 } else { index };
        writeln!(
            writer,
            "{id},{},{},{amount},{quantity},{date},user{}@example.com,{active},\"Nota número {} con texto libre\",{:05}",
            names[id % names.len()],
            categories[id % categories.len()],
            id % 9000,
            id % 700,
            id % 20000,
        )
        .unwrap();
    }
    writer.flush().unwrap();
}

fn timed<T>(label: &str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = run();
    println!(
        "probe {label:<34} {:>8.1} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    value
}

#[test]
#[ignore = "sonda de tiempos opt-in; imprime duraciones del camino interactivo"]
fn perf_probe_interactive_path() {
    let rows = std::env::var("COLUMNIA_PROBE_ROWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300_000usize);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("probe.csv");
    probe_csv(&source, rows);
    println!(
        "probe rows={rows} size={:.1} MiB",
        fs::metadata(&source).unwrap().len() as f64 / (1024.0 * 1024.0)
    );

    let (frame, _) = timed("load csv", || {
        load_dataset_with_header_mode(
            &source,
            SpreadsheetHeaderMode::FirstRow,
            |_, _| {},
            || false,
        )
        .expect("carga")
    });
    timed("profile", || {
        profile_dataset_with_progress(
            &frame,
            |_, _| {},
            || false,
            MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
        )
        .expect("perfil")
    });
    timed("profile: distinct rows", || {
        count_distinct_rows(&frame).unwrap()
    });
    timed("profile: near duplicates", || {
        count_normalized_duplicate_rows(&frame, 0, &|| false, &mut |_, _| {}).unwrap()
    });
    timed("dataset page", || dataset_preview(&source, &frame).unwrap());
    let profile = profile_dataset(&frame).expect("perfil para Explorar");
    timed("explore panel", || {
        explore::explore_panel(
            frame.clone().lazy(),
            &profile,
            &[],
            &ExploreLayout::default(),
        )
        .expect("panel")
    });
    let state_filter: ExploreFilter = serde_json::from_value(serde_json::json!({
        "column": "Categoría", "values": ["Hogar"]
    }))
    .unwrap();
    timed("explore panel, one filter", || {
        explore::explore_panel(
            frame.clone().lazy(),
            &profile,
            &[state_filter],
            &ExploreLayout::default(),
        )
        .expect("panel")
    });

    let mut history = timed("history: new (baseline snapshot)", || {
        HistoryManager::new(&frame).expect("historial")
    });

    let cast = ["Monto".to_owned(), "Cantidad".to_owned()];
    let plan = timed("safe corrections plan", || {
        safe_corrected_plan_frame(
            &frame,
            true,
            true,
            true,
            true,
            true,
            None,
            Some(&cast),
            None,
        )
        .expect("plan")
    });
    let prepared = plan.frame;
    timed("history: record snapshot", || {
        let snapshot = history.prepare_frame_snapshot(&prepared, || false).unwrap();
        history.record_prepared_snapshot(snapshot, "probe").unwrap();
    });
    timed("profile after plan", || {
        profile_dataset_with_progress(
            &prepared,
            |_, _| {},
            || false,
            MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
        )
        .expect("perfil")
    });
    timed("impute missing", || {
        impute_missing_values_in_frame(&frame).unwrap()
    });
    timed("clean text: normalize", || {
        clean_text_columns(
            &frame,
            None,
            TextCleaningMode::Normalize {
                remove_accents: true,
            },
        )
        .unwrap()
    });
    timed("clean text: trim", || {
        clean_text_columns(&frame, None, TextCleaningMode::Trim).unwrap()
    });
    timed("history: restore (undo)", || history.restore(0).unwrap());

    for (label, format, name) in [
        ("export csv", ExportFormat::Csv, "out.csv"),
        ("export parquet", ExportFormat::Parquet, "out.parquet"),
        ("export xlsx", ExportFormat::Excel, "out.xlsx"),
    ] {
        let destination = directory.path().join(name);
        timed(label, || {
            export_frame_atomic(&prepared, &destination, format, |_, _| {}, || false).unwrap()
        });
        println!(
            "probe   {name} size                      {:>8.1} MiB",
            fs::metadata(&destination).unwrap().len() as f64 / (1024.0 * 1024.0)
        );
    }
}

fn process_working_set_bytes() -> u64 {
    use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = get_current_pid().expect("pid");
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(pid).map_or(0, |process| process.memory())
}

/// REN-02: reloads the same file into the active dataset, as the app does
/// (load, profile, history, replace), and prints the working set of each
/// cycle. `COLUMNIA_PROBE_RELOAD_FILE` names the CSV, `COLUMNIA_PROBE_RELOADS`
/// the cycles (default 20).
#[test]
#[ignore = "sonda de memoria opt-in con un archivo grande"]
fn perf_probe_reload_memory() {
    let Ok(path) = std::env::var("COLUMNIA_PROBE_RELOAD_FILE") else {
        println!("probe   COLUMNIA_PROBE_RELOAD_FILE no definida; nada que medir");
        return;
    };
    let cycles = std::env::var("COLUMNIA_PROBE_RELOADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    crate::configure_allocator();
    let state = DatasetState::default();
    let path = PathBuf::from(path);
    let mut sizes = Vec::with_capacity(cycles);
    for cycle in 1..=cycles {
        let (frame, _) = load_dataset_with_progress(&path, |_, _| {}, || false).expect("carga");
        // `COLUMNIA_PROBE_RELOAD_STEPS` (load,profile,history) isolates a step.
        let steps = std::env::var("COLUMNIA_PROBE_RELOAD_STEPS")
            .unwrap_or_else(|_| "load,profile,history".to_owned());
        let profile = steps
            .contains("profile")
            .then(|| profile_dataset(&frame).expect("perfil"));
        let history = if steps.contains("history") {
            HistoryManager::new(&frame).expect("historial")
        } else {
            HistoryManager::deferred().expect("historial diferido")
        };
        let loaded = LoadedDataset {
            source_path: Some(path.clone()),
            file_name: "probe.csv".to_owned(),
            file_size_bytes: fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            row_count: frame.height(),
            frame,
            source_backed: false,
            delimited_header_mode: None,
            profile,
            history,
        };
        *state.current.lock_recovering() = Some(loaded);
        // `COLUMNIA_PROBE_RELOAD_PAUSE_MS`: idle time before measuring, as a
        // person reads the result before loading again.
        if let Some(pause) = std::env::var("COLUMNIA_PROBE_RELOAD_PAUSE_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(pause));
        }
        let bytes = process_working_set_bytes();
        sizes.push(bytes);
        println!(
            "probe   reload {cycle:>2}                        {:>8.1} MiB   en uso {:>8.1} MiB",
            bytes as f64 / (1024.0 * 1024.0),
            crate::ALLOCATED_BYTES.load(std::sync::atomic::Ordering::Relaxed) as f64
                / (1024.0 * 1024.0)
        );
    }
    if let (Some(second), Some(last)) = (sizes.get(1), sizes.last()) {
        println!(
            "probe   last / cycle 2                    {:>8.3}",
            *last as f64 / *second as f64
        );
    }
}

/// REN-01: profiles a file the way `get_dataset_profile` does (in memory below
/// the source-backed threshold, from disk above it) and prints the total time
/// and every progress label with the second it appeared.
/// `COLUMNIA_PROBE_PROFILE_FILE` names the file.
#[test]
#[ignore = "sonda de perfil opt-in con un archivo grande"]
fn perf_probe_profile() {
    let Ok(path) = std::env::var("COLUMNIA_PROBE_PROFILE_FILE") else {
        println!("probe   COLUMNIA_PROBE_PROFILE_FILE no definida; nada que medir");
        return;
    };
    let path = PathBuf::from(path);
    let extension = dataset_extension(&path).expect("extensión");
    let size = fs::metadata(&path).expect("archivo").len();
    // The peak working set of the process, sampled while the profile runs.
    let sampling = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let sampler = {
        let sampling = std::sync::Arc::clone(&sampling);
        let peak = std::sync::Arc::clone(&peak);
        std::thread::spawn(move || {
            while sampling.load(std::sync::atomic::Ordering::Relaxed) {
                peak.fetch_max(
                    process_working_set_bytes(),
                    std::sync::atomic::Ordering::Relaxed,
                );
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        })
    };
    let started = Instant::now();
    let mut last = Instant::now();
    let mut longest_silence = 0.0_f64;
    let mut report = |stage: &'static str, percent: u8| {
        let now = Instant::now();
        longest_silence = longest_silence.max(now.duration_since(last).as_secs_f64());
        last = now;
        println!(
            "probe   {:>7.2} s  {percent:>3} %  {stage}",
            started.elapsed().as_secs_f64()
        );
    };
    let profile_started;
    if should_defer_source_load(&extension, size) {
        let (_, _, row_count) =
            source_backed_load(&path, &extension, || false).expect("carga source-backed");
        println!(
            "probe   carga source-backed           {:>7.2} s",
            started.elapsed().as_secs_f64()
        );
        profile_started = Instant::now();
        let profile = profile_source_backed_with_progress(
            &path,
            &extension,
            size,
            row_count,
            &mut report,
            || false,
            MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
        )
        .expect("perfil source-backed");
        println!(
            "probe   filas repetidas {} · parecidas {}",
            profile.duplicate_row_count, profile.near_duplicate_row_count
        );
    } else {
        let (frame, _) = load_dataset_with_progress(&path, |_, _| {}, || false).expect("carga");
        println!(
            "probe   carga en memoria              {:>7.2} s",
            started.elapsed().as_secs_f64()
        );
        if std::env::var_os("COLUMNIA_PROBE_PROFILE_PARTS").is_some() {
            let part = Instant::now();
            let distinct = count_distinct_rows(&frame).expect("distintas");
            println!(
                "probe   filas distintas               {:>7.2} s",
                part.elapsed().as_secs_f64()
            );
            let part = Instant::now();
            count_normalized_duplicate_rows(
                &frame,
                frame.height() - distinct,
                &|| false,
                &mut |_, _| {},
            )
            .expect("parecidas");
            println!(
                "probe   filas parecidas               {:>7.2} s",
                part.elapsed().as_secs_f64()
            );
        }
        if std::env::var_os("COLUMNIA_PROBE_PROFILE_PARTS").is_some() {
            for column in frame.columns() {
                let part = Instant::now();
                let mut stages = Vec::new();
                profile_column(column, frame.height(), &|| false, |stage, _| {
                    stages.push((stage, part.elapsed().as_secs_f64()))
                })
                .expect("columna");
                println!(
                    "probe   columna {:<24} {:>6.2} s  {}  {:?}",
                    column.name().as_str(),
                    part.elapsed().as_secs_f64(),
                    column.dtype(),
                    stages
                        .iter()
                        .map(|(stage, at)| format!("{stage}@{at:.2}"))
                        .collect::<Vec<_>>()
                );
            }
        }
        profile_started = Instant::now();
        profile_dataset_with_progress(
            &frame,
            &mut report,
            || false,
            MAX_NUMERIC_CORRELATION_SAMPLE_ROWS,
        )
        .expect("perfil");
    }
    println!(
        "probe   perfil                        {:>7.2} s",
        profile_started.elapsed().as_secs_f64()
    );
    println!("probe   mayor silencio entre etiquetas {longest_silence:>7.2} s");
    sampling.store(false, std::sync::atomic::Ordering::Relaxed);
    sampler.join().expect("muestreo de memoria");
    println!(
        "probe   pico de working set           {:>7.0} MiB",
        peak.load(std::sync::atomic::Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
    );
}

/// REN-08: times a keyed comparison between two generated Parquet files of
/// `COLUMNIA_PROBE_COMPARE_ROWS` rows (default 2 000 000) where one row in ten
/// changes its amount: the summary, the first conflict page and a page in the
/// middle. `COLUMNIA_PROBE_COMPARE_SHUFFLED` stores the compared rows in
/// another order, as a file exported by a different system would.
#[test]
#[ignore = "sonda opt-in de comparación por clave con millones de filas"]
fn perf_probe_keyed_comparison() {
    let rows = std::env::var("COLUMNIA_PROBE_COMPARE_ROWS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2_000_000);
    let order = if std::env::var_os("COLUMNIA_PROBE_COMPARE_SHUFFLED").is_some() {
        "ORDER BY hash(i)"
    } else {
        ""
    };
    let directory = tempfile::tempdir().expect("directorio temporal");
    let current = directory.path().join("current.parquet");
    let compared = directory.path().join("compared.parquet");
    let sql_path = |path: &Path| path.to_string_lossy().replace('\\', "/");
    let started = Instant::now();
    let connection = duckdb::Connection::open_in_memory().expect("DuckDB");
    let select = |amount: &str, order: &str| {
        format!(
            "SELECT i AS id, 'cliente ' || (i % 50000) AS cliente, CAST({amount} AS DOUBLE) AS importe, \
             DATE '2024-01-01' + CAST(i % 365 AS INTEGER) AS fecha, \
             CASE i % 3 WHEN 0 THEN 'pagado' WHEN 1 THEN 'enviado' ELSE 'devuelto' END AS estado \
             FROM range({rows}) t(i) {order}"
        )
    };
    connection
        .execute_batch(&format!(
            "COPY ({}) TO '{}' (FORMAT PARQUET); COPY ({}) TO '{}' (FORMAT PARQUET);",
            select("(i % 997) * 1.5", ""),
            sql_path(&current),
            select(
                "CASE WHEN i % 10 = 0 THEN (i % 997) * 1.5 + 1 ELSE (i % 997) * 1.5 END",
                order
            ),
            sql_path(&compared),
        ))
        .expect("archivos de la sonda");
    println!(
        "probe   archivos ({rows} filas)          {:>7.2} s",
        started.elapsed().as_secs_f64()
    );
    let current_schema = read_parquet_schema_frame(&current).expect("esquema");
    let compared_schema = read_parquet_schema_frame(&compared).expect("esquema");
    let key = vec!["id".to_owned()];
    let shared = ["id", "cliente", "importe", "fecha", "estado"].map(str::to_owned);
    let source = |path| ParquetComparisonSource {
        path,
        row_count: rows,
    };
    let part = Instant::now();
    let summary = compare_keyed_parquet_sources_with_cancel(
        source(&current),
        source(&compared),
        &current_schema,
        &compared_schema,
        &key,
        &shared,
        &|| false,
    )
    .expect("resumen");
    println!(
        "probe   resumen ({} conflictos)        {:>7.2} s",
        summary.conflicting_key_count,
        part.elapsed().as_secs_f64()
    );
    for offset in [0, summary.conflicting_key_count / 2] {
        let part = Instant::now();
        let (page, _) = collect_key_conflicts_page_between_parquet_with_cancel(
            source(&current),
            source(&compared),
            &key,
            &shared,
            offset,
            50,
            &|| false,
        )
        .expect("página");
        println!(
            "probe   página en {offset:>9} ({} filas)   {:>7.2} s",
            page.len(),
            part.elapsed().as_secs_f64()
        );
    }

    // In the app the active file pages through the conflict index: the first
    // page builds it, the next ones only read their rows.
    let (frame, _, row_count) =
        source_backed_load(&current, "parquet", || false).expect("fuente source-backed");
    let state = DatasetState::default();
    *state.current.lock().expect("estado activo") = Some(LoadedDataset {
        source_path: Some(current.clone()),
        file_name: "current.parquet".to_owned(),
        file_size_bytes: fs::metadata(&current).expect("metadatos").len(),
        row_count,
        frame,
        source_backed: true,
        delimited_header_mode: None,
        profile: None,
        history: HistoryManager::deferred().expect("historial diferido"),
    });
    for (label, offset) in [
        ("índice + primera", 0),
        ("siguiente", 50),
        ("a mitad", summary.conflicting_key_count / 2),
    ] {
        let part = Instant::now();
        let page = disk_backed_conflict_page(&state, &compared, rows, &key, offset, 50)
            .expect("página")
            .expect("ruta en disco");
        println!(
            "probe   app: {label:<17} ({} filas) {:>7.2} s",
            page.conflicts.len(),
            part.elapsed().as_secs_f64()
        );
    }
}

/// REN-13: writes `COLUMNIA_PROBE_GENERATE_ROWS` rows (default 100 000 000)
/// of numbers, a category and a date to the CSV `COLUMNIA_PROBE_GENERATE_CSV`,
/// to measure what the profile of a very large file needs on disk.
#[test]
#[ignore = "sonda opt-in que genera un CSV muy grande"]
fn perf_probe_generate_csv() {
    let Ok(path) = std::env::var("COLUMNIA_PROBE_GENERATE_CSV") else {
        println!("probe   COLUMNIA_PROBE_GENERATE_CSV no definida; nada que generar");
        return;
    };
    let rows = std::env::var("COLUMNIA_PROBE_GENERATE_ROWS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100_000_000);
    let started = Instant::now();
    let connection = duckdb::Connection::open_in_memory().expect("DuckDB");
    connection
        .execute_batch(&format!(
            "COPY (SELECT i AS id, round((i % 99991) * 0.37, 2) AS importe,              (i * 7919) % 1000003 AS cantidad, round(sin(i) * 1000, 3) AS saldo,              'zona ' || (i % 12) AS zona, DATE '2020-01-01' + CAST(i % 1500 AS INTEGER) AS fecha              FROM range({rows}) t(i)) TO '{}' (FORMAT CSV, HEADER)",
            path.replace('\\', "/")
        ))
        .expect("CSV generado");
    println!(
        "probe   CSV de {rows} filas en {:.1} s ({} MiB)",
        started.elapsed().as_secs_f64(),
        fs::metadata(&path).expect("CSV").len() / (1024 * 1024)
    );
}
