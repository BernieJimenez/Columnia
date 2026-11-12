import type { HistoryState } from "../../bridge";
import { changeProgressMessage, formatChangeSummary, type ChangeStatus } from "./prepareModel";
import { formatBytes } from "../../format";
import { OperationProgressView } from "../../components/OperationProgressView";

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
  const visibleEntries = status.entries.slice(-MAX_VISIBLE_VERSIONS);
  const hiddenCount = status.entries.length - visibleEntries.length;
  const nothingToUndo = status.snapshotsEnabled && !status.canUndo && !status.canRedo;

  return (
    // AUD-053: the landmark is the <aside> in PreparePhase; this block stays unnamed.
    <div className="version-history">
      <div className="version-history__heading">
        <h3>Historial</h3>
        <p>
          {status.snapshotsEnabled
            ? status.entryCount > 0
              ? `Versión actual: ${status.entries.find((entry) => entry.isCurrent)?.label ?? "Dataset cargado"} · ${status.currentIndex + 1} de ${status.entryCount}`
              : "Todavía no hay versiones guardadas."
            : status.degradedReason ?? "El historial reversible no está disponible."}
        </p>
      </div>
      {/* DAT-07: a version too large to keep is announced while the history stays. */}
      {status.snapshotsEnabled && status.degradedReason && (
        <p className="version-history__note" role="note">{status.degradedReason}</p>
      )}
      {/* AUD-057: Deshacer and Rehacer sit under the heading, so a tall list never pushes them out of view. */}
      <div className="history-actions">
        <button type="button" className="secondary-action" onClick={onUndo} disabled={busy || !status.snapshotsEnabled || !status.canUndo}>
          Deshacer
        </button>
        <button type="button" className="secondary-action" onClick={onRedo} disabled={busy || !status.snapshotsEnabled || !status.canRedo}>
          Rehacer
        </button>
      </div>
      {nothingToUndo && <p className="version-history__hint">Aún no hay cambios que deshacer.</p>}
      {status.snapshotsEnabled && visibleEntries.length > 0 && (
        <ol className="version-history__list" aria-label="Versiones del dataset">
          {hiddenCount > 0 && (
            <li className="version-history__more">{hiddenCount === 1 ? "1 versión anterior" : `${hiddenCount.toLocaleString()} versiones anteriores`}</li>
          )}
          {visibleEntries.map((entry) => {
            const state = entry.isCurrent ? "current" : entry.index > status.currentIndex ? "undone" : "done";
            return (
              <li
                key={entry.index}
                className={`version-history__entry version-history__entry--${state}`}
                aria-current={entry.isCurrent ? "step" : undefined}
              >
                <span className="version-history__marker" aria-hidden="true" />
                <span className="version-history__step">{entry.index === 0 ? "Origen" : `Cambio ${entry.index}`}</span>
                <span className="version-history__label">{entry.label}</span>
                {state === "current" && <strong className="version-history__state">Actual</strong>}
                {state === "undone" && <small className="version-history__state">Deshecho · se puede rehacer</small>}
              </li>
            );
          })}
        </ol>
      )}
      {latestChange && (
        <div className="version-history__latest" role="status">
          <span>Último resultado</span>
          <p>{latestChange}</p>
          {status.lastChange != null && <p className="version-history__summary">{formatChangeSummary(status.lastChange)}</p>}
        </div>
      )}
      {status.snapshotsEnabled && (
        <details className="version-history__retention">
          <summary>Espacio del historial</summary>
          <div role="group" aria-label="Uso y retención del historial">
            <p>
              {status.entryCount.toLocaleString()} / {status.maxEntries.toLocaleString()} estados · {formatHistoryBytes(status.diskBytes)} / {formatHistoryBytes(status.diskBudgetBytes)} de historial local
            </p>
            <p>
              Se conservan hasta {status.maxEntries.toLocaleString()} estados o {formatHistoryBytes(status.diskBudgetBytes)} por dataset. Al llegar a un límite, se retiran primero los estados más antiguos del historial y se mantiene el actual. Si un snapshot individual supera el presupuesto, la operación puede quedar sin historial reversible.
            </p>
          </div>
        </details>
      )}
    </div>
  );
}

/** Enough to see where the current version sits; older ones are counted. */
const MAX_VISIBLE_VERSIONS = 12;

function formatHistoryBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "No disponible";
  return formatBytes(bytes);
}

/**
 * AUD-057: the result of the change, with «Deshacer», where the change was made.
 * It is not a live region: the history panel already announces the same text.
 */
export function ChangeResultLine({
  status,
  canUndo,
  busy,
  onUndo,
  onDismiss,
}: {
  status: ChangeStatus;
  canUndo: boolean;
  busy: boolean;
  onUndo: () => void;
  onDismiss: () => void;
}) {
  if (status.kind !== "applied") return null;
  return (
    <div className="change-result" role="group" aria-label="Resultado del último cambio">
      <p>{status.message}</p>
      <div className="change-result__actions">
        {status.unchanged !== true && (
          <button type="button" className="secondary-action" onClick={onUndo} disabled={busy || !canUndo}>
            Deshacer
          </button>
        )}
        <button type="button" className="secondary-action" onClick={onDismiss}>
          Cerrar
        </button>
      </div>
    </div>
  );
}

export function ChangeFeedback({ status, onCancel }: { status: ChangeStatus; onCancel?: () => void }) {
  if (status.kind === "idle" || status.kind === "applied") return null;

  if (status.kind === "working") {
    // REN-14: a long apply shows its step, percent and elapsed time.
    if (status.progress && onCancel) {
      return (
        <OperationProgressView
          progress={status.progress}
          cancellation={status.cancelRequested ? { kind: "requested" } : { kind: "available", onCancel }}
        />
      );
    }
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
