import type { DatasetPage, DatasetPreview, DatasetProfile, OperationProgress } from "../../bridge";
import type { ReadyDatasetStatus } from "../load/loadModel";

export const PAGE_SIZE = 50;

export type ProfileStatus =
  | { kind: "idle" }
  | { kind: "loading"; progress: OperationProgress; cancelRequested: boolean }
  | { kind: "ready"; profile: DatasetProfile }
  | { kind: "error"; message: string };

export function beginProfileAnalysis(): ProfileStatus {
  return {
    kind: "loading",
    progress: { operation: "profile", stage: "Iniciando análisis", percent: 0 },
    cancelRequested: false,
  };
}

export function updateProfileProgress(
  current: ProfileStatus,
  progress: OperationProgress,
): ProfileStatus {
  if (current.kind !== "loading" || progress.percent < current.progress.percent) {
    return current;
  }
  return { ...current, progress: { ...progress, percent: Math.min(100, progress.percent) } };
}

export function requestProfileCancellation(current: ProfileStatus): ProfileStatus {
  return current.kind === "loading" ? { ...current, cancelRequested: true } : current;
}

export function beginPageLoad(current: ReadyDatasetStatus): ReadyDatasetStatus {
  return { ...current, pageLoading: true, pageError: undefined };
}

export function completePageLoad(
  previous: ReadyDatasetStatus,
  page: DatasetPage,
): ReadyDatasetStatus {
  return {
    kind: "ready",
    dataset: { ...previous.dataset, rows: page.rows },
    pageOffset: page.offset,
    pageLoading: false,
  };
}

export function failPageLoad(
  previous: ReadyDatasetStatus,
  message: string,
): ReadyDatasetStatus {
  return { ...previous, pageLoading: false, pageError: message };
}

export function previousPageOffset(pageOffset: number): number {
  return Math.max(0, pageOffset - PAGE_SIZE);
}

export function nextPageOffset(pageOffset: number): number {
  return pageOffset + PAGE_SIZE;
}

export function normalizePageOffset(pageOffset: number, rowCount: number): number {
  if (!Number.isSafeInteger(pageOffset) || !Number.isSafeInteger(rowCount) || pageOffset <= 0 || rowCount <= 0) {
    return 0;
  }
  const lastPageOffset = Math.floor((rowCount - 1) / PAGE_SIZE) * PAGE_SIZE;
  return Math.min(Math.floor(pageOffset / PAGE_SIZE) * PAGE_SIZE, lastPageOffset);
}

export function pageRange(dataset: DatasetPreview, pageOffset: number) {
  const end = pageOffset + dataset.rows.length;
  return {
    end,
    hasPrevious: pageOffset > 0,
    hasNext: end < dataset.rowCount,
  };
}
