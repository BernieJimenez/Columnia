import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import {
  BridgeError,
  errorMessage,
  isUncertainCommitError,
  toBridgeError,
  UNCERTAIN_COMMIT_MESSAGE,
} from "./errors";
import { isCancellationError, OPERATION_CANCELLED_MESSAGE } from "./cancellation";
import { LEGACY_ENCODING_PREFIX, legacyEncodingExample } from "../features/load/loadModel";

describe("errores del puente (PROD-08)", () => {
  it("reconoce cada caso especial por su código, venga como Error o como texto", () => {
    expect(toBridgeError(OPERATION_CANCELLED_MESSAGE).code).toBe("cancelled");
    expect(toBridgeError(new Error(`La carga se detuvo: ${OPERATION_CANCELLED_MESSAGE}`)).isCancellation()).toBe(true);
    const legacy = toBridgeError(`${LEGACY_ENCODING_PREFIX}Población`);
    expect(legacy.code).toBe("legacyEncoding");
    expect(legacy.example).toBe("Población");
    const other = toBridgeError({ toString: () => "disco lleno" });
    expect(other).toBeInstanceOf(BridgeError);
    expect(other.code).toBe("other");
    expect(other.message).toBe("disco lleno");
  });

  it("reconoce el commit incierto y comparte su texto con Rust (AUD-030)", () => {
    const rust = readFileSync(resolve(process.cwd(), "src-tauri/src/remote_databases.rs"), "utf8");
    const declared = /const UNCERTAIN_COMMIT_MESSAGE: &str = "([^"]+)";/.exec(rust)?.[1];
    expect(declared).toBe(UNCERTAIN_COMMIT_MESSAGE);
    expect(toBridgeError(new Error(UNCERTAIN_COMMIT_MESSAGE)).code).toBe("uncertainCommit");
    expect(isUncertainCommitError(`Fallo: ${UNCERTAIN_COMMIT_MESSAGE}`)).toBe(true);
    expect(isUncertainCommitError("No se pudo conectar.")).toBe(false);
  });

  it("los ayudantes de siempre leen el mismo código", () => {
    expect(isCancellationError(OPERATION_CANCELLED_MESSAGE)).toBe(true);
    expect(isCancellationError("otro fallo")).toBe(false);
    expect(legacyEncodingExample(`${LEGACY_ENCODING_PREFIX}Año`)).toBe("Año");
    expect(legacyEncodingExample("otro fallo")).toBeNull();
    expect(errorMessage(new Error("sin acceso"))).toBe("sin acceso");
    expect(errorMessage("sin acceso")).toBe("sin acceso");
  });
});
