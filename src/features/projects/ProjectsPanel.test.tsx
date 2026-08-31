import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProjectSummary } from "../../bridge";
import { ProjectsPanel } from "./ProjectsPanel";

afterEach(cleanup);

const recovery: ProjectSummary = {
  id: "recovery-id",
  name: "Ventas recuperables",
  datasetFileName: "ventas.csv",
  rowCount: 10,
  columnCount: 2,
  createdAt: "2026-08-20T10:00:00Z",
  updatedAt: "2026-08-21T10:00:00Z",
};

function renderPanel(overrides: Partial<ComponentProps<typeof ProjectsPanel>> = {}) {
  const props: ComponentProps<typeof ProjectsPanel> = {
    catalog: { kind: "ready", projects: [recovery], recoveryCandidate: recovery },
    operation: { kind: "idle" },
    deletion: { kind: "idle" },
    activeProject: null,
    datasetFileName: "actual.csv",
    disabled: false,
    onSave: vi.fn(),
    onOpen: vi.fn(),
    onDeleteRequest: vi.fn(),
    onDeleteCancel: vi.fn(),
    onDeleteConfirm: vi.fn(),
    onRetry: vi.fn(),
    onClearFeedback: vi.fn(),
    ...overrides,
  };
  render(<ProjectsPanel {...props} />);
  return props;
}

describe("ProjectsPanel", () => {
  it("destaca la recuperación y explica el estado durable del proyecto", () => {
    const props = renderPanel();
    expect(screen.getByText(/perfil calculado y el historial reversible/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Recuperar proyecto" }));
    expect(props.onOpen).toHaveBeenCalledWith("recovery-id");
    expect(screen.queryByText(/C:\\/)).not.toBeInTheDocument();
  });

  it("guarda el nombre propuesto y exige confirmación explícita al eliminar", () => {
    const props = renderPanel();
    expect(screen.getByRole("textbox", { name: "Nombre del proyecto" })).toHaveValue("actual");
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    expect(props.onSave).toHaveBeenCalledWith("actual");
    fireEvent.click(screen.getByRole("button", { name: "Eliminar" }));
    expect(props.onDeleteRequest).toHaveBeenCalledWith(recovery);
  });

  it("presenta la eliminación como alertdialog y conserva el dataset en memoria", () => {
    const props = renderPanel({ deletion: { kind: "confirming", project: recovery } });
    expect(screen.getByRole("alertdialog", { name: "Eliminar “Ventas recuperables”" })).toHaveTextContent(
      "El dataset abierto en memoria no se descartará.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Eliminar proyecto" }));
    expect(props.onDeleteConfirm).toHaveBeenCalledOnce();
  });
});
