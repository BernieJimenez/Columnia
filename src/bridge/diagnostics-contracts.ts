export type DiagnosticContract = "columnia-diagnostic-report";
export type DiagnosticPhase = "load" | "review" | "prepare" | "deliver";
export type DiagnosticStatus = "ready" | "working" | "issue_reported" | "no_dataset";
export type DiagnosticErrorCode =
  | "DATASET_LOAD_FAILED"
  | "DATASET_PROFILE_FAILED"
  | "DATASET_REVIEW_FAILED"
  | "TRANSFORM_APPLY_FAILED"
  | "LOCAL_EXPORT_FAILED"
  | "DATABASE_EXPORT_FAILED"
  | "PROJECT_SAVE_FAILED"
  | "UNKNOWN_OPERATION_FAILED";

export type DiagnosticRowBucket =
  | "not_applicable"
  | "under_1k"
  | "1k_to_9k"
  | "10k_to_99k"
  | "100k_to_999k"
  | "1m_or_more";

export type DiagnosticColumnBucket =
  | "not_applicable"
  | "under_10"
  | "10_to_49"
  | "50_to_199"
  | "200_or_more";

export type DiagnosticSizeBucket =
  | "not_applicable"
  | "under_1_mib"
  | "1_to_99_mib"
  | "100_to_999_mib"
  | "1_gib_or_more";

export interface DiagnosticMetrics {
  datasetLoaded: boolean;
  rows: DiagnosticRowBucket;
  columns: DiagnosticColumnBucket;
  sourceSize: DiagnosticSizeBucket;
}

export interface DiagnosticReport {
  contract: DiagnosticContract;
  schemaVersion: 1;
  appVersion: string;
  phase: DiagnosticPhase;
  status: DiagnosticStatus;
  errorCodes: DiagnosticErrorCode[];
  metrics: DiagnosticMetrics | null;
}
