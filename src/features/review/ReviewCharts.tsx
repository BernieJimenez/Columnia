import { useEffect, useEffectEvent, useRef, useState, type ReactNode } from "react";

import { cancelOperation, getTemporalAggregation } from "../../bridge";
import type {
  CategoricalGroupSummary,
  ColumnProfile,
  DatasetProfile,
  NumericCorrelationMatrix,
  TemporalAggregationKind,
  TemporalAggregationSeries,
  TemporalPeriod,
  TemporalSeriesSummary,
} from "../../bridge";
import { formatDecimal, formatPercent, withoutNegativeZero } from "../../format";
import { plural } from "../../plural";
import { errorMessage } from "../../bridge/errors";

export function QualityVisuals({ profile, datasetRevision }: { profile: DatasetProfile; datasetRevision: number }) {
  const [activeTemporalAggregation, setActiveTemporalAggregation] = useState<TemporalAggregationOwner | null>(null);
  const activeTemporalAggregationRef = useRef<TemporalAggregationOwner | null>(null);
  function acquireTemporalAggregation(column: string): TemporalAggregationOwner | null {
    if (activeTemporalAggregationRef.current !== null) return null;
    const owner = { token: Symbol(column), column };
    activeTemporalAggregationRef.current = owner;
    setActiveTemporalAggregation(owner);
    return owner;
  }
  function releaseTemporalAggregation(token: symbol) {
    if (activeTemporalAggregationRef.current?.token !== token) return;
    activeTemporalAggregationRef.current = null;
    setActiveTemporalAggregation(null);
  }
  const numericColumns = profile.columns.filter((column) => column.outlierCount !== null);
  const formatColumns = profile.columns.filter((column) => column.typeMatchPercentage !== null);
  const nullPatternColumns = profile.columns
    .filter((column) => column.nullCount > 0)
    .sort((left, right) => {
      const byNullCount = right.nullCount - left.nullCount;
      return byNullCount || left.name.localeCompare(right.name, "es", { sensitivity: "base" });
    });
  const distributionColumns = numericColumns.filter((column) =>
    [column.minimum, column.maximum, column.firstQuartile, column.median, column.thirdQuartile]
      .every((value) => value !== null && Number.isFinite(Number(value))),
  );
  const histogramColumns = numericColumns.filter((column) => (column.histogram?.length ?? 0) > 0);
  const maxOutlierCount = Math.max(
    1,
    ...numericColumns.map((column) => Math.max(0, column.outlierCount ?? 0)),
  );

  if (profile.columns.length === 0) return null;

  return (
    <section className="quality-visuals" aria-labelledby="quality-visuals-title">
      <div className="quality-visuals__heading">
        <div>
          <p className="step">Lectura rápida</p>
          <h4 id="quality-visuals-title">Señales del perfil</h4>
        </div>
        <p>
          Las barras ayudan a detectar patrones; las tablas de abajo conservan los valores exactos
          y el equivalente para lector de pantalla.
        </p>
      </div>
      <div className="quality-chart-grid">
        <div className="quality-chart" role="group" aria-labelledby="quality-completeness-title">
          <h5 id="quality-completeness-title">Completitud por columna</h5>
          <p className="quality-chart__note">Porcentaje de filas con un valor no nulo.</p>
          <div className="quality-chart__bars" role="list" aria-label="Completitud por columna">
            {profile.columns.map((column) => {
              const percentage = clampPercentage(column.completenessPercentage);

              return (
                <div className="quality-chart__item" role="listitem" key={column.name}>
                  <div className="quality-chart__label">
                    <span title={column.name}>{column.name}</span>
                    <strong>{formatPercent(percentage, 1)}</strong>
                  </div>
                  <div className="quality-chart__track" aria-hidden="true">
                    <span style={{ width: `${percentage}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
        {numericColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-outliers-title">
            <h5 id="quality-outliers-title">Posibles outliers</h5>
            <p className="quality-chart__note">Filas fuera del rango IQR de 1.5×.</p>
            <div className="quality-chart__bars" role="list" aria-label="Posibles outliers por columna">
              {numericColumns.map((column) => {
                const count = Math.max(0, column.outlierCount ?? 0);
                const percentage = (count / maxOutlierCount) * 100;

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{count.toLocaleString()}</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {nullPatternColumns.length > 0 && (
          <div className="quality-chart quality-chart--wide" role="group" aria-labelledby="quality-null-patterns-title">
            <h5 id="quality-null-patterns-title">Patrones de nulos</h5>
            <p className="quality-chart__note">
              Prioriza columnas con más valores ausentes antes de transformar o exportar.
            </p>
            <div className="quality-chart__bars" role="list" aria-label="Patrones de nulos por columna">
              {nullPatternColumns.map((column) => {
                const percentage = clampPercentage(100 - column.completenessPercentage);

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{column.nullCount.toLocaleString()} nulos · {formatPercent(percentage, 1)}</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span className="quality-chart__track-fill--warning" style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
            <QualityDataDetails className="quality-chart__table">
              <table aria-label="Tabla de patrones de nulos">
                <caption className="visually-hidden">Tabla de patrones de nulos por columna</caption>
                <thead>
                  <tr>
                    <th scope="col">Columna</th>
                    <th scope="col">Nulos</th>
                    <th scope="col">Porcentaje nulo</th>
                  </tr>
                </thead>
                <tbody>
                  {nullPatternColumns.map((column) => (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>{column.nullCount.toLocaleString()}</td>
                      <td>{formatPercent(clampPercentage(100 - column.completenessPercentage), 1)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </QualityDataDetails>
          </div>
        )}
        {formatColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-format-title">
            <h5 id="quality-format-title">Validación de formato</h5>
            <p className="quality-chart__note">
              Comprueba qué proporción coincide con el tipo sugerido antes de convertir columnas.
            </p>
            <div className="quality-chart__bars" role="list" aria-label="Validación de formato por columna">
              {formatColumns.map((column) => {
                const percentage = clampPercentage(column.typeMatchPercentage ?? 0);
                const invalidCount = Math.max(0, column.invalidTypeCount ?? 0);

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>{formatPercent(percentage, 1)} · {invalidCount.toLocaleString()} inválidos</strong>
                    </div>
                    <div className="quality-chart__track" aria-hidden="true">
                      <span style={{ width: `${percentage}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
            <QualityDataDetails className="quality-chart__table">
              <table aria-label="Tabla de validación de formato">
                <caption className="visually-hidden">Tabla de validación de formato por columna</caption>
                <thead>
                  <tr>
                    <th scope="col">Columna</th>
                    <th scope="col">Tipo sugerido</th>
                    <th scope="col">Coincidencia</th>
                    <th scope="col">Inválidos</th>
                  </tr>
                </thead>
                <tbody>
                  {formatColumns.map((column) => (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>
                        <span aria-hidden="true">{suggestedTypeLabel(column.suggestedType)}</span>
                        <span className="visually-hidden">
                          Tipo sugerido: {suggestedTypeLabel(column.suggestedType)}
                        </span>
                      </td>
                      <td>
                        <span aria-hidden="true">
                          {formatPercent(clampPercentage(column.typeMatchPercentage ?? 0), 1)}
                        </span>
                        <span className="visually-hidden">
                          Coincidencia: {formatPercent(clampPercentage(column.typeMatchPercentage ?? 0), 1)}
                        </span>
                      </td>
                      <td>{Math.max(0, column.invalidTypeCount ?? 0).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </QualityDataDetails>
          </div>
        )}
        {profile.columns.some(isTemporalColumn) && (
          <TemporalCoverageChart
            columns={profile.columns.filter(isTemporalColumn)}
            rowCount={profile.rowCount}
          />
        )}
        {profile.temporalSeries?.map((summary, summaryIndex) => (
          <TemporalTrendChart
            key={summary.column}
            summary={summary}
            summaryIndex={summaryIndex}
            numericColumns={numericColumns.filter((column) => column.privacySignal === null)}
            datasetRevision={datasetRevision}
            activeAggregation={activeTemporalAggregation}
            onAcquireAggregation={acquireTemporalAggregation}
            onReleaseAggregation={releaseTemporalAggregation}
          />
        ))}
        {distributionColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-distribution-title">
            <h5 id="quality-distribution-title">Distribución numérica</h5>
            <p className="quality-chart__note">Rango mínimo–máximo y caja entre Q1 y Q3; la marca central es la mediana.</p>
            <div className="quality-chart__bars" role="list" aria-label="Distribución numérica por columna">
              {distributionColumns.map((column) => {
                const minimum = Number(column.minimum);
                const maximum = Number(column.maximum);
                const firstQuartile = Number(column.firstQuartile);
                const median = Number(column.median);
                const thirdQuartile = Number(column.thirdQuartile);
                const span = Math.max(maximum - minimum, Number.EPSILON);
                const grouping = !readsAsCode(column.name, minimum, maximum);
                const left = ((firstQuartile - minimum) / span) * 100;
                const width = ((thirdQuartile - firstQuartile) / span) * 100;
                const medianPosition = ((median - firstQuartile) / Math.max(thirdQuartile - firstQuartile, Number.EPSILON)) * 100;

                return (
                  <div className="quality-chart__item" role="listitem" key={column.name}>
                    <div className="quality-chart__label">
                      <span title={column.name}>{column.name}</span>
                      <strong>Q1 {formatStatistic(firstQuartile, grouping)} · Mediana {formatStatistic(median, grouping)} · Q3 {formatStatistic(thirdQuartile, grouping)}</strong>
                    </div>
                    <div className="quality-boxplot" aria-hidden="true">
                      <span className="quality-boxplot__whisker" />
                      <span className="quality-boxplot__box" style={{ left: `${clampPercentage(left)}%`, width: `${clampPercentage(width)}%` }}>
                        <span className="quality-boxplot__median" style={{ left: `${clampPercentage(medianPosition)}%` }} />
                      </span>
                    </div>
                    <small className="quality-chart__range">Mín. {formatStatistic(minimum, grouping)} · Máx. {formatStatistic(maximum, grouping)}</small>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {histogramColumns.length > 0 && (
          <div className="quality-chart" role="group" aria-labelledby="quality-histogram-title">
            <h5 id="quality-histogram-title">Histograma numérico</h5>
            <p className="quality-chart__note">
              Frecuencia de valores por intervalo. El último intervalo incluye su límite máximo.
            </p>
            <div className="quality-histograms">
              {histogramColumns.map((column, columnIndex) => {
                const buckets = column.histogram ?? [];
                const maximumCount = Math.max(1, ...buckets.map((bucket) => bucket.count));
                const titleId = `quality-histogram-column-${columnIndex}`;
                const grouping = !readsAsCode(column.name, buckets[0]?.lower ?? 0.5, buckets.at(-1)?.upper ?? 0.5);

                return (
                  <div className="quality-histogram" role="group" aria-labelledby={titleId} key={column.name}>
                    <h6 id={titleId}>{column.name}</h6>
                    <div className="quality-histogram__bars" aria-hidden="true">
                      {buckets.map((bucket, bucketIndex) => {
                        const percentage = (bucket.count / maximumCount) * 100;
                        const interval = histogramIntervalLabel(bucket.lower, bucket.upper, bucketIndex === buckets.length - 1, grouping);

                        return (
                          <div
                            className="quality-histogram__bar"
                            key={`${bucket.lower}-${bucket.upper}-${bucketIndex}`}
                            style={{ height: `${percentage}%` }}
                            title={`${interval}: ${plural(bucket.count, "fila", "filas")}`}
                          >
                            <span />
                          </div>
                        );
                      })}
                    </div>
                    <div className="quality-histogram__axis" aria-hidden="true">
                      <span>{formatStatistic(buckets[0]?.lower ?? null, grouping)}</span>
                      <span>{formatStatistic(buckets[buckets.length - 1]?.upper ?? null, grouping)}</span>
                    </div>
                    <QualityDataDetails className="quality-histogram__table">
                      <table aria-label={`Tabla de frecuencias para ${column.name}`}>
                        <caption className="visually-hidden">Tabla de frecuencias para {column.name}</caption>
                        <thead>
                          <tr>
                            <th scope="col">Intervalo</th>
                            <th scope="col">Filas</th>
                          </tr>
                        </thead>
                        <tbody>
                          {buckets.map((bucket, bucketIndex) => (
                            <tr key={`${bucket.lower}-${bucket.upper}-${bucketIndex}`}>
                              <th scope="row">
                                {histogramIntervalLabel(bucket.lower, bucket.upper, bucketIndex === buckets.length - 1, grouping)}
                              </th>
                              <td>{bucket.count.toLocaleString()}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </QualityDataDetails>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {profile.categoricalGroupSummaries?.map((summary, summaryIndex) => (
          <CategoricalGroupChart
            key={summary.column}
            summary={summary}
            summaryIndex={summaryIndex}
          />
        ))}
        {profile.numericCorrelations && profile.numericCorrelations.columns.length > 1 && (
          <NumericCorrelationChart matrix={profile.numericCorrelations} />
        )}
      </div>
    </section>
  );
}

function QualityDataDetails({
  children,
  className,
  label = "Ver datos exactos",
}: {
  children: ReactNode;
  className: string;
  label?: string;
}) {
  return (
    <details className="quality-data-details">
      <summary>{label}</summary>
      {/* ACC-09: the table scrolls, so the keyboard must be able to reach it. */}
      <div className={className} tabIndex={0} role="region" aria-label={label}>{children}</div>
    </details>
  );
}

function TemporalCoverageChart({
  columns,
  rowCount,
}: {
  columns: ColumnProfile[];
  rowCount: number;
}) {
  return (
    <div className="quality-chart quality-chart--wide quality-temporal" role="group" aria-labelledby="quality-temporal-title">
      <h5 id="quality-temporal-title">Cobertura temporal</h5>
      <p className="quality-chart__note">
        Rango mínimo–máximo y filas con valor para columnas de fecha o fecha-hora. Se calcula con
        el perfil, sin leer ni mostrar celdas.
      </p>
      <div className="quality-temporal__table">
        <table aria-label="Tabla de cobertura temporal">
          <caption className="visually-hidden">Rango y cobertura de columnas temporales</caption>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              <th scope="col">Tipo</th>
              <th scope="col">Rango detectado</th>
              <th scope="col">Filas con valor</th>
              <th scope="col">Cobertura</th>
            </tr>
          </thead>
          <tbody>
            {columns.map((column) => {
              const availableRows = Math.max(0, rowCount - column.nullCount);
              const coverage = clampPercentage(column.completenessPercentage);

              return (
                <tr key={column.name}>
                  <th scope="row">{column.name}</th>
                  <td>{temporalTypeLabel(column)}</td>
                  <td className="quality-temporal__range">
                    <span className="quality-temporal__range-value">
                      {formatTemporalValue(column.minimum)}
                      <span aria-hidden="true"> → </span>
                      <span className="visually-hidden"> hasta </span>
                      {formatTemporalValue(column.maximum)}
                    </span>
                    {(column.minimum === null || column.maximum === null) && (
                      <span className="visually-hidden">Rango parcial o no disponible</span>
                    )}
                  </td>
                  <td>{availableRows.toLocaleString()} de {Math.max(0, rowCount).toLocaleString()}</td>
                  <td>{formatPercent(coverage, 1)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        La cobertura representa valores no nulos; un rango ausente significa que el perfil no pudo
        calcular uno de sus límites.
      </p>
    </div>
  );
}

function TemporalTrendChart({
  summary,
  summaryIndex,
  numericColumns,
  datasetRevision,
  activeAggregation,
  onAcquireAggregation,
  onReleaseAggregation,
}: {
  summary: TemporalSeriesSummary;
  summaryIndex: number;
  numericColumns: ColumnProfile[];
  datasetRevision: number;
  activeAggregation: TemporalAggregationOwner | null;
  onAcquireAggregation: (column: string) => TemporalAggregationOwner | null;
  onReleaseAggregation: (token: symbol) => void;
}) {
  const titleId = `quality-temporal-trend-title-${summaryIndex}`;
  const tableLabel = `Tendencia temporal para ${summary.column}`;
  const [metric, setMetric] = useState<TemporalMetric>("rows");
  const [valueColumn, setValueColumn] = useState(numericColumns[0]?.name ?? "");
  const [aggregation, setAggregation] = useState<TemporalAggregationKind>("sum");
  const [aggregationSeries, setAggregationSeries] = useState<TemporalAggregationSeries | null>(null);
  const [aggregationStatus, setAggregationStatus] = useState<"idle" | "loading" | "cancelling" | "ready" | "error">("idle");
  const [aggregationError, setAggregationError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const requestPendingRef = useRef(false);
  const requestOwnerRef = useRef<TemporalAggregationOwner | null>(null);
  const cancellationPendingRef = useRef<Promise<void> | null>(null);
  // The owner ref changes together with `activeAggregation`, which re-renders.
  const anotherAggregationBusy = activeAggregation !== null
    // eslint-disable-next-line react/refs
    && requestOwnerRef.current?.token !== activeAggregation.token;
  const metricId = `quality-temporal-metric-${summaryIndex}`;
  const valueColumnId = `quality-temporal-value-${summaryIndex}`;
  const aggregationId = `quality-temporal-aggregation-${summaryIndex}`;
  const granularityLabel = temporalGranularityLabel(summary.granularity);
  const seriesMatchesSelection = aggregationSeries?.dateColumn === summary.column
    && aggregationSeries.valueColumn === valueColumn
    && aggregationSeries.aggregation === aggregation;

  function requestTemporalCancellation(): Promise<void> {
    if (cancellationPendingRef.current !== null) return cancellationPendingRef.current;
    let trackedCancellation: Promise<void>;
    trackedCancellation = cancelOperation("temporal").finally(() => {
      if (cancellationPendingRef.current === trackedCancellation) {
        cancellationPendingRef.current = null;
      }
    });
    cancellationPendingRef.current = trackedCancellation;
    return trackedCancellation;
  }

  // COD-06: the cleanup cancels the request of the previous data or column.
  const cancelPendingAggregation = useEffectEvent(() => {
    requestIdRef.current += 1;
    if (requestPendingRef.current && requestOwnerRef.current !== null) {
      void requestTemporalCancellation().catch(() => undefined);
    }
  });
  useEffect(() => () => cancelPendingAggregation(),
    // A new dataset or column ends the aggregation in progress.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [datasetRevision, summary.column]);

  useEffect(() => {
    setAggregationSeries(null);
    setAggregationStatus("idle");
    setAggregationError(null);
    // A new dataset or column clears the aggregation on screen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [datasetRevision, summary.column]);

  useEffect(() => {
    if (numericColumns.some((column) => column.name === valueColumn)) return;
    setValueColumn(numericColumns[0]?.name ?? "");
    // New data re-checks the chosen column.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [datasetRevision, numericColumns, valueColumn]);

  function calculateAggregation() {
    if (!valueColumn || requestPendingRef.current || aggregationStatus === "loading" || aggregationStatus === "cancelling") return;
    const owner = onAcquireAggregation(summary.column);
    if (owner === null) return;
    requestOwnerRef.current = owner;
    const requestId = ++requestIdRef.current;
    requestPendingRef.current = true;
    setAggregationStatus("loading");
    setAggregationError(null);
    setAggregationSeries(null);
    void getTemporalAggregation(summary.column, valueColumn, aggregation)
      .then((series) => {
        if (requestIdRef.current !== requestId) return;
        setAggregationSeries(series);
        setAggregationStatus("ready");
      })
      .catch((error: unknown) => {
        if (requestIdRef.current !== requestId) return;
        const message = errorMessage(error);
        if (/cancelad[oa]/i.test(message)) {
          setAggregationStatus("idle");
          return;
        }
        setAggregationError(message);
        setAggregationStatus("error");
      })
      .finally(async () => {
        const pendingCancellation = cancellationPendingRef.current;
        if (pendingCancellation !== null) await pendingCancellation.catch(() => undefined);
        if (requestOwnerRef.current?.token !== owner.token) return;
        requestPendingRef.current = false;
        requestOwnerRef.current = null;
        onReleaseAggregation(owner.token);
      });
  }

  async function cancelAggregation() {
    const owner = requestOwnerRef.current;
    if (aggregationStatus !== "loading" || owner === null || activeAggregation?.token !== owner.token) return;
    setAggregationStatus("cancelling");
    try {
      await requestTemporalCancellation();
    } catch (error: unknown) {
      setAggregationError(errorMessage(error));
      setAggregationStatus("error");
    }
  }

  return (
    <div
      className="quality-chart quality-chart--wide quality-temporal-trend"
      role="group"
      aria-labelledby={titleId}
    >
      <h5 id={titleId}>Tendencia temporal · {summary.column}</h5>
      <p className="quality-chart__note">
        {metric === "numeric"
          ? `${aggregationLabel(aggregation)} de ${valueColumn || "una columna numérica"} por ${granularityLabel}; el cálculo recorre el dataset completo y solo devuelve agregados. `
          : `${metric === "rows" ? "Conteo de filas" : "Porcentaje de valores interpretables"} por ${granularityLabel}; solo se muestran agregados del perfil, nunca valores de celdas. `}
        Se incluyen {summary.parsedRowCount.toLocaleString()}{" "}
        de {(summary.parsedRowCount + summary.unparsedRowCount).toLocaleString()} filas interpretables.
      </p>
      {summary.periods.length > 0 ? (
        <>
          <div className="quality-temporal-trend__toolbar">
            <div>
              <span className="quality-temporal-trend__metric-caption">Lectura visible</span>
              <strong>{temporalMetricLabel(metric)}</strong>
            </div>
            <label htmlFor={metricId}>
              Medir por
              <select
                id={metricId}
                value={metric}
                aria-label={`Métrica temporal para ${summary.column}`}
                onChange={(event) => {
                  if (aggregationStatus === "loading") void cancelAggregation();
                  setMetric(event.target.value as TemporalMetric);
                }}
              >
                <option value="rows">Filas</option>
                <option value="percentage">Porcentaje</option>
                {numericColumns.length > 0 && <option value="numeric">Métrica numérica</option>}
              </select>
            </label>
            {metric === "numeric" && (
              <>
                <label htmlFor={valueColumnId}>
                  Columna numérica
                  <select
                    id={valueColumnId}
                    value={valueColumn}
                    onChange={(event) => {
                      if (aggregationStatus === "loading") void cancelAggregation();
                      setValueColumn(event.target.value);
                    }}
                  >
                    {numericColumns.map((column) => (
                      <option key={column.name} value={column.name}>{column.name}</option>
                    ))}
                  </select>
                </label>
                <label htmlFor={aggregationId}>
                  Agregación
                  <select
                    id={aggregationId}
                    value={aggregation}
                    onChange={(event) => {
                      if (aggregationStatus === "loading") void cancelAggregation();
                      setAggregation(event.target.value as TemporalAggregationKind);
                    }}
                  >
                    <option value="sum">Suma</option>
                    <option value="mean">Promedio</option>
                  </select>
                </label>
                <button
                  className="secondary-action"
                  type="button"
                  onClick={calculateAggregation}
                  // The pending ref flips together with `aggregationStatus`.
                  // eslint-disable-next-line react/refs
                  disabled={!valueColumn || anotherAggregationBusy || requestPendingRef.current || aggregationStatus === "loading" || aggregationStatus === "cancelling"}
                >
                  {aggregationStatus === "loading" ? "Calculando…" : anotherAggregationBusy ? "Esperando…" : "Calcular tendencia"}
                </button>
              </>
            )}
          </div>
          {metric !== "numeric" ? (
            <TemporalLineChart
              summary={summary}
              metric={metric}
              titleId={`${titleId}-chart`}
            />
          ) : aggregationStatus === "loading" || aggregationStatus === "cancelling" ? (
            <div className="quality-temporal-empty" role="status" aria-live="polite">
              <span>{aggregationStatus === "cancelling" ? "Cancelando el cálculo temporal…" : "Calculando una serie temporal con el dataset completo…"}</span>
              {aggregationStatus === "loading" && (
                <button type="button" className="secondary-action" onClick={() => void cancelAggregation()}>
                  Cancelar cálculo
                </button>
              )}
            </div>
          ) : aggregationStatus === "error" ? (
            <p className="notice notice--error" role="alert">
              No se pudo calcular la tendencia numérica: {aggregationError}
            </p>
          ) : seriesMatchesSelection && aggregationSeries ? (
            <TemporalAggregationChart
              summary={summary}
              series={aggregationSeries}
              titleId={`${titleId}-numeric-chart`}
            />
          ) : anotherAggregationBusy ? (
            <p className="quality-temporal-empty" role="status">
              Otra tendencia temporal se está calculando; espera a que termine antes de iniciar esta.
            </p>
          ) : (
            <p className="quality-temporal-empty" role="status">
              Elige una métrica y calcula la tendencia sobre todas las filas; solo se devolverán agregados.
            </p>
          )}
          {metric !== "numeric" && summary.granularity === "day" && (
            <QualityDataDetails className="quality-calendar-details" label="Ver calendario diario">
              <DailyTemporalCalendar summary={summary} />
            </QualityDataDetails>
          )}
        </>
      ) : metric !== "numeric" && summary.granularity === "day" ? (
        <QualityDataDetails className="quality-calendar-details" label="Ver calendario diario">
          <DailyTemporalCalendar summary={summary} />
        </QualityDataDetails>
      ) : (
        <p className="quality-temporal-empty" role="status">
          No hay periodos interpretables para mostrar en esta columna.
        </p>
      )}
      {metric === "numeric" ? (
        seriesMatchesSelection && aggregationSeries ? (
          <QualityDataDetails className="quality-temporal-trend__table" label="Ver datos agregados">
          <div role="region" tabIndex={0} aria-label={`${tableLabel}: ${aggregationLabel(aggregation)} de ${valueColumn}`}>
            <table aria-label={`${tableLabel}: ${aggregationLabel(aggregation)} de ${valueColumn}`}>
              <caption className="visually-hidden">{tableLabel}: {aggregationLabel(aggregation)} de {valueColumn}</caption>
              <thead>
                <tr>
                  <th scope="col">Periodo</th>
                  <th scope="col">Filas con fecha</th>
                  <th scope="col">Valores numéricos válidos</th>
                  <th scope="col">{aggregationLabel(aggregation)} de {valueColumn}</th>
                </tr>
              </thead>
              <tbody>
                {aggregationSeries.periods.map((period) => (
                  <tr key={`table-${period.period}`}>
                    <th scope="row">{period.period}</th>
                    <td>{period.rowCount.toLocaleString()}</td>
                    <td>{period.valueCount.toLocaleString()}</td>
                    <td>{formatTemporalAggregate(period.value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          </QualityDataDetails>
        ) : null
      ) : (
        <QualityDataDetails className="quality-temporal-trend__table">
          <div role="region" tabIndex={0} aria-label={tableLabel}>
            <table aria-label={tableLabel}>
              <caption className="visually-hidden">{tableLabel}</caption>
              <thead>
                <tr>
                  <th scope="col">Periodo</th>
                  <th scope="col">Filas</th>
                  <th scope="col">Porcentaje de valores interpretables</th>
                </tr>
              </thead>
              <tbody>
                {summary.periods.map((period) => (
                  <tr key={`table-${period.period}`}>
                    <th scope="row">{period.period}</th>
                    <td>{period.rowCount.toLocaleString()}</td>
                    <td>{formatPercent(clampPercentage(period.percentage), 1)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </QualityDataDetails>
      )}
      <p className="profile-note">
        {metric === "numeric" && seriesMatchesSelection
          ? "Nulos y valores no numéricos se excluyen del cálculo; cada periodo muestra cuántos valores válidos se usaron. Los periodos vacíos aparecen sin valor, no como cero. "
          : ""}
        {plural(summary.unparsedRowCount, "fila", "filas")} sin periodo interpretable.
        {summary.truncated ? " Los periodos más antiguos se agruparon para mantener la lectura rápida." : ""}
      </p>
    </div>
  );
}

type TemporalMetric = "rows" | "percentage" | "numeric";

interface TemporalAggregationOwner {
  token: symbol;
  column: string;
}

function TemporalAggregationChart({
  summary,
  series,
  titleId,
}: {
  summary: TemporalSeriesSummary;
  series: TemporalAggregationSeries;
  titleId: string;
}) {
  const chartHeight = 188;
  const chartWidth = Math.max(560, series.periods.length * 72);
  const padding = { top: 18, right: 18, bottom: 34, left: 72 };
  const plotWidth = Math.max(1, chartWidth - padding.left - padding.right);
  const plotHeight = chartHeight - padding.top - padding.bottom;
  const finiteValues = series.periods.flatMap((period) =>
    period.value !== null && Number.isFinite(period.value) ? [period.value] : [],
  );
  const title = `${aggregationLabel(series.aggregation)} de ${series.valueColumn}`;
  if (finiteValues.length === 0) {
    return (
      <p className="quality-temporal-empty" role="status">
        No hay valores numéricos válidos para representar en los periodos interpretables.
      </p>
    );
  }

  let minimumValue = Math.min(0, ...finiteValues);
  let maximumValue = Math.max(0, ...finiteValues);
  if (minimumValue === maximumValue) {
    minimumValue -= 1;
    maximumValue += 1;
  }
  const scaleMagnitude = Math.max(1, Math.abs(minimumValue), Math.abs(maximumValue));
  const scaledMinimum = minimumValue / scaleMagnitude;
  const scaledMaximum = maximumValue / scaleMagnitude;
  const scaledRange = scaledMaximum - scaledMinimum;
  const valueY = (value: number) => padding.top + ((scaledMaximum - value / scaleMagnitude) / scaledRange) * plotHeight;
  const points = series.periods.map((period, index) => {
    const x = series.periods.length === 1
      ? padding.left + plotWidth / 2
      : padding.left + (index / (series.periods.length - 1)) * plotWidth;
    return {
      x,
      y: period.value === null || !Number.isFinite(period.value) ? null : valueY(period.value),
      period,
    };
  });
  const segments: typeof points[] = [];
  let currentSegment: typeof points = [];
  for (const point of points) {
    if (point.y === null) {
      if (currentSegment.length > 0) segments.push(currentSegment);
      currentSegment = [];
    } else {
      currentSegment.push(point);
    }
  }
  if (currentSegment.length > 0) segments.push(currentSegment);
  const baseline = valueY(0);
  const labelIndexes = temporalAxisIndexes(series.periods.length);
  const descriptionId = `${titleId}-description`;

  return (
    <div className="quality-temporal-line" role="group" aria-labelledby={titleId}>
      <div className="quality-temporal-line__legend">
        <span><i aria-hidden="true" /> {title}</span>
        <span>Valores válidos: {series.periods.reduce((total, period) => total + period.valueCount, 0).toLocaleString()}</span>
      </div>
      <div className="quality-temporal-line__viewport" role="region" tabIndex={0} aria-label={`Gráfica desplazable: ${title}`}>
        <svg
          className="quality-temporal-line__chart"
          width={chartWidth}
          height={chartHeight}
          style={{ width: `max(100%, ${chartWidth}px)` }}
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          role="img"
          aria-labelledby={`${titleId} ${descriptionId}`}
        >
          <title id={titleId}>Tendencia temporal: {title} por {series.granularity === "day" ? "día" : series.granularity === "month" ? "mes" : "año"}</title>
          <desc id={descriptionId}>
            Línea calculada con el dataset completo y {series.periods.length.toLocaleString()} periodos. Los periodos sin valores válidos dejan un hueco. La tabla muestra los valores exactos.
          </desc>
          {[0, 0.5, 1].map((ratio) => {
            const y = padding.top + plotHeight * ratio;
            const value = (scaledMaximum * (1 - ratio) + scaledMinimum * ratio) * scaleMagnitude;
            return (
              <g key={ratio}>
                <line
                  className="quality-temporal-line__grid"
                  x1={padding.left}
                  x2={chartWidth - padding.right}
                  y1={y}
                  y2={y}
                />
                <text className="quality-temporal-line__scale" x={padding.left - 10} y={y + 4} textAnchor="end">
                  {formatStatistic(value)}
                </text>
              </g>
            );
          })}
          {baseline > padding.top && baseline < padding.top + plotHeight && (
            <line
              className="quality-temporal-line__grid quality-temporal-line__grid--zero"
              x1={padding.left}
              x2={chartWidth - padding.right}
              y1={baseline}
              y2={baseline}
            />
          )}
          {segments.map((segment, index) => (
            <polyline
              className="quality-temporal-line__path"
              key={`segment-${index}`}
              points={segment.map(({ x, y }) => `${x.toFixed(2)},${y?.toFixed(2)}`).join(" ")}
              aria-hidden="true"
            />
          ))}
          {points.filter((point) => point.y !== null).map(({ x, y, period }) => (
            <circle className="quality-temporal-line__point" key={period.period} cx={x} cy={y!} r="4">
              <title>{`${period.period}: ${formatTemporalAggregate(period.value)}`}</title>
            </circle>
          ))}
          {labelIndexes.map((index) => {
            const period = series.periods[index];
            const point = points[index];
            return period && point ? (
              <text
                className="quality-temporal-line__label"
                key={period.period}
                x={point.x}
                y={chartHeight - 8}
                textAnchor={index === 0 ? "start" : index === series.periods.length - 1 ? "end" : "middle"}
              >
                {formatTemporalAxisLabel(period.period, summary.granularity)}
              </text>
            ) : null;
          })}
        </svg>
      </div>
    </div>
  );
}

function aggregationLabel(aggregation: TemporalAggregationKind): string {
  return aggregation === "sum" ? "Suma" : "Promedio";
}

function temporalGranularityLabel(granularity: TemporalSeriesSummary["granularity"]): string {
  return granularity === "day" ? "día" : granularity === "month" ? "mes" : "año";
}

function formatTemporalAggregate(value: number | null): string {
  return value === null || !Number.isFinite(value)
    ? "—"
    : value.toLocaleString(undefined, { maximumFractionDigits: 6 });
}

function TemporalLineChart({
  summary,
  metric,
  titleId,
}: {
  summary: TemporalSeriesSummary;
  metric: Exclude<TemporalMetric, "numeric">;
  titleId: string;
}) {
  const chartHeight = 188;
  const chartWidth = Math.max(560, summary.periods.length * 72);
  const padding = { top: 18, right: 18, bottom: 34, left: 56 };
  const plotWidth = Math.max(1, chartWidth - padding.left - padding.right);
  const plotHeight = chartHeight - padding.top - padding.bottom;
  const values = summary.periods.map((period) => temporalMetricValue(period, metric));
  const maximumValue = metric === "percentage"
    ? 100
    : Math.max(1, ...values);
  const points = summary.periods.map((period, index) => {
    const x = summary.periods.length === 1
      ? padding.left + plotWidth / 2
      : padding.left + (index / (summary.periods.length - 1)) * plotWidth;
    const y = padding.top + plotHeight - ((values[index] ?? 0) / maximumValue) * plotHeight;
    return { x, y, period };
  });
  const pointList = points.map(({ x, y }) => `${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
  const baseline = padding.top + plotHeight;
  const areaPath = points.length > 0
    ? `M ${points[0]!.x.toFixed(2)} ${baseline.toFixed(2)} L ${points.map(({ x, y }) => `${x.toFixed(2)} ${y.toFixed(2)}`).join(" L ")} L ${points.at(-1)!.x.toFixed(2)} ${baseline.toFixed(2)} Z`
    : "";
  const labelIndexes = temporalAxisIndexes(summary.periods.length);
  const descriptionId = `${titleId}-description`;

  return (
    <div className="quality-temporal-line" role="group" aria-labelledby={titleId}>
      <div className="quality-temporal-line__legend">
        <span><i aria-hidden="true" /> {temporalMetricLabel(metric)}</span>
        <span>Escala máxima: {temporalMetricDisplay(maximumValue, metric)}</span>
      </div>
      <div
        className="quality-temporal-line__viewport"
        role="region"
        tabIndex={0}
        aria-label={`Gráfica desplazable para ${summary.column} · ${temporalMetricLabel(metric)}`}
      >
        <svg
          className="quality-temporal-line__chart"
          width={chartWidth}
          height={chartHeight}
          style={{ width: `max(100%, ${chartWidth}px)` }}
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          role="img"
          aria-labelledby={`${titleId} ${descriptionId}`}
        >
          <title id={titleId}>
            Serie temporal de {summary.column} por {temporalMetricLabel(metric).toLowerCase()}
          </title>
          <desc id={descriptionId}>
            Línea con {summary.periods.length.toLocaleString()} periodos. La tabla inferior contiene los mismos datos.
          </desc>
          {[0, 0.5, 1].map((ratio) => {
            const y = padding.top + plotHeight * ratio;
            return (
              <line
                className="quality-temporal-line__grid"
                key={ratio}
                x1={padding.left}
                x2={chartWidth - padding.right}
                y1={y}
                y2={y}
              />
            );
          })}
          <path className="quality-temporal-line__area" d={areaPath} aria-hidden="true" />
          <polyline className="quality-temporal-line__path" points={pointList} aria-hidden="true" />
          <text className="quality-temporal-line__scale" x={padding.left - 10} y={padding.top + 4} textAnchor="end">
            {temporalMetricDisplay(maximumValue, metric)}
          </text>
          <text
            className="quality-temporal-line__scale"
            x={padding.left - 10}
            y={padding.top + plotHeight / 2 + 4}
            textAnchor="end"
          >
            {temporalMetricDisplay(maximumValue / 2, metric)}
          </text>
          <text className="quality-temporal-line__scale" x={padding.left - 10} y={baseline + 4} textAnchor="end">
            {temporalMetricDisplay(0, metric)}
          </text>
          {points.map(({ x, y, period }) => (
            <circle className="quality-temporal-line__point" key={period.period} cx={x} cy={y} r="4">
              <title>{`${period.period}: ${temporalMetricDisplay(temporalMetricValue(period, metric), metric)}`}</title>
            </circle>
          ))}
          {labelIndexes.map((index) => {
            const period = summary.periods[index];
            const point = points[index];
            return period && point ? (
              <text
                className="quality-temporal-line__label"
                key={period.period}
                x={point.x}
                y={chartHeight - 8}
                textAnchor={index === 0 ? "start" : index === summary.periods.length - 1 ? "end" : "middle"}
              >
                {formatTemporalAxisLabel(period.period, summary.granularity)}
              </text>
            ) : null;
          })}
        </svg>
      </div>
    </div>
  );
}

/** Calendar days never filled beyond about thirteen months. */
const MAX_CALENDAR_DAYS = 400;

/**
 * FUN-52: the calendar grid assumes one cell per day. Days the engine did not
 * return between the first and the last are added with zero rows, so each
 * day lands under its weekday; a period that is not a day goes at the end.
 */
export function contiguousCalendarDays(periods: TemporalPeriod[]): TemporalPeriod[] {
  const days = periods.filter((period) => parseTemporalDay(period.period) !== null);
  const others = periods.filter((period) => parseTemporalDay(period.period) === null);
  if (days.length < 2) return [...days, ...others];
  const byDay = new Map(days.map((period) => [period.period, period]));
  // `days` has at least two entries here.
  const first = parseTemporalDay(days[0]!.period)!;
  const last = parseTemporalDay(days[days.length - 1]!.period)!;
  const span = Math.round((last.getTime() - first.getTime()) / 86_400_000) + 1;
  if (span <= days.length || span > MAX_CALENDAR_DAYS) return [...days, ...others];
  const filled: TemporalPeriod[] = [];
  for (let offset = 0; offset < span; offset += 1) {
    const key = new Date(first.getTime() + offset * 86_400_000).toISOString().slice(0, 10);
    filled.push(byDay.get(key) ?? { period: key, rowCount: 0, percentage: 0 });
  }
  return [...filled, ...others];
}

function DailyTemporalCalendar({ summary: rawSummary }: { summary: TemporalSeriesSummary }) {
  const summary = { ...rawSummary, periods: contiguousCalendarDays(rawSummary.periods) };
  const calendarLabel = `Calendario diario para ${summary.column}`;
  const firstWeekday = temporalDayWeekday(summary.periods[0]?.period);
  const leadingEmptyDays = firstWeekday === null ? 0 : firstWeekday;
  const maximumCount = Math.max(1, ...summary.periods.map((period) => period.rowCount));

  if (summary.periods.length === 0) {
    return (
      <div className="quality-temporal-calendar quality-temporal-calendar--empty" role="group" aria-label={calendarLabel}>
        <h6>Calendario diario</h6>
        <p className="quality-temporal-empty" role="status">
          No hay días interpretables para mostrar en esta columna.
        </p>
      </div>
    );
  }

  return (
    <div className="quality-temporal-calendar" role="group" aria-label={calendarLabel}>
      <div className="quality-temporal-calendar__heading">
        <h6>Calendario diario</h6>
        <p>La intensidad resume la cantidad de filas; los días sin filas permanecen visibles.</p>
      </div>
      <div className="quality-temporal-calendar__weekdays" aria-hidden="true">
        {[
          "Lun",
          "Mar",
          "Mié",
          "Jue",
          "Vie",
          "Sáb",
          "Dom",
        ].map((day) => <span key={day}>{day}</span>)}
      </div>
      <ol className="quality-temporal-calendar__days" aria-label={calendarLabel}>
        {Array.from({ length: leadingEmptyDays }, (_, index) => (
          <li className="quality-temporal-calendar__empty-day" aria-hidden="true" key={`empty-${index}`} />
        ))}
        {summary.periods.map((period) => {
          const level = period.rowCount === 0
            ? 0
            : Math.max(1, Math.ceil((period.rowCount / maximumCount) * 4));
          const rowLabel = `${period.rowCount.toLocaleString()} ${period.rowCount === 1 ? "fila" : "filas"}`;

          return (
            <li
              className={`quality-temporal-calendar__day quality-temporal-calendar__day--level-${level}`}
              key={period.period}
              aria-label={`${period.period}: ${rowLabel}, ${formatPercent(clampPercentage(period.percentage), 1)} de los valores interpretables`}
            >
              <time dateTime={period.period}>{formatCalendarDay(period.period)}</time>
              <strong>{period.rowCount.toLocaleString()}</strong>
              <small>filas</small>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function CategoricalGroupChart({
  summary,
  summaryIndex,
}: {
  summary: CategoricalGroupSummary;
  summaryIndex: number;
}) {
  const titleId = `quality-groups-title-${summaryIndex}`;
  const tableLabel = `Resumen de grupos para ${summary.column}`;

  return (
    <div className="quality-chart quality-chart--wide quality-groups" role="group" aria-labelledby={titleId}>
      <h5 id={titleId}>Distribución por categoría</h5>
      <p className="quality-chart__note">
        Principales categorías de <strong>{summary.column}</strong>. Se muestran como máximo ocho
        grupos; los valores restantes se reúnen en “Resto” para mantener la lectura rápida y
        reducir la exposición de valores poco frecuentes.
      </p>
      <div className="quality-chart__bars" role="list" aria-label={`Distribución de grupos para ${summary.column}`}>
        {summary.groups.map((group) => {
          const percentage = clampPercentage(group.percentage);

          return (
            <div className="quality-chart__item" role="listitem" key={`${group.label}-${group.isOther}`}>
              <div className="quality-chart__label">
                <span title={group.label}>{group.label}</span>
                <strong>{group.rowCount.toLocaleString()} filas · {formatPercent(percentage, 1)}</strong>
              </div>
              <div className="quality-chart__track" aria-hidden="true">
                <span
                  className={group.isOther ? "quality-chart__track-fill--other" : undefined}
                  style={{ width: `${percentage}%` }}
                />
              </div>
            </div>
          );
        })}
      </div>
      <QualityDataDetails className="quality-chart__table">
        <table aria-label={tableLabel}>
          <caption className="visually-hidden">{tableLabel}</caption>
          <thead>
            <tr>
              <th scope="col">Grupo</th>
              <th scope="col">Filas</th>
              <th scope="col">Porcentaje</th>
            </tr>
          </thead>
          <tbody>
            {summary.groups.map((group) => (
              <tr key={`table-${group.label}-${group.isOther}`}>
                <th scope="row">
                  {group.label}
                  {group.isOther && <span className="visually-hidden">, categorías restantes</span>}
                </th>
                <td>{group.rowCount.toLocaleString()}</td>
                <td>{formatPercent(clampPercentage(group.percentage), 1)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </QualityDataDetails>
      <p className="profile-note">
        {summary.distinctCount.toLocaleString()} valores distintos detectados
        {summary.truncated ? "; el resto está agrupado para evitar ruido y preservar privacidad." : "."}
      </p>
    </div>
  );
}

/** PROD-20: whether the correlations read every row or a sample, and of how many. */
function correlationSampleText(matrix: NumericCorrelationMatrix): string {
  const sampled = plural(matrix.sampledRowCount, "fila", "filas");
  if (matrix.rowCount == null) return `La lectura usa ${sampled}`;
  if (matrix.rowCount <= matrix.sampledRowCount) {
    return matrix.rowCount === 1 ? "La lectura usa la única fila" : `La lectura usa las ${plural(matrix.rowCount, "fila", "filas")}`;
  }
  return `La lectura usa una muestra de ${matrix.sampledRowCount.toLocaleString()} de ${plural(matrix.rowCount, "fila", "filas")} repartida por todo el archivo`;
}

function NumericCorrelationChart({ matrix }: { matrix: NumericCorrelationMatrix }) {
  return (
    <div className="quality-chart quality-chart--wide quality-correlation" role="group" aria-labelledby="quality-correlation-title">
      <h5 id="quality-correlation-title">Correlaciones numéricas</h5>
      <p className="quality-chart__note">
        Pearson entre pares disponibles. {correlationSampleText(matrix)}
        {matrix.truncated ? " y muestra las primeras 12 columnas numéricas" : ""}.
      </p>
      <div className="quality-correlation__table" role="region" tabIndex={0} aria-label="Matriz de correlaciones numéricas">
        <table>
          <caption className="visually-hidden">Matriz de correlaciones numéricas de Pearson</caption>
          <thead>
            <tr>
              <th scope="col">Columna</th>
              {matrix.columns.map((column) => (
                <th scope="col" key={column} title={column}>{column}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {matrix.columns.map((rowColumn, rowIndex) => (
              <tr key={rowColumn}>
                <th scope="row" title={rowColumn}>{rowColumn}</th>
                {matrix.columns.map((column, columnIndex) => {
                  const coefficient = correlationAt(matrix, rowIndex, columnIndex);
                  const label = correlationLabel(coefficient);
                  const isDiagonal = rowIndex === columnIndex;

                  return (
                    <td
                      className={correlationClass(coefficient, isDiagonal)}
                      key={column}
                      aria-label={`${rowColumn} con ${column}: ${label}`}
                    >
                      {label}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="profile-note">
        Un valor cercano a 1 o -1 indica una relación lineal fuerte; “—” significa que no hubo
        suficientes valores o variación para calcularla.
      </p>
    </div>
  );
}

function correlationAt(matrix: NumericCorrelationMatrix, rowIndex: number, columnIndex: number): number | null {
  if (rowIndex === columnIndex) return 1;
  const firstIndex = Math.min(rowIndex, columnIndex);
  const secondIndex = Math.max(rowIndex, columnIndex);
  const firstColumn = matrix.columns[firstIndex];
  const secondColumn = matrix.columns[secondIndex];
  return (
    matrix.pairs.find(
      (pair) => pair.firstColumn === firstColumn && pair.secondColumn === secondColumn,
    )?.coefficient ?? null
  );
}

function correlationLabel(coefficient: number | null): string {
  return coefficient === null || !Number.isFinite(coefficient) ? "—" : formatDecimal(coefficient, 2);
}

function correlationClass(coefficient: number | null, isDiagonal: boolean): string {
  if (isDiagonal) return "quality-correlation__cell quality-correlation__cell--diagonal";
  if (coefficient === null || !Number.isFinite(coefficient)) return "quality-correlation__cell";
  if (coefficient >= 0.7 || coefficient <= -0.7) return "quality-correlation__cell quality-correlation__cell--strong";
  if (coefficient >= 0.3 || coefficient <= -0.3) return "quality-correlation__cell quality-correlation__cell--moderate";
  return "quality-correlation__cell quality-correlation__cell--weak";
}

function isTemporalColumn(column: ColumnProfile): boolean {
  const dataType = column.dataType.trim().toLowerCase();
  return dataType === "date"
    || dataType.includes("datetime")
    || dataType.includes("timestamp")
    || column.suggestedType === "date";
}

function temporalTypeLabel(column: ColumnProfile): string {
  const dataType = column.dataType.trim().toLowerCase();
  if (dataType === "date") return "Fecha";
  if (dataType.includes("datetime") || dataType.includes("timestamp")) return "Fecha y hora";
  return "Fecha detectada";
}

function formatTemporalValue(value: string | null): string {
  if (!value) return "No disponible";
  return value.replace("T", " ").replace(/\+00:00$/, " UTC");
}

function temporalMetricLabel(metric: TemporalMetric): string {
  if (metric === "rows") return "Filas";
  if (metric === "percentage") return "Porcentaje de valores";
  return "Métrica numérica";
}

function temporalMetricValue(
  period: { rowCount: number; percentage: number },
  metric: Exclude<TemporalMetric, "numeric">,
): number {
  return metric === "rows" ? Math.max(0, period.rowCount) : clampPercentage(period.percentage);
}

function temporalMetricDisplay(value: number, metric: Exclude<TemporalMetric, "numeric">): string {
  return metric === "rows" ? Math.round(value).toLocaleString() : `${formatPercent(clampPercentage(value), 0)}`;
}

function temporalAxisIndexes(periodCount: number): number[] {
  if (periodCount <= 3) return Array.from({ length: periodCount }, (_, index) => index);
  const indexes = [0, Math.floor((periodCount - 1) / 2), periodCount - 1];
  return [...new Set(indexes)];
}

function formatTemporalAxisLabel(period: string, granularity: TemporalSeriesSummary["granularity"]): string {
  if (granularity === "day") return formatCalendarDay(period);
  return period;
}

function parseTemporalDay(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  if (
    date.getUTCFullYear() !== Number(match[1])
    || date.getUTCMonth() !== Number(match[2]) - 1
    || date.getUTCDate() !== Number(match[3])
  ) {
    return null;
  }
  return date;
}

function temporalDayWeekday(value: string | undefined): number | null {
  if (!value) return null;
  const date = parseTemporalDay(value);
  return date ? (date.getUTCDay() + 6) % 7 : null;
}

function formatCalendarDay(value: string): string {
  const date = parseTemporalDay(value);
  return date
    ? new Intl.DateTimeFormat("es", {
        day: "numeric",
        month: "short",
        timeZone: "UTC",
      }).format(date)
    : value;
}

function clampPercentage(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

export function suggestedTypeLabel(type: string | null): string {
  switch (type) {
    case "boolean":
      return "Booleano";
    case "integer":
      return "Entero";
    case "decimal":
      return "Decimal";
    case "date":
      return "Fecha";
    default:
      return "—";
  }
}

export function profileColumnTypeLabel(column: ColumnProfile): string {
  const dataType = column.dataType.trim().toLowerCase();
  let storedType = column.dataType;

  if (dataType === "str" || dataType === "string") storedType = "Texto";
  else if (dataType === "bool" || dataType === "boolean") storedType = "Booleano";
  else if (dataType === "date") storedType = "Fecha";
  else if (dataType.includes("datetime") || dataType.includes("timestamp")) {
    storedType = "Fecha y hora";
  } else if (/^(?:u?int\d*|[iu]\d+)$/.test(dataType)) {
    storedType = "Entero";
  } else if (/^(?:float\d*|f\d+|decimal)/.test(dataType)) {
    storedType = "Decimal";
  }

  const storedLabel = `Almacenado como ${storedType}`;
  return column.suggestedType
    ? `${storedLabel} · sugerido: ${suggestedTypeLabel(column.suggestedType)}`
    : storedLabel;
}

export function formatStatistic(value: number | null, useGrouping = true): string {
  if (value === null) return "—";
  // FUN-28: small magnitudes keep their significant digits instead of «0».
  if (value !== 0 && Math.abs(value) < 0.001) {
    return value.toLocaleString(undefined, { maximumSignificantDigits: 3, useGrouping });
  }
  // UX-08: -0 (and -0.0001 rounded to three decimals) reads «0».
  return withoutNegativeZero(value, 3).toLocaleString(undefined, { maximumFractionDigits: 3, useGrouping });
}

/**
 * TXT-06: whole numbers that are years or codes (1925, 1001) read without a
 * thousands separator: «1925», not «1,925».
 */
export function readsAsCode(name: string, minimum: number, maximum: number): boolean {
  // UX-08: the name decides first; a year's histogram bounds or quartiles
  // need not be whole numbers.
  if (/(^|[_\s-])(id|cod|codigo|code|year|anio|año)([_\s-]|$)/i.test(name)) return true;
  if (!Number.isInteger(minimum) || !Number.isInteger(maximum)) return false;
  return minimum >= 1000 && maximum <= 2999;
}

function histogramIntervalLabel(lower: number, upper: number, includesMaximum: boolean, useGrouping = true): string {
  return `${formatStatistic(lower, useGrouping)} – ${formatStatistic(upper, useGrouping)}${includesMaximum ? " (incluye máximo)" : ""}`;
}
