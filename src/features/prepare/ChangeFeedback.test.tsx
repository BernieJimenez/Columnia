import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ChangeFeedback } from "./HistoryBar";

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
});
