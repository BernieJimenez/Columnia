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
    { index: 0, label: "Dataset original", isCurrent: false },
    { index: 1, label: "Normalizar nombres", isCurrent: false },
    { index: 2, label: "Imputación conservadora", isCurrent: true },
  ],
  snapshotsEnabled: true,
  degradedReason: null,
  maxEntries: 12,
  diskBytes: 512,
  diskBudgetBytes: 1024,
};

describe("HistoryBar design", () => {
  it("explica el último resultado y mantiene visibles los cambios reversibles", () => {
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
    expect(screen.getByText("Cambios realizados (2)")).toBeInTheDocument();
    expect(screen.getByText("Cambio 2")).toBeInTheDocument();
    expect(screen.getByText("Imputación conservadora")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
  });
});
