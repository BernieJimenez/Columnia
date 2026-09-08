import type { HistoryState } from "../../bridge";
import { changeProgressMessage, type ChangeStatus } from "./prepareModel";

export function HistoryBar({
  status,
  busy,
  latestChange,
  onUndo,
  onRedo,
}: {
  status: HistoryState;
  busy: boolean;
  latestChange?: string;
  onUndo: () => void;
  onRedo: () => void;
}) {
  const quiet = status.snapshotsEnabled && status.entryCount <= 1 && !status.canUndo && !status.canRedo;

  return (
    <section className={`history-bar${quiet ? " history-bar--quiet" : ""}`} aria-label="Historial de cambios">
      <div className="history-bar__content">
        <strong>Historial de cambios</strong>
        <small>
          {status.snapshotsEnabled
            ? status.entryCount > 0
              ? `Versión actual: ${status.entries.find((entry) => entry.isCurrent)?.label ?? "Dataset cargado"} · ${status.currentIndex + 1} de ${status.entryCount}`
              : "Todavía no hay versiones guardadas."
            : status.degradedReason ?? "El historial reversible no está disponible."}
        </small>
        {latestChange && (
          <div className="history-latest" role="status">
            <span>Último resultado</span>
            <p>{latestChange}</p>
          </div>
        )}
        {status.entries.length > 1 && (
          <details className="history-details" open>
            <summary>Cambios realizados ({status.entryCount - 1})</summary>
            <ol>
              {status.entries.slice(-12).map((entry) => (
                <li key={entry.index} aria-current={entry.isCurrent ? "step" : undefined}>
                  <span>{entry.index === 0 ? "Origen" : `Cambio ${entry.index}`}</span>
                  <span>{entry.label}</span>
                  {entry.isCurrent && <strong>Actual</strong>}
                </li>
              ))}
            </ol>
          </details>
        )}
      </div>
      <div className="history-actions">
        <button type="button" onClick={onUndo} disabled={busy || !status.snapshotsEnabled || !status.canUndo}>
          Deshacer
        </button>
        <button type="button" onClick={onRedo} disabled={busy || !status.snapshotsEnabled || !status.canRedo}>
          Rehacer
        </button>
      </div>
    </section>
  );
}
export function ChangeFeedback({ status }: { status: ChangeStatus }) {
  if (status.kind === "idle" || status.kind === "applied") return null;

  if (status.kind === "working") {
    const message = changeProgressMessage(status.action);
    return (
      <p className="notice" role="status">
        {message}
      </p>
    );
  }

  if (status.kind === "error") {
    return (
      <div className="change-feedback change-feedback--error" role="alert">
        <span>{status.message}</span>
      </div>
    );
  }

  return null;
}
