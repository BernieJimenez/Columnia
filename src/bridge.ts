import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
}

export interface DatasetColumn {
  name: string;
  dataType: string;
}

export interface DatasetPreview {
  fileName: string;
  fileSizeBytes: number;
  rowCount: number;
  columnCount: number;
  columns: DatasetColumn[];
  rows: Array<Array<string | null>>;
}

export interface DatasetPage {
  offset: number;
  rows: Array<Array<string | null>>;
}

export interface ColumnProfile {
  name: string;
  dataType: string;
  nullCount: number;
  completenessPercentage: number;
  uniqueCount: number;
  minimum: string | null;
  maximum: string | null;
  mean: number | null;
  emptyCount: number | null;
  minimumLength: number | null;
  maximumLength: number | null;
  averageLength: number | null;
}

export interface DatasetProfile {
  rowCount: number;
  duplicateRowCount: number;
  duplicatePercentage: number;
  columns: ColumnProfile[];
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export function pickAndLoadCsv(): Promise<DatasetPreview | null> {
  return invoke<DatasetPreview | null>("pick_and_load_csv");
}

export function getDatasetPage(offset: number, limit: number): Promise<DatasetPage> {
  return invoke<DatasetPage>("get_dataset_page", { offset, limit });
}

export function getDatasetProfile(): Promise<DatasetProfile> {
  return invoke<DatasetProfile>("get_dataset_profile");
}
