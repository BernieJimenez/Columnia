import type { ExploreFilter, ExploreKpi } from "../../bridge";

/** One filter per column: several values for bars, one range for the histogram. */
export function toggleValue(filters: ExploreFilter[], column: string, value: string | null): ExploreFilter[] {
  const current = filters.find((filter) => filter.column === column);
  const others = filters.filter((filter) => filter.column !== column);
  const values = current?.values ?? [];
  const next = values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
  return next.length === 0 ? others : [...others, { column, values: next }];
}

export function toggleRange(filters: ExploreFilter[], column: string, min: number, max: number): ExploreFilter[] {
  const current = filters.find((filter) => filter.column === column);
  const others = filters.filter((filter) => filter.column !== column);
  const same = current?.range?.min === min && current.range.max === max;
  return same ? others : [...others, { column, range: { min, max } }];
}

export function isValueSelected(filters: ExploreFilter[], column: string, value: string | null): boolean {
  return filters.some((filter) => filter.column === column && (filter.values ?? []).includes(value));
}

export function isRangeSelected(filters: ExploreFilter[], column: string, min: number, max: number): boolean {
  return filters.some((filter) => filter.column === column && filter.range?.min === min && filter.range.max === max);
}

export function valueLabel(value: string | null): string {
  return value === null ? "Sin dato" : value;
}

export function formatNumber(value: number, fractionDigits = 0): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: fractionDigits });
}

/** Chip text for an active filter. */
export function filterLabel(filter: ExploreFilter): string {
  if (filter.range) {
    return `${filter.column}: ${formatNumber(filter.range.min, 2)}–${formatNumber(filter.range.max, 2)}`;
  }
  return `${filter.column}: ${(filter.values ?? []).map(valueLabel).join(", ")}`;
}

export function kpiLabel(kpi: ExploreKpi): string {
  if (kpi.kind === "count") return "Filas";
  return kpi.kind === "median" ? `Mediana de ${kpi.column}` : `Media de ${kpi.column}`;
}

export function kpiValue(kpi: ExploreKpi): string {
  if (kpi.value === null) return "—";
  return formatNumber(kpi.value, kpi.kind === "count" ? 0 : 2);
}
