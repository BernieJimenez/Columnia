import { useEffect, useId, useRef, useState } from "react";

import { busyDescribedBy } from "../../busyReasons";
import { listDatabaseDrivers, type DatabaseConnection } from "../../bridge";
import {
  EMPTY_SQL_SERVER_CONNECTION,
  sqlServerConnection,
  sqlServerConnectionProblem,
} from "./sqlServerConnection";

interface SqlServerConnectionFormProps {
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
 * Server, database, authentication and encryption instead of an ODBC string:
 * the fields go to Rust, which builds the string. The driver is the newest
 * installed one. Nothing is saved.
 */
export function SqlServerConnectionForm({ disabled, blockedByBusy = false, onChange, errorId, initial, onDraft }: SqlServerConnectionFormProps) {
  const id = useId();
  // Fields the user has left or changed: a problem is only shown for those.
  const [touched, setTouched] = useState<ReadonlySet<string>>(new Set());
  const [fields, setFields] = useState<DatabaseConnection>(() => ({ ...EMPTY_SQL_SERVER_CONNECTION, ...initial }));
  const [drivers, setDrivers] = useState<string[] | null>(null);

  useEffect(() => {
    let active = true;
    listDatabaseDrivers("sqlserver")
      .then((found) => {
        if (!active) return;
        setDrivers(found);
        setFields((current) => current.driver ? current : { ...current, driver: found[0] ?? "" });
      })
      .catch(() => active && setDrivers([]));
    return () => {
      active = false;
    };
  }, []);

  // The parent hears about every change of the fields, including the driver
  // that arrives after the user has typed (or after the fields were carried over).
  const onChangeRef = useRef(onChange);
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  useEffect(() => {
    onChangeRef.current(sqlServerConnectionProblem(fields) === null ? sqlServerConnection(fields) : null);
  }, [fields]);

  useEffect(() => {
    onDraft?.(fields);
  }, [fields, onDraft]);

  const touch = (field: string) => setTouched((current) => current.has(field) ? current : new Set(current).add(field));
  function update(next: DatabaseConnection, field?: string) {
    if (field) touch(field);
    setFields(next);
  }

  const problem = drivers === null ? null : sqlServerConnectionProblem(fields);
  const statusId = `${id}-status`;
  // The field the problem is about; the user only counts with SQL Server sign-in.
  // A missing driver is no field's fault.
  const missing = problem === null || !fields.driver ? null
    : !fields.server.trim() ? "server"
    : fields.authentication === "password" && !fields.user.trim() ? "user"
    : null;
  // Do not flag a freshly opened form: a field is only flagged once the user
  // has left or changed it, or the parent reports the incomplete connection.
  const flagged = missing ?? (errorId && problem !== null ? "server" : null);
  const showing = flagged !== null && (Boolean(errorId) || touched.has(flagged));
  const invalid = (field: string) => showing && flagged === field ? true : undefined;
  const describedBy = (field: string) =>
    [showing && flagged === field ? statusId : undefined, field === "server" ? errorId : undefined].filter(Boolean).join(" ") || undefined;
  // A missing driver belongs to no field, so it is said at once.
  const statusText = problem !== null && (showing || !fields.driver) ? problem : null;

  return (
    <div className="connection-form" role="group" aria-label="Datos de conexión" aria-describedby={busyDescribedBy(blockedByBusy)}>
      <div className="connection-form__row">
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
            placeholder="localhost, servidor\instancia o servidor,1433"
            onChange={(event) => update({ ...fields, server: event.target.value }, "server")}
          />
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
          <p id={`${id}-database-hint`} className="connection-form__hint">Si lo dejas vacío, se usa la base por defecto del usuario.</p>
        </div>
      </div>
      <fieldset className="connection-form__auth">
        <legend>Autenticación</legend>
        <label>
          <input
            type="radio"
            name={`${id}-auth`}
            checked={fields.authentication === "windows"}
            disabled={disabled}
            onChange={() => update({ ...fields, authentication: "windows" })}
          />
          Windows (tu usuario actual)
        </label>
        <label>
          <input
            type="radio"
            name={`${id}-auth`}
            checked={fields.authentication === "password"}
            disabled={disabled}
            onChange={() => update({ ...fields, authentication: "password" })}
          />
          Usuario y contraseña de SQL Server
        </label>
      </fieldset>
      {fields.authentication === "password" && (
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
      )}
      <label className="connection-form__check">
        <input
          type="checkbox"
          checked={fields.trustServerCertificate}
          disabled={disabled}
          onChange={(event) => update({ ...fields, trustServerCertificate: event.target.checked })}
        />
        <span>
          Confiar en el certificado del servidor
          <small>Solo en servidores propios o de prueba, sin certificado de una entidad reconocida. La conexión sigue cifrada.</small>
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
        className={drivers !== null && drivers.length === 0 ? "notice" : "connection-form__note"}
        role="status"
      >
        {statusText}
      </p>
      <p className="connection-form__footer">
        <small>No se guarda nada de la conexión: solo vive mientras la usas.</small>
      </p>
    </div>
  );
}
