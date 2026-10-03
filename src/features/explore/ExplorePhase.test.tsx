import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, ExplorePanel } from "../../bridge";
import { ExplorePhase } from "./ExplorePhase";
import {
  axisTicks,
  filterLabel,
  isPeriodSelected,
  isRangeSelected,
  isValueSelected,
  kpiLabel,
  toggleChart,
  toggleExpanded,
  togglePeriod,
  toggleRange,
  toggleValue,
} from "./exploreModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "ventas.csv",
  fileSizeBytes: 100,
  rowCount: 8,
  columnCount: 3,
  columns: [],
  rows: [],
};

const panel = (rowCount: number): ExplorePanel => ({
  rowCount,
  totalRowCount: 8,
  kpis: [
    { kind: "count", column: null, value: rowCount },
    { kind: "median", column: "precio", value: 450 },
  ],
  categories: [
    { column: "estado", bars: [{ value: "CA", count: 3 }, { value: "FL", count: 3 }, { value: null, count: 2 }], otherCount: 0, distinctCount: 3 },
  ],
  options: { categories: ["estado", "ciudad"], measures: ["precio", "metros"], dates: ["fecha", "alta"] },
  histogram: { column: "precio", bins: [{ lower: 100, upper: 450, count: 4 }, { lower: 450, upper: 800, count: 4 }] },
  trend: { column: "fecha", granularity: "month", points: [{ period: "2025-01", count: 2 }, { period: "2025-02", count: 6 }] },
});

describe("exploreModel", () => {
  it("keeps one filter per column and toggles its values", () => {
    let filters = toggleValue([], "estado", "FL");
    filters = toggleValue(filters, "estado", null);
    expect(filters).toEqual([{ column: "estado", values: ["FL", null] }]);
    expect(isValueSelected(filters, "estado", null)).toBe(true);
    expect(filterLabel(filters[0])).toBe("estado: FL, Sin dato");
    filters = toggleValue(toggleValue(filters, "estado", "FL"), "estado", null);
    expect(filters).toEqual([]);
  });

  it("replaces the histogram range and clears it on a second click", () => {
    let filters = toggleRange([], "precio", 100, 450);
    filters = toggleRange(filters, "precio", 450, 800);
    expect(filters).toEqual([{ column: "precio", range: { min: 450, max: 800 } }]);
    expect(isRangeSelected(filters, "precio", 450, 800)).toBe(true);
    expect(toggleRange(filters, "precio", 450, 800)).toEqual([]);
  });

  it("filters by one trend period, picks charts and spaces the axis", () => {
    let filters = togglePeriod([], "fecha", "2025-01");
    filters = togglePeriod(filters, "fecha", "2025-02");
    expect(filters).toEqual([{ column: "fecha", period: "2025-02" }]);
    expect(isPeriodSelected(filters, "fecha", "2025-02")).toBe(true);
    expect(filterLabel(filters[0])).toBe("fecha: 2025-02");
    expect(togglePeriod(filters, "fecha", "2025-02")).toEqual([]);

    expect(toggleChart({}, ["estado"], "ciudad")).toEqual({ categories: ["estado", "ciudad"] });
    expect(toggleChart({ categories: ["estado", "ciudad"] }, [], "estado")).toEqual({ categories: ["ciudad"] });
    expect(toggleChart({}, ["a", "b", "c", "d", "e", "f"], "g").categories).toHaveLength(6);
    expect(toggleExpanded({}, "estado")).toEqual({ expanded: ["estado"] });
    expect(toggleExpanded({ expanded: ["estado"] }, "estado")).toEqual({ expanded: [] });
    expect(axisTicks(0, 100)).toEqual([0, 25, 50, 75, 100]);
  });

  it("names the indicators in plain words", () => {
    expect(kpiLabel({ kind: "count", column: null, value: 1 })).toBe("Filas");
    expect(kpiLabel({ kind: "mean", column: "sqfeet", value: 1 })).toBe("Media de sqfeet");
  });
});

describe("ExplorePhase", () => {
  it("shows the panel and filters every chart when a bar is pressed", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel")
      .mockResolvedValueOnce(panel(8))
      .mockResolvedValueOnce(panel(3));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);

    const chart = await screen.findByRole("region", { name: "Filas por estado" });
    expect(within(chart).getByRole("button", { name: /Sin dato/ })).toBeInTheDocument();
    expect(screen.getByText("Mediana de precio")).toBeInTheDocument();
    expect(screen.getByText("Pulsa una barra para filtrar todo el panel.")).toBeInTheDocument();
    expect(getPanel).toHaveBeenCalledWith([], {});

    fireEvent.click(within(chart).getByRole("button", { name: /FL/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([{ column: "estado", values: ["FL"] }], {}));
    expect(await screen.findByText("de 8")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quitar filtro estado: FL" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /precio de 450 a 800/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([
      { column: "estado", values: ["FL"] },
      { column: "precio", range: { min: 450, max: 800 } },
    ], {}));
  });

  it("lets the person choose the charts and go back to the automatic panel", async () => {
    const many: ExplorePanel = {
      ...panel(8),
      categories: [{ column: "estado", bars: [{ value: "CA", count: 3 }], otherCount: 5, distinctCount: 14 }],
    };
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(many);
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);

    const customize = await screen.findByRole("button", { name: "Personalizar" });
    expect(customize).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText(/Columnia eligió estos gráficos/)).toBeInTheDocument();
    fireEvent.click(customize);
    expect(screen.getByRole("checkbox", { name: "estado" })).toBeChecked();

    fireEvent.click(screen.getByRole("checkbox", { name: "ciudad" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], { categories: ["estado", "ciudad"] }));
    expect(screen.getByText(/Tú elegiste estos gráficos/)).toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox", { name: "Medida" }), { target: { value: "metros" } });
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], { categories: ["estado", "ciudad"], measure: "metros" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Fecha" }), { target: { value: "alta" } });
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], { categories: ["estado", "ciudad"], measure: "metros", date: "alta" }));

    fireEvent.click(screen.getByRole("button", { name: "Volver a lo automático" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], {}));

    // «Otros 13 valores» opens every value of that chart, and closes again.
    expect(screen.getByText(/Otros 13 valores: 5 filas/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ver todos los valores de estado" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], { expanded: ["estado"] }));
    fireEvent.click(screen.getByRole("button", { name: "Ver menos los valores de estado" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], { expanded: [] }));

    // The trend filters like any other chart.
    fireEvent.click(screen.getByRole("button", { name: "2025-02: 6 filas" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([{ column: "fecha", period: "2025-02" }], { expanded: [] }));
    expect(screen.getByRole("button", { name: "Quitar filtro fecha: 2025-02" })).toBeInTheDocument();
  });

  it("waits for the analysis and explains a failure", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockRejectedValue(new Error("sin perfil"));
    const view = render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("Analizando la calidad");
    expect(getPanel).not.toHaveBeenCalled();

    view.rerender(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo preparar el panel: sin perfil");

    getPanel.mockResolvedValueOnce(panel(8));
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(await screen.findByRole("region", { name: "Filas por estado" })).toBeInTheDocument();
    expect(getPanel).toHaveBeenCalledTimes(2);
  });

  it("filters a histogram bar as it counts it: [a, b) except the last bar", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);

    fireEvent.click(await screen.findByRole("button", { name: /precio de 100 a 450/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([
      { column: "precio", range: { min: 100, max: 450, exclusiveMax: true } },
    ], {}));
  });

  it("keeps the filters on screen after a failure so they can be removed (UX-01)", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel")
      .mockResolvedValueOnce(panel(8))
      .mockRejectedValueOnce(new Error("columna renombrada"))
      .mockResolvedValueOnce(panel(8));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);

    const chart = await screen.findByRole("region", { name: "Filas por estado" });
    fireEvent.click(within(chart).getByRole("button", { name: /FL/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("columna renombrada");
    expect(screen.getByRole("button", { name: "Quitar filtro estado: FL" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Quitar filtros y volver a lo automático" }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], {}));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });
});
