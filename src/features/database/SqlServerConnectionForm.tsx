import { useEffect, useId, useState } from "react";

import { listDatabaseDrivers } from "../../bridge";
import {
  buildSqlServerConnectionString,
  EMPTY_SQL_SERVER_CONNECTION,
  sqlServerConnectionProblem,
  type SqlServerConnectionFields,
} from "./sqlServerConnection";

interface SqlServerConnectionFormProps {
  disabled: boolean;
  /** The connection string, or null while something is missing. */
  onChange: (connectionString: string | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
}

/**
 * Server, database, authentication and encryption instead of an ODBC string.
 * The driver is the newest installed one; «Escribir la cadena ODBC» stays for
 * cases the fields do not cover. Nothing is saved.
 */
export function SqlServerConnectionForm({ disabled, onChange, errorId }: SqlServerConnectionFormProps) {
  const id = useId();
  const [fields, setFields] = useState<SqlServerConnectionFields>(EMPTY_SQL_SERVER_CONNECTION);
  const [drivers, setDrivers] = useState<string[] | null>(null);
  const [manual, setManual] = useState(false);
  const [manualString, setManualString] = useState("");

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

  function update(next: SqlServerConnectionFields) {
    setFields(next);
    onChange(sqlServerConnectionProblem(next) === null ? buildSqlServerConnectionString(next) : null);
  }

  function changeManual(nextManual: boolean, value = manualString) {
    setManual(nextManual);
    if (nextManual) {
      onChange(value.trim() === "" ? null : value);
    } else {
      onChange(sqlServerConnectionProblem(fields) === null ? buildSqlServerConnectionString(fields) : null);
    }
  }

  const problem = drivers === null ? null : sqlServerConnectionProblem(fields);

  return (
    <div className="connection-form">
      {!manual ? (
        <>
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
                checked={fields.authentication === "sql"}
                disabled={disabled}
                onChange={() => update({ ...fields, authentication: "sql" })}
              />
              Usuario y contraseña de SQL Server
            </label>
          </fieldset>
          {fields.authentication === "sql" && (
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
        </>
      ) : (
        <div className="connection-form__field">
          <label htmlFor={`${id}-manual`}>Cadena de conexión ODBC</label>
          <input
            id={`${id}-manual`}
            type="password"
            aria-invalid={errorId ? true : undefined}
            aria-describedby={errorId}
            autoComplete="off"
            spellCheck={false}
            value={manualString}
            disabled={disabled}
            placeholder="Driver={ODBC Driver 18 for SQL Server};Server=...;Database=...;Trusted_Connection=yes;Encrypt=yes"
            onChange={(event) => {
              setManualString(event.target.value);
              changeManual(true, event.target.value);
            }}
          />
        </div>
      )}
      <p className="connection-form__footer">
        <small>No se guarda nada de la conexión: solo vive mientras la usas.</small>
        <button
          type="button"
          className="connection-form__toggle"
          disabled={disabled}
          onClick={() => changeManual(!manual)}
        >
          {manual ? "Usar el formulario" : "Escribir la cadena ODBC"}
        </button>
      </p>
    </div>
  );
}
