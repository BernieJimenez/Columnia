use serde::Serialize;

use super::DatasetPreview;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResult {
    pub(super) dataset: DatasetPreview,
    pub(super) history: HistoryState,
    pub(super) message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryState {
    pub(super) id: Option<String>,
    pub(super) index: usize,
    pub(super) label: String,
    pub(super) is_current: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub(super) can_undo: bool,
    pub(super) can_redo: bool,
    pub(super) current_index: usize,
    pub(super) entry_count: usize,
    pub(super) entries: Vec<HistoryEntryState>,
    pub(super) snapshots_enabled: bool,
    pub(super) degraded_reason: Option<String>,
    pub(super) max_entries: usize,
    pub(super) disk_bytes: u64,
    pub(super) disk_budget_bytes: u64,
}
