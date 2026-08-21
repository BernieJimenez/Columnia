import type {
  DatasetPreview,
  DatasetSourceInspection,
  OperationProgress,
  SpreadsheetHeaderMode,
} from "../../bridge";

export type ReadyDatasetStatus = {
  kind: "ready";
  dataset: DatasetPreview;
  pageOffset: number;
  pageLoading: boolean;
  pageError?: string;
};

export type DatasetStatus =
  | { kind: "empty" }
  | {
      kind: "loading";
      progress: OperationProgress;
      cancelRequested: boolean;
      previous?: ReadyDatasetStatus;
    }
  | ReadyDatasetStatus
  | { kind: "error"; message: string };

export type LoadInspectionState =
  | { kind: "idle" }
  | { kind: "inspecting" }
  | {
      kind: "sheet";
      source: DatasetSourceInspection;
      selectedSheetId: string;
      headerMode: SpreadsheetHeaderMode;
      error: string | null;
    }
  | { kind: "error"; message: string };

export type SheetSelectionAction =
  | { kind: "sheet_changed"; sheetId: string }
  | { kind: "header_mode_changed"; headerMode: SpreadsheetHeaderMode }
  | { kind: "confirmed" }
  | { kind: "cancelled" };

export function beginDatasetLoad(current: DatasetStatus): DatasetStatus {
  return {
    kind: "loading",
    progress: { operation: "load", stage: "Preparando carga", percent: 0 },
    cancelRequested: false,
    previous: current.kind === "ready" ? current : undefined,
  };
}

export function updateDatasetLoadProgress(
  current: DatasetStatus,
  progress: OperationProgress,
): DatasetStatus {
  return current.kind === "loading" ? { ...current, progress } : current;
}

export function requestDatasetLoadCancellation(current: DatasetStatus): DatasetStatus {
  return current.kind === "loading" ? { ...current, cancelRequested: true } : current;
}

export function restoreDatasetAfterLoadFailure(current: DatasetStatus): DatasetStatus {
  return current.kind === "loading" ? current.previous ?? { kind: "empty" } : current;
}

export function createReadyDatasetStatus(dataset: DatasetPreview): ReadyDatasetStatus {
  return { kind: "ready", dataset, pageOffset: 0, pageLoading: false };
}

export function workbookInspection(source: DatasetSourceInspection): LoadInspectionState {
  return {
    kind: "sheet",
    source,
    selectedSheetId: source.defaultSheetId ?? source.sheets[0]?.id ?? "",
    headerMode: "firstRow",
    error: null,
  };
}

export function updateSheetSelection(
  current: LoadInspectionState,
  action: Exclude<SheetSelectionAction, { kind: "confirmed" } | { kind: "cancelled" }>,
): LoadInspectionState {
  if (current.kind !== "sheet") return current;
  return action.kind === "sheet_changed"
    ? { ...current, selectedSheetId: action.sheetId }
    : { ...current, headerMode: action.headerMode };
}

export function setLoadInspectionError(
  current: LoadInspectionState,
  message: string,
): LoadInspectionState {
  return current.kind === "sheet"
    ? { ...current, error: message }
    : { kind: "error", message };
}

export function clearLoadInspectionError(current: LoadInspectionState): LoadInspectionState {
  if (current.kind === "sheet") return { ...current, error: null };
  return current.kind === "error" ? { kind: "idle" } : current;
}
