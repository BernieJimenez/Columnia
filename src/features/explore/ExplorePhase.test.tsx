import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, ExplorePanel } from "../../bridge";
import { ExplorePhase } from "./ExplorePhase";
import { filterLabel, isRangeSelected, isValueSelected, kpiLabel, toggleRange, toggleValue } from "./exploreModel";

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
    expect(getPanel).toHaveBeenCalledWith([]);

    fireEvent.click(within(chart).getByRole("button", { name: /FL/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([{ column: "estado", values: ["FL"] }]));
    expect(await screen.findByText("de 8")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quitar filtro estado: FL" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /precio de 450 a 800/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([
      { column: "estado", values: ["FL"] },
      { column: "precio", range: { min: 450, max: 800 } },
    ]));
  });

  it("waits for the analysis and explains a failure", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockRejectedValue(new Error("sin perfil"));
    const view = render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("Analizando la calidad");
    expect(getPanel).not.toHaveBeenCalled();

    view.rerender(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo preparar el panel: sin perfil");
  });
});
