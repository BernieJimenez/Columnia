import { useId } from "react";
import type { ProjectSummary } from "../../bridge";
import { suggestedProjectName, type ProjectAutoSaveState, type ProjectOperationState } from "./projectModel";

interface SidebarSaveProps {
  fileName: string;
  activeProject: ProjectSummary | null;
  autoSave: ProjectAutoSaveState;
  operation: ProjectOperationState;
  disabled: boolean;
  /** What the app is waiting for («el análisis de calidad»); explains why saving is not available yet. */
  busyReason?: string | null;
  onSave: (name: string) => void;
}

function savedTime(value: string): string {
  const parsed = new Date(value);
  return Number.isNaN(parsed.valueOf())
    ? value
    : new Intl.DateTimeFormat("es", { timeStyle: "short" }).format(parsed);
}

/**
 * Saving from any phase (DAT-01): the first click saves the dataset as a
 * project named after its file; after that, changes are saved on their own.
 */
export function SidebarSave({ fileName, activeProject, autoSave, operation, disabled, busyReason, onSave }: SidebarSaveProps) {
  const statusId = useId();
  const saving = autoSave.kind === "saving" || (operation.kind === "working" && operation.operation === "save");
  const status = saving
    ? "Guardando…"
    : autoSave.kind === "saved"
      ? `Guardado a las ${savedTime(autoSave.savedAt)}`
      : autoSave.kind === "error"
        ? "No se pudo guardar el último cambio."
        : activeProject
          ? autoSave.kind === "disabled" ? "Guardado automático desactivado" : "Los cambios se guardan solos"
          : "Sin guardar";
  const needsClick = !activeProject || autoSave.kind === "disabled" || autoSave.kind === "error";
  const waiting = needsClick && disabled && !saving;
  const blockedBy = waiting && busyReason ? busyReason : null;
  // The visible text stays short so the top bar keeps its height (AUD-117); the whole reason goes in the title and in #busy-reason.
  const statusText = blockedBy && !activeProject ? `${status} · en espera` : status;
  const fullReason = blockedBy ? `${status} · disponible al terminar ${blockedBy}` : undefined;

  return (
    <div className="dataset-save">
      <small id={statusId} role="status" title={fullReason} className={`dataset-save__status dataset-save__status--${saving ? "saving" : autoSave.kind === "error" ? "error" : activeProject ? "saved" : "unsaved"}`}>{statusText}</small>
      {needsClick && (
        <button
          type="button"
          className="secondary-action"
          disabled={disabled || saving}
          aria-describedby={waiting ? (blockedBy ? `${statusId} busy-reason` : statusId) : undefined}
          onClick={() => onSave(activeProject?.name ?? (suggestedProjectName(fileName) || "Proyecto"))}
        >
          Guardar proyecto
        </button>
      )}
    </div>
  );
}
