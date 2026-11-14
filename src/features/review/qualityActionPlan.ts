import type { DatasetProfile } from "../../bridge";
import { plural } from "../../plural";
import { isRowAuditColumn } from "../../rowAudit";
import { COMMA_DECIMAL_SETTING, commaDecimalColumns, nonFiniteColumns } from "./reviewModel";

export type QualityActionTarget = "missingValues" | "duplicates" | "incompatibleTypes";

export function qualityActionTargetDomId(target: QualityActionTarget): string {
  return `prepare-quality-${target}`;
}

/** Where a problem Preparar does not fix on its own is dealt with. */
export type UncorrectedAction =
  | { kind: "prepare"; label: string; target?: QualityActionTarget }
  | { kind: "load"; label: string };

export interface UncorrectedIssue {
  id: "nulls" | "incompatible" | "personal" | "commaDecimal" | "nonFinite";
  /** What it is and how many cells, rows or columns, counted over every row. */
  title: string;
  /** Why it is not fixed alone, or what to do about it. */
  detail: string;
  /** Null when there is nothing to press: the issue only informs. */
  action: UncorrectedAction | null;
}

const MAX_LISTED_COLUMNS = 3;

/** «la única fila» or «las 48.213 filas»: the figures count every row. */
export function allRows(rowCount: number): string {
  return rowCount === 1 ? "la única fila" : `las ${rowCount.toLocaleString()} filas`;
}

function columnNames(names: string[]): string {
  const shown = names.slice(0, MAX_LISTED_COLUMNS).map((name) => `«${name}»`).join(", ");
  const rest = names.length - MAX_LISTED_COLUMNS;
  return rest > 0 ? `${shown} y ${rest.toLocaleString()} más` : shown;
}

/**
 * What the Preparar proposal leaves alone, so «No se corrige solo» lists the
 * rest of what the profile found: nulls, values that do not fit the suggested
 * type, personal data and decimal commas. Duplicates, «sin dato» markers and
 * spaces are not here: the proposal lists them.
 *
 * `filledColumns` are the columns the proposal's «Rellenar nulos» item fills
 * (the same names `buildPrepareProposal` puts in that item): their nulls are
 * not counted here, so «Rellenar 347 nulos» above and this list never show
 * two unrelated figures for the same gaps.
 */
export function buildUncorrectedIssues(
  profile: DatasetProfile,
  filledColumns: ReadonlySet<string> = new Set(),
): UncorrectedIssue[] {
  const issues: UncorrectedIssue[] = [];
  const columns = profile.columns.filter((column) => !isRowAuditColumn(column.name));

  const nullColumns = columns.filter((column) => column.nullCount > 0 && !filledColumns.has(column.name));
  const nullCount = nullColumns.reduce((total, column) => total + column.nullCount, 0);
  const offeredCount = columns
    .filter((column) => column.nullCount > 0 && filledColumns.has(column.name))
    .reduce((total, column) => total + column.nullCount, 0);
  if (nullCount > 0) {
    issues.push({
      id: "nulls",
      title: `${plural(nullCount, "nulo", "nulos")} en ${plural(nullColumns.length, "columna", "columnas")} que la propuesta no rellena`,
      detail: `${offeredCount > 0 ? `Son distintos de los ${offeredCount.toLocaleString()} que la propuesta ofrece rellenar. ` : ""}Un nulo puede ser válido: revisa por qué falta antes de rellenarlo o de retirar la columna.`,
      action: { kind: "prepare", label: "Ver nulos en Preparar", target: "missingValues" },
    });
  }

  const invalidColumns = columns.filter((column) => (column.invalidTypeCount ?? 0) > 0);
  const invalidCount = invalidColumns.reduce((total, column) => total + Math.max(0, column.invalidTypeCount ?? 0), 0);
  if (invalidCount > 0) {
    issues.push({
      id: "incompatible",
      title: `${plural(invalidCount, "valor incompatible", "valores incompatibles")} con el tipo sugerido en ${plural(invalidColumns.length, "columna", "columnas")}`,
      detail: "Apartarlos como nulos puede ocultar códigos o excepciones válidas: confirma la sugerencia antes de aplicarla.",
      action: { kind: "prepare", label: "Ver incompatibles en Preparar", target: "incompatibleTypes" },
    });
  }

  const personal = columns.filter((column) => column.privacySignal !== null && column.privacySignal !== "identifier");
  if (personal.length > 0) {
    issues.push({
      id: "personal",
      title: `${plural(personal.length, "columna", "columnas")} con datos personales: ${columnNames(personal.map((column) => column.name))}`,
      detail: `Presentes en ${allRows(profile.rowCount)}. Retíralas o enmascara sus valores antes de entregar.`,
      action: { kind: "prepare", label: "Ver datos personales en Preparar" },
    });
  }

  const commaNames = commaDecimalColumns(profile);
  if (commaNames.length > 0) {
    const commaCells = columns
      .filter((column) => commaNames.includes(column.name))
      .reduce((total, column) => total + (column.commaDecimalCount ?? 0), 0);
    issues.push({
      id: "commaDecimal",
      title: `${plural(commaCells, "celda", "celdas")} con coma decimal leída${commaCells === 1 ? "" : "s"} como texto en ${columnNames(commaNames)}`,
      detail: `Por eso no se analizan como número (1,5 o 1.234,56). Vuelve a cargar el archivo y elige «${COMMA_DECIMAL_SETTING}» en Números.`,
      action: { kind: "load", label: "Volver a cargar con coma decimal" },
    });
  }

  const nonFinite = nonFiniteColumns(profile).filter(({ name }) => !isRowAuditColumn(name));
  if (nonFinite.length > 0) {
    const total = nonFinite.reduce((sum, { count }) => sum + count, 0);
    issues.push({
      id: "nonFinite",
      title: `${plural(total, "valor no finito", "valores no finitos")} (NaN o infinito) en ${columnNames(nonFinite.map(({ name }) => name))}`,
      detail: "Quedan fuera del mínimo, el máximo, la media y los gráficos.",
      action: null,
    });
  }

  return issues;
}
