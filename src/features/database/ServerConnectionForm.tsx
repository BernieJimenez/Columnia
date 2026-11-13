import { useEffect, useId, useRef, useState } from "react";

import { busyDescribedBy } from "../../busyReasons";
import { listDatabaseDrivers, type DatabaseConnection } from "../../bridge";
import { errorMessage } from "../../bridge/errors";
import {
  EMPTY_SERVER_CONNECTION,
  SERVER_ENGINES,
  serverConnectionProblem,
  type ServerDatabaseKind,
} from "./serverConnection";

interface ServerConnectionFormProps {
  kind: ServerDatabaseKind;
  disabled: boolean;
  /** The app is busy (not just this form): the fields point to the global reason while disabled. */
  blockedByBusy?: boolean;
  /** The connection fields, or null while something is missing. */
  onChange: (connection: DatabaseConnection | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
  /** Fields the form starts with (kept from another engine). */
  initial?: Partial<DatabaseConnection>;
  /** Every state of the fields, complete or not, so the parent can carry them to another engine. */
  onDraft?: (fields: DatabaseConnection) => void;
}

/**
 * PostgreSQL or MySQL: server, port, database, user and password instead of an
 * ODBC string (Rust builds it from the fields), with the preferred installed
 * driver. Nothing is saved.
 */
export function ServerConnectionForm({ kind, disabled, blockedByBusy = false, onChange, errorId, initial, onDraft }: ServerConnectionFormProps) {
  const id = useId();
  const engine = SERVER_ENGINES[kind];
  const statusId = `${id}-status`;
  // Fields the user has left or changed: a problem is only shown for those.
  const [touched, setTouched] = useState<ReadonlySet<string>>(new Set());
  const [fields, setFields] = useState<DatabaseConnection>(() => ({ ...EMPTY_SERVER_CONNECTION, ...initial }));
  // null while loading, the installed drivers, or why they could not be listed.
  const [driverList, setDriverList] = useState<null | string[] | { error: string }>(null);
  const driverError = driverList !== null && !Array.isArray(driverList) ? driverList.error : null;
  const drivers = Array.isArray(driverList) ? driverList : driverError !== null ? [] : null;

  useEffect(() => {
    let active = true;
    listDatabaseDrivers(kind)
      .then((found) => {
        if (!active) return;
        setDriverList(found);
        setFields((current) => current.driver ? current : { ...current, driver: found[0] ?? "" });
      })
      .catch((error: unknown) => {
        if (active) setDriverList({ error: errorMessage(error) });
      });
    return () => {
      active = false;
    };
  }, [kind]);

  // The parent hears about every change of the fields, including the driver
  // that arrives after the user has already typed. A ref keeps an inline
  // callback from re-running the effect.
  const onChangeRef = useRef(onChange);
  // Declared first, so it runs before the notifying effect below.
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  useEffect(() => {
    onChangeRef.current(serverConnectionProblem(kind, fields) === null ? fields : null);
  }, [kind, fields]);

  useEffect(() => {
    onDraft?.(fields);
  }, [fields, onDraft]);

  const touch = (field: string) => setTouched((current) => current.has(field) ? current : new Set(current).add(field));
  function update(next: DatabaseConnection, field?: string) {
    if (field) touch(field);
    setFields(next);
  }

  const problem = drivers === null ? null : serverConnectionProblem(kind, fields);
  // The field the problem is about, so the reason can be tied to it.
  // Same order as `serverConnectionProblem`; a missing driver is no field's fault.
  const missing = problem === null || !fields.driver ? null
    : !fields.server.trim() ? "server"
    : serverConnectionProblem(kind, { ...fields, port: "" }) !== problem ? "port"
    : !fields.user.trim() ? "user"
    : null;
  // Do not flag a freshly opened form: a field is only flagged once the user
  // has left or changed it, or the parent reports the incomplete connection.
  const flagged = missing ?? (errorId && problem !== null ? "server" : null);
  const showing = flagged !== null && (Boolean(errorId) || touched.has(flagged));
  const invalid = (field: string) => showing && flagged === field ? true : undefined;
  const describedBy = (field: string) =>
    [showing && flagged === field ? statusId : undefined, field === "server" ? errorId : undefined].filter(Boolean).join(" ") || undefined;
  // A missing driver belongs to no field, so it is said at once.
  const statusText = driverError !== null ? null : problem !== null && (showing || !fields.driver) ? problem : null;

  return (
    <div className="connection-form" role="group" aria-label="Datos de conexión" aria-describedby={busyDescribedBy(blockedByBusy)}>
      <div className="connection-form__row connection-form__row--server">
        <div className="connection-form__field">
          <label htmlFor={`${id}-server`} className="connection-form__required-label">Servidor</label>
          <input
            id={`${id}-server`}
            value={fields.server}
            required
            aria-invalid={invalid("server")}
            aria-describedby={describedBy("server")}
            onBlur={() => touch("server")}
            disabled={disabled}
            autoComplete="off"
            spellCheck={false}
            placeholder="localhost o db.ejemplo.com"
            onChange={(event) => update({ ...fields, server: event.target.value }, "server")}
          />
        </div>
        <div className="connection-form__field connection-form__port">
          <label htmlFor={`${id}-port`}>Puerto</label>
          <input
            id={`${id}-port`}
            aria-invalid={invalid("port")}
            aria-describedby={describedBy("port")}
            onBlur={() => touch("port")}
            value={fields.port}
            disabled={disabled}
            inputMode="numeric"
            autoComplete="off"
            placeholder={engine.port}
            onChange={(event) => update({ ...fields, port: event.target.value }, "port")}
          />
        </div>
      </div>
      <div className="connection-form__field">
        <label htmlFor={`${id}-database`}>Base de datos</label>
        <input
          id={`${id}-database`}
          value={fields.database}
          disabled={disabled}
          autoComplete="off"
          spellCheck={false}
          aria-describedby={`${id}-database-hint`}
          onChange={(event) => update({ ...fields, database: event.target.value })}
        />
        <p id={`${id}-database-hint`} className="connection-form__hint">{engine.databaseHint}</p>
      </div>
      <div className="connection-form__row">
        <div className="connection-form__field">
          <label htmlFor={`${id}-user`} className="connection-form__required-label">Usuario</label>
          <input
            id={`${id}-user`}
            required
            aria-invalid={invalid("user")}
            aria-describedby={describedBy("user")}
            onBlur={() => touch("user")}
            value={fields.user}
            disabled={disabled}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => update({ ...fields, user: event.target.value }, "user")}
          />
        </div>
        <div className="connection-form__field">
          <label htmlFor={`${id}-password`}>Contraseña</label>
          <input
            id={`${id}-password`}
            type="password"
            value={fields.password}
            disabled={disabled}
            autoComplete="off"
            onChange={(event) => update({ ...fields, password: event.target.value })}
          />
        </div>
      </div>
      <label className="connection-form__check">
        <input
          type="checkbox"
          checked={fields.allowUnencrypted}
          disabled={disabled}
          onChange={(event) => update({ ...fields, allowUnencrypted: event.target.checked })}
        />
        <span>
          Conectar sin cifrado si el servidor no lo ofrece
          <small>Solo en servidores propios o de prueba. El cifrado es oportunista: alguien con acceso a la red puede forzar una conexión sin cifrar.</small>
        </span>
      </label>
      {drivers !== null && drivers.length > 1 && (
        <div className="connection-form__field connection-form__driver">
          <label htmlFor={`${id}-driver`}>Controlador ODBC</label>
          <select
            id={`${id}-driver`}
            value={fields.driver}
            disabled={disabled}
            onChange={(event) => update({ ...fields, driver: event.target.value })}
          >
            {drivers.map((driver) => <option key={driver}>{driver}</option>)}
          </select>
        </div>
      )}
      {drivers !== null && drivers.length === 1 && (
        <p className="connection-form__note">Controlador: {drivers[0]}.</p>
      )}
      {/* Always rendered, so a screen reader hears the text change. */}
      <p
        id={statusId}
        className={driverError === null && drivers !== null && drivers.length === 0 ? "notice" : "connection-form__note"}
        role="status"
      >
        {statusText}
      </p>
      {driverError !== null && (
        <p className="notice" role="alert">No se pudieron comprobar los controladores ODBC instalados. {driverError}</p>
      )}
      <p className="connection-form__footer">
        <small>No se guarda nada de la conexión: solo vive mientras la usas.</small>
      </p>
    </div>
  );
}
