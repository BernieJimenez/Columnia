import { describe, expect, it } from "vitest";

import { userErrorMessage } from "./errorText";

describe("userErrorMessage (TXT-02)", () => {
  it("oculta la ruta y conserva el motivo", () => {
    expect(userErrorMessage(new Error("Error: no se pudo eliminar C:\\a\\b: acceso denegado")))
      .toBe("Error: no se pudo eliminar una ruta local: acceso denegado");
  });

  it("oculta rutas con espacios, UNC y POSIX", () => {
    expect(userErrorMessage("No se pudo leer C:\\Mis datos\\ventas 2026.csv")).toBe("No se pudo leer una ruta local");
    expect(userErrorMessage("Falta \\\\servidor\\compartido\\datos.csv: sin acceso")).toBe("Falta una ruta local: sin acceso");
    expect(userErrorMessage("No existe /home/ana/datos.csv: borrado")).toBe("No existe una ruta local: borrado");
  });

  it("da un mensaje genérico si no queda texto", () => {
    expect(userErrorMessage("")).toBe("No se pudo completar la operación.");
  });
});
