import { describe, expect, it } from "vitest";
// Fixtures use the type names the engine really sends ("str", "i64").

import type { ColumnProfile, DatasetPreview, DatasetProfile } from "../../bridge";
import {
  applyLabel,
  buildPrepareProposal,
  looksLikeIdentifier,
  dateExample,
  dateExamples,
  defaultProposalSelection,
  imputationExamples,
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
          // QA-33: Spanish identifier names too.
          column({ name: "id_cliente", nullCount: 2, uniqueCount: 5 }),
          column({ name: "codigo_postal", nullCount: 2, uniqueCount: 5 }),
          column({ name: "num_factura", dataType: "i64", nullCount: 2, uniqueCount: 50, median: 7 }),
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

  it("uses the engine's simulation of the whole chain when there is one (RV17)", () => {
    const items = buildPrepareProposal(
      profile({
        duplicateRowCount: 1,
        columns: [
          column({ name: "categoria", nullCount: 3, uniqueCount: 3, sentinelCount: 2 }),
          column({ name: "monto", dataType: "i64", nullCount: 2, uniqueCount: 80, median: 30 }),
        ],
      }),
      dataset,
    );
    const impute = items.find((item) => item.id === "impute")!;
    const selection = { ...defaultProposalSelection(items), impute: true };
    // Removing duplicates took two gaps away: the profile estimate says 7.
    const preview = {
      removedRowCount: 1,
      imputedCellCount: 5,
      imputations: [
        { column: "categoria", value: "A", cellCount: 4 },
        { column: "monto", value: "30", cellCount: 1 },
      ],
    };

    expect(proposalItemTitle(impute, selection, preview)).toBe("Rellenar 5 valores vacíos en 2 columnas");
    expect(imputationExamples(preview)).toEqual([
      { column: "categoria", before: null, after: "A · 4 celdas" },
      { column: "monto", before: null, after: "30 · 1 celda" },
    ]);
    expect(proposalItemTitle(impute, selection, { removedRowCount: 0, imputedCellCount: 0, imputations: [] })).toBe(
      "Rellenar valores vacíos: no queda ninguno tras los demás cambios",
    );
  });
});

describe("numeric typing (RV18 / FUN-07)", () => {
  const typedProfile = profile({
    columns: [
      column({ name: "Quantity", suggestedType: "integer", invalidTypeCount: 0 }),
      column({ name: "UnitPrice", suggestedType: "decimal", invalidTypeCount: 0 }),
      column({ name: "InvoiceNo", suggestedType: "integer", invalidTypeCount: 9291 }),
      column({ name: "CustomerID", suggestedType: "integer", invalidTypeCount: 0 }),
      column({ name: "telefono", suggestedType: "integer", invalidTypeCount: 0, privacySignal: "phone" }),
      column({ name: "cantidad", dataType: "i64", suggestedType: null }),
    ],
  });

  it("proposes typing only fully numeric text columns that are not keys or personal data", () => {
    const items = buildPrepareProposal(typedProfile, dataset);
    const types = items.find((item) => item.id === "types");
    expect(types?.title).toBe("Convertir 2 columnas a número");
    expect(types?.columns?.map((entry) => entry.name)).toEqual(["Quantity", "UnitPrice"]);
    expect(types?.examples.map((example) => `${example.column}: ${example.after}`)).toEqual([
      "Quantity: Entero",
      "UnitPrice: Decimal",
    ]);
  });

  it("comes checked, because no value changes, and sends the exact columns", () => {
    const items = buildPrepareProposal(typedProfile, dataset);
    const selection = defaultProposalSelection(items);
    expect(selection.types).toBe(true);
    expect(proposalOptions(items, selection).castColumns).toEqual(["Quantity", "UnitPrice"]);
    expect(proposalOptions(items, { ...selection, types: false }).castColumns).toBeUndefined();
  });

  it("types a column whose only non-numbers are «sin dato» markers, while they are converted", () => {
    const withMarkers = profile({
      columns: [
        column({ name: "lat1", suggestedType: "decimal", invalidTypeCount: 0, sentinelCount: 1200 }),
        column({ name: "precio", suggestedType: "integer", invalidTypeCount: 0 }),
      ],
    });
    const items = buildPrepareProposal(withMarkers, dataset);
    const types = items.find((item) => item.id === "types")!;
    const selection = defaultProposalSelection(items);
    expect(selection.sentinels).toBe(true);
    expect(proposalItemTitle(types, selection)).toBe("Convertir 2 columnas a número");
    expect(proposalOptions(items, selection).castColumns).toEqual(["lat1", "precio"]);

    // Without converting the markers, «NA» stays text and the column cannot type.
    const keepMarkers = { ...selection, sentinels: false };
    expect(proposalItemTitle(types, keepMarkers)).toBe("Convertir 1 columna a número");
    expect(proposalOptions(items, keepMarkers).castColumns).toEqual(["precio"]);
  });

  it("does not count the numeric change when every column waits on the markers", () => {
    const onlyMarkers = profile({
      columns: [column({ name: "lat1", suggestedType: "decimal", invalidTypeCount: 0, sentinelCount: 10 })],
    });
    const items = buildPrepareProposal(onlyMarkers, dataset);
    const types = items.find((item) => item.id === "types")!;
    const keepMarkers = { ...defaultProposalSelection(items), sentinels: false };
    expect(proposalItemTitle(types, keepMarkers)).toBe("Convertir a número: requiere convertir los marcadores «sin dato»");
    expect(selectedProposalCount(items, keepMarkers)).toBe(selectedProposalCount(items, { ...keepMarkers, types: false }));
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

describe("date typing (RV18)", () => {
  const dates: DatasetPreview = {
    ...dataset,
    columns: [
      { name: "InvoiceDate", dataType: "str" },
      { name: "alta", dataType: "str" },
      { name: "nacimiento", dataType: "str" },
    ],
    rows: [[null, "01/02/2024", "03/04/1990"], ["12/1/2010 8:26", "05/06/2024", "07/08/1991"]],
  };
  const datesProfile = profile({
    columns: [
      column({ name: "InvoiceDate", dateOrder: "mdy", dateHasTime: true }),
      column({ name: "alta", dateOrder: "ambiguous" }),
      column({ name: "nacimiento", dateOrder: "dmy", privacySignal: "name" }),
      column({ name: "notas", dateOrder: null }),
    ],
  });

  it("proposes the date columns, checked when their order is known, and never personal data", () => {
    const items = buildPrepareProposal(datesProfile, dates);
    const item = items.find((entry) => entry.id === "dates")!;

    expect(item.title).toBe("Convertir 2 columnas a fecha");
    expect(item.dateColumns).toEqual([
      { name: "InvoiceDate", order: "mdy", sample: "12/1/2010 8:26" },
      { name: "alta", order: null, sample: "01/02/2024" },
    ]);
    expect(defaultProposalSelection(items).dates).toBe(true);
    expect(proposalOptions(items, defaultProposalSelection(items)).dateColumns).toEqual([
      { column: "InvoiceDate", order: "mdy" },
    ]);
  });

  it("sends ambiguous columns only once the person says how to read them", () => {
    const items = buildPrepareProposal(datesProfile, dates);
    const selection = defaultProposalSelection(items);
    const item = items.find((entry) => entry.id === "dates")!;

    expect(proposalOptions(items, selection, false, "dmy").dateColumns).toEqual([
      { column: "InvoiceDate", order: "mdy" },
      { column: "alta", order: "dmy" },
    ]);
    expect(dateExamples(item, null)).toEqual([
      { column: "InvoiceDate", before: "12/1/2010 8:26", after: "2010-12-01 08:26" },
    ]);
    expect(dateExamples(item, "mdy")[1]).toEqual({ column: "alta", before: "01/02/2024", after: "2024-01-02" });
  });

  it("leaves the item unchecked when every date column is ambiguous", () => {
    const onlyAmbiguous = profile({ columns: [column({ name: "alta", dateOrder: "ambiguous" })] });
    const items = buildPrepareProposal(onlyAmbiguous, dates);

    expect(defaultProposalSelection(items).dates).toBe(false);
  });

  it("shows each order as an unambiguous date", () => {
    expect(dateExample("01/02/2024", "dmy")).toBe("2024-02-01");
    expect(dateExample("01/02/2024", "mdy")).toBe("2024-01-02");
    expect(dateExample("2024-12-25T10:30:00", "iso")).toBe("2024-12-25 10:30");
    expect(dateExample("12/1/10 8:26", "mdy")).toBe("2010-12-01 08:26");
    expect(dateExample("1/2/45", "dmy")).toBe("1945-02-01");
    expect(dateExample("pendiente", "dmy")).toBeNull();
  });
});

describe("trim count (UX-01)", () => {
  it("says how many cells trimming changes and leaves the item out when none would", () => {
    const withSpaces = profile({
      columns: [
        column({ name: "ciudad", untrimmedCount: 1204 }),
        column({ name: "estado", untrimmedCount: 1 }),
      ],
    });
    const trim = buildPrepareProposal(withSpaces, dataset).find((item) => item.id === "trim");
    expect(trim?.title).toBe(`Recortar espacios en ${(1205).toLocaleString()} celdas`);

    const clean = profile({ columns: [column({ name: "ciudad", untrimmedCount: 0 })] });
    expect(buildPrepareProposal(clean, dataset).some((item) => item.id === "trim")).toBe(false);
  });

  it("keeps the generic item for profiles saved before the count existed", () => {
    const older = profile({ columns: [column({ name: "ciudad", untrimmedCount: null })] });
    expect(buildPrepareProposal(older, dataset).find((item) => item.id === "trim")?.title)
      .toBe("Recortar espacios al inicio y al final del texto");
  });
});

describe("identifier names (FUN-22)", () => {
  it("recognises the key word first or last, in Spanish and English order", () => {
    for (const name of ["id_cliente", "codigo_postal", "num_factura", "ref_pedido", "CustomerID", "vm_id", "InvoiceNo", "StockCode"]) {
      expect(looksLikeIdentifier(name), name).toBe(true);
    }
    for (const name of ["importe", "no_contesta", "numero_de_hijos", "cantidad"]) {
      expect(looksLikeIdentifier(name), name).toBe(false);
    }
  });
});
