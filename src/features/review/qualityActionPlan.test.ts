import { describe, expect, it } from "vitest";

import type { ColumnProfile, DatasetProfile } from "../../bridge";
import type { ProposalItem } from "../prepare/proposalModel";
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

const imputeOf = (...columns: [string, number, number][]) => ({
  id: "impute",
  title: "",
  hint: "",
  examples: [],
  columns: columns.map(([name, missing, sentinels]) => ({ name, missing, sentinels })),
}) as ProposalItem;

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
    const [issue] = buildUncorrectedIssues(profile, imputeOf(["a", 4, 0], ["c", 5, 0]));
    expect(issue!.title).toBe("3 nulos en 1 columna que la propuesta no rellena");
    expect(issue!.detail).toContain("Son distintos de los 9 que la propuesta ofrece rellenar. ");
    // When the proposal fills every column with nulls, the block does not repeat them.
    expect(buildUncorrectedIssues(profile, imputeOf(["a", 4, 0], ["b", 3, 0], ["c", 5, 0]))).toEqual([]);
  });

  it("cuenta los marcadores que la propuesta convierte con la misma regla que su título", () => {
    const profile = profileOf([column("a", { nullCount: 300, sentinelCount: 2300 }), column("b", { nullCount: 3 })]);
    const [issue] = buildUncorrectedIssues(profile, imputeOf(["a", 300, 2300]));
    const n = (value: number) => value.toLocaleString("es-ES");
    expect(issue!.detail).toContain(`Son distintos de los ${n(2600)} que la propuesta ofrece rellenar (${n(2300)} de ellos son marcadores «sin dato»).`);
  });

  it("avisa de una columna que se rellena con la media o mediana contaminada por -999 o 9999", () => {
    const numeric = (name: string, minimum: string, maximum: string) => column(name, {
      dataType: "Int64", nullCount: 4, outlierCount: 0, minimum, maximum, median: 40,
    });
    const profile = profileOf([numeric("edad", "-999", "90"), numeric("precio", "1", "50"), numeric("visitas", "0", "9999")]);
    const issues = buildUncorrectedIssues(profile, imputeOf(["edad", 4, 0], ["precio", 4, 0], ["visitas", 4, 0]));
    expect(issues.map(({ id }) => id)).toEqual(["skewedNumbers"]);
    expect(issues[0]!.title).toBe("2 columnas con valores como -999 que el perfil cuenta como números: «edad», «visitas»");
    expect(issues[0]!.detail).toContain("La media y el relleno con la mediana pueden estar sesgados");
    expect(issues[0]!.action).toEqual({ kind: "prepare", label: "Ver en Preparar" });
    const [single] = buildUncorrectedIssues(profileOf([numeric("edad", "-999", "90")]), imputeOf(["edad", 4, 0]));
    expect(single!.title).toBe("«edad» tiene valores como -999 que el perfil cuenta como números");
    // Not offered for filling, or no suspicious extreme: nothing to warn about.
    expect(buildUncorrectedIssues(profileOf([numeric("edad", "-999", "90")]))
      .map(({ id }) => id)).toEqual(["nulls"]);
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
