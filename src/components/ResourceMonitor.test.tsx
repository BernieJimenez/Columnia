import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ResourceMonitor } from "./ResourceMonitor";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ResourceMonitor", () => {
  it("muestra CPU y RAM de Columnia y del equipo con nombres accesibles", async () => {
    const fetchUsage = vi.fn().mockResolvedValue({
      processCpuPercentage: 12.5,
      systemCpuPercentage: 1.2,
      processMemoryBytes: 120 * 1024 * 1024,
      systemMemoryUsedBytes: 8.9 * 1024 * 1024 * 1024,
      systemMemoryTotalBytes: 31.9 * 1024 * 1024 * 1024,
    });

    render(<ResourceMonitor enabled fetchUsage={fetchUsage} />);

    await waitFor(() => expect(screen.getByText("120 MB")).toBeInTheDocument());
    expect(screen.getByText("12.5%")).toBeInTheDocument();
    expect(screen.getByText("1.2%")).toBeInTheDocument();
    expect(screen.getByText("8.9 / 31.9 GB")).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "CPU de Columnia: 12.5%" })).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "RAM del equipo: 8.9 / 31.9 GB" })).toBeInTheDocument();
    expect(fetchUsage).toHaveBeenCalledTimes(1);
  });

  it("no intenta consultar recursos fuera del runtime de escritorio", () => {
    const fetchUsage = vi.fn();

    render(<ResourceMonitor enabled={false} fetchUsage={fetchUsage} />);

    expect(screen.getByText("Disponible en la app de escritorio")).toBeInTheDocument();
    expect(fetchUsage).not.toHaveBeenCalled();
  });
});

