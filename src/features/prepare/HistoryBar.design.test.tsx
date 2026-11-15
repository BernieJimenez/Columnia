import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

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

afterEach(cleanup);

describe("HistoryBar design", () => {
  it("avisa de una versión que no se guardó sin ocultar el historial (DAT-07)", () => {
    render(
      <HistoryBar
        status={{
          ...history,
          entries: [...history.entries.slice(0, 2), { id: null, index: 2, label: "Paso grande", isCurrent: true }],
          degradedReason: "El resultado de «Paso grande» no se guardó como versión: deshacer vuelve a «Normalizar nombres».",
        }}
        busy={false}
        onUndo={() => undefined}
        onRedo={() => undefined}
      />,
    );
    expect(screen.getByRole("note")).toHaveTextContent("deshacer vuelve a «Normalizar nombres»");
    expect(screen.getByText(/Versión actual: Paso grande/)).toBeInTheDocument();
  });

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
    // The versions are always in view, oldest first, with the current one marked.
    const versions = screen.getByRole("list", { name: "Versiones del dataset" });
    expect(within(versions).getAllByRole("listitem").map((item) => item.querySelector(".version-history__label")?.textContent))
      .toEqual(["Dataset original", "Normalizar nombres", "Imputación conservadora"]);
    expect(within(versions).getByText("Cambio 2").closest("li")).toHaveAttribute("aria-current", "step");
    // Retention is a detail: closed until asked for.
    const space = screen.getByText("Espacio del historial").closest("details");
    expect(space).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("Espacio del historial"));
    expect(space).toHaveAttribute("open");
    const retention = screen.getByLabelText("Uso y retención del historial");
    expect(retention).toHaveTextContent(/3 \/ 12 estados · 512 B \/ 1[.,]0 KiB de historial local/);
    expect(retention).toHaveTextContent("se retiran primero los estados más antiguos");
    expect(retention).toHaveTextContent("ese cambio puede quedar sin historial reversible");
    expect(retention).not.toHaveTextContent(/snapshot/i);
    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
  });
});

describe("HistoryBar versiones deshechas", () => {
  it("marca las versiones que se pueden rehacer y dice cuándo no hay nada que deshacer", () => {
    const { rerender } = render(
      <HistoryBar
        status={{ ...history, currentIndex: 1, canRedo: true, entries: history.entries.map((entry) => ({ ...entry, isCurrent: entry.index === 1 })) }}
        busy={false}
        onUndo={() => undefined}
        onRedo={() => undefined}
      />,
    );
    expect(screen.getByText("Imputación conservadora").closest("li")).toHaveTextContent("Deshecho · se puede rehacer");
    expect(screen.queryByText("Aún no hay cambios que deshacer.")).not.toBeInTheDocument();

    rerender(
      <HistoryBar
        status={{ ...history, canUndo: false, currentIndex: 0, entryCount: 1, entries: [{ id: "history-test-0", index: 0, label: "Dataset original", isCurrent: true }] }}
        busy={false}
        onUndo={() => undefined}
        onRedo={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "Deshacer" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("Aún no hay cambios que deshacer.")).toBeVisible();

    // Once the oldest versions are retired, the first one kept is not the origin.
    rerender(
      <HistoryBar
        status={{ ...history, currentIndex: 1, entryCount: 2, entries: [
          { id: "history-test-5", index: 0, label: "Rellenar nulos de cpu_usage", isCurrent: false },
          { id: "history-test-6", index: 1, label: "Rellenar nulos de memory_usage", isCurrent: true },
        ] }}
        busy={false}
        onUndo={() => undefined}
        onRedo={() => undefined}
      />,
    );
    expect(screen.queryByText("Origen")).not.toBeInTheDocument();
    expect(screen.getByText("Rellenar nulos de cpu_usage").closest("li")).toHaveTextContent("Versión 1");
    expect(screen.getByText(/El origen y los primeros cambios se retiraron/)).toBeVisible();
  });

  it("pone Deshacer y Rehacer antes de la lista de versiones (AUD-057)", () => {
    render(<HistoryBar status={history} busy={false} onUndo={() => undefined} onRedo={() => undefined} />);

    const undo = screen.getByRole("button", { name: "Deshacer" });
    const list = screen.getByRole("list", { name: "Versiones del dataset" });
    expect(undo.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

describe("HistoryBar resumen común (PROD-10)", () => {
  it("añade al último resultado las celdas nulas, las filas quitadas y si se deshace", () => {
    render(
      <HistoryBar
        status={{ ...history, lastChange: { nulledCellCount: 2, removedRowCount: 1, reversible: true } }}
        busy={false}
        latestChange="Se convirtió 1 columna a número."
        onUndo={() => undefined}
        onRedo={() => undefined}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("2 celdas quedaron nulas · se quitó 1 fila · puedes deshacer el cambio.");
  });
});

describe("HistoryBar foco (ACC)", () => {
  it("Deshacer sigue enfocable mientras trabaja y al terminar el foco pasa a Rehacer", () => {
    const onUndo = vi.fn();
    const { rerender } = render(<HistoryBar status={history} busy={false} onUndo={onUndo} onRedo={() => undefined} />);
    const undo = screen.getByRole("button", { name: "Deshacer" });
    undo.focus();
    fireEvent.click(undo);
    expect(onUndo).toHaveBeenCalledOnce();

    rerender(<HistoryBar status={history} busy onUndo={onUndo} onRedo={() => undefined} />);
    expect(undo).toBeEnabled();
    expect(undo).toHaveAttribute("aria-disabled", "true");
    expect(undo).toHaveAttribute("aria-describedby", "busy-reason");
    expect(undo).toHaveFocus();
    fireEvent.click(undo);
    expect(onUndo).toHaveBeenCalledOnce();

    rerender(
      <HistoryBar
        status={{ ...history, currentIndex: 1, canRedo: true, entries: history.entries.map((entry) => ({ ...entry, isCurrent: entry.index === 1 })) }}
        busy={false}
        onUndo={onUndo}
        onRedo={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "Rehacer" })).toHaveFocus();
  });
});