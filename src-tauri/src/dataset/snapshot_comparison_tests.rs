use super::snapshot_comparison::compare_snapshot_frames;
use super::*;

fn before_frame() -> DataFrame {
    DataFrame::new(
        3,
        vec![
            Series::new("monto".into(), [Some("malo"), Some("2"), None]).into_column(),
            Series::new(
                "correo".into(),
                [Some("alice@example.test"), Some("bob@example.test"), None],
            )
            .into_column(),
            Series::new("solo_antes".into(), [Some("A"), Some("B"), Some("C")]).into_column(),
        ],
    )
    .expect("la revisión inicial debe ser válida")
}

fn after_frame() -> DataFrame {
    DataFrame::new(
        2,
        vec![
            Series::new("monto".into(), [2.0_f64, 3.0_f64]).into_column(),
            Series::new("correo".into(), ["alice@example.test", "bob@example.test"]).into_column(),
            Series::new("solo_despues".into(), ["D", "E"]).into_column(),
        ],
    )
    .expect("la revisión limpia debe ser válida")
}

/// The fields of a revision comparison; adding one means reviewing what it
/// reveals (QA-45).
const EXPECTED_COMPARISON_FIELDS: [&str; 9] = [
    "beforeSnapshotId",
    "afterSnapshotId",
    "beforeLabel",
    "afterLabel",
    "before",
    "after",
    "deltas",
    "columns",
    "quality",
];

/// QA-45: no text cell of either revision and no path of the history may
/// reach the serialized comparison, whatever its spelling.
fn assert_reveals_no_cell_or_path(
    serialized: &str,
    frames: &[&DataFrame],
    history: &HistoryManager,
) {
    for frame in frames {
        for column in frame.columns() {
            let Ok(values) = column.str() else { continue };
            for value in values.iter().flatten().filter(|value| value.len() > 1) {
                assert!(
                    !serialized.contains(value),
                    "la comparación revela la celda {value}"
                );
            }
        }
    }
    let directory = history.directory.path();
    for path in [
        directory.to_string_lossy().into_owned(),
        directory.to_string_lossy().replace('\\', "/"),
    ] {
        assert!(
            !serialized.contains(&path),
            "la comparación revela la ruta {path}"
        );
    }
    assert!(!serialized.contains(".parquet"));
}

fn rule(column: &str) -> QualityRule {
    serde_json::from_value(serde_json::json!({
        "column": column,
        "kind": "not_null",
        "maxInvalid": 0
    }))
    .expect("la regla de prueba debe ser válida")
}

#[test]
fn history_comparison_profiles_immutable_revisions_and_returns_aggregate_only_deltas() {
    let before = before_frame();
    let after = after_frame();
    let mut history = HistoryManager::new(&before).unwrap();
    let before_id = history.entries[0].id.clone();
    history.record(&after, "Corregir tipos y nulos").unwrap();
    let after_id = history.entries[1].id.clone();

    let (restored_before, before_label) = history.restore_by_id(&before_id).unwrap();
    let (restored_after, after_label) = history.restore_by_id(&after_id).unwrap();
    assert!(restored_before.equals_missing(&before));
    assert!(restored_after.equals_missing(&after));

    let comparison = compare_snapshot_frames(
        before_id.clone(),
        after_id.clone(),
        before_label,
        after_label,
        &restored_before,
        &restored_after,
        &[rule("correo"), rule("solo_antes")],
        |_, _| {},
        || false,
    )
    .expect("dos IDs explícitos deben compararse");

    assert_eq!(comparison.before_snapshot_id, before_id);
    assert_eq!(comparison.after_snapshot_id, after_id);
    assert_eq!(comparison.deltas.row_count, Some(-1));
    assert_eq!(comparison.deltas.column_count, Some(0));
    assert_eq!(comparison.deltas.null_count, Some(-2));
    let monto = comparison
        .columns
        .iter()
        .find(|column| column.name == "monto")
        .unwrap();
    assert!(monto.comparable);
    assert_eq!(monto.before.as_ref().unwrap().data_type, "str");
    assert_eq!(monto.after.as_ref().unwrap().data_type, "f64");
    assert_eq!(monto.type_changed, Some(true));
    let solo_antes = comparison
        .columns
        .iter()
        .find(|column| column.name == "solo_antes")
        .unwrap();
    assert!(!solo_antes.comparable);
    assert!(solo_antes
        .reason
        .as_deref()
        .unwrap()
        .contains("solo en una"));

    let correo_rule = &comparison.quality.rules[0];
    assert!(correo_rule.comparable);
    assert_eq!(correo_rule.before_invalid_count, Some(1));
    assert_eq!(correo_rule.after_invalid_count, Some(0));
    assert_eq!(comparison.quality.improved_rule_count, 1);
    let missing_rule = &comparison.quality.rules[1];
    assert!(!missing_rule.comparable);
    assert!(missing_rule
        .reason
        .as_deref()
        .unwrap()
        .contains("solo_antes"));

    let serialized = serde_json::to_string(&comparison).unwrap();
    assert_reveals_no_cell_or_path(&serialized, &[&before, &after], &history);
    assert!(!serialized.contains("histogram"));
    // A closed list: a new field has to be reviewed here before it ships.
    let value = serde_json::to_value(&comparison).unwrap();
    let mut fields = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    fields.sort();
    let expected = {
        let mut expected = EXPECTED_COMPARISON_FIELDS.map(str::to_owned).to_vec();
        expected.sort();
        expected
    };
    assert_eq!(fields, expected);

    let (still_before, _) = history.restore_by_id(&before_id).unwrap();
    assert!(
        still_before.equals_missing(&before),
        "una revisión guardada no debe mutar al registrar otra"
    );
}

#[test]
fn history_comparison_rejects_identical_and_obsolete_snapshot_ids() {
    let before = before_frame();
    let after = after_frame();
    let mut history = HistoryManager::with_limits(&before, 4, 1024 * 1024).unwrap();
    let original_id = history.entries[0].id.clone();
    assert!(compare_snapshot_frames(
        original_id.clone(),
        original_id.clone(),
        "Original".to_owned(),
        "Original".to_owned(),
        &before,
        &before,
        &[],
        |_, _| {},
        || false,
    )
    .unwrap_err()
    .contains("dos revisiones distintas"));

    history.record(&after, "Cambio A").unwrap();
    let obsolete_id = history.entries[1].id.clone();
    // QA-45: go back through the public undo, not by moving the cursor.
    let mut dataset = LoadedDataset {
        source_path: None,
        file_name: "datos.csv".to_owned(),
        file_size_bytes: 0,
        row_count: after.height(),
        frame: after.clone(),
        source_backed: false,
        delimited_header_mode: None,
        profile: None,
        history,
    };
    undo_dataset(&mut dataset).unwrap();
    let branch =
        DataFrame::new(1, vec![Series::new("monto".into(), [9_i64]).into_column()]).unwrap();
    dataset.history.record(&branch, "Nueva rama").unwrap();
    let history = &dataset.history;
    assert!(!history.contains_id(&obsolete_id));
    assert!(history
        .restore_by_id(&obsolete_id)
        .unwrap_err()
        .contains("ya no está disponible"));
    assert!(history.contains_id(&original_id));
}

#[test]
fn disabled_or_degraded_history_exposes_no_revision_ids() {
    let frame = before_frame();
    let mut history = HistoryManager::new(&frame).unwrap();
    history.disable_for_size("Cambio", 2);
    let state = history.state();
    // DAT-07: the original version stays; the unsaved one has no revision id.
    assert!(state.snapshots_enabled);
    assert_eq!(state.entries.len(), 2);
    assert!(state.entries[0].id.is_some());
    assert!(state.entries[1].id.is_none());
    assert!(state.entries[1].is_current);
    assert!(state.can_undo && !state.can_redo);
    assert!(history.restore_by_id("rev-obsolete-1").is_err());
}
