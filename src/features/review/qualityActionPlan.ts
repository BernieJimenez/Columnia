import type { DatasetProfile } from "../../bridge";
import { plural } from "../../plural";
import { isRowAuditColumn } from "../../rowAudit";
import { proposalImputeCells, type ProposalItem } from "../prepare/proposalModel";
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
  id: "nulls" | "skewedNumbers" | "incompatible" | "personal" | "commaDecimal" | "nonFinite";
  /** What it is and how many cells, rows or columns, counted over every row. */
  title: string;
  /** Why it is not fixed alone, or what to do about it. */
  detail: string;
  /** Null when there is nothing to press: the issue only informs. */
  action: UncorrectedAction | null;
}

const MAX_LISTED_COLUMNS = 3;
/** Numbers that files typically use as «no data»: -999, 9999 and their relatives. */
const PLACEHOLDER_NUMBERS: ReadonlySet<number> = new Set([-99999, -9999, -999, 9999, 99999, 999999]);

/** «la única fila» or «las 48.213 filas»: the figures count every row. */
export function allRows(rowCount: number): string {
  return rowCount === 1 ? "la única fila" : `las ${rowCount.toLocaleString("es-ES")} filas`;
}

function columnNames(names: string[]): string {
  const shown = names.slice(0, MAX_LISTED_COLUMNS).map((name) => `«${name}»`).join(", ");
  const rest = names.length - MAX_LISTED_COLUMNS;
  return rest > 0 ? `${shown} y ${rest.toLocaleString("es-ES")} más` : shown;
}

/**
 * What the Preparar proposal leaves alone, so «No se corrige solo» lists the
 * rest of what the profile found: nulls, values that do not fit the suggested
 * type, personal data and decimal commas. Duplicates, «sin dato» markers and
 * spaces are not here: the proposal lists them.
 *
 * `imputeItem` is the proposal's «Rellenar nulos» item: the nulls of its
 * columns are not counted here, and the figure it offers comes from
 * `proposalImputeCells` (gaps plus converted markers), so the title above and
 * this list never show two unrelated figures for the same gaps.
 */
export function buildUncorrectedIssues(
  profile: DatasetProfile,
  imputeItem?: ProposalItem,
): UncorrectedIssue[] {
  const issues: UncorrectedIssue[] = [];
  const columns = profile.columns.filter((column) => !isRowAuditColumn(column.name));

  const filledColumns = new Set((imputeItem?.columns ?? []).map((column) => column.name));
  const nullColumns = columns.filter((column) => column.nullCount > 0 && !filledColumns.has(column.name));
  const nullCount = nullColumns.reduce((total, column) => total + column.nullCount, 0);
  // The same rule as the proposal's title: gaps plus the markers it converts first.
  const offeredCount = imputeItem ? proposalImputeCells(imputeItem) : 0;
  const offeredMarkers = (imputeItem?.columns ?? []).reduce((total, column) => total + column.sentinels, 0);
  if (nullCount > 0) {
    const offered = offeredCount > 0
      ? `Son distintos de los ${offeredCount.toLocaleString("es-ES")} que la propuesta ofrece rellenar${offeredMarkers > 0 ? ` (${offeredMarkers.toLocaleString("es-ES")} de ellos ${offeredMarkers === 1 ? "es un marcador" : "son marcadores"} «sin dato»)` : ""}. `
      : "";
    issues.push({
      id: "nulls",
      title: `${plural(nullCount, "nulo", "nulos")} en ${plural(nullColumns.length, "columna", "columnas")} que la propuesta no rellena`,
      detail: `${offered}Un nulo puede ser válido: revisa por qué falta antes de rellenarlo o de retirar la columna.`,
      action: { kind: "prepare", label: "Ver nulos en Preparar", target: "missingValues" },
    });
  }

  // A numeric column the proposal fills whose range ends on a typical code:
  // the profile counts it as a number, so its mean and median may be skewed.
  const skewed = columns.flatMap((column) => {
    if (!filledColumns.has(column.name) || column.outlierCount === null) return [];
    const code = [column.minimum, column.maximum].find((value) => value !== null && PLACEHOLDER_NUMBERS.has(Number(value)));
    return code === undefined || code === null ? [] : [{ name: column.name, code: Number(code) }];
  });
  if (skewed.length > 0) {
    const first = skewed[0]!;
    issues.push({
      id: "skewedNumbers",
      title: skewed.length === 1
        ? `«${first.name}» tiene valores como ${first.code} que el perfil cuenta como números`
        : `${plural(skewed.length, "columna", "columnas")} con valores como ${first.code} que el perfil cuenta como números: ${columnNames(skewed.map(({ name }) => name))}`,
      detail: "La media y el relleno con la mediana pueden estar sesgados. Si son marcadores de «sin dato», conviértelos antes de rellenar.",
      action: { kind: "prepare", label: "Ver en Preparar" },
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
