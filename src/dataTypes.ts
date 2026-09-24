// Column type predicates. The engine reports Polars' short names ("str", "i64",
// "f64", "bool", "date", "datetime[μs]"); fixtures and older code used the long
// ones ("String", "Int64", ...). Comparing against a single spelling silently
// disabled features in the real app, so every type check goes through here.

function normalize(dataType: string): string {
  return dataType.trim().toLowerCase();
}

export function isTextType(dataType: string): boolean {
  return ["str", "string", "utf8", "varchar"].includes(normalize(dataType));
}

export function isIntegerType(dataType: string): boolean {
  return /^(u?int(8|16|32|64|128)|[iu](8|16|32|64|128)|(tiny|small|big|hug)?int(eger)?)$/.test(normalize(dataType));
}

export function isDecimalType(dataType: string): boolean {
  return /^(float(32|64)|f(32|64)|double|real|decimal.*)$/.test(normalize(dataType));
}

export function isNumericType(dataType: string): boolean {
  return isIntegerType(dataType) || isDecimalType(dataType);
}

export function isBooleanType(dataType: string): boolean {
  return ["bool", "boolean"].includes(normalize(dataType));
}

export function isDateType(dataType: string): boolean {
  return normalize(dataType) === "date";
}

export function isDatetimeType(dataType: string): boolean {
  const normalized = normalize(dataType);
  return normalized.startsWith("datetime") || normalized.startsWith("timestamp");
}
