import { describe, expect, it } from "vitest";

import {
  isBooleanType,
  isDateType,
  isDatetimeType,
  isDecimalType,
  isIntegerType,
  isNumericType,
  isTextType,
} from "./dataTypes";

describe("dataTypes", () => {
  it("recognizes the names the engine really sends", () => {
    expect(isTextType("str")).toBe(true);
    expect(isIntegerType("i64")).toBe(true);
    expect(isIntegerType("u32")).toBe(true);
    expect(isDecimalType("f64")).toBe(true);
    expect(isNumericType("i8")).toBe(true);
    expect(isBooleanType("bool")).toBe(true);
    expect(isDateType("date")).toBe(true);
    expect(isDatetimeType("datetime[μs]")).toBe(true);
  });

  it("keeps accepting the long names used by fixtures", () => {
    expect(isTextType("String")).toBe(true);
    expect(isIntegerType("Int64")).toBe(true);
    expect(isDecimalType("Float64")).toBe(true);
    expect(isDateType("Date")).toBe(true);
    expect(isDatetimeType("Datetime")).toBe(true);
  });

  it("does not mix categories", () => {
    expect(isTextType("i64")).toBe(false);
    expect(isNumericType("str")).toBe(false);
    expect(isDateType("datetime[μs]")).toBe(false);
    expect(isNumericType("bool")).toBe(false);
  });
});
