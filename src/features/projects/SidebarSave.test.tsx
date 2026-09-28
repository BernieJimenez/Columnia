import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProjectSummary } from "../../bridge";
import { SidebarSave } from "./SidebarSave";

const project: ProjectSummary = {
  id: "p1", name: "Ventas", datasetFileName: "ventas.csv", rowCount: 1, columnCount: 1,
  createdAt: "2026-09-28T09:00:00Z", updatedAt: "2026-09-28T09:00:00Z",
};

afterEach(cleanup);

describe("SidebarSave (DAT-01)", () => {
  it("saves an unsaved dataset in one click, named after its file", () => {
    const onSave = vi.fn();
    render(<SidebarSave fileName="ventas-2026.csv" activeProject={null} autoSave={{ kind: "disabled" }} operation={{ kind: "idle" }} disabled={false} onSave={onSave} />);

    expect(screen.getByRole("status")).toHaveTextContent("Sin guardar");
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto" }));
    expect(onSave).toHaveBeenCalledWith("ventas-2026");
  });

  it("says when a saved project was last saved and needs no click", () => {
    render(<SidebarSave fileName="ventas.csv" activeProject={project} autoSave={{ kind: "saved", savedAt: "2026-09-28T15:40:00Z" }} operation={{ kind: "idle" }} disabled={false} onSave={vi.fn()} />);

    expect(screen.getByRole("status")).toHaveTextContent(/^Guardado a las \d/);
    expect(screen.queryByRole("button", { name: "Guardar proyecto" })).not.toBeInTheDocument();
  });

  it("offers the button again when autosave is off or failed, keeping the project name", () => {
    const onSave = vi.fn();
    const { rerender } = render(<SidebarSave fileName="ventas.csv" activeProject={project} autoSave={{ kind: "disabled" }} operation={{ kind: "idle" }} disabled={false} onSave={onSave} />);
    expect(screen.getByRole("status")).toHaveTextContent("Guardado automático desactivado");
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto" }));
    expect(onSave).toHaveBeenCalledWith("Ventas");

    rerender(<SidebarSave fileName="ventas.csv" activeProject={project} autoSave={{ kind: "error", message: "disco lleno" }} operation={{ kind: "idle" }} disabled={false} onSave={onSave} />);
    expect(screen.getByRole("status")).toHaveTextContent("No se pudo guardar el último cambio.");
    expect(screen.getByRole("button", { name: "Guardar proyecto" })).toBeEnabled();
  });

  it("shows progress and blocks a second save while saving", () => {
    render(<SidebarSave fileName="ventas.csv" activeProject={null} autoSave={{ kind: "disabled" }} operation={{ kind: "working", operation: "save", projectId: null }} disabled={false} onSave={vi.fn()} />);

    expect(screen.getByRole("status")).toHaveTextContent("Guardando…");
    expect(screen.getByRole("button", { name: "Guardar proyecto" })).toBeDisabled();
  });
});
