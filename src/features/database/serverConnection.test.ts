import { describe, expect, it } from "vitest";

import { EMPTY_SERVER_CONNECTION, serverConnectionProblem } from "./serverConnection";

const mysql = { ...EMPTY_SERVER_CONNECTION, driver: "MySQL ODBC 9.1 Unicode Driver", server: "localhost", user: "root" };

describe("serverConnection", () => {
  it("empieza con usuario y contraseña, cifrado exigido y el puerto habitual", () => {
    expect(EMPTY_SERVER_CONNECTION).toMatchObject({ authentication: "password", port: "", allowUnencrypted: false });
  });

  it("dice qué falta: controlador, servidor, puerto válido y usuario", () => {
    expect(serverConnectionProblem("postgresql", { ...mysql, driver: "" })).toMatch(/Instala psqlODBC/);
    expect(serverConnectionProblem("mysql", { ...mysql, server: " " })).toBe("Indica el servidor.");
    expect(serverConnectionProblem("mysql", { ...mysql, port: "0" })).toBe("El puerto es un número entre 1 y 65535.");
    expect(serverConnectionProblem("mysql", { ...mysql, port: "33a" })).toBe("El puerto es un número entre 1 y 65535.");
    expect(serverConnectionProblem("mysql", { ...mysql, user: "" })).toBe("Indica el usuario de MySQL.");
    expect(serverConnectionProblem("mysql", { ...mysql, port: "3307" })).toBeNull();
  });
});
