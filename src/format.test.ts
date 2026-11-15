import { describe, expect, it } from "vitest";

import { formatBytes, formatDataType, formatDecimal, formatPercent } from "./format";

// The interface is only in Spanish: the decimal comma does not depend on the system locale.
const decimalSeparator = ",";

describe("formateo visible", () => {
  it("usa la coma decimal española en decimales y porcentajes, sea cual sea el idioma del sistema", () => {
    expect(formatDecimal(8.333)).toBe(`8${decimalSeparator}3`);
    expect(formatPercent(8.333)).toBe(`8${decimalSeparator}3%`);
  });

  it("no redondea a 100 % con un nulo en 100.000 filas ni a 0 % con un valor (FUN-51)", () => {
    expect(formatPercent(99.999)).toBe(`>99${decimalSeparator}9%`);
    expect(formatPercent(0.001)).toBe(`<0${decimalSeparator}1%`);
    expect(formatPercent(100)).toBe(`100${decimalSeparator}0%`);
    expect(formatPercent(0)).toBe(`0${decimalSeparator}0%`);
  });

  it("expresa tamaños en unidades binarias coherentes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe(`1${decimalSeparator}0 KiB`);
    expect(formatBytes(1024 ** 2 * 150)).toBe("150 MiB");
    expect(formatBytes(1024 ** 3)).toBe(`1${decimalSeparator}0 GiB`);
    // TXT-09: rounding never leaves «1024 KiB» or «NaN B».
    expect(formatBytes(1_048_575)).toBe(`1${decimalSeparator}0 MiB`);
    expect(formatBytes(1023.6)).toBe(`1${decimalSeparator}0 KiB`);
    expect(formatBytes(Number.NaN)).toBe("—");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("—");
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
    // TXT-08: categories and binary data read in Spanish.
    expect(formatDataType("Categorical")).toBe("Categoría");
    expect(formatDataType("cat")).toBe("Categoría");
    expect(formatDataType("enum")).toBe("Categoría");
    expect(formatDataType("binary")).toBe("Binario");
    expect(formatDataType("MiTipoRaro")).toBe("MiTipoRaro");
  });
});

describe("cifras en español", () => {
  it("agrupa los miles con punto aunque el sistema esté en inglés", () => {
    expect((12400).toLocaleString("es-ES")).toBe("12.400");
    expect(formatDecimal(1234.5)).toBe("1234,5");
    expect(formatDecimal(12345.5)).toBe("12.345,5");
  });
});
