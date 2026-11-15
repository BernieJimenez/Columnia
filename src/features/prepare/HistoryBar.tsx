import { useEffect, useRef } from "react";

import type { HistoryState } from "../../bridge";
import { busyDescribedBy } from "../../busyReasons";
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
  const undoBlocked = busy || !status.snapshotsEnabled || !status.canUndo;
  const redoBlocked = busy || !status.snapshotsEnabled || !status.canRedo;
  const undoRef = useRef<HTMLButtonElement>(null);
  const redoRef = useRef<HTMLButtonElement>(null);
  // ACC: the pressed button stays focusable while the change runs
  // (aria-disabled, not disabled); when it ends, the focus moves to the
  // opposite action, so «Rehacer» is one key away after «Deshacer».
  const focusAfter = useRef<"undo" | "redo" | null>(null);

  useEffect(() => {
    if (busy || focusAfter.current === null) return;
    const target = focusAfter.current === "redo"
      ? (status.canRedo ? redoRef : undoRef)
      : (status.canUndo ? undoRef : redoRef);
    focusAfter.current = null;
    target.current?.focus();
  }, [busy, status.canUndo, status.canRedo]);

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
        <button
          ref={undoRef}
          type="button"
          className="secondary-action"
          aria-disabled={undoBlocked || undefined}
          aria-describedby={busyDescribedBy(busy)}
          onClick={() => {
            if (undoBlocked) return;
            focusAfter.current = "redo";
            onUndo();
          }}
        >
          Deshacer
        </button>
        <button
          ref={redoRef}
          type="button"
          className="secondary-action"
          aria-disabled={redoBlocked || undefined}
          aria-describedby={busyDescribedBy(busy)}
          onClick={() => {
            if (redoBlocked) return;
            focusAfter.current = "undo";
            onRedo();
          }}
        >
          Rehacer
        </button>
      </div>
      {nothingToUndo && <p className="version-history__hint">Aún no hay cambios que deshacer.</p>}
      {status.snapshotsEnabled && visibleEntries.length > 0 && (
        <ol className="version-history__list" aria-label="Versiones del dataset">
          {hiddenCount > 0 && (
            <li className="version-history__more">{hiddenCount === 1 ? "1 versión anterior" : `${hiddenCount.toLocaleString("es-ES")} versiones anteriores`}</li>
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
              {status.entryCount.toLocaleString("es-ES")} / {status.maxEntries.toLocaleString("es-ES")} estados · {formatHistoryBytes(status.diskBytes)} / {formatHistoryBytes(status.diskBudgetBytes)} de historial local
            </p>
            <p>
              Se conservan hasta {status.maxEntries.toLocaleString("es-ES")} estados o {formatHistoryBytes(status.diskBudgetBytes)} por dataset. Al llegar a un límite, se retiran primero los estados más antiguos del historial y se mantiene el actual. Si la copia de un solo estado supera el espacio disponible, ese cambio puede quedar sin historial reversible.
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
 * AUD-057: the result of the change where the change was made, with the way
 * back: «Deshacer» after a change, «Rehacer» after undoing one. It is not a
 * live region: the history panel already announces the same text.
 */
export function ChangeResultLine({
  status,
  canUndo,
  canRedo = false,
  busy,
  focusAction = false,
  onFocusHandled,
  onUndo,
  onRedo,
  onDismiss,
}: {
  status: ChangeStatus;
  canUndo: boolean;
  canRedo?: boolean;
  busy: boolean;
  /** Move the focus here when the line appears (after «Deshacer» elsewhere). */
  focusAction?: boolean;
  onFocusHandled?: () => void;
  onUndo: () => void;
  onRedo?: () => void;
  onDismiss: () => void;
}) {
  const groupRef = useRef<HTMLDivElement>(null);
  const actionRef = useRef<HTMLButtonElement>(null);
  const applied = status.kind === "applied";
  useEffect(() => {
    if (!focusAction || !applied) return;
    (actionRef.current ?? groupRef.current)?.focus();
    onFocusHandled?.();
  }, [focusAction, applied, onFocusHandled]);

  if (status.kind !== "applied") return null;
  const undone = status.origin === "undo";
  const action = undone
    ? onRedo && { label: "Rehacer", name: "Rehacer el cambio deshecho", available: canRedo, run: onRedo }
    : status.unchanged !== true && { label: "Deshacer", name: "Deshacer este cambio", available: canUndo, run: onUndo };
  const blocked = !action || busy || !action.available;
  return (
    <div ref={groupRef} tabIndex={-1} className="change-result" role="group" aria-label="Resultado del último cambio">
      <p>{status.message}</p>
      <div className="change-result__actions">
        {action && (
          <button
            ref={actionRef}
            type="button"
            className="secondary-action"
            aria-label={action.name}
            aria-disabled={blocked || undefined}
            aria-describedby={busyDescribedBy(busy)}
            onClick={() => { if (!blocked) action.run(); }}
          >
            {action.label}
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
