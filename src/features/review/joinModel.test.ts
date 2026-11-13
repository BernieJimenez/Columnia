import { describe, expect, it } from "vitest";

import { beginJoin, clearJoin, failJoin } from "./joinModel";

describe("joinModel", () => {
  it("representa el ciclo de una unión y conserva el mensaje de error", () => {
    expect(beginJoin("full")).toEqual({ kind: "loading", joinType: "full" });
    expect(failJoin("La clave no existe")).toEqual({
      kind: "error",
      message: "La clave no existe",
    });
    expect(clearJoin()).toEqual({ kind: "idle" });
  });
});

import type { DatasetComparison } from "../../bridge";
import { combinationResultMessage, consolidationEstimate, joinEstimate, joinTypeLabel } from "./joinModel";

describe("estimaciones de combinar con otro archivo", () => {
  const comparison = {
    currentRowCount: 10,
    comparedRowCount: 9,
    matchedKeyCount: 7,
    comparedOnlyKeyCount: 2,
    duplicateKeyCount: 0,
    keyColumns: ["id"],
    sharedColumns: ["id", "importe"],
    currentOnlyColumns: ["nota"],
    comparedOnlyColumns: ["zona"],
  } as DatasetComparison;

  it("cuenta filas y columnas de cada unión y de consolidar", () => {
    expect(joinEstimate(comparison, "inner")).toEqual({ rowCount: 7, columnCount: 5 });
    expect(joinEstimate(comparison, "left")).toEqual({ rowCount: 10, columnCount: 5 });
    expect(joinEstimate(comparison, "full")).toEqual({ rowCount: 12, columnCount: 5 });
    expect(joinEstimate({ ...comparison, duplicateKeyCount: 1 }, "inner").rowCount).toBeNull();
    expect(consolidationEstimate(comparison)).toEqual({ addedRowCount: 2, rowCount: 12, columnCount: 3 });
    expect(consolidationEstimate({ ...comparison, keyColumns: [] })).toEqual({ addedRowCount: 9, rowCount: 19, columnCount: 3 });
  });

  it("nombra la unión en español con el término SQL y redacta el resultado", () => {
    expect(joinTypeLabel("left")).toBe("Todas tus filas (left)");
    expect(combinationResultMessage("join", { rowCount: 1, columnCount: 3 }, "b.csv", "full"))
      .toBe("Unido con «b.csv», todas las filas de ambos (full): quedan 1 fila y 3 columnas.");
    expect(combinationResultMessage("consolidate", { rowCount: 4, columnCount: 1 }, null, "inner"))
      .toBe("Filas nuevas del otro archivo añadidas: quedan 4 filas y 1 columna.");
  });
});
