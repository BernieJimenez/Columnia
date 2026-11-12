import { useId } from "react";

import type { ColumnProfile, SqlServerScriptOptions } from "../../bridge";
import { busyDescribedBy } from "../../busyReasons";
import { keyColumnWarnings } from "./deliverySignals";

/** A new table named after the saved file, in dbo, with fitted text. */
export const DEFAULT_SQL_SERVER_SCRIPT: SqlServerScriptOptions = {
  schema: "",
  table: "",
  mode: "create",
  keyColumns: [],
  textSize: "fitted",
};

const MODES: ReadonlyArray<{ mode: SqlServerScriptOptions["mode"]; label: string; description: string }> = [
  {
    mode: "create",
    label: "Crear una tabla nueva",
    description: "Si ya existe una tabla con ese nombre, el script se detiene sin tocarla.",
  },
  {
    mode: "append",
    label: "Añadir filas a una tabla existente",
    description: "Antes comprueba que la tabla exista y tenga todas las columnas.",
  },
  {
    mode: "upsert",
    label: "Actualizar por clave",
    description: "Actualiza las filas cuya clave ya está en la tabla y añade las nuevas.",
  },
];

interface SqlServerScriptTargetProps {
  value: SqlServerScriptOptions;
  columns: readonly string[];
  disabled: boolean;
  onChange: (next: SqlServerScriptOptions) => void;
  /** Quality profile of the columns; without it no key warning is shown. */
  columnProfiles?: readonly Pick<ColumnProfile, "name" | "nullCount" | "uniqueCount">[] | null;
  rowCount?: number;
}

/**
 * The table of «Script T-SQL» and «Paquete BULK INSERT» (2026-10-31): what
 * the script does with it, its name and, to update by key, which columns
 * identify a row. Columnia never connects: the script runs where the person
 * chooses.
 */
export function SqlServerScriptTarget({
  value,
  columns,
  disabled,
  onChange,
  columnProfiles = null,
  rowCount = 0,
}: SqlServerScriptTargetProps) {
  const id = useId();
  const change = (patch: Partial<SqlServerScriptOptions>) => onChange({ ...value, ...patch });
  const keys = value.keyColumns.filter((column) => columns.includes(column));
  const keyWarnings = keyColumnWarnings(keys, columnProfiles, rowCount);

  function toggleKey(column: string, checked: boolean) {
    change({
      keyColumns: checked
        ? columns.filter((name) => name === column || keys.includes(name))
        : keys.filter((name) => name !== column),
    });
  }

  return (
    <fieldset className="database-target sqlserver-script-target" disabled={disabled} aria-describedby={busyDescribedBy(disabled)}>
      <legend>Tabla en SQL Server</legend>
      <fieldset className="sqlserver-script-target__modes">
        <legend>Qué hace el script con la tabla</legend>
        {MODES.map((option) => (
          <label key={option.mode} className="sqlserver-script-target__mode">
            <input
              type="radio"
              name={`${id}-mode`}
              value={option.mode}
              checked={value.mode === option.mode}
              onChange={() => change({ mode: option.mode })}
            />
            <span>
              <strong>{option.label}</strong>
              <small>{option.description}</small>
            </span>
          </label>
        ))}
      </fieldset>
      <div className="database-target__grid sqlserver-script-target__names">
        <label>
          Esquema
          <input
            value={value.schema}
            placeholder="dbo"
            autoComplete="off"
            spellCheck={false}
            maxLength={128}
            onChange={(event) => change({ schema: event.target.value })}
          />
        </label>
        <label>
          Tabla
          <input
            value={value.table}
            placeholder="Nombre del archivo"
            aria-describedby={`${id}-names-note`}
            autoComplete="off"
            spellCheck={false}
            maxLength={128}
            onChange={(event) => change({ table: event.target.value })}
          />
        </label>
      </div>
      <p id={`${id}-names-note`}>Si los dejas vacíos, se usan dbo y el nombre del archivo que guardes.</p>
      {value.mode === "upsert" && (
        <fieldset className="sqlserver-script-target__keys" aria-describedby={keyWarnings.length > 0 ? `${id}-keys-note ${id}-keys-warning` : `${id}-keys-note`}>
          <legend>Columnas clave</legend>
          <p id={`${id}-keys-note`}>
            Identifican cada fila, como un código o un ID. En los datos no pueden tener nulos ni repetirse;
            si pasa, el script se detiene sin tocar la tabla.
          </p>
          <div className="sqlserver-script-target__key-list">
            {columns.map((column) => (
              <label key={column} className="sqlserver-script-target__key">
                <input
                  type="checkbox"
                  checked={keys.includes(column)}
                  onChange={(event) => toggleKey(column, event.target.checked)}
                />
                <span title={column}>{column}</span>
              </label>
            ))}
          </div>
          {keyWarnings.length > 0 && (
            <div id={`${id}-keys-warning`} className="notice notice--warning" role="status">
              {keyWarnings.map((warning) => <p key={warning}>{warning}</p>)}
            </div>
          )}
        </fieldset>
      )}
      {value.mode === "create" && (
        <label className="sqlserver-script-target__text-size">
          Largo de las columnas de texto
          <select
            aria-label="Largo de las columnas de texto"
            value={value.textSize}
            aria-describedby={`${id}-text-size-note`}
            onChange={(event) => change({ textSize: event.target.value as SqlServerScriptOptions["textSize"] })}
          >
            <option value="fitted">Ajustado a los datos (recomendado)</option>
            <option value="max">Sin límite: NVARCHAR(MAX)</option>
          </select>
          <small id={`${id}-text-size-note`}>
            {value.textSize === "fitted"
              ? "Cada columna de texto toma el largo de su valor más largo con margen (por ejemplo NVARCHAR(100)): la tabla ocupa menos y admite índices."
              : "Cualquier texto cabe, pero las columnas no se pueden indexar y ocupan más."}
          </small>
        </label>
      )}
    </fieldset>
  );
}
