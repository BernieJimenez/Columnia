// COD-04: responses that come from files on disk (a project, a recipe, a
// reusable task) are checked for their minimal shape, so a field Rust no
// longer sends fails with a readable message instead of `undefined` on screen.

type FieldKind = "string" | "number" | "object" | "array" | "object-or-null";

function kindOf(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function matches(value: unknown, kind: FieldKind): boolean {
  if (kind === "object-or-null") return value === null || kindOf(value) === "object";
  return kindOf(value) === kind;
}

/** `value` unchanged when it has every field; otherwise a readable error. */
export function expectShape<T>(value: unknown, label: string, fields: Record<string, FieldKind>): T {
  if (kindOf(value) !== "object") {
    throw new Error(`Columnia recibió ${label} sin el formato esperado.`);
  }
  const record = value as Record<string, unknown>;
  const missing = Object.entries(fields)
    .filter(([field, kind]) => !matches(record[field], kind))
    .map(([field]) => `«${field}»`);
  if (missing.length > 0) {
    throw new Error(`Columnia recibió ${label} incompleto: falta o no es válido ${missing.join(", ")}. Puede venir de otra versión de Columnia.`);
  }
  return value as T;
}
