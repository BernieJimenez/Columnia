import { describe, expect, it } from "vitest";

import {
  EMPTY_SQL_SERVER_CONNECTION,
  sqlServerConnection,
  sqlServerConnectionProblem,
} from "./sqlServerConnection";

const base = { ...EMPTY_SQL_SERVER_CONNECTION, driver: "ODBC Driver 18 for SQL Server", server: "localhost" };

describe("sqlServerConnection", () => {
  it("con Windows no envía usuario ni contraseña aunque se hayan escrito", () => {
    expect(sqlServerConnection({ ...base, user: "ana", password: "s3creta" })).toEqual(base);
  });

  it("con usuario de SQL Server envía los campos tal cual: Rust arma la cadena", () => {
    const fields = {
      ...base,
      server: "db.example.com,1433",
      authentication: "password" as const,
      user: "ana",
      password: "a;Encrypt=no",
      trustServerCertificate: true,
    };
    expect(sqlServerConnection(fields)).toBe(fields);
  });

  it("dice qué falta antes de intentar conectar", () => {
    expect(sqlServerConnectionProblem({ ...base, driver: "" })).toMatch(/ODBC Driver 18/);
    expect(sqlServerConnectionProblem({ ...base, server: "  " })).toBe("Indica el servidor.");
    expect(sqlServerConnectionProblem({ ...base, authentication: "password" })).toBe("Indica el usuario de SQL Server.");
    expect(sqlServerConnectionProblem(base)).toBeNull();
  });
});
