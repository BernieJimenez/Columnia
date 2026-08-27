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
    expect(screen.getByRole("heading", { name: "Cargando dataset" })).toBeInTheDocument();
    expect(screen.getByText("Importación")).toBeInTheDocument();
    expect(screen.getByText("Etapa actual")).toBeInTheDocument();
    expect(screen.getByText("En curso")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "true");
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
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "false");
    expect(screen.getByText("Cancelación solicitada")).toBeInTheDocument();
  });

  it("mantiene el porcentaje dentro de los límites visuales", () => {
    render(
      <OperationProgressView
        progress={{ operation: "profile", stage: "Analizando columnas", percent: 140 }}
        cancellation={{ kind: "available", onCancel: vi.fn() }}
      />,
    );

    expect(screen.getByRole("heading", { name: "Analizando calidad" })).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Progreso: Analizando columnas" })).toHaveValue(100);
    expect(screen.getByText("100%", { selector: ".operation-progress__percent" })).toBeInTheDocument();
  });

  it("muestra el tiempo transcurrido para operaciones largas", async () => {
    vi.useFakeTimers();
    try {
      render(
        <OperationProgressView
          progress={{ operation: "profile", stage: "Analizando columnas", percent: 40 }}
          cancellation={{ kind: "available", onCancel: vi.fn() }}
        />,
      );

      expect(screen.getByLabelText("Tiempo transcurrido: 00:00")).toBeInTheDocument();
      await vi.advanceTimersByTimeAsync(65_000);
      expect(screen.getByLabelText("Tiempo transcurrido: 01:05")).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });
});
