import { useEffect, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";

import { getExplorePanel } from "../../bridge";
import type { DatasetPreview, ExploreFilter, ExploreLayout, ExplorePanel } from "../../bridge";
import {
  MAX_CHARTS,
  axisTicks,
  binLabel,
  filterLabel,
  histogramYears,
  formatNumber,
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
import "./explore.css";
import { plural } from "../../plural";
import { errorMessage } from "../../bridge/errors";

interface ExplorePhaseProps {
  dataset: DatasetPreview;
  /** Changes whenever the data changes, so the panel is recomputed. */
  datasetRevision: number;
  profileReady: boolean;
  /** UX-09: the filters a reopened project had. */
  initialFilters?: ExploreFilter[];
  onFiltersChange?: (filters: ExploreFilter[]) => void;
  /** Opens Preparar, where text dates are interpreted; without it the notice only explains. */
  onGoToPrepare?: () => void;
}

type PanelState =
  | { kind: "loading"; previous: ExplorePanel | null }
  | { kind: "ready"; panel: ExplorePanel }
  | { kind: "error"; message: string; previous: ExplorePanel | null };

/**
 * ACC-18: the chart's numbers as a table, for screen readers and for reading
 * exact values; closed by default so the panel stays visual.
 */
function ChartTable({ caption, headers, rows }: {
  caption: string;
  headers: [string, string];
  rows: [string, number][];
}) {
  return (
    <details className="explore__table">
      <summary>Ver como tabla</summary>
      <div className="explore__table-scroll" role="group" tabIndex={0} aria-label={`Tabla: ${caption}`}>
      <table>
        <caption className="visually-hidden">{caption}</caption>
        <thead>
          <tr><th scope="col">{headers[0]}</th><th scope="col">{headers[1]}</th></tr>
        </thead>
        <tbody>
          {rows.map(([label, count]) => (
            <tr key={label}><th scope="row">{label}</th><td>{formatNumber(count)}</td></tr>
          ))}
        </tbody>
      </table>
      </div>
    </details>
  );
}

/** ACC-18: more than 24 monthly or daily periods read better by year. */
function trendTableRows(points: { period: string; count: number }[]): { byYear: boolean; rows: [string, number][] } {
  if (points.length <= 24 || !points.every((point) => /^\d{4}-/.test(point.period))) {
    return { byYear: false, rows: points.map((point) => [point.period, point.count]) };
  }
  const years = new Map<string, number>();
  for (const point of points) {
    const year = point.period.slice(0, 4);
    years.set(year, (years.get(year) ?? 0) + point.count);
  }
  return { byYear: true, rows: [...years.entries()] };
}

/**
 * ACC-18: one Tab stop per chart; the arrows, Home and End move between its
 * columns or bars (up and down too in a list of bars).
 */
function moveAmong(vertical: boolean) {
  return (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button")];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (index < 0) return;
    const targets: Record<string, number> = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: buttons.length - 1 };
    if (vertical) { targets.ArrowDown = index + 1; targets.ArrowUp = index - 1; }
    const target = targets[event.key];
    if (target === undefined) return;
    event.preventDefault();
    buttons[Math.max(0, Math.min(buttons.length - 1, target))]?.focus();
  };
}
const moveAmongColumns = moveAmong(false);
const moveAmongBars = moveAmong(true);

/**
 * The columns and their axis scroll together; the hint shows only when the
 * columns (at least 24 px each, WCAG 2.5.8) do not fit. Outside the box that
 * scrolls stay the whole range as text and a reading of the column the pointer
 * or the focus is on (or the selected one), so neither depends on `title`.
 */
function ScrollPlot({ count, hint, range, readings, selected, idle, children }: {
  count: number;
  hint: string;
  /** «De 0 a 700»: always visible, the axis marks inside may be scrolled away. */
  range: string;
  /** One line per column, in order. */
  readings: string[];
  /** Index of the selected column, -1 if none. */
  selected: number;
  idle: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [overflows, setOverflows] = useState(false);
  // The keyboard focus and the pointer each point at a column; the focus wins while it is inside.
  const [focused, setFocused] = useState<number | null>(null);
  const [hovered, setHovered] = useState<number | null>(null);
  // A click focuses its column too, but that is not keyboard navigation: it must not freeze the reading.
  const pointerFocus = useRef(false);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const measure = () => setOverflows(element.scrollWidth > element.clientWidth + 1);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
    // `count` re-measures when the columns change; the observer covers resizes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [count]);
  function indexOfColumn(event: { target: EventTarget }): number | null {
    const column = (event.target as HTMLElement).closest<HTMLElement>("button.explore__column");
    if (!column?.parentElement) return null;
    return Array.prototype.indexOf.call(column.parentElement.children, column);
  }
  const shown = focused ?? hovered ?? (selected >= 0 ? selected : null);
  return (
    <>
      <div
        ref={ref}
        className="explore__scroll"
        onPointerDown={() => { pointerFocus.current = true; }}
        onFocus={(event) => {
          const viaPointer = pointerFocus.current;
          pointerFocus.current = false;
          setFocused(viaPointer ? null : indexOfColumn(event));
        }}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocused(null);
        }}
        onPointerOver={(event) => setHovered(indexOfColumn(event))}
        onPointerLeave={() => setHovered(null)}
      >
        <div className="explore__plot" style={{ "--columns": count } as CSSProperties}>{children}</div>
      </div>
      <div className="explore__readout">
        <p className="explore__range">{range}</p>
        {/* The column's accessible name already says this: the line is for eyes, not for the screen reader. */}
        <p className="explore__reading" aria-hidden="true">{shown === null ? idle : readings[shown]}</p>
      </div>
      {overflows && <p className="explore__note">{hint}</p>}
    </>
  );
}

/** Axis marks placed by position (0 to 1) along the plot. */
function Axis({ ticks }: { ticks: { label: string; position: number }[] }) {
  return (
    <p className="explore__axis">
      {ticks.map((tick, index) => (
        <span
          key={index}
          className={tick.position <= 0 ? "explore__tick--first" : tick.position >= 1 ? "explore__tick--last" : undefined}
          style={{ left: `${tick.position * 100}%` }}
        >
          {tick.label}
        </span>
      ))}
    </p>
  );
}

/** How long a plain announcement stays on screen. */
const ANNOUNCEMENT_MS = 6000;

type FocusTarget = { chip: string } | { chart: string; index?: number } | "customize";


export function ExplorePhase({ dataset, datasetRevision, profileReady, initialFilters, onFiltersChange, onGoToPrepare }: ExplorePhaseProps) {
  const [filters, setFilters] = useState<ExploreFilter[]>(initialFilters ?? []);
  useEffect(() => { onFiltersChange?.(filters); }, [filters, onFiltersChange]);
  // «Personalizar»: empty means Columnia chooses the charts.
  const [layout, setLayout] = useState<ExploreLayout>({});
  const [customizing, setCustomizing] = useState(false);
  const [state, setState] = useState<PanelState>({ kind: "loading", previous: null });
  const [attempt, setAttempt] = useState(0);
  // What a change did that the person may not see; shown while the filters are the ones it produced.
  const [announcement, setAnnouncement] = useState<{ text: string; filters: ExploreFilter[]; undo?: () => void } | null>(null);
  const rootRef = useRef<HTMLElement>(null);
  const pendingFocus = useRef<FocusTarget | null>(null);
  const custom = Object.keys(layout).length > 0;
  const [shownRevision, setShownRevision] = useState(datasetRevision);
  if (shownRevision !== datasetRevision) {
    // New data: filters and chosen columns may no longer exist.
    setShownRevision(datasetRevision);
    setFilters([]);
    setLayout({});
    setAnnouncement(null);
  }

  useEffect(() => {
    if (!profileReady) return;
    let current = true;
    setState((previous) => ({
      kind: "loading",
      previous: previous.kind === "ready" ? previous.panel : previous.previous,
    }));
    getExplorePanel(filters, layout).then(
      (panel) => { if (current) setState({ kind: "ready", panel }); },
      (error: unknown) => {
        if (!current) return;
        // The last panel stays, so its chips can still remove the filter that failed (UX-01).
        setState((previous) => ({
          kind: "error",
          message: errorMessage(error),
          previous: previous.kind === "ready" ? previous.panel : previous.previous,
        }));
      },
    );
    return () => { current = false; };
    // New data (`datasetRevision`) or a retry (`attempt`) asks for the panel again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters, layout, datasetRevision, profileReady, attempt]);

  // A removed filter takes its chip, and the focus with it: put the focus where the person can go on.
  useEffect(() => {
    const target = pendingFocus.current;
    const root = rootRef.current;
    if (!target || !root) return;
    pendingFocus.current = null;
    let element: HTMLElement | null | undefined;
    if (typeof target === "object" && "chip" in target) {
      element = [...root.querySelectorAll<HTMLElement>("[data-filter-chip]")].find((chip) => chip.dataset.filterChip === target.chip);
    } else if (typeof target === "object") {
      // The bar or column that made the filter, else the first.
      const buttons = [...root.querySelectorAll<HTMLElement>("[data-chart]")]
        .find((chart) => chart.dataset.chart === target.chart)
        ?.querySelectorAll<HTMLElement>("button");
      element = buttons?.[target.index ?? 0] ?? buttons?.[0];
    }
    (element ?? root.querySelector<HTMLElement>(".explore__customize"))?.focus();
    // Runs after the render that changed the filters or the layout; the target travels in a ref.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters, layout]);

  const panel = state.kind === "ready" ? state.panel : state.previous;
  const shownCharts = panel?.categories.map((chart) => chart.column) ?? [];
  const text = (filter: ExploreFilter) => filterLabel(filter, panel?.histogram ?? null);
  function say(message: string, next: ExploreFilter[], undo?: () => void) { setAnnouncement({ text: message, filters: next, undo }); }
  // Plain announcements fade after a few seconds (the live region has said them by then); one with «Deshacer» stays until the next change.
  useEffect(() => {
    if (!announcement || announcement.undo) return;
    const timer = setTimeout(() => setAnnouncement(null), ANNOUNCEMENT_MS);
    return () => clearTimeout(timer);
  }, [announcement]);
  function removeFilter(column: string) {
    const index = filters.findIndex((filter) => filter.column === column);
    if (index < 0) return;
    const next = filters.filter((filter) => filter.column !== column);
    const neighbour = filters[index + 1] ?? filters[index - 1];
    let focusIndex: number | undefined;
    if (!neighbour) {
      // Where the filter came from: the pressed bar or column, read before the re-render clears it.
      const chart = [...(rootRef.current?.querySelectorAll<HTMLElement>("[data-chart]") ?? [])].find((item) => item.dataset.chart === column);
      const position = [...(chart?.querySelectorAll("button") ?? [])].findIndex((button) => button.getAttribute("aria-pressed") === "true");
      focusIndex = position >= 0 ? position : undefined;
    }
    pendingFocus.current = neighbour ? { chip: neighbour.column } : { chart: column, index: focusIndex };
    setFilters(next);
    say(`Filtro quitado: ${text(filters[index]!)}.`, next);
  }
  function clearFilters() {
    pendingFocus.current = "customize";
    const next: ExploreFilter[] = [];
    const before = filters;
    setFilters(next);
    say("Filtros quitados.", next, () => {
      pendingFocus.current = "customize";
      setFilters(before);
      say("Se restauraron los filtros.", before);
    });
  }
  function resetToAutomatic() {
    pendingFocus.current = "customize";
    const before = { layout, filters };
    const next: ExploreFilter[] = [];
    setLayout({});
    setFilters(next);
    say(before.filters.length > 0 ? "Se volvió a lo automático y se quitaron los filtros." : "Se volvió a lo automático.", next, () => {
      pendingFocus.current = "customize";
      setLayout(before.layout);
      setFilters(before.filters);
      say("Se restauró lo que habías elegido.", before.filters);
    });
  }
  // A chart that leaves the panel takes its filter with it, and says so.
  function changeLayout(next: ExploreLayout, dropped: string | null | undefined, reason: string) {
    setLayout(next);
    const gone = dropped ? filters.find((filter) => filter.column === dropped) : undefined;
    if (!gone) return;
    const remaining = filters.filter((filter) => filter.column !== dropped);
    const before = { layout, filters };
    setFilters(remaining);
    say(`Se quitó el filtro ${text(gone)} ${reason}.`, remaining, () => {
      pendingFocus.current = "customize";
      setLayout(before.layout);
      setFilters(before.filters);
      say(`Se restauró el filtro ${text(gone)}.`, before.filters);
    });
  }

  const hasTrend = !!(panel?.trend && panel.trend.points.length > 0);
  const noCharts = !!panel && panel.categories.length === 0 && !panel.histogram && !hasTrend;
  const filtered = filters.length > 0 && !!panel;
  const summary = filters.length === 0 || !panel
    ? "Pulsa una barra para filtrar todo el panel."
    : `${plural(panel.rowCount, "fila", "filas")} de ${formatNumber(panel.totalRowCount)} cumplen los filtros.`;
  const shownAnnouncement = announcement && announcement.filters === filters ? announcement.text : null;
  const shownUndo = announcement && announcement.filters === filters ? announcement.undo : undefined;

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Explora los datos</h2>
          <p className="phase-lead">
            {custom ? "Tú elegiste estos gráficos" : "Columnia eligió estos gráficos"}{" "}
            {filtered
              ? `sobre ${formatNumber(panel.rowCount)} de ${formatNumber(panel.totalRowCount)} filas (filtradas)`
              : `sobre las ${formatNumber(dataset.rowCount)} filas`}.
          </p>
        </div>
      </header>

      {!profileReady && (
        <p className="explore__info" role="status">Analizando la calidad para preparar el panel…</p>
      )}
      {/* UX-11: the first panel of a large dataset takes a moment. */}
      {profileReady && state.kind === "loading" && !state.previous && (
        <p className="explore__info" role="status">Preparando el panel…</p>
      )}
      {state.kind === "error" && (
        <div className="notice notice--error" role="alert">
          <p>No se pudo preparar el panel: {state.message}</p>
          {filters.length > 0 || custom ? (
            <button type="button" className="secondary-action" onClick={() => { pendingFocus.current = "customize"; setFilters([]); setLayout({}); }}>
              Quitar filtros y volver a lo automático
            </button>
          ) : (
            <button type="button" className="secondary-action" onClick={() => setAttempt((value) => value + 1)}>
              Reintentar
            </button>
          )}
        </div>
      )}

      {panel && (
        <section ref={rootRef} className="explore" aria-labelledby="explore-title" aria-busy={state.kind === "loading"}>
          <h3 id="explore-title" className="visually-hidden">Panel de los datos</h3>
          {(!noCharts || filters.length > 0) && (
            <div className="explore__filters">
              {/* Only this text is announced: not the buttons beside it. */}
              <div className="explore__message">
                <p className="explore__summary" role="status">
                  {shownAnnouncement && <span>{shownAnnouncement}</span>}
                  {!shownUndo && <span className="explore__hint">{summary}</span>}
                </p>
                {shownUndo && (
                  <button type="button" className="explore__clear" onClick={shownUndo}>Deshacer</button>
                )}
              </div>
              {filters.map((filter) => (
                <button
                  key={filter.column}
                  type="button"
                  className="explore__chip"
                  data-filter-chip={filter.column}
                  aria-label={`Quitar filtro ${text(filter)}`}
                  onClick={() => removeFilter(filter.column)}
                >
                  {text(filter)} ✕
                </button>
              ))}
              {filters.length > 0 && (
                <button type="button" className="explore__clear" onClick={clearFilters}>Quitar filtros</button>
              )}
              {!noCharts && (
                <button
                  type="button"
                  className="secondary-action explore__customize"
                  aria-expanded={customizing}
                  aria-controls="explore-custom"
                  onClick={() => setCustomizing((open) => !open)}
                >
                  Personalizar
                </button>
              )}
            </div>
          )}

          {customizing && !noCharts && (
            <div id="explore-custom" className="explore__custom">
              <fieldset>
                <legend>Gráficos de barras (hasta {MAX_CHARTS})</legend>
                <div className="explore__choices">
                  {panel.options.categories.map((column) => {
                    const checked = shownCharts.includes(column);
                    return (
                      <label key={column}>
                        <input
                          type="checkbox"
                          checked={checked}
                          disabled={!checked && shownCharts.length >= MAX_CHARTS}
                          onChange={() => changeLayout(toggleChart(layout, shownCharts, column), checked ? column : null, "porque ya no se muestra ese gráfico")}
                        />
                        {column}
                      </label>
                    );
                  })}
                </div>
              </fieldset>
              {panel.options.measures.length > 1 && (
                <label>
                  Medida
                  <select
                    value={panel.histogram?.column ?? ""}
                    onChange={(event) => changeLayout({ ...layout, measure: event.target.value }, panel.histogram?.column, "al cambiar la medida")}
                  >
                    {panel.options.measures.map((column) => <option key={column}>{column}</option>)}
                  </select>
                </label>
              )}
              {panel.options.dates.length > 1 && (
                <label>
                  Fecha
                  <select
                    value={panel.trend?.column ?? ""}
                    onChange={(event) => changeLayout({ ...layout, date: event.target.value }, panel.trend?.column, "al cambiar la fecha")}
                  >
                    {panel.options.dates.map((column) => <option key={column}>{column}</option>)}
                  </select>
                </label>
              )}
              {custom && (
                <button type="button" className="explore__clear" onClick={resetToAutomatic}>
                  Volver a lo automático
                </button>
              )}
            </div>
          )}

          <dl className="explore__kpis">
            {panel.kpis.map((kpi) => (
              <div key={`${kpi.kind}:${kpi.column ?? ""}`} className="explore__kpi">
                <dt>{kpiLabel(kpi)}</dt>
                <dd className={kpi.value === null ? "explore__kpi-empty" : undefined}>{kpiValue(kpi)}</dd>
                {kpi.kind === "count" && panel.rowCount !== panel.totalRowCount && (
                  <small>de {formatNumber(panel.totalRowCount)}</small>
                )}
                {ignoredRowsText(kpi.ignoredCount) && <small>{ignoredRowsText(kpi.ignoredCount)}</small>}
              </div>
            ))}
          </dl>

          <div className="explore__grid">
            {panel.categories.map((chart, index) => {
              const max = Math.max(1, ...chart.bars.map((bar) => bar.count));
              const active = filters.some((filter) => filter.column === chart.column);
              // The value column grows with the longest figure so it never cuts one.
              const valueWidth = Math.max(...chart.bars.map((bar) => formatNumber(bar.count).length), 1) + 1;
              // An odd chart out takes the whole row instead of leaving half of it empty.
              const alone = index === panel.categories.length - 1 && panel.categories.length % 2 === 1;
              return (
                <section key={chart.column} className={`explore__panel${alone ? " explore__panel--wide" : ""}`} aria-label={`Filas por ${chart.column}`}>
                  <div className="explore__heading">
                    <h4>{chart.column}</h4>
                    <p className="explore__kind">Filas por valor</p>
                  </div>
                  <div
                    className={`explore__bars${active ? " explore__bars--dim" : ""}`}
                    data-chart={chart.column}
                    style={{ "--value-width": `${valueWidth}ch` } as CSSProperties}
                    onKeyDown={moveAmongBars}
                  >
                    {chart.bars.map((bar, barIndex) => {
                      const selected = isValueSelected(filters, chart.column, bar.value);
                      return (
                        <button
                          key={bar.value ?? "\u0000"}
                          type="button"
                          className="explore__bar"
                          tabIndex={barIndex === 0 ? 0 : -1}
                          aria-pressed={selected}
                          aria-label={`${valueLabel(bar.value)}: ${plural(bar.count, "fila", "filas")}`}
                          onClick={() => setFilters((current) => toggleValue(current, chart.column, bar.value))}
                        >
                          <span className="explore__label" title={bar.value === null ? undefined : bar.value}>{valueLabel(bar.value)}</span>
                          <span className="explore__track"><span className="explore__fill" style={{ width: `${(bar.count / max) * 100}%` }} /></span>
                          <span className="explore__value">{formatNumber(bar.count)}</span>
                        </button>
                      );
                    })}
                  </div>
                  {(chart.otherCount > 0 || layout.expanded?.includes(chart.column)) && (
                    <p className="explore__note">
                      {chart.otherCount > 0 && `${chart.distinctCount - chart.bars.length === 1 ? "Otro" : "Otros"} ${plural(chart.distinctCount - chart.bars.length, "valor", "valores")}: ${plural(chart.otherCount, "fila", "filas")} `}
                      <button
                        type="button"
                        className="explore__clear"
                        aria-label={`${layout.expanded?.includes(chart.column) ? "Ver menos" : "Ver todos"} los valores de ${chart.column}`}
                        onClick={() => setLayout(toggleExpanded(layout, chart.column))}
                      >
                        {layout.expanded?.includes(chart.column) ? "Ver menos" : "Ver todos"}
                      </button>
                    </p>
                  )}
                </section>
              );
            })}

            {panel.histogram && (() => {
              const histogram = panel.histogram;
              const max = Math.max(1, ...histogram.bins.map((bin) => bin.count));
              const active = filters.some((filter) => filter.column === histogram.column);
              const years = histogramYears(histogram);
              const ignored = ignoredRowsText(histogram.ignoredCount);
              const tickText = (value: number) => formatNumber(histogram.integer ? Math.round(value) : value, 2, !years);
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Distribución de ${histogram.column}`}>
                  <div className="explore__heading">
                    <h4>{histogram.column}</h4>
                    <p className="explore__kind">Distribución</p>
                  </div>
                  <ScrollPlot
                    count={histogram.bins.length}
                    hint="Desliza para ver todos los tramos."
                    range={`De ${tickText(histogram.bins[0]?.lower ?? 0)} a ${tickText(histogram.bins.at(-1)?.upper ?? 0)}`}
                    readings={histogram.bins.map((bin, index) => `${histogram.column} de ${binLabel(bin.lower, bin.upper, histogram.integer, index === histogram.bins.length - 1, years)}: ${plural(bin.count, "fila", "filas")}`)}
                    selected={histogram.bins.findIndex((bin) => isRangeSelected(filters, histogram.column, bin.lower, bin.upper))}
                    idle="Enfoca o toca un tramo para leer sus filas."
                  >
                    <div className={`explore__columns${active ? " explore__bars--dim" : ""}`} data-chart={histogram.column} onKeyDown={moveAmongColumns}>
                      {histogram.bins.map((bin, index) => {
                        const selected = isRangeSelected(filters, histogram.column, bin.lower, bin.upper);
                        const label = binLabel(bin.lower, bin.upper, histogram.integer, index === histogram.bins.length - 1, years);
                        return (
                          <button
                            key={bin.lower}
                            type="button"
                            className="explore__column"
                            tabIndex={index === 0 ? 0 : -1}
                            aria-pressed={selected}
                            aria-label={`${histogram.column} de ${label}: ${plural(bin.count, "fila", "filas")}`}
                            onClick={() => setFilters((current) =>
                              toggleRange(current, histogram.column, bin.lower, bin.upper, index === histogram.bins.length - 1))}
                          >
                            <span className="explore__area"><span className="explore__fill" style={{ height: `${(bin.count / max) * 100}%` }} /></span>
                          </button>
                        );
                      })}
                    </div>
                    <Axis ticks={axisTicks(histogram.bins).map((tick) => ({ label: tickText(tick.value), position: tick.position }))} />
                  </ScrollPlot>
                  <p className="explore__note">
                    Filas por tramo de {histogram.column}; el tramo más alto tiene {plural(max, "fila", "filas")}.
                    {ignored ? ` ${ignored}: no aparece en ningún tramo.` : ""}
                  </p>
                  <ChartTable
                    caption={`Filas por tramo de ${histogram.column}`}
                    headers={["Tramo", "Filas"]}
                    rows={histogram.bins.map((bin, index) => [
                      binLabel(bin.lower, bin.upper, histogram.integer, index === histogram.bins.length - 1, years),
                      bin.count,
                    ])}
                  />
                </section>
              );
            })()}

            {panel.trend && hasTrend && (() => {
              const trend = panel.trend;
              const max = Math.max(1, ...trend.points.map((point) => point.count));
              const peak = trend.points.reduce((best, point) => (point.count > best.count ? point : best), trend.points[0]!);
              const active = filters.some((filter) => filter.column === trend.column);
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Filas en el tiempo por ${trend.column}`}>
                  <div className="explore__heading">
                    <h4>{trend.column}</h4>
                    <p className="explore__kind">Filas en el tiempo</p>
                  </div>
                  <ScrollPlot
                    count={trend.points.length}
                    hint="Desliza para ver todos los periodos."
                    range={`De ${trend.points[0]?.period} a ${trend.points.at(-1)?.period}`}
                    readings={trend.points.map((point) => `${point.period}: ${plural(point.count, "fila", "filas")}`)}
                    selected={trend.points.findIndex((point) => isPeriodSelected(filters, trend.column, point.period))}
                    idle="Enfoca o toca un periodo para leer sus filas."
                  >
                    <div className={`explore__columns${active ? " explore__bars--dim" : ""}`} data-chart={trend.column} onKeyDown={moveAmongColumns}>
                      {trend.points.map((point, index) => (
                        <button
                          key={point.period}
                          type="button"
                          className="explore__column"
                          tabIndex={index === 0 ? 0 : -1}
                          aria-pressed={isPeriodSelected(filters, trend.column, point.period)}
                          aria-label={`${point.period}: ${plural(point.count, "fila", "filas")}`}
                          onClick={() => setFilters((current) => togglePeriod(current, trend.column, point.period))}
                        >
                          <span className="explore__area"><span className="explore__fill" style={{ height: `${(point.count / max) * 100}%` }} /></span>
                        </button>
                      ))}
                    </div>
                    <Axis ticks={[
                      { label: trend.points[0]?.period ?? "", position: 0 },
                      { label: trend.points.at(-1)?.period ?? "", position: 1 },
                    ]} />
                  </ScrollPlot>
                  <p className="explore__note">
                    Filas por periodo de {trend.column}; el periodo con más filas es {peak.period}, con {plural(peak.count, "fila", "filas")}.
                  </p>
                  {(() => {
                    const table = trendTableRows(trend.points);
                    return (
                      <ChartTable
                        caption={table.byYear ? `Filas por año de ${trend.column}` : `Filas por periodo de ${trend.column}`}
                        headers={[table.byYear ? "Año" : "Periodo", "Filas"]}
                        rows={table.rows}
                      />
                    );
                  })()}
                </section>
              );
            })()}
          </div>
          {/* UX-09: say why a panel has no charts or no trend, once, with the way out. */}
          {(noCharts || (!panel.trend && panel.options.textDates.length > 0)) && (
            <div className="explore__info">
              <p>
                {noCharts && (panel.totalRowCount <= 1
                  ? "Con una sola fila no hay distribución que mostrar: los gráficos aparecen con más filas. "
                  : "Ninguna columna sirve para un gráfico: hacen falta categorías con valores repetidos, números o fechas. ")}
                {!panel.trend && panel.options.textDates.length === 1 &&
                  `«${panel.options.textDates[0]}» parece una fecha guardada como texto. Interprétala en Preparar («Interpretar fechas») para ver su tendencia.`}
                {!panel.trend && panel.options.textDates.length > 1 &&
                  `${panel.options.textDates.map((column) => `«${column}»`).join(", ")} parecen fechas guardadas como texto. Interprétalas en Preparar («Interpretar fechas») para ver su tendencia.`}
              </p>
              {!panel.trend && panel.options.textDates.length > 0 && onGoToPrepare && (
                <button type="button" className="secondary-action" onClick={onGoToPrepare}>Interpretar fechas en Preparar</button>
              )}
            </div>
          )}
          {!noCharts && (
            <p className="explore__note">
              {filters.length > 0 ? "Los gráficos usan todas las filas que cumplen los filtros, no una muestra." : "Los gráficos usan todas las filas, no una muestra."} Nada sale de tu equipo.
            </p>
          )}
        </section>
      )}
    </>
  );
}
