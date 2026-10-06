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
  // TXT-09: no «NaN B»; the unit is chosen after rounding, so 1 048 575
  // bytes read «1,0 MiB» and never «1024 KiB».
  if (!Number.isFinite(bytes)) return "—";
  const whole = Math.max(Math.round(bytes), 0);
  if (whole < 1024) return `${whole} B`;
  const shown = (value: number) => value >= 100 ? Math.round(value) : Math.round(value * 10) / 10;
  let value = whole;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (shown(value) >= 1024 && unitIndex < BINARY_UNITS.length - 1);
  return `${formatDecimal(value, shown(value) >= 100 ? 0 : 1)} ${BINARY_UNITS[unitIndex]}`;
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
  // TXT-08: categorical and enum columns are categories, not English names.
  cat: "Categoría",
  categorical: "Categoría",
  enum: "Categoría",
  binary: "Binario",
  binaryoffset: "Binario",
  blob: "Binario",
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
  if (normalized.startsWith("enum") || normalized.startsWith("categorical")) return "Categoría";
  return dataType;
}
