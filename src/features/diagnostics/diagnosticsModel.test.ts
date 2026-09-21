import { describe, expect, it } from "vitest";

import {
  bucketDatasetMetrics,
  createDiagnosticReport,
  serializeDiagnosticReport,
} from "./diagnosticsModel";

describe("contrato de diagnóstico local v1", () => {
  it("solo expresa métricas en buckets acotados y se satura en el bucket superior", () => {
    expect(bucketDatasetMetrics(null)).toEqual({
      datasetLoaded: false,
      rows: "not_applicable",
      columns: "not_applicable",
      sourceSize: "not_applicable",
    });
    expect(bucketDatasetMetrics({ rowCount: 9_999, columnCount: 49, fileSizeBytes: 99 * 1024 ** 2 }))
      .toEqual({ datasetLoaded: true, rows: "1k_to_9k", columns: "10_to_49", sourceSize: "1_to_99_mib" });
    expect(bucketDatasetMetrics({ rowCount: 9_000_000_000, columnCount: 9_000_000, fileSizeBytes: 9_000_000_000 }))
      .toEqual({ datasetLoaded: true, rows: "1m_or_more", columns: "200_or_more", sourceSize: "1_gib_or_more" });
  });

  it("crea un preview tipado sin valores libres ni campos no declarados", () => {
    const report = createDiagnosticReport({
      appVersion: "0.168.0",
      phase: "prepare",
      status: "issue_reported",
      errorCodes: ["TRANSFORM_APPLY_FAILED"],
      metrics: bucketDatasetMetrics({ rowCount: 12_000, columnCount: 7, fileSizeBytes: 4_000_000 }),
    });
    const serialized = serializeDiagnosticReport(report);

    expect(report).toEqual({
      contract: "columnia-diagnostic-report",
      schemaVersion: 1,
      appVersion: "0.168.0",
      phase: "prepare",
      status: "issue_reported",
      errorCodes: ["TRANSFORM_APPLY_FAILED"],
      metrics: { datasetLoaded: true, rows: "10k_to_99k", columns: "under_10", sourceSize: "1_to_99_mib" },
    });
    expect(serialized).not.toMatch(/path|query|credential|stack|email|secret|machine|errorMessage|fileName|datasetName/i);
    expect(serialized).not.toContain("12,000");
  });

  it("limita códigos y exige coherencia entre el estado y los códigos seleccionados", () => {
    expect(() => createDiagnosticReport({
      appVersion: "0.168.0", phase: "review", status: "issue_reported", errorCodes: [], metrics: null,
    })).toThrow();
    expect(() => createDiagnosticReport({
      appVersion: "0.168.0", phase: "review", status: "ready", errorCodes: ["DATASET_LOAD_FAILED"], metrics: null,
    })).toThrow();
    expect(() => createDiagnosticReport({
      appVersion: "0.168.0",
      phase: "review",
      status: "issue_reported",
      errorCodes: [
        "DATASET_LOAD_FAILED", "DATASET_PROFILE_FAILED", "DATASET_REVIEW_FAILED",
        "TRANSFORM_APPLY_FAILED", "LOCAL_EXPORT_FAILED", "DATABASE_EXPORT_FAILED",
      ],
      metrics: null,
    })).toThrow("Selecciona como máximo cinco códigos.");
  });
});
