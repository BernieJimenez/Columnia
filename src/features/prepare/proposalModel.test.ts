import { describe, expect, it } from "vitest";
// Fixtures use the type names the engine really sends ("str", "i64").

import type { ColumnProfile, DatasetPreview, DatasetProfile } from "../../bridge";
import {
  applyLabel,
  buildPrepareProposal,
  defaultProposalSelection,
  proposalItemTitle,
  proposalOptions,
  selectedProposalCount,
} from "./proposalModel";

const column = (overrides: Partial<ColumnProfile>) =>
  ({
    nullCount: 0,
    uniqueCount: 4,
    sentinelCount: 0,
    median: null,
    dataType: "str",
    averageLength: 8,
    suggestedType: null,
    privacySignal: null,
    ...overrides,
  }) as ColumnProfile;

const dataset: DatasetPreview = {
  fileName: "clientes.csv",
  fileSizeBytes: 100,
  rowCount: 100,
  columnCount: 3,
  columns: [
    { name: "ciudad", dataType: "str" },
    { name: "monto", dataType: "i64" },
    { name: "_cambios", dataType: "str" },
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
    rowCount: 100,
    duplicateRowCount: 0,
    columns: [
      column({ name: "ciudad", nullCount: 1, uniqueCount: 2 }),
      column({ name: "monto", dataType: "i64", nullCount: 1, uniqueCount: 3, median: 30 }),
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
    expect(items[1].examples.map((example) => example.after)).toEqual([
      "valor más frecuente · 1 celda",
      "mediana (30) · 1 celda",
    ]);
  });

  it("adds sentinels and duplicates and skips text columns without a repeated value", () => {
    const items = buildPrepareProposal(
      profile({
        duplicateRowCount: 1,
        columns: [
          column({ name: "ciudad", nullCount: 1, uniqueCount: 99, sentinelCount: 2 }),
          column({ name: "monto", dataType: "i64", nullCount: 4, median: null }),
        ],
      }),
      dataset,
    );
    expect(items.map((item) => item.id)).toEqual(["sentinels", "trim", "duplicates"]);
    expect(items[0].title).toBe("Convertir 2 marcadores de «sin dato» en 1 columna");
    expect(items[2].title).toBe("Quitar 1 fila duplicada");
  });

  it("never fills identifiers, names, dates, free text or columns with many gaps (RV17)", () => {
    const items = buildPrepareProposal(
      profile({
        columns: [
          column({ name: "CustomerID", nullCount: 3, uniqueCount: 40 }),
          column({ name: "InvoiceNo", nullCount: 2, uniqueCount: 10 }),
          column({ name: "vm_id", dataType: "i64", nullCount: 2, uniqueCount: 50, median: 7 }),
          column({ name: "cliente", nullCount: 2, uniqueCount: 5, privacySignal: "name" }),
          column({ name: "alta", nullCount: 2, uniqueCount: 5, suggestedType: "date" }),
          column({ name: "comentario", nullCount: 2, uniqueCount: 60 }),
          column({ name: "nota", nullCount: 2, uniqueCount: 3, averageLength: 120 }),
          column({ name: "segmento", nullCount: 30, uniqueCount: 3 }),
          column({ name: "categoria", nullCount: 3, uniqueCount: 3, sentinelCount: 2 }),
          column({ name: "monto", dataType: "i64", nullCount: 2, uniqueCount: 80, median: 30 }),
        ],
      }),
      dataset,
    );
    const impute = items.find((item) => item.id === "impute");
    expect(impute?.columns?.map((entry) => entry.name)).toEqual(["categoria", "monto"]);
  });

  it("announces exactly the cells it will fill, following the selection (FUN-06)", () => {
    const items = buildPrepareProposal(
      profile({
        columns: [
          column({ name: "categoria", nullCount: 3, uniqueCount: 3, sentinelCount: 2 }),
          column({ name: "monto", dataType: "i64", nullCount: 2, uniqueCount: 80, median: 30 }),
        ],
      }),
      dataset,
    );
    const impute = items.find((item) => item.id === "impute")!;
    const selection = { ...defaultProposalSelection(items), impute: true };
    expect(proposalItemTitle(impute, selection)).toBe("Rellenar 7 valores vacíos en 2 columnas");
    expect(proposalItemTitle(impute, { ...selection, sentinels: false })).toBe(
      "Rellenar 5 valores vacíos en 2 columnas",
    );
  });
});

describe("proposal selection", () => {
  it("leaves imputation unchecked: inventing values is an explicit decision", () => {
    const items = buildPrepareProposal(profile({ duplicateRowCount: 2 }), dataset);
    expect(defaultProposalSelection(items)).toMatchObject({ trim: true, duplicates: true, impute: false });
  });

  it("maps the selection to one safe-corrections request with the announced columns", () => {
    const items = buildPrepareProposal(profile({ duplicateRowCount: 2 }), dataset);
    const selection = { ...defaultProposalSelection(items), trim: false, impute: true };
    expect(selectedProposalCount(items, selection)).toBe(2);
    expect(proposalOptions(items, selection, true)).toEqual({
      trimText: false,
      normalizeSentinels: false,
      normalizeColumnNames: true,
      removeDuplicates: true,
      imputeMissing: true,
      imputeColumns: ["ciudad", "monto"],
    });
    expect(proposalOptions(items, { ...selection, impute: false }).imputeColumns).toBeUndefined();
    expect(applyLabel(1)).toBe("Aplicar 1 cambio");
    expect(applyLabel(3)).toBe("Aplicar 3 cambios");
  });
});
