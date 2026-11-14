import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, ExplorePanel } from "../../bridge";
import { ExplorePhase } from "./ExplorePhase";
import {
  axisTicks,
  binLabel,
  filterLabel,
  ignoredRowsText,
  isPeriodSelected,
  isRangeSelected,
  isValueSelected,
  kpiLabel,
  kpiValue,
  toggleChart,
  toggleExpanded,
  togglePeriod,
  toggleRange,
  toggleValue,
  valueLabel,
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
  options: { categories: ["estado", "ciudad"], measures: ["precio", "metros"], dates: ["fecha", "alta"], textDates: [] },
  histogram: { column: "precio", bins: [{ lower: 100, upper: 450, count: 4 }, { lower: 450, upper: 800, count: 4 }], ignoredCount: 0, integer: false },
  trend: { column: "fecha", granularity: "month", points: [{ period: "2025-01", count: 2 }, { period: "2025-02", count: 6 }] },
});

describe("exploreModel", () => {
  it("keeps one filter per column and toggles its values", () => {
    let filters = toggleValue([], "estado", "FL");
    filters = toggleValue(filters, "estado", null);
    expect(filters).toEqual([{ column: "estado", values: ["FL", null] }]);
    expect(isValueSelected(filters, "estado", null)).toBe(true);
    expect(filterLabel(filters[0])).toBe("estado: FL, nulo");
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
    const bins = (edges: number[]) => edges.slice(0, -1).map((lower, index) => ({ lower, upper: edges[index + 1]! }));
    // Few bins: every edge. Many: five marks, each on a real edge, spread by position.
    expect(axisTicks(bins([0, 100, 200])).map((tick) => tick.value)).toEqual([0, 100, 200]);
    const many = axisTicks(bins([0, 50, 100, 150, 200, 250, 300, 350, 400, 450, 500]));
    expect(many.map((tick) => tick.value)).toEqual([0, 150, 250, 400, 500]);
    expect(many.map((tick) => tick.position)).toEqual([0, 0.3, 0.5, 0.8, 1]);
    expect(axisTicks([])).toEqual([]);
  });

  it("labels whole-number bins and the rows a measure leaves out (TXT-06, FUN-78)", () => {
    expect(binLabel(1925, 1930, true, false, true)).toBe("1925 a 1929");
    expect(binLabel(1995, 1999, true, true, true)).toBe("1995 a 1999");
    expect(binLabel(3, 4, true, false, false)).toBe("3");
    expect(ignoredRowsText(0)).toBeNull();
    expect(ignoredRowsText(1)).toBe("1 fila sin número no cuenta");
  });

  it("writes a range filter like the histogram bin it came from", () => {
    const histogram = { column: "precio", bins: [{ lower: 100, upper: 150, count: 1 }, { lower: 150, upper: 200, count: 1 }], ignoredCount: 0, integer: true };
    expect(filterLabel({ column: "precio", range: { min: 100, max: 150, exclusiveMax: true } }, histogram)).toBe("precio: 100 a 149");
    expect(filterLabel({ column: "precio", range: { min: 150, max: 200 } }, histogram)).toBe("precio: 150 a 200");
    const years = { column: "anio", bins: [{ lower: 1925, upper: 1930, count: 1 }, { lower: 1930, upper: 1934, count: 1 }], ignoredCount: 0, integer: true };
    expect(filterLabel({ column: "anio", range: { min: 1925, max: 1930, exclusiveMax: true } }, years)).toBe("anio: 1925 a 1929");
    expect(filterLabel({ column: "x", range: { min: 1, max: 2 } })).toBe("x: 1 a 2");
    expect(kpiValue({ kind: "median", column: "precio", value: null })).toBe("sin valores numéricos");
    expect(valueLabel(null)).toBe("nulo");
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
    expect(within(chart).getByRole("button", { name: /nulo: 2 filas/ })).toBeInTheDocument();
    // AUD-070: the value is readable in full on hover even when the label is cut; "nulo" is never cut, so it carries no title.
    expect(within(chart).getByText("CA")).toHaveAttribute("title", "CA");
    expect(within(chart).getByText("nulo")).not.toHaveAttribute("title");
    expect(screen.getByText("Mediana de precio")).toBeInTheDocument();
    expect(screen.getByText("Cada barra cuenta las filas de un valor; pulsa una para filtrar todo el panel.")).toBeInTheDocument();
    expect(screen.queryByText("Filas por valor")).not.toBeInTheDocument();
    expect(getPanel).toHaveBeenCalledWith([], {});

    fireEvent.click(within(chart).getByRole("button", { name: /FL/ }));
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([{ column: "estado", values: ["FL"] }], {}));
    expect(await screen.findByText("de 8")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quitar filtro estado: FL" })).toBeInTheDocument();
    // With a filter, the numbers say which rows they count.
    expect(screen.getByText("Columnia eligió estos gráficos sobre 3 de 8 filas (filtradas).")).toBeInTheDocument();
    expect(screen.getByText("Los gráficos usan todas las filas que cumplen los filtros, no una muestra. Nada sale de tu equipo.")).toBeInTheDocument();

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

  it("reads each chart as a table and moves between columns with the arrows (ACC-18)", async () => {
    const months = Array.from({ length: 30 }, (_, index) => ({
      period: `${2023 + Math.floor(index / 12)}-${String((index % 12) + 1).padStart(2, "0")}`,
      count: 1,
    }));
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue({ ...panel(8), trend: { column: "fecha", granularity: "month", points: months } });
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);

    const histogram = await screen.findByRole("region", { name: "Distribución de precio" });
    fireEvent.click(within(histogram).getByText("Ver como tabla"));
    const histogramTable = within(histogram).getByRole("table", { name: "Filas por tramo de precio" });
    expect(within(histogramTable).getAllByRole("row")).toHaveLength(3);

    // More than 24 months are summarised by year in the table.
    const trend = screen.getByRole("region", { name: "Filas en el tiempo por fecha" });
    const trendTable = within(trend).getByRole("table", { name: "Filas por año de fecha" });
    expect(within(trendTable).getAllByRole("row").map((row) => row.textContent)).toEqual([
      "AñoFilas", "202312", "202412", "20256",
    ]);

    // One Tab stop per chart; the arrows move between its columns.
    const columns = within(histogram).getAllByRole("button", { name: /^precio de/ });
    expect(columns.map((column) => column.tabIndex)).toEqual([0, -1]);
    columns[0]!.focus();
    fireEvent.keyDown(columns[0]!, { key: "ArrowRight" });
    expect(columns[1]).toHaveFocus();
    fireEvent.keyDown(columns[1]!, { key: "Home" });
    expect(columns[0]).toHaveFocus();
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

  it("announces the first panel while it is computed (UX-11)", async () => {
    let resolve!: (value: ExplorePanel) => void;
    vi.spyOn(bridge, "getExplorePanel").mockReturnValue(new Promise((done) => { resolve = done; }));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(screen.getByRole("status")).toHaveTextContent("Preparando el panel…");
    resolve(panel(8));
    expect(await screen.findByRole("region", { name: "Filas por estado" })).toBeInTheDocument();
    expect(screen.queryByText("Preparando el panel…")).not.toBeInTheDocument();
  });

  it("says how many rows a measure left out and writes years as years (FUN-78, TXT-06)", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue({
      ...panel(8),
      kpis: [
        { kind: "count", column: null, value: 8 },
        { kind: "median", column: "release_year", value: 1927, ignoredCount: 2 },
      ],
      histogram: {
        column: "release_year",
        bins: [{ lower: 1925, upper: 1930, count: 3 }, { lower: 1930, upper: 1934, count: 3 }],
        ignoredCount: 2,
        integer: true,
      },
    });
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByText("2 filas sin número no cuentan")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "release_year de 1925 a 1929: 3 filas" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "release_year de 1930 a 1934: 3 filas" })).toBeInTheDocument();
    expect(screen.getByText(/2 filas sin número no cuentan: no aparece en ningún tramo/)).toBeInTheDocument();
  });

  it("explains a panel without charts and suggests interpreting text dates (UX-09)", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue({
      ...panel(1),
      totalRowCount: 1,
      categories: [],
      histogram: null,
      trend: null,
      options: { categories: [], measures: [], dates: [], textDates: ["fecha"] },
    });
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    // One notice with the reason and the way out; nothing that only makes sense with charts.
    const notice = (await screen.findByText(/Con una sola fila no hay distribución que mostrar/)).closest(".explore__info")! as HTMLElement;
    expect(notice).toHaveTextContent(/«fecha» parece una fecha guardada como texto/);
    expect(screen.queryByRole("button", { name: "Personalizar" })).not.toBeInTheDocument();
    expect(screen.queryByText(/pulsa una para filtrar todo el panel/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Los gráficos usan todas las filas/)).not.toBeInTheDocument();
    expect(within(notice).queryByRole("button")).not.toBeInTheDocument();
  });

  it("offers a button to Preparar when a date is saved as text and nothing can be charted", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue({
      ...panel(5),
      categories: [],
      histogram: null,
      trend: null,
      options: { categories: [], measures: [], dates: [], textDates: ["fecha"] },
    });
    const onGoToPrepare = vi.fn();
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady onGoToPrepare={onGoToPrepare} />);
    fireEvent.click(await screen.findByRole("button", { name: "Interpretar fechas en Preparar" }));
    expect(onGoToPrepare).toHaveBeenCalledTimes(1);
  });

  it("starts from the filters a reopened project kept and reports changes (UX-09)", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    const onFiltersChange = vi.fn();
    render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={[{ column: "estado", values: ["FL"] }]}
      onFiltersChange={onFiltersChange}
    />);
    await waitFor(() => expect(getPanel).toHaveBeenCalledWith([{ column: "estado", values: ["FL"] }], {}));
    fireEvent.click(await screen.findByRole("button", { name: "Quitar filtro estado: FL" }));
    await waitFor(() => expect(onFiltersChange).toHaveBeenLastCalledWith([]));
  });
  it("names every text date when several columns could be a trend (UX-09)", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue({
      ...panel(5),
      categories: [],
      histogram: null,
      trend: null,
      options: { categories: [], measures: [], dates: [], textDates: ["alta", "baja"] },
    });
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByText(/«alta», «baja» parecen fechas guardadas como texto/)).toBeInTheDocument();
    expect(screen.getByText(/Ninguna columna sirve para un gráfico/)).toBeInTheDocument();
  });

  it("clears every filter at once and drops them when the data changes", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    const onFiltersChange = vi.fn();
    const filters = [{ column: "estado", values: ["FL"] }, { column: "precio", range: { min: 450, max: 800 } }];
    const view = render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={filters}
      onFiltersChange={onFiltersChange}
    />);
    fireEvent.click(await screen.findByRole("button", { name: "Quitar filtros" }));
    await waitFor(() => expect(onFiltersChange).toHaveBeenLastCalledWith([]));
    expect(screen.queryByRole("button", { name: /Quitar filtro estado/ })).not.toBeInTheDocument();

    // A new revision of the data starts without filters or chosen columns that may no longer exist.
    view.unmount();
    const reopened = render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={filters}
      onFiltersChange={onFiltersChange}
    />);
    expect(await screen.findByRole("button", { name: "Quitar filtro estado: FL" })).toBeInTheDocument();
    reopened.rerender(<ExplorePhase
      dataset={dataset}
      datasetRevision={2}
      profileReady
      initialFilters={filters}
      onFiltersChange={onFiltersChange}
    />);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Quitar filtro estado: FL" })).not.toBeInTheDocument());
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith([], {}));
  });

  it("moves the focus to a neighbour when a filter is removed, and announces it", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    const filters = [
      { column: "estado", values: ["FL"] },
      { column: "precio", range: { min: 450, max: 800 } },
      { column: "fecha", period: "2025-02" },
    ];
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady initialFilters={filters} />);
    // The live region wraps the summary only, never the buttons.
    const status = (await screen.findByText(/de 8 cumplen los filtros/)).closest("[role=status]")! as HTMLElement;
    expect(within(status).queryAllByRole("button")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Personalizar" }).closest("[role=status]")).toBeNull();

    // The middle chip goes: the next one gets the focus.
    fireEvent.click(await screen.findByRole("button", { name: "Quitar filtro precio: 450 a 800" }));
    expect(await screen.findByRole("button", { name: "Quitar filtro fecha: 2025-02" })).toHaveFocus();
    expect(status).toHaveTextContent("Filtro quitado: precio: 450 a 800.");
    // The last one goes: the previous one.
    fireEvent.click(screen.getByRole("button", { name: "Quitar filtro fecha: 2025-02" }));
    expect(await screen.findByRole("button", { name: "Quitar filtro estado: FL" })).toHaveFocus();
    // The only one goes: the chart that made it.
    fireEvent.click(screen.getByRole("button", { name: "Quitar filtro estado: FL" }));
    // The chart that made it, on the bar that made it (FL, the second), not the first.
    await waitFor(() => expect(within(screen.getByRole("region", { name: "Filas por estado" })).getAllByRole("button")[1]).toHaveFocus());
  });

  it("leaves the focus on «Personalizar» after «Quitar filtros» and says so", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={[{ column: "estado", values: ["FL"] }, { column: "precio", range: { min: 450, max: 800 } }]}
    />);
    fireEvent.click(await screen.findByRole("button", { name: "Quitar filtros" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Personalizar" })).toHaveFocus());
    expect(screen.getByRole("status")).toHaveTextContent("Filtros quitados.");
  });

  it("says when changing the measure removed its filter", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={[{ column: "precio", range: { min: 450, max: 800 } }]}
    />);
    fireEvent.click(await screen.findByRole("button", { name: "Personalizar" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Medida" }), { target: { value: "metros" } });
    expect(await screen.findByRole("status")).toHaveTextContent("Se quitó el filtro precio: 450 a 800 al cambiar la medida.");
  });

  it("walks the bars of a bar chart with the arrows, one Tab stop, and names them like the columns", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    const chart = await screen.findByRole("region", { name: "Filas por estado" });
    const bars = within(chart).getAllByRole("button");
    expect(bars.map((bar) => bar.getAttribute("aria-label"))).toEqual(["CA: 3 filas", "FL: 3 filas", "nulo: 2 filas"]);
    expect(bars.map((bar) => bar.tabIndex)).toEqual([0, -1, -1]);
    bars.forEach((bar) => expect(bar).toHaveAttribute("aria-pressed", "false"));
    bars[0]!.focus();
    fireEvent.keyDown(bars[0]!, { key: "ArrowDown" });
    expect(bars[1]).toHaveFocus();
    fireEvent.keyDown(bars[1]!, { key: "End" });
    expect(bars[2]).toHaveFocus();
    fireEvent.keyDown(bars[2]!, { key: "ArrowUp" });
    expect(bars[1]).toHaveFocus();
  });

  it("keeps the axis inside the box that scrolls and the whole range outside it", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    const { container } = render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    const bin = await screen.findByRole("button", { name: /precio de 100 a 450/ });
    expect(bin).not.toHaveAttribute("title");
    expect(bin.closest(".explore__scroll")!.querySelector(".explore__axis")).not.toBeNull();
    expect(container.querySelectorAll(".explore__scroll")).toHaveLength(2);
    // Outside the scrolling box, so it is always visible.
    const ranges = [...container.querySelectorAll(".explore__range")];
    expect(ranges.map((range) => range.textContent)).toEqual(["De 100 a 800", "De 2025-01 a 2025-02"]);
    ranges.forEach((range) => expect(range.closest(".explore__scroll")).toBeNull());
    // Axis marks sit on bin edges: 100, 450 and 800 at 0, 50 and 100 %.
    const ticks = [...bin.closest(".explore__scroll")!.querySelectorAll<HTMLElement>(".explore__axis span")];
    expect(ticks.map((tick) => [tick.textContent, tick.style.left])).toEqual([["100", "0%"], ["450", "50%"], ["800", "100%"]]);
  });

  it("reads the focused, hovered or selected column in a line that screen readers skip", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    const { container } = render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    const bin = await screen.findByRole("button", { name: /precio de 450 a 800/ });
    const readings = () => [...container.querySelectorAll(".explore__reading")].map((line) => line.textContent);
    expect(readings()).toEqual(["Enfoca o toca un tramo para leer sus filas.", "Enfoca o toca un periodo para leer sus filas."]);
    container.querySelectorAll(".explore__reading").forEach((line) => expect(line).toHaveAttribute("aria-hidden", "true"));
    act(() => bin.focus());
    expect(readings()[0]).toBe("precio de 450 a 800: 4 filas");
    fireEvent.pointerOver(screen.getByRole("button", { name: "2025-01: 2 filas" }));
    expect(readings()[1]).toBe("2025-01: 2 filas");
    fireEvent.pointerLeave(screen.getByRole("button", { name: "2025-01: 2 filas" }).closest(".explore__scroll")!);
    expect(readings()[1]).toBe("Enfoca o toca un periodo para leer sus filas.");
    // A tap selects: the selected column stays on the line once the focus leaves.
    fireEvent.click(screen.getByRole("button", { name: "2025-02: 6 filas" }));
    act(() => (document.activeElement as HTMLElement | null)?.blur());
    expect(readings()[1]).toBe("2025-02: 6 filas");
  });

  it("notes the busiest period like the tallest bin", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByText(/el periodo con más filas es 2025-02, con 6 filas/)).toBeInTheDocument();
  });

  it("offers «Deshacer» after the measure change removed a filter, and restores measure and filter", async () => {
    const getPanel = vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    render(<ExplorePhase
      dataset={dataset}
      datasetRevision={1}
      profileReady
      initialFilters={[{ column: "precio", range: { min: 450, max: 800 } }]}
    />);
    fireEvent.click(await screen.findByRole("button", { name: "Personalizar" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Medida" }), { target: { value: "metros" } });
    const undo = await screen.findByRole("button", { name: "Deshacer" });
    // The button is not part of the live region.
    expect(undo.closest("[role=status]")).toBeNull();
    fireEvent.click(undo);
    await waitFor(() => expect(getPanel).toHaveBeenLastCalledWith(
      [{ column: "precio", range: { min: 450, max: 800 } }],
      {},
    ));
    expect(await screen.findByRole("button", { name: "Quitar filtro precio: 450 a 800" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Se restauró el filtro precio: 450 a 800.");
    expect(screen.queryByRole("button", { name: "Deshacer" })).not.toBeInTheDocument();
  });

  it("retires «Filtros quitados.» after a few seconds without losing the summary", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(3));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(<ExplorePhase
        dataset={dataset}
        datasetRevision={1}
        profileReady
        initialFilters={[{ column: "estado", values: ["FL"] }]}
      />);
      fireEvent.click(await screen.findByRole("button", { name: "Quitar filtros" }));
      expect(screen.getByRole("status")).toHaveTextContent("Filtros quitados.");
      act(() => { vi.advanceTimersByTime(6500); });
      expect(screen.getByRole("status")).not.toHaveTextContent("Filtros quitados.");
      expect(screen.getByRole("status")).toHaveTextContent("cuenta las filas de un valor");
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows the scroll hint only when the columns overflow", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    await screen.findByRole("region", { name: "Filas por estado" });
    expect(screen.queryByText(/Desliza para ver/)).not.toBeInTheDocument();
  });

  it("explains the scroll when the columns do not fit", async () => {
    vi.spyOn(bridge, "getExplorePanel").mockResolvedValue(panel(8));
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(400);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
    render(<ExplorePhase dataset={dataset} datasetRevision={1} profileReady />);
    expect(await screen.findByText("Desliza para ver todos los tramos.")).toBeInTheDocument();
    expect(screen.getByText("Desliza para ver todos los periodos.")).toBeInTheDocument();
  });
});
