import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ResourceMonitor } from "./ResourceMonitor";

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  vi.restoreAllMocks();
});

describe("ResourceMonitor", () => {
  it("muestra CPU y RAM de Columnia y del equipo con nombres accesibles", async () => {
    const fetchUsage = vi.fn().mockResolvedValue({
      processCpuPercentage: 12.5,
      systemCpuPercentage: 1.2,
      logicalCpuCount: 12,
      processMemoryBytes: 120 * 1024 * 1024,
      systemMemoryUsedBytes: 8.9 * 1024 * 1024 * 1024,
      systemMemoryTotalBytes: 31.9 * 1024 * 1024 * 1024,
    });

    render(<ResourceMonitor enabled fetchUsage={fetchUsage} />);

    await waitFor(() => expect(screen.getByText("120 MB")).toBeInTheDocument());
    expect(screen.getByText("0.1 / 12 hilos")).toBeInTheDocument();
    expect(screen.getByText("1.2%")).toBeInTheDocument();
    expect(screen.getByText("8.9 / 31.9 GB")).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "CPU de Columnia: 0.1 / 12 hilos" })).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "RAM de Columnia: 120 MB" })).toBeInTheDocument();
    expect(screen.getByText("no disponible")).toBeInTheDocument();
    expect(screen.getByText("No disponible")).toBeInTheDocument();
    expect(fetchUsage).toHaveBeenCalledTimes(1);
  });

  it("muestra memoria disponible y métricas GPU solo cuando el backend las confirma", async () => {
    const fetchUsage = vi.fn().mockResolvedValue({
      processCpuPercentage: 0,
      systemCpuPercentage: 0,
      logicalCpuCount: 8,
      processMemoryBytes: 2 * 1024 * 1024 * 1024,
      systemMemoryUsedBytes: 12 * 1024 * 1024 * 1024,
      systemMemoryTotalBytes: 32 * 1024 * 1024 * 1024,
      systemMemoryAvailableBytes: 20 * 1024 * 1024 * 1024,
      gpu: {
        status: "available",
        usagePercentage: 42.5,
        memoryUsedBytes: 512 * 1024 * 1024,
        memoryTotalBytes: 4 * 1024 * 1024 * 1024,
        reason: null,
      },
    });

    render(<ResourceMonitor enabled fetchUsage={fetchUsage} />);

    await waitFor(() => expect(screen.getByText("20.0 GB")).toBeInTheDocument());
    expect(screen.getByText("42.5%")).toBeInTheDocument();
    expect(screen.getByText("Activa")).toBeInTheDocument();
  });

  it("no intenta consultar recursos fuera del runtime de escritorio", () => {
    const fetchUsage = vi.fn();

    render(<ResourceMonitor enabled={false} fetchUsage={fetchUsage} />);

    expect(screen.getByText("Solo escritorio")).toHaveAccessibleName(
      "Disponible en la app de escritorio",
    );
    expect(fetchUsage).not.toHaveBeenCalled();
  });

  it("aplica el perfil de concurrencia elegido antes de la siguiente operación", async () => {
    const fetchUsage = vi.fn().mockResolvedValue({
      processCpuPercentage: 0,
      systemCpuPercentage: 0,
      logicalCpuCount: 8,
      processMemoryBytes: 0,
      systemMemoryUsedBytes: 0,
      systemMemoryTotalBytes: 1,
    });
    const settings = {
      requestedProfile: "balanced" as const,
      activeProfile: "balanced" as const,
      requestedThreads: 4,
      activeThreads: 4,
      applied: true,
      locked: false,
      reason: null,
    };
    const fetchPerformanceSettings = vi.fn().mockResolvedValue(settings);
    const setPerformanceProfile = vi.fn().mockResolvedValue({
      ...settings,
      requestedProfile: "maximum" as const,
      activeProfile: "maximum" as const,
      requestedThreads: 8,
      activeThreads: 8,
    });

    render(
      <ResourceMonitor
        enabled
        fetchUsage={fetchUsage}
        fetchPerformanceSettings={fetchPerformanceSettings}
        setPerformanceProfile={setPerformanceProfile}
      />,
    );

    const select = await screen.findByLabelText("Modo de rendimiento");
    expect(select).toHaveValue("balanced");
    expect(setPerformanceProfile).not.toHaveBeenCalled();
    fireEvent.change(select, { target: { value: "maximum" } });

    await waitFor(() => expect(setPerformanceProfile).toHaveBeenCalledWith("maximum"));
    expect(await screen.findByText("Activo: 8 hilos")).toBeInTheDocument();
    expect(window.localStorage.getItem("columnia.performance-profile")).toBe("maximum");
  });
});
