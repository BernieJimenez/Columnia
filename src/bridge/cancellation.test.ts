import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { isCancellationError, OPERATION_CANCELLED_MESSAGE } from "./cancellation";

describe("cancelación (COD-01)", () => {
  it("usa el mismo texto que el motor de Rust", () => {
    const rust = readFileSync(resolve(process.cwd(), "src-tauri/src/dataset.rs"), "utf8");
    const constant = rust.match(/const OPERATION_CANCELLED_MESSAGE: &str = "([^"]+)";/);
    expect(constant?.[1]).toBe(OPERATION_CANCELLED_MESSAGE);
  });

  it("reconoce la cancelación en errores y textos, y nada más", () => {
    expect(isCancellationError(new Error(OPERATION_CANCELLED_MESSAGE))).toBe(true);
    expect(isCancellationError(`Fallo: ${OPERATION_CANCELLED_MESSAGE}`)).toBe(true);
    expect(isCancellationError(new Error("El archivo no existe."))).toBe(false);
  });
});
