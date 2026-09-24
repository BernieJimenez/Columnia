import { describe, expect, it } from "vitest";

import type { ColumnProfile, DatasetPreview, DatasetProfile } from "../../bridge";
import {
  applyLabel,
  buildPrepareProposal,
  defaultProposalSelection,
  proposalOptions,
  selectedProposalCount,
} from "./proposalModel";

const column = (overrides: Partial<ColumnProfile>) =>
  ({ nullCount: 0, uniqueCount: 4, sentinelCount: 0, median: null, dataType: "String", ...overrides }) as ColumnProfile;

const dataset: DatasetPreview = {
  fileName: "clientes.csv",
  fileSizeBytes: 100,
  rowCount: 4,
  columnCount: 3,
  columns: [
    { name: "ciudad", dataType: "String" },
    { name: "monto", dataType: "Int64" },
    { name: "_cambios", dataType: "String" },
  ],
  rows: [
    [" Santiago ", "10", "x"],
    ["Santiago", null, " y "],
    [null, "30", null],
    ["La Vega", "40", null],
  ],
};

const profile = (overrides: Partial<DatasetProfile> = {}): DatasetProfile =>
  ({
    rowCount: 4,
    duplicateRowCount: 0,
    columns: [
      column({ name: "ciudad", nullCount: 1, uniqueCount: 2 }),
      column({ name: "monto", dataType: "Int64", nullCount: 1, uniqueCount: 3, median: 30 }),
      column({ name: "_cambios", nullCount: 2, uniqueCount: 2, sentinelCount: 5 }),
    ],
    ...overrides,
  }) as DatasetProfile;

describe("buildPrepareProposal", () => {
  it("proposes trimming with real examples and imputation per column", () => {
    const items = buildPrepareProposal(profile(), dataset);
    expect(items.map((item) => item.id)).toEqual(["trim", "impute"]);
    expect(items[0].examples).toEqual([{ column: "ciudad", before: " Santiago ", after: "Santiago" }]);
    expect(items[1].title).toBe("Rellenar 2 valores vacíos en 2 columnas");
    expect(items[1].examples.map((example) => example.after)).toEqual(["valor más frecuente", "mediana (30)"]);
  });

  it("adds sentinels and duplicates and skips text columns without a repeated value", () => {
    const items = buildPrepareProposal(
      profile({
        duplicateRowCount: 1,
        columns: [
          column({ name: "ciudad", nullCount: 1, uniqueCount: 3, sentinelCount: 2 }),
          column({ name: "monto", dataType: "Int64", nullCount: 4, median: null }),
        ],
      }),
      dataset,
    );
    expect(items.map((item) => item.id)).toEqual(["sentinels", "trim", "duplicates"]);
    expect(items[0].title).toBe("Convertir 2 marcadores de «sin dato» en 1 columna");
    expect(items[2].title).toBe("Quitar 1 fila duplicada");
  });

  it("maps the selection to one safe-corrections request", () => {
    const items = buildPrepareProposal(profile({ duplicateRowCount: 2 }), dataset);
    const selection = { ...defaultProposalSelection(items), trim: false };
    expect(selectedProposalCount(items, selection)).toBe(2);
    expect(proposalOptions(items, selection, true)).toEqual({
      trimText: false,
      normalizeSentinels: false,
      normalizeColumnNames: true,
      removeDuplicates: true,
      imputeMissing: true,
    });
    expect(applyLabel(1)).toBe("Aplicar 1 cambio");
    expect(applyLabel(3)).toBe("Aplicar 3 cambios");
  });
});
