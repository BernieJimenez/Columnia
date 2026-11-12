import type { DatabaseConnection, DatabaseKind } from "../../bridge";
import { ServerConnectionForm } from "./ServerConnectionForm";
import { SqlServerConnectionForm } from "./SqlServerConnectionForm";

interface DatabaseConnectionFormProps {
  kind: DatabaseKind;
  disabled: boolean;
  /** The app is busy (not just this form): the fields point to the global reason while disabled. */
  blockedByBusy?: boolean;
  /** The connection fields, or null while something is missing. */
  onChange: (connection: DatabaseConnection | null) => void;
  /** The id of a message saying the connection is incomplete; marks its first field. */
  errorId?: string;
}

/** The guided connection of each engine; a new engine starts with empty fields. */
export function DatabaseConnectionForm({ kind, disabled, blockedByBusy, onChange, errorId }: DatabaseConnectionFormProps) {
  return kind === "sqlserver"
    ? <SqlServerConnectionForm disabled={disabled} blockedByBusy={blockedByBusy} onChange={onChange} errorId={errorId} />
    : <ServerConnectionForm key={kind} kind={kind} disabled={disabled} blockedByBusy={blockedByBusy} onChange={onChange} errorId={errorId} />;
}
