//! The snapshot of «Dataset original» written in the background after a load
//! (`PendingBaseline`), and the frame a schema review keeps for the load
//! (`CachedSelectionFrame`).

use super::history::write_frame_snapshot;
use super::page_reader::dataset_page_from_state;
use super::*;
use std::time::Duration;

fn values(values: &[i64]) -> DataFrame {
    DataFrame::new(
        values.len(),
        vec![Series::new("value".into(), values.to_vec()).into()],
    )
    .unwrap()
}

fn loaded(frame: DataFrame, history: HistoryManager) -> LoadedDataset {
    LoadedDataset {
        source_path: None,
        file_name: "datos.csv".to_owned(),
        file_size_bytes: 64,
        row_count: frame.height(),
        frame,
        source_backed: false,
        delimited_header_mode: None,
        profile: None,
        history,
    }
}

fn state_with(dataset: LoadedDataset) -> DatasetState {
    DatasetState {
        current: CurrentDataset::new(Some(dataset)),
        ..DatasetState::default()
    }
}

/// Lets the gated writer start after `delay`, from another thread, so the
/// caller is already waiting when it does.
fn release_after(gate: std::sync::mpsc::Sender<()>, delay: Duration) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _ = gate.send(());
    })
}

#[test]
fn a_background_original_snapshot_ends_as_the_synchronous_one() {
    let original = values(&[1, 2, 3]);
    let synchronous = HistoryManager::new(&original).unwrap();
    let mut background = HistoryManager::with_background_baseline(&original).unwrap();
    if let Some(slot) = background.baseline_in_progress() {
        slot.wait();
    }
    assert!(background.settle_baseline());
    assert!(!background.baseline_pending());

    let (expected, actual) = (synchronous.state(), background.state());
    assert_eq!(actual.entry_count, expected.entry_count);
    assert_eq!(actual.current_index, expected.current_index);
    assert_eq!(actual.can_undo, expected.can_undo);
    assert_eq!(actual.snapshots_enabled, expected.snapshots_enabled);
    assert_eq!(actual.entries[0].label, "Dataset original");
    assert_eq!(background.revision, synchronous.revision);
    assert_eq!(background.next_id, synchronous.next_id);
    assert_eq!(background.entries[0].bytes, synchronous.entries[0].bytes);
    assert!(background.restore(0).unwrap().equals_missing(&original));
    // Only the published snapshot is left in the folder.
    let files = fs::read_dir(background.directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "parquet"))
        .count();
    assert_eq!(files, 1);
}

#[test]
fn a_change_right_after_the_load_waits_for_the_original_and_undo_returns_to_it() {
    let original = values(&[1, 2, 3]);
    let (history, gate) = HistoryManager::with_gated_background_baseline(
        &original,
        HISTORY_MAX_ENTRIES,
        HISTORY_DISK_BUDGET_BYTES,
    )
    .unwrap();
    let state = state_with(loaded(original.clone(), history));

    // While it is pending, a page and the stamp of a profile do not wait.
    let stamp = {
        let current = state.current.lock_now();
        let dataset = current.as_ref().unwrap();
        assert!(dataset.history.baseline_pending());
        assert!(dataset.history.entries.is_empty());
        DatasetMutationStamp::capture(dataset)
    };
    let page = dataset_page_from_state(&state, 0, 10, || false).unwrap();
    assert_eq!(page.rows.len(), 3);

    let releaser = release_after(gate, Duration::from_millis(100));
    let mut current = state.current.lock_recovering();
    releaser.join().unwrap();
    let dataset = current.as_mut().unwrap();
    assert!(!dataset.history.baseline_pending());
    // A profile computed meanwhile is still cached for this version.
    assert!(stamp.matches(dataset));
    let history = dataset.history.state();
    assert_eq!(history.entry_count, 1);
    assert!(!history.can_undo);

    publish_candidate(dataset, values(&[10, 20, 30]), "Multiplicar").unwrap();
    assert!(dataset.history.state().can_undo);
    undo_dataset(dataset).unwrap();
    assert!(dataset.frame.equals_missing(&original));
}

#[test]
fn undo_and_history_state_while_pending_see_the_settled_history() {
    let original = values(&[4, 5]);
    let (history, gate) = HistoryManager::with_gated_background_baseline(
        &original,
        HISTORY_MAX_ENTRIES,
        HISTORY_DISK_BUDGET_BYTES,
    )
    .unwrap();
    let state = state_with(loaded(original, history));
    let releaser = release_after(gate, Duration::from_millis(50));
    let mut current = state.current.lock_recovering();
    releaser.join().unwrap();
    let dataset = current.as_mut().unwrap();
    assert_eq!(dataset.history.state().entry_count, 1);
    assert_eq!(
        undo_dataset(dataset).unwrap_err(),
        "No hay un cambio disponible para deshacer."
    );
}

#[test]
fn saving_a_project_while_the_original_is_pending_keeps_it_in_the_history() {
    let original = values(&[7, 8, 9]);
    let (history, gate) = HistoryManager::with_gated_background_baseline(
        &original,
        HISTORY_MAX_ENTRIES,
        HISTORY_DISK_BUDGET_BYTES,
    )
    .unwrap();
    let state = state_with(loaded(original.clone(), history));
    let releaser = release_after(gate, Duration::from_millis(50));
    let snapshot = state
        .active_project_snapshot()
        .expect("el guardado espera la copia del original");
    releaser.join().unwrap();
    assert_eq!(snapshot.history.entries.len(), 1);
    assert_eq!(snapshot.history.cursor, 0);
    assert!(snapshot.history.snapshots_enabled);
    assert_eq!(snapshot.history.entries[0].label, "Dataset original");
    assert!(read_parquet_frame(&snapshot.history.entries[0].path)
        .unwrap()
        .equals_missing(&original));
}

#[test]
fn loading_another_dataset_cancels_a_pending_original_and_removes_its_folder() {
    let original = values(&[1, 2]);
    // The gate is never opened: only the cancellation can end the thread.
    let (history, _gate) = HistoryManager::with_gated_background_baseline(
        &original,
        HISTORY_MAX_ENTRIES,
        HISTORY_DISK_BUDGET_BYTES,
    )
    .unwrap();
    let directory = history.directory.path().to_owned();
    let state = state_with(loaded(original, history));

    let next = values(&[3]);
    let replacement = loaded(next.clone(), HistoryManager::new(&next).unwrap());
    let started = std::time::Instant::now();
    // As `commit_load` does: replace without waiting, drop after the lock.
    let previous = state.current.lock_now().replace(replacement);
    drop(previous);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(!directory.exists(), "la carpeta del historial anterior debe borrarse");

    let current = state.current.lock_recovering();
    assert!(current.as_ref().unwrap().frame.equals_missing(&next));
}

#[test]
fn an_original_over_the_disk_budget_disables_the_history_with_its_reason() {
    let original = values(&[1, 2, 3]);
    let history =
        HistoryManager::with_background_baseline_and_limits(&original, HISTORY_MAX_ENTRIES, 0)
            .unwrap();
    let state = state_with(loaded(original.clone(), history));
    let mut current = state.current.lock_recovering();
    let dataset = current.as_mut().unwrap();
    let history = dataset.history.state();
    assert!(!history.snapshots_enabled);
    assert!(!history.can_undo);
    assert!(history.degraded_reason.unwrap().contains("supera el límite"));
    // A project still sees a consistent degraded history.
    let capture =
        capture_project_history_with_cancel(&dataset.history, &dataset.frame, || false).unwrap();
    assert!(capture.entries.is_empty());
    // And a change still applies, without history.
    publish_candidate(dataset, values(&[0, 0, 0]), "Vaciar").unwrap();
    assert!(!dataset.frame.equals_missing(&original));
}

#[test]
fn a_cancelled_record_leaves_the_history_as_it_was() {
    let original = values(&[1, 2, 3]);
    let mut history = HistoryManager::new(&original).unwrap();
    let revision = history.revision;
    let error = history
        .record_with_cancel(&values(&[4]), "Paso", || true)
        .unwrap_err();
    assert_eq!(error, OPERATION_CANCELLED_MESSAGE);
    assert_eq!(history.revision, revision);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.current_label, "Dataset original");
}

#[test]
fn the_snapshot_writer_keeps_one_row_group_per_block_of_a_chunked_frame() {
    // Many small chunks, as a streamed read leaves them.
    let mut frame = values(&[0]);
    for value in 1..50_i64 {
        frame.vstack_mut(&values(&[value])).unwrap();
    }
    assert!(frame.first_col_n_chunks() > 1);
    let directory = tempfile::tempdir().unwrap();
    let prepared = write_frame_snapshot(directory.path(), &frame, || false).unwrap();
    let written = read_parquet_frame(prepared.temporary.path()).unwrap();
    assert!(written.equals_missing(&frame));
    let groups = ParquetReader::new(File::open(prepared.temporary.path()).unwrap())
        .get_metadata()
        .unwrap()
        .row_groups
        .len();
    assert_eq!(groups, 1);
}

#[test]
fn a_cached_review_frame_is_reused_only_for_the_same_reading_of_the_same_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("datos.csv");
    fs::write(&path, "a,b\n1,2\n3,4\n").unwrap();
    let (size, modified) = CachedSelectionFrame::file_identity(&path).unwrap();
    let cached = || CachedSelectionFrame {
        path: path.clone(),
        header_mode: SpreadsheetHeaderMode::FirstRow,
        file_size_bytes: size,
        modified,
        frame: values(&[1]),
    };
    assert!(cached()
        .into_frame_for(&path, SpreadsheetHeaderMode::FirstRow)
        .is_some());
    assert!(cached()
        .into_frame_for(&path, SpreadsheetHeaderMode::Generated)
        .is_none());
    let other = directory.path().join("copia.csv");
    fs::copy(&path, &other).unwrap();
    assert!(cached()
        .into_frame_for(&other, SpreadsheetHeaderMode::FirstRow)
        .is_none());
    // A file written again (another size) is read again.
    fs::write(&path, "a,b\n1,2\n3,4\n5,6\n").unwrap();
    assert!(cached()
        .into_frame_for(&path, SpreadsheetHeaderMode::FirstRow)
        .is_none());
}

#[test]
fn taking_a_selection_removes_its_cached_frame() {
    let state = DatasetState::default();
    *state.pending_selection.lock_recovering() = Some(PendingSelection {
        id: "selection-1".to_owned(),
        generation: 1,
        path: PathBuf::from("datos.csv"),
        file_size_bytes: 10,
        sheets: Vec::new(),
        cached_frame: Some(CachedSelectionFrame {
            path: PathBuf::from("datos.csv"),
            header_mode: SpreadsheetHeaderMode::FirstRow,
            file_size_bytes: 10,
            modified: None,
            frame: values(&[1]),
        }),
    });
    assert!(take_pending_selection(&state, "otra").is_err());
    let (pending, cached) = take_pending_selection(&state, "selection-1").unwrap();
    assert!(pending.cached_frame.is_none());
    assert!(cached.is_some());
    let selection = state.pending_selection.lock_recovering();
    assert!(selection.as_ref().unwrap().cached_frame.is_none());
}
