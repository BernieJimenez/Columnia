import { describe, expect, it } from "vitest";

import {
  beginComparison,
  clearComparison,
  completeComparison,
  failComparison,
} from "./compareModel";

const comparison = {
  currentFileName: "a.csv",
  comparedFileName: "b.csv",
  currentRowCount: 2,
  comparedRowCount: 2,
  commonRowCount: 1,
  currentOnlyRowCount: 1,
  comparedOnlyRowCount: 1,
  sharedColumns: ["id"],
  currentOnlyColumns: [],
  comparedOnlyColumns: [],
  schemaCompatible: true,
  keyColumns: ["id"],
  matchedKeyCount: 1,
  currentOnlyKeyCount: 0,
  comparedOnlyKeyCount: 0,
  conflictingKeyCount: 0,
  duplicateKeyCount: 0,
  canConsolidate: true,
};

describe("compareModel", () => {
  it("expone estados de ciclo y conserva el resultado", () => {
    expect(beginComparison()).toEqual({ kind: "loading" });
    expect(completeComparison(comparison)).toEqual({ kind: "ready", comparison });
    expect(failComparison("falló")).toEqual({ kind: "error", message: "falló" });
    expect(clearComparison()).toEqual({ kind: "idle" });
  });
});
