/**
 * The guided PostgreSQL and MySQL connection (2026-11-02), like SQL Server's:
 * a few fields instead of an ODBC connection string. The string is built here
 * and lives only in memory.
 */
import { odbcValue } from "./sqlServerConnection";

export type ServerDatabaseKind = "postgresql" | "mysql";

export interface ServerConnectionFields {
  driver: string;
  server: string;
  /** Empty: the engine's usual port. */
  port: string;
  database: string;
  user: string;
  password: string;
  /** Connect without encryption when the server offers none: own or test servers only. */
  allowUnencrypted: boolean;
}

export const EMPTY_SERVER_CONNECTION: ServerConnectionFields = {
  driver: "",
  server: "",
  port: "",
  database: "",
  user: "",
  password: "",
  allowUnencrypted: false,
};

interface ServerEngine {
  label: string;
  port: string;
  driverToInstall: string;
  databaseHint: string;
  /** The encryption attribute: required, or only when the server offers it. */
  encrypted: string;
  optionalEncryption: string;
}

export const SERVER_ENGINES: Record<ServerDatabaseKind, ServerEngine> = {
  postgresql: {
    label: "PostgreSQL",
    port: "5432",
    driverToInstall: "psqlODBC («PostgreSQL Unicode(x64)»)",
    databaseHint: "La del usuario si la dejas vacía",
    encrypted: "SSLmode=require",
    optionalEncryption: "SSLmode=prefer",
  },
  mysql: {
    label: "MySQL",
    port: "3306",
    driverToInstall: "MySQL Connector/ODBC (el controlador Unicode)",
    databaseHint: "Vacía: todas las bases que el usuario puede leer",
    encrypted: "SSLMODE=REQUIRED",
    optionalEncryption: "SSLMODE=PREFERRED",
  },
};

/** What is still missing, in words; null when the connection can be tried. */
export function serverConnectionProblem(kind: ServerDatabaseKind, fields: ServerConnectionFields): string | null {
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

/** The ODBC connection string for these fields; asks for encryption unless allowed otherwise. */
export function buildServerConnectionString(kind: ServerDatabaseKind, fields: ServerConnectionFields): string {
  const engine = SERVER_ENGINES[kind];
  const parts = [
    `Driver={${fields.driver.replace(/\}/g, "}}")}}`,
    `Server=${odbcValue(fields.server.trim())}`,
  ];
  if (fields.port.trim()) parts.push(`Port=${fields.port.trim()}`);
  if (fields.database.trim()) parts.push(`Database=${odbcValue(fields.database.trim())}`);
  parts.push(`Uid=${odbcValue(fields.user.trim())}`, `Pwd=${odbcValue(fields.password)}`);
  parts.push(fields.allowUnencrypted ? engine.optionalEncryption : engine.encrypted);
  return `${parts.join(";")};`;
}
