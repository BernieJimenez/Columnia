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
            file_size_bytes: fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0),
            row_count: frame.height(),
            frame,
            source_backed: false,
            delimited_header_mode: None,
            profile,
            history,
        };
        *state.current.lock_recovering() = Some(loaded);
        let bytes = process_working_set_bytes();
        sizes.push(bytes);
        println!(
            "probe   reload {cycle:>2}                        {:>8.1} MiB",
            bytes as f64 / (1024.0 * 1024.0)
        );
    }
    if let (Some(second), Some(last)) = (sizes.get(1), sizes.last()) {
        println!(
            "probe   last / cycle 2                    {:>8.3}",
            *last as f64 / *second as f64
        );
    }
}
