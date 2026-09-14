import type {
  DatasetPreview,
  DatasetSourceInspection,
  DelimitedHeaderReview,
  ImportDateConvention,
  ImportNumberConvention,
  ImportProfile,
  ImportProfileMismatch,
  OperationProgress,
  SpreadsheetHeaderMode,
} from "../../bridge";
import { importProfileApplicability } from "./importProfile";

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
  | { kind: "resource_preflight"; source: DatasetSourceInspection }
  | {
      kind: "sheet";
      source: DatasetSourceInspection;
      selectedSheetId: string;
      headerMode: SpreadsheetHeaderMode;
      suggestedProfile: ImportProfile | null;
      savedProfile: ImportProfile | null;
      profileCanBeApplied: boolean;
      useSavedProfile: boolean;
      headerReview?: DelimitedHeaderReview | null;
      headerReviewLoading?: boolean;
      error: string | null;
    }
  | {
      kind: "profile_review";
      source: DatasetSourceInspection;
      profile: ImportProfile;
      dateConvention: ImportDateConvention;
      numberConvention: ImportNumberConvention;
    }
  | {
      kind: "schema_mismatch";
      source: DatasetSourceInspection;
      profile: ImportProfile;
      mismatch: ImportProfileMismatch;
      sheetId: string | null;
      headerMode: SpreadsheetHeaderMode | null;
    }
  | { kind: "error"; message: string };

export type SheetSelectionAction =
  | { kind: "sheet_changed"; sheetId: string }
  | { kind: "header_mode_changed"; headerMode: SpreadsheetHeaderMode }
  | { kind: "profile_toggled"; useProfile: boolean }
  | { kind: "confirmed" }
  | { kind: "cancelled" };

export type ProfileReviewAction =
  | { kind: "use_profile" }
  | { kind: "use_defaults" }
  | { kind: "date_convention_changed"; value: ImportDateConvention }
  | { kind: "number_convention_changed"; value: ImportNumberConvention }
  | { kind: "cancelled" };

export type SchemaMismatchAction = { kind: "import_new_schema" } | { kind: "cancelled" };

export type ResourcePreflightAction = { kind: "confirmed" } | { kind: "cancelled" };

export function needsResourcePreflight(source: DatasetSourceInspection): boolean {
  const meaningfulFileSize = 64 * 1024 * 1024;
  return source.resourceEstimate.processingPath === "sourceBacked" ||
    source.fileSizeBytes >= meaningfulFileSize || source.isCompressedContainer;
}

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

export function workbookInspection(
  source: DatasetSourceInspection,
  savedProfile: ImportProfile | null = null,
): LoadInspectionState {
  const applicability = savedProfile
    ? importProfileApplicability(savedProfile, source)
    : null;
  const applicableProfile = applicability?.kind === "applicable" ? applicability : null;
  const profileCanBeApplied = applicableProfile !== null;
  const sameFormatProfile = savedProfile?.format === source.format ? savedProfile : null;
  return {
    kind: "sheet",
    source,
    selectedSheetId: applicableProfile && applicableProfile.sheetId !== null
      ? applicableProfile.sheetId
      : source.defaultSheetId ?? source.sheets[0]?.id ?? "",
    headerMode: applicableProfile && applicableProfile.headerMode !== null
      ? applicableProfile.headerMode
      : "firstRow",
    suggestedProfile: sameFormatProfile,
    savedProfile: profileCanBeApplied ? savedProfile : null,
    profileCanBeApplied,
    useSavedProfile: profileCanBeApplied,
    error: null,
  };
}

export function delimitedHeaderInspection(
  source: DatasetSourceInspection,
  savedProfile: ImportProfile | null = null,
): LoadInspectionState {
  const inspection = workbookInspection(source, savedProfile);
  if (inspection.kind !== "sheet") return inspection;
  return {
    ...inspection,
    selectedSheetId: "",
    headerReview: null,
    headerReviewLoading: true,
  };
}

export function completeDelimitedHeaderReview(
  current: LoadInspectionState,
  preview: DelimitedHeaderReview,
): LoadInspectionState {
  if (current.kind !== "sheet" || current.source.format === "excel") return current;
  return { ...current, headerReview: preview, headerReviewLoading: false, error: null };
}

export function beginDelimitedHeaderReview(current: LoadInspectionState): LoadInspectionState {
  if (current.kind !== "sheet" || current.source.format === "excel") return current;
  return { ...current, headerReview: null, headerReviewLoading: true, error: null };
}

export function updateSheetSelection(
  current: LoadInspectionState,
  action: Exclude<SheetSelectionAction, { kind: "confirmed" } | { kind: "cancelled" }>,
): LoadInspectionState {
  if (current.kind !== "sheet") return current;
  if (action.kind === "sheet_changed") {
    return { ...current, selectedSheetId: action.sheetId, useSavedProfile: false };
  }
  if (action.kind === "header_mode_changed") {
    return { ...current, headerMode: action.headerMode, useSavedProfile: false };
  }
  if (action.useProfile && current.profileCanBeApplied && current.savedProfile) {
    const applicability = importProfileApplicability(current.savedProfile, current.source);
    if (applicability.kind === "applicable") {
      return {
        ...current,
        selectedSheetId: applicability.sheetId ?? current.selectedSheetId,
        headerMode: applicability.headerMode ?? current.headerMode,
        useSavedProfile: true,
      };
    }
  }
  return { ...current, useSavedProfile: false };
}

export function updateProfileReview(
  current: LoadInspectionState,
  action: Exclude<ProfileReviewAction, { kind: "cancelled" }>,
): LoadInspectionState {
  if (current.kind !== "profile_review") return current;
  if (action.kind === "date_convention_changed") {
    return { ...current, dateConvention: action.value };
  }
  if (action.kind === "number_convention_changed") {
    return { ...current, numberConvention: action.value };
  }
  return current;
}

export function schemaMismatchInspection(
  source: DatasetSourceInspection,
  profile: ImportProfile,
  mismatch: ImportProfileMismatch,
  sheetId: string | null,
  headerMode: SpreadsheetHeaderMode | null,
): LoadInspectionState {
  return { kind: "schema_mismatch", source, profile, mismatch, sheetId, headerMode };
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
