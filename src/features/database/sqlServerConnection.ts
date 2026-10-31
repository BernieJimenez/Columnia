/**
 * The guided SQL Server connection (2026-10-31): a few fields instead of an
 * ODBC connection string. The string is built here and lives only in memory.
 */

type SqlServerAuthentication = "windows" | "sql";

export interface SqlServerConnectionFields {
  driver: string;
  server: string;
  database: string;
  authentication: SqlServerAuthentication;
  user: string;
  password: string;
  /** Accept a self-signed certificate: for own or test servers only. */
  trustServerCertificate: boolean;
}

export const EMPTY_SQL_SERVER_CONNECTION: SqlServerConnectionFields = {
  driver: "",
  server: "",
  database: "",
  authentication: "windows",
  user: "",
  password: "",
  trustServerCertificate: false,
};

/**
 * An ODBC attribute value. A value with `;`, braces or spaces at either end is
 * braced, with `}` doubled, so a password can never add attributes.
 */
export function odbcValue(value: string): string {
  return /[;{}=]|^\s|\s$/.test(value) ? `{${value.replace(/\}/g, "}}")}}` : value;
}

/** What is still missing, in words; null when the connection can be tried. */
export function sqlServerConnectionProblem(fields: SqlServerConnectionFields): string | null {
  if (!fields.driver) return "No hay un controlador ODBC de SQL Server instalado. Instala «ODBC Driver 18 for SQL Server» de Microsoft.";
  if (!fields.server.trim()) return "Indica el servidor.";
  if (fields.authentication === "sql" && !fields.user.trim()) return "Indica el usuario de SQL Server.";
  return null;
}

/** The ODBC connection string for these fields; always asks for encryption. */
export function buildSqlServerConnectionString(fields: SqlServerConnectionFields): string {
  const parts = [
    `Driver={${fields.driver.replace(/\}/g, "}}")}}`,
    `Server=${odbcValue(fields.server.trim())}`,
  ];
  if (fields.database.trim()) parts.push(`Database=${odbcValue(fields.database.trim())}`);
  if (fields.authentication === "windows") {
    parts.push("Trusted_Connection=yes");
  } else {
    parts.push(`Uid=${odbcValue(fields.user.trim())}`, `Pwd=${odbcValue(fields.password)}`);
  }
  parts.push("Encrypt=yes");
  if (fields.trustServerCertificate) parts.push("TrustServerCertificate=yes");
  return `${parts.join(";")};`;
}
