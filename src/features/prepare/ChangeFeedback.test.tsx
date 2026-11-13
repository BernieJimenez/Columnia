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

  it("la línea de resultado ofrece «Deshacer este cambio» y Cerrar, sin duplicar el anuncio (AUD-057)", () => {
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
    fireEvent.click(screen.getByRole("button", { name: "Deshacer este cambio" }));
    expect(onUndo).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("sin a qué volver, Deshacer sigue enfocable con aria-disabled; si nada cambió, no aparece (AUD-057)", () => {
    const onUndo = vi.fn();
    const { rerender } = render(
      <ChangeResultLine status={{ kind: "applied", message: "Listo." }} canUndo={false} busy={false} onUndo={onUndo} onDismiss={vi.fn()} />,
    );
    const undo = screen.getByRole("button", { name: "Deshacer este cambio" });
    expect(undo).toBeEnabled();
    expect(undo).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(undo);
    expect(onUndo).not.toHaveBeenCalled();

    rerender(
      <ChangeResultLine status={{ kind: "applied", message: "Sin cambios.", unchanged: true }} canUndo busy={false} onUndo={vi.fn()} onDismiss={vi.fn()} />,
    );
    expect(screen.queryByRole("button", { name: /Deshacer/ })).not.toBeInTheDocument();

    rerender(<ChangeResultLine status={{ kind: "idle" }} canUndo busy={false} onUndo={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.queryByLabelText("Resultado del último cambio")).not.toBeInTheDocument();
  });

  it("tras deshacer ofrece «Rehacer» y se lleva el foco cuando se lo piden", () => {
    const onRedo = vi.fn();
    const onFocusHandled = vi.fn();
    render(
      <ChangeResultLine
        status={{ kind: "applied", message: "Se deshizo «Recortar espacios».", origin: "undo" }}
        canUndo
        canRedo
        busy={false}
        focusAction
        onFocusHandled={onFocusHandled}
        onUndo={vi.fn()}
        onRedo={onRedo}
        onDismiss={vi.fn()}
      />,
    );
    const redo = screen.getByRole("button", { name: "Rehacer el cambio deshecho" });
    expect(redo).toHaveFocus();
    expect(onFocusHandled).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: /Deshacer/ })).not.toBeInTheDocument();
    fireEvent.click(redo);
    expect(onRedo).toHaveBeenCalledOnce();
  });

  it("mientras trabaja, la acción no se ejecuta y dice por qué", () => {
    const onUndo = vi.fn();
    render(<ChangeResultLine status={{ kind: "applied", message: "Listo." }} canUndo busy onUndo={onUndo} onDismiss={vi.fn()} />);
    const undo = screen.getByRole("button", { name: "Deshacer este cambio" });
    expect(undo).toHaveAttribute("aria-disabled", "true");
    expect(undo).toHaveAttribute("aria-describedby", "busy-reason");
    fireEvent.click(undo);
    expect(onUndo).not.toHaveBeenCalled();
  });
});