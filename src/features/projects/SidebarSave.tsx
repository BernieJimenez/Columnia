import type { ProjectSummary } from "../../bridge";
import { suggestedProjectName, type ProjectAutoSaveState, type ProjectOperationState } from "./projectModel";

interface SidebarSaveProps {
  fileName: string;
  activeProject: ProjectSummary | null;
  autoSave: ProjectAutoSaveState;
  operation: ProjectOperationState;
  disabled: boolean;
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
export function SidebarSave({ fileName, activeProject, autoSave, operation, disabled, onSave }: SidebarSaveProps) {
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

  return (
    <div className="sidebar__save">
      <small role="status">{status}</small>
      {needsClick && (
        <button
          type="button"
          className="secondary-action"
          disabled={disabled || saving}
          onClick={() => onSave(activeProject?.name ?? (suggestedProjectName(fileName) || "Proyecto"))}
        >
          Guardar proyecto
        </button>
      )}
    </div>
  );
}
