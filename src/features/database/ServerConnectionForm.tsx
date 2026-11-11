import { useEffect, useId, useState } from "react";

import { listDatabaseDrivers, type DatabaseConnection } from "../../bridge";
import {
  EMPTY_SERVER_CONNECTION,
  SERVER_ENGINES,
  serverConnectionProblem,
  type ServerDatabaseKind,
} from "./serverConnection";

interface ServerConnectionFormProps {
  kind: ServerDatabaseKind;
  disabled: boolean;
  /** The connection fields, or null while something is missing. */
  onChange: (connection: DatabaseConnection | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
}

/**
 * PostgreSQL or MySQL: server, port, database, user and password instead of an
 * ODBC string (Rust builds it from the fields), with the preferred installed
 * driver. Nothing is saved.
 */
export function ServerConnectionForm({ kind, disabled, onChange, errorId }: ServerConnectionFormProps) {
  const id = useId();
  const engine = SERVER_ENGINES[kind];
  const [fields, setFields] = useState<DatabaseConnection>(EMPTY_SERVER_CONNECTION);
  const [drivers, setDrivers] = useState<string[] | null>(null);

  useEffect(() => {
    let active = true;
    listDatabaseDrivers(kind)
      .then((found) => {
        if (!active) return;
        setDrivers(found);
        setFields((current) => current.driver ? current : { ...current, driver: found[0] ?? "" });
      })
      .catch(() => active && setDrivers([]));
    return () => {
      active = false;
    };
  }, [kind]);

  function update(next: DatabaseConnection) {
    setFields(next);
    onChange(serverConnectionProblem(kind, next) === null ? next : null);
  }

  const problem = drivers === null ? null : serverConnectionProblem(kind, fields);

  return (
    <div className="connection-form">
      <div className="connection-form__row connection-form__row--server">
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
            placeholder="localhost o db.ejemplo.com"
            onChange={(event) => update({ ...fields, server: event.target.value })}
          />
        </div>
        <div className="connection-form__field connection-form__port">
          <label htmlFor={`${id}-port`}>Puerto</label>
          <input
            id={`${id}-port`}
            value={fields.port}
            disabled={disabled}
            inputMode="numeric"
            autoComplete="off"
            placeholder={engine.port}
            onChange={(event) => update({ ...fields, port: event.target.value })}
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
          placeholder={engine.databaseHint}
          onChange={(event) => update({ ...fields, database: event.target.value })}
        />
      </div>
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
      {problem && fields.server.trim() !== "" && drivers !== null && drivers.length > 0 && (
        <p className="connection-form__note" role="status">{problem}</p>
      )}
      {drivers !== null && drivers.length === 0 && <p className="notice" role="status">{problem}</p>}
      <p className="connection-form__footer">
        <small>No se guarda nada de la conexión: solo vive mientras la usas.</small>
      </p>
    </div>
  );
}
