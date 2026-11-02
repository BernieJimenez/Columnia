import type { DatabaseKind } from "../../bridge";
import { ServerConnectionForm } from "./ServerConnectionForm";
import { SqlServerConnectionForm } from "./SqlServerConnectionForm";

interface DatabaseConnectionFormProps {
  kind: DatabaseKind;
  disabled: boolean;
  /** The connection string, or null while something is missing. */
  onChange: (connectionString: string | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
}

/** The guided connection of each engine; a new engine starts with empty fields. */
export function DatabaseConnectionForm({ kind, disabled, onChange, errorId }: DatabaseConnectionFormProps) {
  return kind === "sqlserver"
    ? <SqlServerConnectionForm disabled={disabled} onChange={onChange} errorId={errorId} />
    : <ServerConnectionForm key={kind} kind={kind} disabled={disabled} onChange={onChange} errorId={errorId} />;
}
