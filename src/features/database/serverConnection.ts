/**
 * The guided PostgreSQL and MySQL connection (2026-11-02), like SQL Server's:
 * a few fields instead of an ODBC connection string. The interface only sends
 * the fields; Rust checks them and builds the string.
 */
import type { DatabaseConnection } from "../../bridge";

export type ServerDatabaseKind = "postgresql" | "mysql";

export const EMPTY_SERVER_CONNECTION: DatabaseConnection = {
  driver: "",
  server: "",
  port: "",
  database: "",
  authentication: "password",
  user: "",
  password: "",
  trustServerCertificate: false,
  allowUnencrypted: false,
};

interface ServerEngine {
  label: string;
  port: string;
  driverToInstall: string;
  databaseHint: string;
}

export const SERVER_ENGINES: Record<ServerDatabaseKind, ServerEngine> = {
  postgresql: {
    label: "PostgreSQL",
    port: "5432",
    driverToInstall: "psqlODBC («PostgreSQL Unicode(x64)»)",
    databaseHint: "La del usuario si la dejas vacía",
  },
  mysql: {
    label: "MySQL",
    port: "3306",
    driverToInstall: "MySQL Connector/ODBC (el controlador Unicode)",
    databaseHint: "Vacía: todas las bases que el usuario puede leer",
  },
};

/** What is still missing, in words; null when the connection can be tried. */
export function serverConnectionProblem(kind: ServerDatabaseKind, fields: DatabaseConnection): string | null {
  const engine = SERVER_ENGINES[kind];
  if (!fields.driver) return `No hay un controlador ODBC de ${engine.label} instalado. Instala ${engine.driverToInstall}.`;
  if (!fields.server.trim()) return "Indica el servidor.";
  const port = fields.port.trim();
  if (port && (!/^\d{1,5}$/.test(port) || Number(port) < 1 || Number(port) > 65535)) {
    return "El puerto es un número entre 1 y 65535.";
  }
  if (!fields.user.trim()) return `Indica el usuario de ${engine.label}.`;
  return null;
}

