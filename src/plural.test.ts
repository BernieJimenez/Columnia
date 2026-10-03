import { describe, expect, it } from "vitest";

import { plural } from "./plural";

describe("plural", () => {
  it("usa el singular solo para 1", () => {
    expect(plural(0, "fila", "filas")).toBe("0 filas");
    expect(plural(1, "fila", "filas")).toBe("1 fila");
    expect(plural(2, "fila", "filas")).toBe("2 filas");
  });
});
