import type { ColumnProfile, QualityRule } from "../../bridge";
import { plural } from "../../plural";
import { ROW_AUDIT_COLUMN } from "../../rowAudit";

// Signals about the active dataset that App reads before Entregar is loaded; kept apart
// from deliveryModel so the entry chunk does not carry the whole delivery model.

const PERSONAL_DATA_SIGNALS = new Set<ColumnProfile["privacySignal"]>(["email", "phone", "address", "name"]);

/** Columns whose profile detected personal data; identifiers are excluded because they are not contact data. */
export function personalDataColumnNames(columns: readonly ColumnProfile[] | null): string[] {
  if (!columns) return [];
  return columns
    .filter((column) => column.name !== ROW_AUDIT_COLUMN && PERSONAL_DATA_SIGNALS.has(column.privacySignal))
    .map((column) => column.name);
}
/** Excel limits (RV19 / FUN-05); export_io.rs rejects the same cases when exporting. */
const EXCEL_MAX_DATA_ROWS = 1_048_575;
const EXCEL_MAX_CELL_CHARS = 32_767;

/** Why this dataset would not open in Excel, known before exporting. */
export function excelLimitIssues(rowCount: number, columns: readonly ColumnProfile[] | null): string[] {
  const issues: string[] = [];
  if (rowCount > EXCEL_MAX_DATA_ROWS) {
    issues.push(`Tiene ${rowCount.toLocaleString()} filas y una hoja de Excel admite ${EXCEL_MAX_DATA_ROWS.toLocaleString()}.`);
  }
  const longColumns = (columns ?? [])
    .filter((column) => (column.maximumLength ?? 0) > EXCEL_MAX_CELL_CHARS)
    .map((column) => column.name);
  if (longColumns.length > 0) {
    issues.push(`${longColumns.join(", ")}: hay celdas de más de ${EXCEL_MAX_CELL_CHARS.toLocaleString()} caracteres, el máximo de Excel.`);
  }
  return issues;
}

/**
 * Warnings for the columns chosen as key of «Actualizar por clave»: the script stops
 * on nulls or repeats. Repeats are only known with a single key column, because the
 * profile does not know the combination of several. Without a profile there are none.
 */
export function keyColumnWarnings(
  keyColumns: readonly string[],
  profiles: readonly Pick<ColumnProfile, "name" | "nullCount" | "uniqueCount">[] | null | undefined,
  rowCount: number,
): string[] {
  if (!profiles) return [];
  const warnings: string[] = [];
  const found = keyColumns
    .map((name) => profiles.find((profile) => profile.name === name))
    .filter((profile): profile is NonNullable<typeof profile> => profile !== undefined);
  for (const profile of found) {
    if (profile.nullCount > 0) {
      warnings.push(
        `La columna «${profile.name}» tiene ${plural(profile.nullCount, "nulo", "nulos")}: el script se detendrá sin tocar la tabla.`,
      );
    }
  }
  const profile = found[0];
  if (keyColumns.length === 1 && profile) {
    const repeated = rowCount - profile.nullCount - profile.uniqueCount;
    if (repeated > 0) {
      warnings.push(
        `La columna «${profile.name}» tiene valores repetidos (${plural(repeated, "fila repite", "filas repiten")} una clave): el script se detendrá sin tocar la tabla.`,
      );
    }
  }
  return warnings;
}

export const MAX_SUGGESTED_QUALITY_RULES = 6;
const IDENTIFIER_NAME = /(^|[_\s-])(id|codigo|código|code|key|clave)([_\s-]|$)/i;

/**
 * Quality rules the current data already meets, proposed so a delivery can be
 * validated with one click and later deliveries keep the same guarantees.
 * Identifier columns first (not null and unique), then complete columns.
 */
export function suggestQualityRules(
  columns: readonly ColumnProfile[] | null,
  rowCount: number,
): QualityRule[] {
  if (!columns || rowCount === 0) return [];
  const candidates = columns.filter((column) => column.name !== ROW_AUDIT_COLUMN && column.nullCount === 0);
  const isIdentifier = (column: ColumnProfile) => column.uniqueCount === rowCount
    && (column.privacySignal === "identifier" || IDENTIFIER_NAME.test(column.name));
  const identifiers = candidates.filter(isIdentifier);
  const rules: QualityRule[] = [];
  for (const column of identifiers) {
    rules.push({ column: column.name, kind: "not_null", maxInvalid: 0 });
    rules.push({ column: column.name, kind: "unique", maxInvalid: 0 });
  }
  for (const column of candidates) {
    if (identifiers.includes(column)) continue;
    rules.push({ column: column.name, kind: "not_null", maxInvalid: 0 });
  }
  return rules.slice(0, MAX_SUGGESTED_QUALITY_RULES);
}
