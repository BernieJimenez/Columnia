import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ChangeFeedback, ChangeResultLine } from "./HistoryBar";

afterEach(cleanup);

describe("ChangeFeedback", () => {
  it("muestra el paso y el avance de un plan largo y permite cancelarlo (REN-14)", () => {
    const onCancel = vi.fn();
    render(
      <ChangeFeedback
        status={{
          kind: "working",
          action: "safe",
          progress: { operation: "prepare", stage: "Escribiendo la versión corregida", percent: 42 },
        }}
        onCancel={onCancel}
      />,
    );

    expect(screen.getByRole("heading", { name: "Aplicando cambios" })).toBeInTheDocument();
    expect(screen.getByText("Escribiendo la versión corregida")).toBeInTheDocument();
    expect(screen.getByText("42%")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("mantiene el aviso breve mientras el plan no informa su avance", () => {
    render(<ChangeFeedback status={{ kind: "working", action: "safe" }} onCancel={vi.fn()} />);

    expect(screen.queryByRole("heading", { name: "Aplicando cambios" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
  });

  it("la línea de resultado ofrece Deshacer y Cerrar, sin duplicar el anuncio (AUD-057)", () => {
    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ChangeResultLine
        status={{ kind: "applied", message: "Se eliminó 1 fila vacía." }}
        canUndo
        busy={false}
        onUndo={onUndo}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText("Se eliminó 1 fila vacía.")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("la línea de resultado deshabilita Deshacer si no hay a qué volver y lo omite si nada cambió (AUD-057)", () => {
    const { rerender } = render(
      <ChangeResultLine status={{ kind: "applied", message: "Listo." }} canUndo={false} busy={false} onUndo={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeDisabled();

    rerender(
      <ChangeResultLine status={{ kind: "applied", message: "Sin cambios.", unchanged: true }} canUndo busy={false} onUndo={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.queryByRole("button", { name: "Deshacer" })).not.toBeInTheDocument();

    rerender(<ChangeResultLine status={{ kind: "idle" }} canUndo busy={false} onUndo={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.queryByLabelText("Resultado del último cambio")).not.toBeInTheDocument();
  });
});
