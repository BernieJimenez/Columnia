import { describe, expect, it } from "vitest";

import type { ColumnProfile, DatasetProfile } from "../../bridge";
import { actionLabel, buildColumnFixes, columnFixScopes } from "./columnFixes";
import { dateConversionCandidates } from "./prepareModel";

function column(overrides: Partial<ColumnProfile> & { name: string }): ColumnProfile {
  return {
    dataType: "String", nullCount: 0, completenessPercentage: 100, uniqueCount: 10, minimum: null, maximum: null,
    mean: null, emptyCount: 0, minimumLength: 1, maximumLength: 10, averageLength: 5, suggestedType: null,
    typeMatchPercentage: null, invalidTypeCount: null, sentinelCount: 0, encodingIssueCount: 0, privacySignal: null,
    standardDeviation: null, firstQuartile: null, median: null, thirdQuartile: null, outlierCount: null, histogram: null,
    dateOrder: null, dateHasTime: null, untrimmedCount: 0, ...overrides,
  };
}

describe("columnFixScopes", () => {
  it("«Desconocido» no toca números, fechas ni sí/no guardados como texto, que se ofrecen para convertir", () => {
    const profile: DatasetProfile = {
      rowCount: 100, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0,
      columns: [
        column({ name: "cliente", nullCount: 5, completenessPercentage: 95 }),
        column({ name: "importe", nullCount: 5, completenessPercentage: 95, suggestedType: "decimal", typeMatchPercentage: 97 }),
        column({ name: "alta", nullCount: 5, completenessPercentage: 95, suggestedType: "date", typeMatchPercentage: 100 }),
        column({ name: "activo", nullCount: 5, completenessPercentage: 95, suggestedType: "boolean", typeMatchPercentage: 100 }),
      ],
    };
    expect(columnFixScopes(profile).fillUnknown.map((item) => item.name)).toEqual(["cliente"]);
  });
});

function dataset(columns: ColumnProfile[], rowCount = 100): DatasetProfile {
  return { rowCount, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns };
}

describe("buildColumnFixes", () => {
  it("no recomienda convertir las fechas ambiguas, que el motor no convierte: lo dice y remite a la propuesta", () => {
    const profile = dataset([
      column({ name: "pago", suggestedType: "date", typeMatchPercentage: 100, dateOrder: "ambiguous" }),
      column({ name: "alta", suggestedType: "date", typeMatchPercentage: 100, dateOrder: "dmy" }),
    ]);
    const plan = buildColumnFixes(profile);
    expect(plan.scopes.parseDates.map((item) => item.name)).toEqual(["alta"]);
    expect(dateConversionCandidates(profile).map((item) => item.name)).toEqual(["alta"]);
    const pago = plan.rows.find((row) => row.column.name === "pago")!;
    expect(pago.recommended).toBeNull();
    expect(pago.alternatives).toEqual([]);
    expect(pago.group).toBe("types");
    expect(pago.issues).toEqual(["texto con fechas"]);
    expect(pago.notes).toEqual(["Sus fechas se leen de dos formas (día/mes o mes/día): elige el orden en la propuesta de arriba."]);
    const alta = plan.rows.find((row) => row.column.name === "alta")!;
    expect(alta.recommended).toEqual({ kind: "shared", id: "parseDates" });
    // One column, nothing left null: no confirmation, no «…».
    expect(actionLabel(alta.recommended!, plan.scopes)).toBe("Convertir «alta» a fecha");
  });

  it("una columna identificadora lleva una nota, no una recomendación; eliminarla queda en «Otras opciones»", () => {
    const plan = buildColumnFixes(dataset([column({ name: "customer_id", privacySignal: "identifier" })]));
    const [row] = plan.rows;
    expect(row?.recommended).toBeNull();
    expect(row?.notes).toEqual(["Parece un identificador: suele ser la clave de la tabla."]);
    expect(row?.alternatives).toEqual([{ kind: "shared", id: "removeIdentifier" }]);
    expect(row?.group).toBe("identifier");
    expect(actionLabel(row!.alternatives[0]!, plan.scopes)).toBe("Eliminar columna identificadora «customer_id»…");
  });

  it("ocultar un dato personal se recomienda diciendo por qué y sin presentarlo como obligatorio", () => {
    const plan = buildColumnFixes(dataset([column({ name: "correo", privacySignal: "email" })]));
    const [row] = plan.rows;
    expect(row?.recommended).toEqual({ kind: "shared", id: "maskPersonal" });
    expect(row?.notes).toEqual(["Ocultar sus valores protege la privacidad si vas a entregar los datos; no es obligatorio."]);
    expect(row?.alternatives.map((action) => actionLabel(action, plan.scopes))).toEqual(["Eliminar columna «correo»…"]);
  });

  it("ordena las filas por gravedad: primero lo que se elimina, al final los huecos y las claves", () => {
    const plan = buildColumnFixes(dataset([
      column({ name: "id_cliente", privacySignal: "identifier" }),
      column({ name: "ciudad", nullCount: 3, uniqueCount: 3, averageLength: 6 }),
      column({ name: "texto", encodingIssueCount: 2 }),
      column({ name: "importe", dataType: "Int64", outlierCount: 2, median: 10 }),
      column({ name: "correo", privacySignal: "email" }),
      column({ name: "cantidad", suggestedType: "integer", typeMatchPercentage: 95, invalidTypeCount: 5 }),
      column({ name: "estado", uniqueCount: 1 }),
      column({ name: "vacia", nullCount: 100, uniqueCount: 0 }),
    ]));
    expect(plan.rows.map((row) => [row.column.name, row.group])).toEqual([
      ["vacia", "removal"],
      ["estado", "removal"],
      ["cantidad", "types"],
      ["correo", "personal"],
      ["importe", "outliers"],
      ["texto", "text"],
      ["ciudad", "fill"],
      ["id_cliente", "identifier"],
    ]);
    // A conversion that would leave values null asks first: its button ends in «…».
    const cantidad = plan.rows.find((row) => row.column.name === "cantidad")!;
    expect(actionLabel(cantidad.recommended!, plan.scopes)).toBe("Convertir «cantidad» a número…");
  });

  it("una acción que la aplicación no ofrece cede el sitio a la siguiente", () => {
    const plan = buildColumnFixes(dataset([column({ name: "correo", privacySignal: "email" })]), (id) => id !== "maskPersonal");
    expect(plan.rows[0]?.recommended).toEqual({ kind: "shared", id: "removePersonal" });
    expect(plan.rows[0]?.notes).toEqual([]);
  });
});
