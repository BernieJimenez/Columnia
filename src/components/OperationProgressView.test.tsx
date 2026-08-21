import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OperationProgressView } from "./OperationProgressView";

afterEach(cleanup);

describe("OperationProgressView", () => {
  it("anuncia el progreso y permite cancelar", () => {
    const onCancel = vi.fn();
    render(
      <OperationProgressView
        progress={{ operation: "load", stage: "Leyendo columnas", percent: 35 }}
        cancellation={{ kind: "available", onCancel }}
      />,
    );

    expect(screen.getByRole("status")).toHaveAttribute("aria-atomic", "true");
    expect(screen.getByRole("progressbar", { name: "Progreso: Leyendo columnas" })).toHaveValue(35);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("deshabilita la cancelación repetida", () => {
    render(
      <OperationProgressView
        progress={{ operation: "export", stage: "Cancelando", percent: 80 }}
        cancellation={{ kind: "requested" }}
      />,
    );

    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();
  });
});
