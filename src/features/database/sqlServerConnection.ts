/**
 * The guided SQL Server connection (2026-10-31): a few fields instead of an
 * ODBC connection string. The interface only sends the fields; Rust checks
 * them and builds the string, so no field can add or repeat an attribute.
 */
import type { DatabaseConnection } from "../../bridge";

export const EMPTY_SQL_SERVER_CONNECTION: DatabaseConnection = {
  driver: "",
  server: "",
  port: "",
  database: "",
  authentication: "windows",
  user: "",
  password: "",
  trustServerCertificate: false,
  allowUnencrypted: false,
};

/** What is still missing, in words; null when the connection can be tried. */
export function sqlServerConnectionProblem(fields: DatabaseConnection): string | null {
  if (!fields.driver) return "No hay un controlador ODBC de SQL Server instalado. Instala «ODBC Driver 18 for SQL Server» de Microsoft.";
  if (!fields.server.trim()) return "Indica el servidor.";
  if (fields.authentication === "password" && !fields.user.trim()) return "Indica el usuario de SQL Server.";
  return null;
}

/** The fields to send: with Windows sign-in there is no user nor password. */
export function sqlServerConnection(fields: DatabaseConnection): DatabaseConnection {
  return fields.authentication === "windows" ? { ...fields, user: "", password: "" } : fields;
}
