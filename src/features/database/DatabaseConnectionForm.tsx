import { useCallback, useState } from "react";

import type { DatabaseConnection, DatabaseKind } from "../../bridge";
import { carryConnection, type CarriedConnection } from "./carryConnection";
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

/**
 * The guided connection of each engine. Changing the engine keeps the server,
 * the database and the user (see `carryConnection`) and says what was kept and
 * what was dropped; the password never travels.
 */
export function DatabaseConnectionForm({ kind, disabled, blockedByBusy, onChange, errorId }: DatabaseConnectionFormProps) {
  const [draft, setDraft] = useState<{ kind: DatabaseKind; fields: DatabaseConnection } | null>(null);
  const [carried, setCarried] = useState<(CarriedConnection & { kind: DatabaseKind }) | null>(null);
  const onDraft = useCallback((fields: DatabaseConnection) => setDraft({ kind, fields }), [kind]);

  // The new form starts from the fields of the engine just left; this runs
  // while rendering so the form already mounts with them.
  let seed = carried;
  if (draft !== null && draft.kind !== kind && carried?.kind !== kind) {
    seed = { ...carryConnection(draft.kind, draft.fields, kind), kind };
    setCarried(seed);
  }
  const initial = seed?.kind === kind ? seed.fields : undefined;
  const notice = seed?.kind === kind ? seed.notice : null;

  return (
    <>
      {kind === "sqlserver"
        ? <SqlServerConnectionForm disabled={disabled} blockedByBusy={blockedByBusy} onChange={onChange} errorId={errorId} initial={initial} onDraft={onDraft} />
        : <ServerConnectionForm key={kind} kind={kind} disabled={disabled} blockedByBusy={blockedByBusy} onChange={onChange} errorId={errorId} initial={initial} onDraft={onDraft} />}
      {/* Always rendered, so a screen reader hears the text appear. */}
      <p id="connection-carry-notice" className="connection-form__note" role="status">{notice}</p>
    </>
  );
}
