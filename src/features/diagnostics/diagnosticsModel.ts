import type {
  DiagnosticErrorCode,
  DiagnosticMetrics,
  DiagnosticPhase,
  DiagnosticReport,
  DiagnosticStatus,
} from "../../bridge/diagnostics-contracts";

export type {
  DiagnosticColumnBucket,
  DiagnosticErrorCode,
  DiagnosticMetrics,
  DiagnosticPhase,
  DiagnosticReport,
  DiagnosticRowBucket,
  DiagnosticSizeBucket,
  DiagnosticStatus,
} from "../../bridge/diagnostics-contracts";

export const DIAGNOSTIC_PHASES: readonly DiagnosticPhase[] = ["load", "review", "prepare", "deliver"];
export const DIAGNOSTIC_STATUSES: readonly DiagnosticStatus[] = ["ready", "working", "issue_reported", "no_dataset"];
export const DIAGNOSTIC_ERROR_CODES: readonly DiagnosticErrorCode[] = [
  "DATASET_LOAD_FAILED",
  "DATASET_PROFILE_FAILED",
  "DATASET_REVIEW_FAILED",
  "TRANSFORM_APPLY_FAILED",
  "LOCAL_EXPORT_FAILED",
  "DATABASE_EXPORT_FAILED",
  "PROJECT_SAVE_FAILED",
  "UNKNOWN_OPERATION_FAILED",
];

export interface DatasetMetricInput {
  rowCount: number;
  columnCount: number;
  fileSizeBytes: number;
}

const MIB = 1024 ** 2;
const GIB = 1024 ** 3;

function nonnegativeFinite(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

export function bucketDatasetMetrics(dataset: DatasetMetricInput | null): DiagnosticMetrics {
  if (!dataset) {
    return {
      datasetLoaded: false,
      rows: "not_applicable",
      columns: "not_applicable",
      sourceSize: "not_applicable",
    };
  }

  const rows = nonnegativeFinite(dataset.rowCount);
  const columns = nonnegativeFinite(dataset.columnCount);
  const bytes = nonnegativeFinite(dataset.fileSizeBytes);

  return {
    datasetLoaded: true,
    rows: rows < 1_000
      ? "under_1k"
      : rows < 10_000
        ? "1k_to_9k"
        : rows < 100_000
          ? "10k_to_99k"
          : rows < 1_000_000
            ? "100k_to_999k"
            : "1m_or_more",
    columns: columns < 10
      ? "under_10"
      : columns < 50
        ? "10_to_49"
        : columns < 200
          ? "50_to_199"
          : "200_or_more",
    sourceSize: bytes < MIB
      ? "under_1_mib"
      : bytes < 100 * MIB
        ? "1_to_99_mib"
        : bytes < GIB
          ? "100_to_999_mib"
          : "1_gib_or_more",
  };
}

export interface DiagnosticReportInput {
  appVersion: string;
  phase: DiagnosticPhase;
  status: DiagnosticStatus;
  errorCodes: DiagnosticErrorCode[];
  metrics: DiagnosticMetrics | null;
}

export function createDiagnosticReport(input: DiagnosticReportInput): DiagnosticReport {
  const errorCodes = [...new Set(input.errorCodes)].sort();
  if (errorCodes.length > 5) {
    throw new Error("Selecciona como máximo cinco códigos.");
  }
  if ((input.status === "issue_reported") !== (errorCodes.length > 0)) {
    throw new Error("El estado y los códigos seleccionados deben coincidir.");
  }

  return {
    contract: "columnia-diagnostic-report",
    schemaVersion: 1,
    appVersion: input.appVersion,
    phase: input.phase,
    status: input.status,
    errorCodes,
    metrics: input.metrics,
  };
}

export function serializeDiagnosticReport(report: DiagnosticReport): string {
  return JSON.stringify(report, null, 2);
}
