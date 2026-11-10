import { useEffect, useId, useRef, useState } from "react";

import {
  cancelOperation,
  importDatabaseTable,
  listDatabaseTables,
  type DatabaseConnection,
  type DatabaseImportResult,
  type DatabaseKind,
  type DatabaseSource,
  type DatabaseSourceTable,
  type DatasetSourceInspection,
  type OperationProgress,
} from "../../bridge";
import { errorMessage, isCancellationError } from "../../bridge/errors";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatabaseConnectionForm } from "../database/DatabaseConnectionForm";
import { plural } from "../../plural";

const ENGINES: ReadonlyArray<{ kind: DatabaseKind; label: string }> = [
  { kind: "sqlserver", label: "SQL Server" },
  { kind: "postgresql", label: "PostgreSQL" },
  { kind: "mysql", label: "MySQL" },
];

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

/** Where the focus goes when the control that had it stops being usable; only if it was not moved elsewhere. */
function focusIfLost(target: HTMLElement | null, ...owners: Array<HTMLElement | null>) {
  const active = document.activeElement;
  if (target && (active === null || active === document.body || owners.includes(active as HTMLElement))) target.focus();
}

/**
 * Cargar desde una base de datos (SQL Server, PostgreSQL or MySQL, 2026-11-02):
 * lists the tables the server allows to read and
 * brings one into this computer. Columnia only reads; there is no free SQL.
 */
export function DatabaseImportPanel({ connected, blocked, onImport }: DatabaseImportPanelProps) {
  const id = useId();
  const [kind, setKind] = useState<DatabaseKind>("sqlserver");
  const [connection, setConnection] = useState<DatabaseConnection | null>(null);
  const [list, setList] = useState<ListState>({ kind: "idle" });
  const [selected, setSelected] = useState(0);
  const [rowLimit, setRowLimit] = useState<number>(DEFAULT_ROW_LIMIT);
  const [importState, setImportState] = useState<ImportState>({ kind: "idle" });

  const showButtonRef = useRef<HTMLButtonElement>(null);
  const tableRef = useRef<HTMLSelectElement>(null);
  const listMessageRef = useRef<HTMLDivElement>(null);
  const importButtonRef = useRef<HTMLButtonElement>(null);
  const listErrorRef = useRef<HTMLParagraphElement>(null);
  const importMessageRef = useRef<HTMLDivElement>(null);
  const importErrorRef = useRef<HTMLParagraphElement>(null);

  const source: DatabaseSource | null = connection ? { kind, connection } : null;
  const busy = blocked || list.kind === "listing" || importState.kind === "importing";
  const tables = list.kind === "listed" ? list.tables : [];
  const chosen = tables[selected];

  useEffect(() => {
    if (list.kind === "listed") {
      focusIfLost(list.tables.length > 0 ? tableRef.current : listMessageRef.current, showButtonRef.current);
    } else if (list.kind === "error") {
      focusIfLost(listErrorRef.current, showButtonRef.current);
    }
  }, [list]);

  useEffect(() => {
    if (importState.kind === "done" || importState.kind === "error") {
      focusIfLost(importState.kind === "done" ? importMessageRef.current : importErrorRef.current, importButtonRef.current);
    }
  }, [importState]);

  async function showTables() {
    if (!source || busy) return;
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
    if (!chosen || !source || busy) return;
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
    <section className="database-import" aria-labelledby={`${id}-title`}>
      <h3 id={`${id}-title`}>Cargar desde una base de datos</h3>
      <div>
        {!connected ? (
          <p className="recipe-hint" role="status">Conectar con una base de datos solo es posible en la aplicación de escritorio.</p>
        ) : (
          <div className="database-import__form">
            <p className="database-import__lead">
              Columnia lee la tabla que elijas y la guarda en este equipo para revisarla y prepararla.
              Solo lee: no escribe ni modifica nada en el servidor. Windows te pedirá confirmar la conexión.
            </p>
            <fieldset className="database-import__engines">
              <legend>Motor</legend>
              {ENGINES.map((engine) => (
                <label key={engine.kind}>
                  <input
                    type="radio"
                    name={`${id}-engine`}
                    value={engine.kind}
                    checked={kind === engine.kind}
                    disabled={busy}
                    onChange={() => {
                      setKind(engine.kind);
                      setConnection(null);
                      setList({ kind: "idle" });
                      setImportState({ kind: "idle" });
                    }}
                  />
                  {engine.label}
                </label>
              ))}
            </fieldset>
            <DatabaseConnectionForm
              kind={kind}
              disabled={busy}
              onChange={(next) => {
                setConnection(next);
                setList({ kind: "idle" });
                setImportState({ kind: "idle" });
              }}
            />
            <div className="database-import__actions">
              <button
                type="button"
                className="secondary-action"
                ref={showButtonRef}
                disabled={connection === null}
                aria-disabled={busy || undefined}
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
            {connection === null && list.kind === "idle" && (
              <p className="database-import__hint">Completa la conexión para ver las tablas.</p>
            )}
            <div ref={listMessageRef} role="status" tabIndex={-1}>
              {list.kind === "listing" && <p className="database-import__hint">Conectando…</p>}
              {list.kind === "listed" && tables.length === 0 && (
                <p className="notice">El servidor no muestra tablas ni vistas que este usuario pueda leer.</p>
              )}
              {list.kind === "listed" && tables.length > 0 && (
                <p className="database-import__hint">
                  {plural(tables.length, "tabla o vista disponible", "tablas y vistas disponibles")}.
                </p>
              )}
            </div>
            {list.kind === "error" && <p className="notice notice--error" role="alert" tabIndex={-1} ref={listErrorRef}>{list.message}</p>}
            {list.kind === "listed" && tables.length > 0 && (
              <>
                <div className="database-import__choice">
                  <div className="database-import__field">
                    <label htmlFor={`${id}-table`}>Tabla o vista</label>
                    <select
                      id={`${id}-table`}
                      ref={tableRef}
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
                  <button
                    type="button"
                    className="primary-action"
                    ref={importButtonRef}
                    disabled={!chosen}
                    aria-disabled={busy || undefined}
                    onClick={importChosen}
                  >
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
            {importState.kind === "error" && <p className="notice notice--error" role="alert" tabIndex={-1} ref={importErrorRef}>{importState.message}</p>}
            <div ref={importMessageRef} role="status" tabIndex={-1}>
              {importState.kind === "done" && (
                <div className="notice notice--success">
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
          </div>
        )}
      </div>
    </section>
  );
}
