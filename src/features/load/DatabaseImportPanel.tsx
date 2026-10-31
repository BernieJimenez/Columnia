import { useId, useState } from "react";

import {
  cancelOperation,
  importDatabaseTable,
  listDatabaseTables,
  type DatabaseImportResult,
  type DatabaseSourceTable,
  type DatasetSourceInspection,
  type OperationProgress,
} from "../../bridge";
import { errorMessage, isCancellationError } from "../../bridge/errors";
import { OperationProgressView } from "../../components/OperationProgressView";
import { plural } from "../../plural";

/** Row limits offered; the engine accepts up to 5,000,000. */
const DATABASE_ROW_LIMITS = [10_000, 100_000, 1_000_000, 5_000_000] as const;
const DEFAULT_ROW_LIMIT = 100_000;

type ListState =
  | { kind: "idle" }
  | { kind: "listing" }
  | { kind: "listed"; tables: DatabaseSourceTable[] }
  | { kind: "error"; message: string };

type ImportState =
  | { kind: "idle" }
  | { kind: "importing"; progress: OperationProgress; cancelRequested: boolean }
  | { kind: "done"; result: DatabaseImportResult; table: string }
  | { kind: "error"; message: string };

interface DatabaseImportPanelProps {
  /** The local engine answers; outside the desktop app nothing can connect. */
  connected: boolean;
  /** Another operation is running. */
  blocked: boolean;
  /**
   * Runs the import once the current work may be replaced; the inspection then
   * follows the ordinary Cargar flow. Resolving null leaves Cargar as it was.
   */
  onImport: (run: () => Promise<DatasetSourceInspection | null>) => void;
}

function tableLabel(table: DatabaseSourceTable): string {
  return table.schema ? `${table.schema}.${table.name}` : table.name;
}

/**
 * Cargar desde SQL Server: lists the tables the server allows to read and
 * brings one into this computer. Columnia only reads; there is no free SQL.
 */
export function DatabaseImportPanel({ connected, blocked, onImport }: DatabaseImportPanelProps) {
  const id = useId();
  const [connectionString, setConnectionString] = useState("");
  const [list, setList] = useState<ListState>({ kind: "idle" });
  const [selected, setSelected] = useState(0);
  const [rowLimit, setRowLimit] = useState<number>(DEFAULT_ROW_LIMIT);
  const [importState, setImportState] = useState<ImportState>({ kind: "idle" });

  const source = { kind: "sqlserver" as const, connectionString };
  const busy = blocked || list.kind === "listing" || importState.kind === "importing";
  const tables = list.kind === "listed" ? list.tables : [];
  const chosen = tables[selected];

  async function showTables() {
    setList({ kind: "listing" });
    setImportState({ kind: "idle" });
    try {
      const listed = await listDatabaseTables(source);
      setSelected(0);
      setList({ kind: "listed", tables: listed });
    } catch (error: unknown) {
      setList(isCancellationError(error) ? { kind: "idle" } : { kind: "error", message: errorMessage(error) });
    }
  }

  function importChosen() {
    if (!chosen) return;
    const table = tableLabel(chosen);
    onImport(async () => {
      setImportState({
        kind: "importing",
        progress: { operation: "load", stage: "Conectando con la base de datos", percent: 0 },
        cancelRequested: false,
      });
      try {
        const result = await importDatabaseTable(source, chosen.schema, chosen.name, rowLimit, (progress) => {
          setImportState((current) => current.kind === "importing" ? { ...current, progress } : current);
        });
        setImportState({ kind: "done", result, table });
        return result.inspection;
      } catch (error: unknown) {
        setImportState(isCancellationError(error) ? { kind: "idle" } : { kind: "error", message: errorMessage(error) });
        return null;
      }
    });
  }

  function cancel() {
    setImportState((current) => current.kind === "importing" ? { ...current, cancelRequested: true } : current);
    void cancelOperation("databaseConnection").catch(() => undefined);
  }

  const schemas = [...new Set(tables.map((table) => table.schema))];

  return (
    <details className="load-secondary database-import">
      <summary>
        <span>Cargar desde SQL Server</span>
        <small>Trae una tabla o vista a este equipo, solo lectura</small>
      </summary>
      <div className="load-secondary__content">
        {!connected ? (
          <p className="recipe-hint" role="status">Conectar con SQL Server solo es posible en la aplicación de escritorio.</p>
        ) : (
          <div className="database-import__form">
            <p className="database-import__lead">
              Columnia lee la tabla que elijas y la guarda en este equipo para revisarla y prepararla.
              Solo lee: no escribe ni modifica nada en el servidor. Windows te pedirá confirmar la conexión.
            </p>
            <div className="database-import__field">
              <label htmlFor={`${id}-connection`}>Cadena de conexión ODBC</label>
              <input
                id={`${id}-connection`}
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={connectionString}
                disabled={busy}
                placeholder="Driver={ODBC Driver 18 for SQL Server};Server=...;Database=...;Trusted_Connection=yes;Encrypt=yes"
                aria-describedby={`${id}-connection-note`}
                onChange={(event) => {
                  setConnectionString(event.target.value);
                  setList({ kind: "idle" });
                  setImportState({ kind: "idle" });
                }}
              />
              <small id={`${id}-connection-note`}>No se guarda: solo vive mientras la usas. Usa un controlador ODBC instalado.</small>
            </div>
            <div className="database-import__actions">
              <button
                type="button"
                className="secondary-action"
                disabled={busy || connectionString.trim() === ""}
                onClick={() => void showTables()}
              >
                {list.kind === "listing" ? "Conectando…" : "Ver tablas"}
              </button>
              {list.kind === "listing" && (
                <button type="button" className="secondary-action" onClick={() => void cancelOperation("databaseConnection").catch(() => undefined)}>
                  Cancelar
                </button>
              )}
            </div>
            {connectionString.trim() === "" && list.kind === "idle" && (
              <p className="database-import__hint">Pega la cadena de conexión para ver las tablas.</p>
            )}
            {list.kind === "error" && <p className="notice notice--error" role="alert">{list.message}</p>}
            {list.kind === "listed" && tables.length === 0 && (
              <p className="notice" role="status">El servidor no muestra tablas ni vistas que este usuario pueda leer.</p>
            )}
            {list.kind === "listed" && tables.length > 0 && (
              <>
                <p className="database-import__hint" role="status">
                  {plural(tables.length, "tabla o vista disponible", "tablas y vistas disponibles")}.
                </p>
                <div className="database-import__choice">
                  <div className="database-import__field">
                    <label htmlFor={`${id}-table`}>Tabla o vista</label>
                    <select
                      id={`${id}-table`}
                      value={selected}
                      disabled={busy}
                      onChange={(event) => setSelected(Number(event.target.value))}
                    >
                      {schemas.map((schema) => (
                        <optgroup key={schema} label={schema || "Sin esquema"}>
                          {tables.map((table, index) => table.schema === schema && (
                            <option key={`${table.schema}.${table.name}`} value={index}>
                              {table.name}{table.isView ? " (vista)" : ""}
                            </option>
                          ))}
                        </optgroup>
                      ))}
                    </select>
                  </div>
                  <div className="database-import__field">
                    <label htmlFor={`${id}-limit`}>Filas como máximo</label>
                    <select
                      id={`${id}-limit`}
                      value={rowLimit}
                      disabled={busy}
                      onChange={(event) => setRowLimit(Number(event.target.value))}
                    >
                      {DATABASE_ROW_LIMITS.map((limit) => (
                        <option key={limit} value={limit}>{limit.toLocaleString()}</option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className="database-import__actions">
                  <button type="button" className="primary-action" disabled={busy || !chosen} onClick={importChosen}>
                    {chosen ? `Traer ${tableLabel(chosen)}` : "Traer tabla"}
                  </button>
                </div>
              </>
            )}
            {importState.kind === "importing" && (
              <OperationProgressView
                progress={importState.progress}
                cancellation={importState.cancelRequested ? { kind: "requested" } : { kind: "available", onCancel: cancel }}
              />
            )}
            {importState.kind === "error" && <p className="notice notice--error" role="alert">{importState.message}</p>}
            {importState.kind === "done" && (
              <div className="notice notice--success" role="status">
                <p>
                  Se trajeron {plural(importState.result.rowsImported, "fila", "filas")} de {importState.table}.
                  {importState.result.moreRowsAvailable && " La tabla tiene más filas: sube el límite para traerlas."}
                </p>
                {importState.result.truncatedColumns.length > 0 && (
                  <p>
                    Algunos textos de {importState.result.truncatedColumns.map((name) => `«${name}»`).join(", ")} tenían más de 8.000 caracteres y se recortaron a esa longitud.
                  </p>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </details>
  );
}
