import { useEffect, useId, useState } from "react";

import { listDatabaseDrivers, type DatabaseConnection } from "../../bridge";
import {
  EMPTY_SQL_SERVER_CONNECTION,
  sqlServerConnection,
  sqlServerConnectionProblem,
} from "./sqlServerConnection";

interface SqlServerConnectionFormProps {
  disabled: boolean;
  /** The connection fields, or null while something is missing. */
  onChange: (connection: DatabaseConnection | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
}

/**
 * Server, database, authentication and encryption instead of an ODBC string:
 * the fields go to Rust, which builds the string. The driver is the newest
 * installed one. Nothing is saved.
 */
export function SqlServerConnectionForm({ disabled, onChange, errorId }: SqlServerConnectionFormProps) {
  const id = useId();
  const [fields, setFields] = useState<DatabaseConnection>(EMPTY_SQL_SERVER_CONNECTION);
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

  function update(next: DatabaseConnection) {
    setFields(next);
    onChange(sqlServerConnectionProblem(next) === null ? sqlServerConnection(next) : null);
  }

  const problem = drivers === null ? null : sqlServerConnectionProblem(fields);

  return (
    <div className="connection-form">
      <div className="connection-form__row">
        <div className="connection-form__field">
          <label htmlFor={`${id}-server`}>Servidor</label>
          <input
            id={`${id}-server`}
            value={fields.server}
            aria-invalid={errorId ? true : undefined}
            aria-describedby={errorId}
            disabled={disabled}
            autoComplete="off"
            spellCheck={false}
            placeholder="localhost, servidor\instancia o servidor,1433"
            onChange={(event) => update({ ...fields, server: event.target.value })}
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
            placeholder="La predeterminada del usuario si la dejas vacía"
            onChange={(event) => update({ ...fields, database: event.target.value })}
          />
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
            <label htmlFor={`${id}-user`}>Usuario</label>
            <input
              id={`${id}-user`}
              value={fields.user}
              disabled={disabled}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => update({ ...fields, user: event.target.value })}
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
      {problem && fields.server.trim() !== "" && <p className="connection-form__note" role="status">{problem}</p>}
      {drivers !== null && drivers.length === 0 && <p className="notice" role="status">{problem}</p>}
      <p className="connection-form__footer">
        <small>No se guarda nada de la conexión: solo vive mientras la usas.</small>
      </p>
    </div>
  );
}
