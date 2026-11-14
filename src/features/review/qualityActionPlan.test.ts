import { describe, expect, it } from "vitest";

import type { ColumnProfile, DatasetProfile } from "../../bridge";
import { buildUncorrectedIssues, qualityActionTargetDomId } from "./qualityActionPlan";

const column = (name: string, extra: Partial<ColumnProfile> = {}) => ({
  name,
  dataType: "String",
  nullCount: 0,
  invalidTypeCount: null,
  privacySignal: null,
  emptyCount: 0,
  ...extra,
}) as ColumnProfile;

const profileOf = (columns: ColumnProfile[], rowCount = 100) => ({
  rowCount,
  duplicateRowCount: 0,
  nearDuplicateRowCount: 0,
  duplicatePercentage: 0,
  columns,
}) as DatasetProfile;

describe("qualityActionPlan", () => {
  it("lista lo que la propuesta no arregla, con cifras de todas las filas y su acción", () => {
    const issues = buildUncorrectedIssues(profileOf([
      column("a", { nullCount: 4, invalidTypeCount: 3 }),
      column("b", { nullCount: 3 }),
      column("correo", { privacySignal: "email" }),
      column("precio", { commaDecimalCount: 100 }),
    ]));

    expect(issues.map(({ id }) => id)).toEqual(["nulls", "incompatible", "personal", "commaDecimal"]);
    expect(issues[0]!.title).toBe("7 nulos en 2 columnas que la propuesta no rellena");
    expect(issues[0]!.action).toEqual({ kind: "prepare", label: "Ver nulos en Preparar", target: "missingValues" });
    expect(issues[1]!.title).toBe("3 valores incompatibles con el tipo sugerido en 1 columna");
    expect(issues[1]!.action).toMatchObject({ kind: "prepare", target: "incompatibleTypes" });
    expect(issues[2]!.title).toBe("1 columna con datos personales: «correo»");
    expect(issues[2]!.detail).toContain("100 filas");
    expect(issues[3]!.title).toBe("100 celdas con coma decimal leídas como texto en «precio»");
    expect(issues[3]!.detail).toContain("«Decimal coma · miles punto»");
    expect(issues[3]!.action).toEqual({ kind: "load", label: "Volver a cargar con coma decimal" });
    expect(qualityActionTargetDomId("missingValues")).toBe("prepare-quality-missingValues");
  });

  it("no cuenta los nulos que la propuesta rellena: una sola cifra por hueco", () => {
    const profile = profileOf([column("a", { nullCount: 4 }), column("b", { nullCount: 3 }), column("c", { nullCount: 5 })]);
    const [issue] = buildUncorrectedIssues(profile, new Set(["a", "c"]));
    expect(issue!.title).toBe("3 nulos en 1 columna que la propuesta no rellena");
    expect(issue!.detail).toContain("Son distintos de los 9 que la propuesta ofrece rellenar");
    // When the proposal fills every column with nulls, the block does not repeat them.
    expect(buildUncorrectedIssues(profile, new Set(["a", "b", "c"]))).toEqual([]);
  });

  it("no cuenta la columna de auditoría ni los identificadores como datos personales", () => {
    expect(buildUncorrectedIssues(profileOf([
      column("_cambios", { nullCount: 50 }),
      column("cliente_id", { privacySignal: "identifier" }),
    ]))).toEqual([]);
  });

  it("no inventa nada cuando el perfil no detecta estas señales", () => {
    expect(buildUncorrectedIssues(profileOf([column("a")]))).toEqual([]);
  });
});
