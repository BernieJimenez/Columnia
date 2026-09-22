import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { HistoryState } from "../../bridge";
import { HistoryBar } from "./HistoryBar";

const history: HistoryState = {
  canUndo: true,
  canRedo: false,
  currentIndex: 2,
  entryCount: 3,
  entries: [
    { id: "history-test-0", index: 0, label: "Dataset original", isCurrent: false },
    { id: "history-test-1", index: 1, label: "Normalizar nombres", isCurrent: false },
    { id: "history-test-2", index: 2, label: "Imputación conservadora", isCurrent: true },
  ],
  snapshotsEnabled: true,
  degradedReason: null,
  maxEntries: 12,
  diskBytes: 512,
  diskBudgetBytes: 1024,
};

describe("HistoryBar design", () => {
  it("mantiene compacto el historial y permite desplegar cambios reversibles", () => {
    const onUndo = vi.fn();
    render(
      <HistoryBar
        status={history}
        busy={false}
        latestChange="Se imputaron 3 valores nulos en: monto."
        onUndo={onUndo}
        onRedo={() => undefined}
      />,
    );

    expect(screen.getByText("Último resultado")).toBeInTheDocument();
    expect(screen.getByText("Se imputaron 3 valores nulos en: monto.")).toBeInTheDocument();
    const changes = screen.getByText("Cambios realizados (2)").closest("details");
    expect(changes).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("Cambios realizados (2)"));
    expect(changes).toHaveAttribute("open");
    expect(screen.getByText("Cambio 2")).toBeInTheDocument();
    expect(screen.getByText("Imputación conservadora")).toBeInTheDocument();
    const retention = screen.getByLabelText("Uso y retención del historial");
    expect(retention).toHaveTextContent("3 / 12 estados · 512 B / 1 KiB de historial local");
    fireEvent.click(screen.getByText("Política local"));
    expect(retention).toHaveTextContent("se retiran primero los estados más antiguos");
    expect(retention).toHaveTextContent("la operación puede quedar sin historial reversible");
    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
  });
});
