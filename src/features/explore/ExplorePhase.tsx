import { useEffect, useState } from "react";

import { getExplorePanel } from "../../bridge";
import type { DatasetPreview, ExploreFilter, ExplorePanel } from "../../bridge";
import {
  filterLabel,
  formatNumber,
  isRangeSelected,
  isValueSelected,
  kpiLabel,
  kpiValue,
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
  | { kind: "error"; message: string };

export function ExplorePhase({ dataset, datasetRevision, profileReady }: ExplorePhaseProps) {
  const [filters, setFilters] = useState<ExploreFilter[]>([]);
  const [state, setState] = useState<PanelState>({ kind: "loading", previous: null });
  const [shownRevision, setShownRevision] = useState(datasetRevision);
  if (shownRevision !== datasetRevision) {
    // New data: filters may name values that no longer exist.
    setShownRevision(datasetRevision);
    setFilters([]);
  }

  useEffect(() => {
    if (!profileReady) return;
    let current = true;
    setState((previous) => ({
      kind: "loading",
      previous: previous.kind === "ready" ? previous.panel : previous.kind === "loading" ? previous.previous : null,
    }));
    getExplorePanel(filters).then(
      (panel) => { if (current) setState({ kind: "ready", panel }); },
      (error: unknown) => {
        if (current) setState({ kind: "error", message: error instanceof Error ? error.message : String(error) });
      },
    );
    return () => { current = false; };
  }, [filters, datasetRevision, profileReady]);

  const panel = state.kind === "ready" ? state.panel : state.kind === "loading" ? state.previous : null;

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Explora los datos limpios</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
          <p className="phase-meta">
            {formatNumber(dataset.rowCount)} filas · Columnia eligió estos gráficos
          </p>
        </div>
      </header>

      {!profileReady && (
        <p className="notice" role="status">Analizando la calidad para preparar el panel…</p>
      )}
      {state.kind === "error" && (
        <p className="notice notice--error" role="alert">No se pudo preparar el panel: {state.message}</p>
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
          </div>

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
                  {chart.otherCount > 0 && (
                    <p className="explore__note">
                      Otros {formatNumber(chart.distinctCount - chart.bars.length)} valores: {formatNumber(chart.otherCount)} filas
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
                    {histogram.bins.map((bin) => {
                      const selected = isRangeSelected(filters, histogram.column, bin.lower, bin.upper);
                      return (
                        <button
                          key={bin.lower}
                          type="button"
                          className="explore__column"
                          aria-pressed={selected}
                          aria-label={`${histogram.column} de ${formatNumber(bin.lower, 2)} a ${formatNumber(bin.upper, 2)}: ${formatNumber(bin.count)} filas`}
                          onClick={() => setFilters((current) => toggleRange(current, histogram.column, bin.lower, bin.upper))}
                        >
                          <span className="explore__area"><span className="explore__fill" style={{ height: `${(bin.count / max) * 100}%` }} /></span>
                        </button>
                      );
                    })}
                  </div>
                  <p className="explore__axis">
                    <span>{formatNumber(histogram.bins[0]?.lower ?? 0, 2)}</span>
                    <span>{formatNumber(histogram.bins.at(-1)?.upper ?? 0, 2)}</span>
                  </p>
                </section>
              );
            })()}

            {panel.trend && panel.trend.points.length > 0 && (() => {
              const trend = panel.trend;
              const max = Math.max(1, ...trend.points.map((point) => point.count));
              return (
                <section className="explore__panel explore__panel--wide" aria-label={`Filas en el tiempo por ${trend.column}`}>
                  <h4>{trend.column}</h4>
                  <div className="explore__columns">
                    {trend.points.map((point) => (
                      <span
                        key={point.period}
                        className="explore__column explore__column--static"
                        role="img"
                        aria-label={`${point.period}: ${formatNumber(point.count)} filas`}
                        title={`${point.period}: ${formatNumber(point.count)}`}
                      >
                        <span className="explore__area"><span className="explore__fill" style={{ height: `${(point.count / max) * 100}%` }} /></span>
                      </span>
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
