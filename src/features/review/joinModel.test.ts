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
