import { useEffect, useState } from "react";

import { getExplorePanel } from "../../bridge";
import type { DatasetPreview, ExploreFilter, ExploreLayout, ExplorePanel } from "../../bridge";
import {
  MAX_CHARTS,
  axisTicks,
  binLabel,
  filterLabel,
  formatNumber,
  ignoredRowsText,
  looksLikeYears,
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

/** ACC-18: one Tab stop per chart; the arrows, Home and End move between its columns. */
function moveAmongColumns(event: React.KeyboardEvent<HTMLDivElement>) {
  const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("button")];
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
  if (index < 0) return;
  const targets: Record<string, number> = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: buttons.length - 1 };
  const target = targets[event.key];
  if (target === undefined) return;
  event.preventDefault();
  buttons[Math.max(0, Math.min(buttons.length - 1, target))]?.focus();
}

export function ExplorePhase({ dataset, datasetRevision, profileReady, initialFilters, onFiltersChange }: ExplorePhaseProps) {
  const [filters, setFilters] = useState<ExploreFilter[]>(initialFilters ?? []);
  useEffect(() => { onFiltersChange?.(filters); }, [filters, onFiltersChange]);
  // «Personalizar»: empty means Columnia chooses the charts.
  const [layout, setLayout] = useState<ExploreLayout>({});
  const [customizing, setCustomizing] = useState(false);
  const [state, setState] = useState<PanelState>({ kind: "loading", previous: null });
  const [attempt, setAttempt] = useState(0);
  const custom = Object.keys(layout).length > 0;
  const [shownRevision, setShownRevision] = useState(datasetRevision);
  if (shownRevision !== datasetRevision) {
    // New data: filters and chosen columns may no longer exist.
    setShownRevision(datasetRevision);
    setFilters([]);
    setLayout({});
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

  const panel = state.kind === "ready" ? state.panel : state.previous;
  const shownCharts = panel?.categories.map((chart) => chart.column) ?? [];
  // A chart that leaves the panel takes its filter with it.
  function changeLayout(next: ExploreLayout, dropped?: string | null) {
    setLayout(next);
    if (dropped) setFilters((current) => current.filter((filter) => filter.column !== dropped));
  }

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Explora los datos limpios</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
          <p className="phase-meta">
            {formatNumber(dataset.rowCount)} filas · {custom ? "Tú elegiste estos gráficos" : "Columnia eligió estos gráficos"}
          </p>
        </div>
      </header>

      {!profileReady && (
        <p className="notice" role="status">Analizando la calidad para preparar el panel…</p>
      )}
      {/* UX-11: the first panel of a large dataset takes a moment. */}
      {profileReady && state.kind === "loading" && !state.previous && (
        <p className="notice" role="status">Preparando el panel…</p>
      )}
      {state.kind === "error" && (
        <div className="notice notice--error" role="alert">
          <p>No se pudo preparar el panel: {state.message}</p>
          {filters.length > 0 || custom ? (
            <button type="button" className="secondary-action" onClick={() => { setFilters([]); setLayout({}); }}>
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
        <section className="explore" aria-labelledby="explore-title" aria-busy={state.kind === "loading"}>
          <h3 id="explore-title" className="visually-hidden">Panel de los datos</h3>
          <div className="explore__filters" aria-live="polite">
            {filters.length === 0 ? (
              <span className="explore__hint">Pulsa una barra para filtrar todo el panel.</span>
            ) : (
              <>
                {filters.map((filter) => (
                  <button
                    key={filter.column}
                    type="button"
                    className="explore__chip"
                    aria-label={`Quitar filtro ${filterLabel(filter)}`}
                    onClick={() => setFilters((current) => current.filter((item) => item.column !== filter.column))}
                  >
                    {filterLabel(filter)} ✕
                  </button>
                ))}
                <button type="button" className="explore__clear" onClick={() => setFilters([])}>Quitar filtros</button>
              </>
            )}
            <button
              type="button"
              className="secondary-action explore__customize"
              aria-expanded={customizing}
              aria-controls="explore-custom"
              onClick={() => setCustomizing((open) => !open)}
            >
              Personalizar
            </button>
          </div>

          {customizing && (
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
                          onChange={() => changeLayout(toggleChart(layout, shownCharts, column), checked ? column : null)}
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
                    onChange={(event) => changeLayout({ ...layout, measure: event.target.value }, panel.histogram?.column)}
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
                    onChange={(event) => changeLayout({ ...layout, date: event.target.value }, panel.trend?.column)}
                  >
                    {panel.options.dates.map((column) => <option key={column}>{column}</option>)}
                  </select>
                </label>
              )}
              {custom && (
                <button type="button" className="explore__clear" onClick={() => { setLayout({}); setFilters([]); }}>
                  Volver a lo automático
                </button>
              )}
            </div>
          )}

          <dl className="explore__kpis">
            {panel.kpis.map((kpi) => (
              <div key={`${kpi.kind}:${kpi.column ?? ""}`} className="explore__kpi">
                <dt>{kpiLabel(kpi)}</dt>
                <dd>{kpiValue(kpi)}</dd>
                {kpi.kind === "count" && panel.rowCount !== panel.totalRowCount && (
                  <small>de {formatNumber(panel.totalRowCount)}</small>
                )}
                {ignoredRowsText(kpi.ignoredCount) && <small>{ignoredRowsText(kpi.ignoredCount)}</small>}
              </div>
            ))}
          </dl>

          <div className="explore__grid">
            {panel.categories.map((chart) => {
              const max = Math.max(1, ...chart.bars.map((bar) => bar.count));
              const active = filters.some((filter) => filter.column === chart.column);
              return (
                <section key={chart.column} className="explore__panel" aria-label={`Filas por ${chart.column}`}>
                  <h4>{chart.column}</h4>
                  <div className={`explore__bars${active ? " explore__bars--dim" : ""}`}>
                    {chart.bars.map((bar) => {
                      const selected = isValueSelected(filters, chart.column, bar.value);
                      return (
                        <button
                          key={bar.value ?? "\u0000"}
                          type="button"
                          className="explore__bar"
                          aria-pressed={selected}
                          onClick={() => setFilters((current) => toggleValue(current, chart.column, bar.value))}
                        >
                          <span className="explore__label">{valueLabel(bar.value)}</span>
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
              const years = histogram.integer && looksLikeYears(histogram.bins[0]?.lower ?? 0, histogram.bins.at(-1)?.upper ?? 0);
              const ignored = ignoredRowsText(histogram.ignoredCount);
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Distribución de ${histogram.column}`}>
                  <h4>{histogram.column}</h4>
                  <div className={`explore__columns${active ? " explore__bars--dim" : ""}`} onKeyDown={moveAmongColumns}>
                    {histogram.bins.map((bin, index) => {
                      const selected = isRangeSelected(filters, histogram.column, bin.lower, bin.upper);
                      return (
                        <button
                          key={bin.lower}
                          type="button"
                          className="explore__column"
                          tabIndex={index === 0 ? 0 : -1}
                          aria-pressed={selected}
                          aria-label={`${histogram.column} de ${binLabel(bin.lower, bin.upper, histogram.integer, index === histogram.bins.length - 1, years)}: ${plural(bin.count, "fila", "filas")}`}
                          onClick={() => setFilters((current) =>
                            toggleRange(current, histogram.column, bin.lower, bin.upper, index === histogram.bins.length - 1))}
                        >
                          <span className="explore__area"><span className="explore__fill" style={{ height: `${(bin.count / max) * 100}%` }} /></span>
                        </button>
                      );
                    })}
                  </div>
                  <p className="explore__axis">
                    {axisTicks(histogram.bins[0]?.lower ?? 0, histogram.bins.at(-1)?.upper ?? 0).map((tick, index) => (
                      <span key={index}>{formatNumber(histogram.integer ? Math.round(tick) : tick, 2, !years)}</span>
                    ))}
                  </p>
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

            {panel.trend && panel.trend.points.length > 0 && (() => {
              const trend = panel.trend;
              const max = Math.max(1, ...trend.points.map((point) => point.count));
              const active = filters.some((filter) => filter.column === trend.column);
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Filas en el tiempo por ${trend.column}`}>
                  <h4>{trend.column}</h4>
                  <div className={`explore__columns${active ? " explore__bars--dim" : ""}`} onKeyDown={moveAmongColumns}>
                    {trend.points.map((point, index) => (
                      <button
                        key={point.period}
                        type="button"
                        className="explore__column"
                        tabIndex={index === 0 ? 0 : -1}
                        aria-pressed={isPeriodSelected(filters, trend.column, point.period)}
                        aria-label={`${point.period}: ${plural(point.count, "fila", "filas")}`}
                        title={`${point.period}: ${formatNumber(point.count)}`}
                        onClick={() => setFilters((current) => togglePeriod(current, trend.column, point.period))}
                      >
                        <span className="explore__area"><span className="explore__fill" style={{ height: `${(point.count / max) * 100}%` }} /></span>
                      </button>
                    ))}
                  </div>
                  <p className="explore__axis">
                    <span>{trend.points[0]?.period}</span>
                    <span>{trend.points.at(-1)?.period}</span>
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
          {/* UX-09: say why a panel has no charts or no trend. */}
          {panel.categories.length === 0 && !panel.histogram && !(panel.trend && panel.trend.points.length > 0) && (
            <p className="notice">
              {panel.totalRowCount <= 1
                ? "Con una sola fila no hay distribución que mostrar: los gráficos aparecen con más filas."
                : "Ninguna columna sirve para un gráfico: hacen falta categorías con valores repetidos, números o fechas."}
            </p>
          )}
          {!panel.trend && panel.options.textDates.length > 0 && (
            <p className="notice">
              {panel.options.textDates.length === 1
                ? `«${panel.options.textDates[0]}» parece una fecha guardada como texto. Interprétala en Preparar («Interpretar fechas») para ver su tendencia.`
                : `${panel.options.textDates.map((column) => `«${column}»`).join(", ")} parecen fechas guardadas como texto. Interprétalas en Preparar («Interpretar fechas») para ver su tendencia.`}
            </p>
          )}
          <p className="explore__note">Los gráficos usan todas las filas, no una muestra. Nada sale de tu equipo.</p>
        </section>
      )}
    </>
  );
}
