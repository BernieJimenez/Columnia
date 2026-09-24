import { describe, expect, it } from "vitest";

import { formatBytes, formatDataType, formatDecimal, formatPercent } from "./format";

const decimalSeparator = new Intl.NumberFormat(undefined).format(1.5).charAt(1);

describe("formateo visible", () => {
  it("usa el separador decimal del sistema en decimales y porcentajes", () => {
    expect(formatDecimal(8.333)).toBe(`8${decimalSeparator}3`);
    expect(formatPercent(8.333)).toBe(`8${decimalSeparator}3%`);
  });

  it("expresa tamaños en unidades binarias coherentes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe(`1${decimalSeparator}0 KiB`);
    expect(formatBytes(1024 ** 2 * 150)).toBe("150 MiB");
    expect(formatBytes(1024 ** 3)).toBe(`1${decimalSeparator}0 GiB`);
  });

  it("traduce los tipos del motor y conserva los desconocidos", () => {
    // Names the engine really sends (Polars' short form).
    expect(formatDataType("i64")).toBe("Entero");
    expect(formatDataType("f64")).toBe("Decimal");
    expect(formatDataType("str")).toBe("Texto");
    expect(formatDataType("datetime[μs]")).toBe("Fecha y hora");
    expect(formatDataType("Int64")).toBe("Entero");
    expect(formatDataType("Float64")).toBe("Decimal");
    expect(formatDataType("String")).toBe("Texto");
    expect(formatDataType("Datetime(Microseconds, None)")).toBe("Fecha y hora");
    expect(formatDataType("Categorical")).toBe("Categorical");
  });
});
