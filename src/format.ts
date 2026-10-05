// Single place for user-visible number, size and type formatting. Numbers use
// the system locale, like the existing toLocaleString() calls, so decimals and
// thousands separators never mix within one screen.

import { isDatetimeType, isDecimalType, isIntegerType } from "./dataTypes";

const BINARY_UNITS = ["KiB", "MiB", "GiB", "TiB", "PiB"] as const;

export function formatDecimal(value: number, fractionDigits = 1): string {
  return new Intl.NumberFormat(undefined, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(value);
}

/** A number with up to `maximumFractionDigits` decimals, no trailing zeros. */
export function formatNumber(value: number, maximumFractionDigits = 3): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits }).format(value);
}

/**
 * A share never rounds to 100 % while something is missing, nor to 0 % while
 * something is there (FUN-51): 99.999 % reads «>99,9%», 0.001 % «<0,1%».
 */
export function formatPercent(value: number, fractionDigits = 1): string {
  const step = 10 ** -fractionDigits;
  const rounded = Math.round(value / step) * step;
  if (value < 100 && rounded >= 100) return `>${formatDecimal(100 - step, fractionDigits)}%`;
  if (value > 0 && rounded <= 0) return `<${formatDecimal(step, fractionDigits)}%`;
  return `${formatDecimal(value, fractionDigits)}%`;
}

/** Sizes are computed in powers of 1024, so they use binary units. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(Math.round(bytes), 0)} B`;
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < BINARY_UNITS.length - 1);
  return `${formatDecimal(value, value >= 100 ? 0 : 1)} ${BINARY_UNITS[unitIndex]}`;
}

const DATA_TYPE_LABELS: Record<string, string> = {
  string: "Texto",
  str: "Texto",
  utf8: "Texto",
  varchar: "Texto",
  boolean: "Booleano",
  bool: "Booleano",
  date: "Fecha",
  time: "Hora",
  null: "Vacío",
};

/** Spanish label for an engine data type (Polars/DuckDB), keeping unknown names. */
export function formatDataType(dataType: string): string {
  const normalized = dataType.trim().toLowerCase();
  if (normalized in DATA_TYPE_LABELS) return DATA_TYPE_LABELS[normalized];
  if (isIntegerType(normalized)) return "Entero";
  if (isDecimalType(normalized)) return "Decimal";
  if (isDatetimeType(normalized)) return "Fecha y hora";
  if (normalized.startsWith("duration")) return "Duración";
  if (normalized.startsWith("list") || normalized.startsWith("array")) return "Lista";
  if (normalized.startsWith("struct")) return "Estructura";
  return dataType;
}
