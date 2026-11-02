import { describe, expect, it } from "vitest";

import { buildServerConnectionString, EMPTY_SERVER_CONNECTION, serverConnectionProblem } from "./serverConnection";

const mysql = { ...EMPTY_SERVER_CONNECTION, driver: "MySQL ODBC 9.1 Unicode Driver", server: "localhost", user: "root" };

describe("serverConnection", () => {
  it("MySQL pide cifrado salvo que se permita conectar sin él", () => {
    expect(buildServerConnectionString("mysql", { ...mysql, password: "x" })).toBe(
      "Driver={MySQL ODBC 9.1 Unicode Driver};Server=localhost;Uid=root;Pwd=x;SSLMODE=REQUIRED;",
    );
    expect(buildServerConnectionString("mysql", { ...mysql, database: "tienda", allowUnencrypted: true })).toBe(
      "Driver={MySQL ODBC 9.1 Unicode Driver};Server=localhost;Database=tienda;Uid=root;Pwd=;SSLMODE=PREFERRED;",
    );
  });

  it("PostgreSQL usa el puerto indicado y una contraseña con ; o llaves no añade atributos", () => {
    expect(buildServerConnectionString("postgresql", {
      ...mysql,
      driver: "PostgreSQL Unicode(x64)",
      port: " 5433 ",
      password: "a}b;Driver=x",
      allowUnencrypted: true,
    })).toBe("Driver={PostgreSQL Unicode(x64)};Server=localhost;Port=5433;Uid=root;Pwd={a}}b;Driver=x};SSLmode=prefer;");
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
