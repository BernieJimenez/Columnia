import type { ExploreFilter, ExploreKpi, ExploreLayout, ExplorePanel } from "../../bridge";

export const MAX_CHARTS = 6;

/** One filter per column: several values for bars, one range for the histogram. */
export function toggleValue(filters: ExploreFilter[], column: string, value: string | null): ExploreFilter[] {
  const current = filters.find((filter) => filter.column === column);
  const others = filters.filter((filter) => filter.column !== column);
  const values = current?.values ?? [];
  const next = values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
  return next.length === 0 ? others : [...others, { column, values: next }];
}

/** `lastBin` keeps the upper edge, as the histogram counts it there. */
export function toggleRange(
  filters: ExploreFilter[],
  column: string,
  min: number,
  max: number,
  lastBin = true,
): ExploreFilter[] {
  const current = filters.find((filter) => filter.column === column);
  const others = filters.filter((filter) => filter.column !== column);
  const same = current?.range?.min === min && current.range.max === max;
  const range = lastBin ? { min, max } : { min, max, exclusiveMax: true };
  return same ? others : [...others, { column, range }];
}

export function isValueSelected(filters: ExploreFilter[], column: string, value: string | null): boolean {
  return filters.some((filter) => filter.column === column && (filter.values ?? []).includes(value));
}

export function isRangeSelected(filters: ExploreFilter[], column: string, min: number, max: number): boolean {
  return filters.some((filter) => filter.column === column && filter.range?.min === min && filter.range.max === max);
}

/** One trend period at a time; a second click clears it. */
export function togglePeriod(filters: ExploreFilter[], column: string, period: string): ExploreFilter[] {
  const others = filters.filter((filter) => filter.column !== column);
  return isPeriodSelected(filters, column, period) ? others : [...others, { column, period }];
}

export function isPeriodSelected(filters: ExploreFilter[], column: string, period: string): boolean {
  return filters.some((filter) => filter.column === column && filter.period === period);
}

/** Adds or removes a bar chart, starting from the ones on screen. */
export function toggleChart(layout: ExploreLayout, shown: string[], column: string): ExploreLayout {
  const current = layout.categories ?? shown;
  const categories = current.includes(column)
    ? current.filter((item) => item !== column)
    : [...current, column].slice(0, MAX_CHARTS);
  return { ...layout, categories };
}

/** «Ver todos» / «Ver menos» for one bar chart. */
export function toggleExpanded(layout: ExploreLayout, column: string): ExploreLayout {
  const current = layout.expanded ?? [];
  const expanded = current.includes(column) ? current.filter((item) => item !== column) : [...current, column];
  return { ...layout, expanded };
}

/**
 * Three to five axis marks that sit on real bin edges (never between two
 * columns). `position` is the edge's place along the plot, from 0 to 1.
 */
export function axisTicks(bins: { lower: number; upper: number }[]): { value: number; position: number }[] {
  if (bins.length === 0) return [];
  const edges = [...bins.map((bin) => bin.lower), bins[bins.length - 1]!.upper];
  const last = edges.length - 1;
  const picked = edges.length <= 5
    ? edges.map((_, index) => index)
    : [...new Set([0, 1, 2, 3, 4].map((step) => Math.round((last * step) / 4)))];
  return picked.map((index) => ({ value: edges[index]!, position: index / last }));
}

export function valueLabel(value: string | null): string {
  return value === null ? "nulo" : value;
}

export function formatNumber(value: number, fractionDigits = 0, useGrouping = true): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: fractionDigits, useGrouping });
}

/** TXT-06: whole numbers between 1000 and 2999 read as years, written as such. */
export function looksLikeYears(lower: number, upper: number): boolean {
  return Number.isInteger(lower) && Number.isInteger(upper) && lower >= 1000 && upper <= 2999;
}

/** One histogram bin: «1925–1929» for whole-number bins, «1,5–3,2» otherwise. */
export function binLabel(lower: number, upper: number, integer: boolean, last: boolean, years: boolean): string {
  if (!integer) return `${formatNumber(lower, 2)} a ${formatNumber(upper, 2)}`;
  const top = last ? upper : upper - 1;
  const text = (value: number) => formatNumber(value, 0, !years);
  return top <= lower ? text(lower) : `${text(lower)} a ${text(top)}`;
}

/** FUN-78: what a median, a mean or the histogram left out. */
export function ignoredRowsText(count: number | undefined): string | null {
  if (!count) return null;
  return count === 1 ? "1 fila sin número no cuenta" : `${formatNumber(count)} filas sin número no cuentan`;
}

type Histogram = NonNullable<ExplorePanel["histogram"]>;

/** Whether a histogram's whole-number bins read as years. */
export function histogramYears(histogram: Pick<Histogram, "integer" | "bins">): boolean {
  return histogram.integer && looksLikeYears(histogram.bins[0]?.lower ?? 0, histogram.bins.at(-1)?.upper ?? 0);
}

/**
 * Chip text for an active filter. A range reads like the histogram bin it came
 * from («100 a 149», years as years), so it needs that histogram.
 */
export function filterLabel(filter: ExploreFilter, histogram?: Pick<Histogram, "column" | "integer" | "bins"> | null): string {
  if (filter.period) return `${filter.column}: ${filter.period}`;
  if (filter.range) {
    const { min, max, exclusiveMax } = filter.range;
    const label = histogram?.column === filter.column
      ? binLabel(min, max, histogram.integer, !exclusiveMax, histogramYears(histogram))
      : `${formatNumber(min, 2)} a ${formatNumber(max, 2)}`;
    return `${filter.column}: ${label}`;
  }
  return `${filter.column}: ${(filter.values ?? []).map(valueLabel).join(", ")}`;
}

export function kpiLabel(kpi: ExploreKpi): string {
  if (kpi.kind === "count") return "Filas";
  return kpi.kind === "median" ? `Mediana de ${kpi.column}` : `Media de ${kpi.column}`;
}

export function kpiValue(kpi: ExploreKpi): string {
  if (kpi.value === null) return "sin valores numéricos";
  return formatNumber(kpi.value, kpi.kind === "count" ? 0 : 2);
}
