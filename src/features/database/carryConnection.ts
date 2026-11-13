/**
 * What survives a change of engine (2026-11-13): the server, the database and
 * the user still mean the same thing; the port only when it was not the
 * default of the engine being left; the password is dropped, because it was
 * chosen for another server and the interface says so.
 */
import type { DatabaseConnection, DatabaseKind } from "../../bridge";

const LABELS: Record<DatabaseKind, string> = { sqlserver: "SQL Server", postgresql: "PostgreSQL", mysql: "MySQL" };
const DEFAULT_PORTS: Record<DatabaseKind, string> = { sqlserver: "1433", postgresql: "5432", mysql: "3306" };

export interface CarriedConnection {
  /** Initial fields for the new engine's form. */
  fields: Partial<DatabaseConnection>;
  /** What was kept and what was dropped, for the interface; null when there was nothing to say. */
  notice: string | null;
}

export function carryConnection(from: DatabaseKind, fields: DatabaseConnection, to: DatabaseKind): CarriedConnection {
  let server = fields.server.trim();
  let port = fields.port.trim();
  let instanceDropped = false;
  if (from === "sqlserver" && to !== "sqlserver") {
    // «host\instancia» and «host,puerto» only exist in SQL Server.
    const match = /^([^\\,]*)(?:\\[^,]*)?(?:,\s*(\d+))?$/.exec(server);
    if (match) {
      instanceDropped = server.includes("\\");
      server = (match[1] ?? "").trim();
      port = match[2] && match[2] !== DEFAULT_PORTS[to] ? match[2] : "";
    }
  } else if (from !== "sqlserver" && to === "sqlserver") {
    if (server && port && port !== DEFAULT_PORTS[from]) server = `${server},${port}`;
    port = "";
  } else {
    // Empty means the default of the engine; a custom port is kept.
    port = port === DEFAULT_PORTS[from] ? "" : port;
  }
  const user = from === "sqlserver" && fields.authentication === "windows" ? "" : fields.user.trim();
  const database = fields.database.trim();

  const kept = [server && "el servidor", database && "la base de datos", user && "el usuario"].filter(Boolean);
  const dropped = [fields.password && "la contraseña", instanceDropped && "el nombre de instancia"].filter(Boolean);
  const sentences: string[] = [];
  if (kept.length > 0) sentences.push(`Al cambiar a ${LABELS[to]} se conservaron ${joinList(kept)}.`);
  if (dropped.length > 0) {
    const verb = dropped.length === 1 ? "se traslada" : "se trasladan";
    sentences.push(kept.length > 0 ? `No ${verb} ${joinList(dropped)}.` : `Al cambiar a ${LABELS[to]} no ${verb} ${joinList(dropped)}.`);
  }

  return {
    fields: {
      server,
      port,
      database,
      user,
      ...(to === "sqlserver" ? { authentication: user ? ("password" as const) : ("windows" as const) } : {}),
    },
    notice: sentences.length > 0 ? sentences.join(" ") : null,
  };
}

function joinList(items: Array<string | false | "">): string {
  const words = items.filter(Boolean) as string[];
  return words.length <= 1 ? words.join("") : `${words.slice(0, -1).join(", ")} y ${words[words.length - 1]}`;
}
