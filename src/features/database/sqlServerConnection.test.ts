import { describe, expect, it } from "vitest";

import {
  buildSqlServerConnectionString,
  EMPTY_SQL_SERVER_CONNECTION,
  odbcValue,
  sqlServerConnectionProblem,
} from "./sqlServerConnection";

const base = { ...EMPTY_SQL_SERVER_CONNECTION, driver: "ODBC Driver 18 for SQL Server", server: "localhost" };

describe("sqlServerConnection", () => {
  it("con Windows usa la cuenta actual y siempre pide cifrado", () => {
    expect(buildSqlServerConnectionString({ ...base, database: "VentasRetail" })).toBe(
      "Driver={ODBC Driver 18 for SQL Server};Server=localhost;Database=VentasRetail;Trusted_Connection=yes;Encrypt=yes;",
    );
  });

  it("con usuario de SQL Server incluye usuario y contraseña, y el certificado solo si se acepta", () => {
    expect(buildSqlServerConnectionString({
      ...base,
      server: "db.example.com,1433",
      authentication: "sql",
      user: "ana",
      password: "s3creta",
      trustServerCertificate: true,
    })).toBe("Driver={ODBC Driver 18 for SQL Server};Server=db.example.com,1433;Uid=ana;Pwd=s3creta;Encrypt=yes;TrustServerCertificate=yes;");
  });

  it("una contraseña con ; o llaves no puede añadir atributos", () => {
    expect(odbcValue("a;Encrypt=no")).toBe("{a;Encrypt=no}");
    expect(odbcValue("x}y")).toBe("{x}}y}");
    expect(odbcValue(" espacio")).toBe("{ espacio}");
    expect(odbcValue("normal")).toBe("normal");
    const built = buildSqlServerConnectionString({ ...base, authentication: "sql", user: "ana", password: "a;Encrypt=no" });
    expect(built).toContain("Pwd={a;Encrypt=no};Encrypt=yes;");
  });

  it("dice qué falta antes de intentar conectar", () => {
    expect(sqlServerConnectionProblem({ ...base, driver: "" })).toMatch(/ODBC Driver 18/);
    expect(sqlServerConnectionProblem({ ...base, server: "  " })).toBe("Indica el servidor.");
    expect(sqlServerConnectionProblem({ ...base, authentication: "sql" })).toBe("Indica el usuario de SQL Server.");
    expect(sqlServerConnectionProblem(base)).toBeNull();
  });
});
