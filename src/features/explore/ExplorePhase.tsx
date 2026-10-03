import { useEffect, useState } from "react";

import { getExplorePanel } from "../../bridge";
import type { DatasetPreview, ExploreFilter, ExploreLayout, ExplorePanel } from "../../bridge";
import {
  MAX_CHARTS,
  axisTicks,
  filterLabel,
  formatNumber,
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

interface ExplorePhaseProps {
  dataset: DatasetPreview;
  /** Changes whenever the data changes, so the panel is recomputed. */
  datasetRevision: number;
  profileReady: boolean;
}

type PanelState =
  | { kind: "loading"; previous: ExplorePanel | null }
  | { kind: "ready"; panel: ExplorePanel }
  | { kind: "error"; message: string; previous: ExplorePanel | null };

export function ExplorePhase({ dataset, datasetRevision, profileReady }: ExplorePhaseProps) {
  const [filters, setFilters] = useState<ExploreFilter[]>([]);
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
          message: error instanceof Error ? error.message : String(error),
          previous: previous.kind === "ready" ? previous.panel : previous.previous,
        }));
      },
    );
    return () => { current = false; };
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
                      {chart.otherCount > 0 && `Otros ${formatNumber(chart.distinctCount - chart.bars.length)} valores: ${formatNumber(chart.otherCount)} filas `}
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
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Distribución de ${histogram.column}`}>
                  <h4>{histogram.column}</h4>
                  <div className={`explore__columns${active ? " explore__bars--dim" : ""}`}>
                    {histogram.bins.map((bin, index) => {
                      const selected = isRangeSelected(filters, histogram.column, bin.lower, bin.upper);
                      return (
                        <button
                          key={bin.lower}
                          type="button"
                          className="explore__column"
                          aria-pressed={selected}
                          aria-label={`${histogram.column} de ${formatNumber(bin.lower, 2)} a ${formatNumber(bin.upper, 2)}: ${formatNumber(bin.count)} filas`}
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
                      <span key={index}>{formatNumber(tick, 2)}</span>
                    ))}
                  </p>
                  <p className="explore__note">Filas por tramo de {histogram.column}; el tramo más alto tiene {formatNumber(max)}.</p>
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
                  <div className={`explore__columns${active ? " explore__bars--dim" : ""}`}>
                    {trend.points.map((point) => (
                      <button
                        key={point.period}
                        type="button"
                        className="explore__column"
                        aria-pressed={isPeriodSelected(filters, trend.column, point.period)}
                        aria-label={`${point.period}: ${formatNumber(point.count)} filas`}
                        title={`${point.period}: ${formatNumber(point.count)}`}
                        onClick={() => setFilters((current) => togglePeriod(current, trend.column, point.period))}
                      >
                        <span className="explore__area"><span className="explore__fill" style={{ height: `${(point.count / max) * 100}%` }} /></span>
                      </button>
                    ))}
                  </div>
                  <p className="explore__axis">
                    <span>{trend.points[0].period}</span>
                    <span>{trend.points.at(-1)?.period}</span>
                  </p>
                </section>
              );
            })()}
          </div>
          <p className="explore__note">Los gráficos usan todas las filas, no una muestra. Nada sale de tu equipo.</p>
        </section>
      )}
    </>
  );
}
