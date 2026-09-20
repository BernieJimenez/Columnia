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
        {status.snapshotsEnabled && (
          <div className="history-retention" aria-label="Uso y retención del historial">
            <p>
              {status.entryCount.toLocaleString()} / {status.maxEntries.toLocaleString()} estados · {formatHistoryBytes(status.diskBytes)} / {formatHistoryBytes(status.diskBudgetBytes)} de historial local
            </p>
            <details>
              <summary>Política local</summary>
              <p>
                Se conservan hasta {status.maxEntries.toLocaleString()} estados o {formatHistoryBytes(status.diskBudgetBytes)} por dataset. Al llegar a un límite, se retiran primero los estados más antiguos del historial y se mantiene el actual. Si un snapshot individual supera el presupuesto, la operación puede quedar sin historial reversible.
              </p>
            </details>
          </div>
        )}
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

function formatHistoryBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "No disponible";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);
  return `${value.toLocaleString("es", { maximumFractionDigits: 1 })} ${units[unitIndex]}`;
}

export function ChangeFeedback({ status, onCancel }: { status: ChangeStatus; onCancel?: () => void }) {
  if (status.kind === "idle" || status.kind === "applied") return null;

  if (status.kind === "working") {
    const message = changeProgressMessage(status.action);
    return (
      <div className="notice change-feedback__working" role="status">
        <span>{status.cancelRequested ? "Cancelando preparación…" : message}</span>
        {onCancel && (
          <button type="button" onClick={onCancel} disabled={status.cancelRequested}>
            {status.cancelRequested ? "Cancelando…" : "Cancelar"}
          </button>
        )}
      </div>
    );
  }

  if (status.kind === "cancelled") {
    return <p className="notice" role="status">{status.message}</p>;
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
