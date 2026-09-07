import { invoke } from "@tauri-apps/api/core";
import type {
  DatasetPreview,
  DatasetComparison,
  DatasetConflictPage,
  DatasetJoinType,
  DatasetSourceInspection,
  SampleDatasetDescriptor,
  SpreadsheetHeaderMode,
  DatasetPage,
  DatasetQueryResult,
  DatasetQueryEngine,
  DatasetProfile,
  CancellableOperation,
  ConflictResolution,
} from "./contracts";
import { progressChannel, type ProgressHandler } from "./progress";

export function listSampleDatasets(): Promise<SampleDatasetDescriptor[]> {
  return invoke<SampleDatasetDescriptor[]>("list_sample_datasets");
}

export function inspectSampleDataset(sampleId: string): Promise<DatasetSourceInspection> {
  return invoke<DatasetSourceInspection>("inspect_sample_dataset", { sampleId });
}

export function pickDatasetSource(): Promise<DatasetSourceInspection | null> {
  return invoke<DatasetSourceInspection | null>("pick_dataset_source");
}

/** Inspects the path captured by Tauri's native drag/drop event without exposing it to React. */
export function inspectDroppedDataset(): Promise<DatasetSourceInspection | null> {
  return invoke<DatasetSourceInspection | null>("inspect_dropped_dataset");
}

export function loadDatasetSelection(
  selectionId: string,
  sheetId: string | null,
  headerMode: SpreadsheetHeaderMode | null,
  onProgress?: ProgressHandler,
): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("load_dataset_selection", {
    selectionId,
    sheetId,
    headerMode,
    onProgress: progressChannel(onProgress),
  });
}

export function discardDatasetSelection(selectionId: string): Promise<void> {
  return invoke<void>("discard_dataset_selection", { selectionId });
}

export function compareDataset(keyColumns: string[] = []): Promise<DatasetComparison | null> {
  return invoke<DatasetComparison | null>("compare_dataset", { keyColumns });
}

export function getDatasetConflictPage(
  offset: number,
  limit: number,
): Promise<DatasetConflictPage | null> {
  return invoke<DatasetConflictPage | null>("get_dataset_conflict_page", { offset, limit });
}

export function joinDataset(
  keyColumns: string[],
  joinType: DatasetJoinType,
): Promise<DatasetPreview | null> {
  return invoke<DatasetPreview | null>("join_dataset", { keyColumns, joinType });
}

export function resolveDatasetConflicts(
  decisions: ConflictResolution[],
): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("resolve_dataset_conflicts", { decisions });
}

export function clearDatasetComparison(): Promise<void> {
  return invoke<void>("clear_dataset_comparison");
}

export function useConsolidatedDataset(): Promise<DatasetPreview> {
  return invoke<DatasetPreview>("use_consolidated_dataset");
}

export function getDatasetPage(offset: number, limit: number): Promise<DatasetPage> {
  return invoke<DatasetPage>("get_dataset_page", { offset, limit });
}

/**
 * Runs the bounded local SELECT contract. JOIN can read only the comparison
 * already loaded by Review through the logical table `compared`. Polars is
 * the default engine; DuckDB is an explicit local alternative.
 */
export function queryDataset(
  query: string,
  engine: DatasetQueryEngine = "polars",
): Promise<DatasetQueryResult> {
  return invoke<DatasetQueryResult>("query_dataset", { query, engine });
}

export function getDatasetProfile(
  onProgress?: ProgressHandler,
  correlationSampleRows?: number,
): Promise<DatasetProfile> {
  return invoke<DatasetProfile>("get_dataset_profile", {
    onProgress: progressChannel(onProgress),
    correlationSampleRows: correlationSampleRows,
  });
}

export function cancelOperation(operation: CancellableOperation): Promise<void> {
  return invoke<void>("cancel_operation", { operation });
}
