import type { DatasetComparison, DatasetJoinType } from "../../bridge";
import { plural } from "../../plural";

export type JoinStatus =
  | { kind: "idle" }
  | { kind: "loading"; joinType: DatasetJoinType }
  | { kind: "error"; message: string };

export type ReviewMutationKind = "join" | "consolidate" | "resolveConflicts";

export type ReviewMutationStatus =
  | { kind: "idle" }
  | {
      kind: "running";
      mutation: ReviewMutationKind;
      cancellation: "available" | "requested";
      cancellationError?: string;
    }
  | { kind: "finalizing"; mutation: ReviewMutationKind }
  | { kind: "error"; mutation: ReviewMutationKind; message: string };

export function beginJoin(joinType: DatasetJoinType): JoinStatus {
  return { kind: "loading", joinType };
}

export function failJoin(message: string): JoinStatus {
  return { kind: "error", message };
}

export function clearJoin(): JoinStatus {
  return { kind: "idle" };
}

/** The join types in Spanish, with the SQL term between brackets. */
export const JOIN_TYPE_OPTIONS: ReadonlyArray<{ value: DatasetJoinType; label: string; description: string }> = [
  {
    value: "inner",
    label: "Solo coincidencias (inner)",
    description: "Solo las filas cuya clave está en los dos archivos.",
  },
  {
    value: "left",
    label: "Todas tus filas (left)",
    description: "Todas tus filas; las columnas del otro archivo se rellenan donde coincide la clave.",
  },
  {
    value: "full",
    label: "Todas las filas de ambos (full)",
    description: "Las filas de los dos archivos, coincida o no la clave.",
  },
];

export function joinTypeLabel(joinType: DatasetJoinType): string {
  return JOIN_TYPE_OPTIONS.find((option) => option.value === joinType)?.label ?? joinType;
}

/** What a combination leaves: `rowCount` is null when repeated keys make it unknown in advance. */
export interface CombinationEstimate {
  rowCount: number | null;
  columnCount: number;
}

function currentColumnCount(comparison: DatasetComparison): number {
  return comparison.sharedColumns.length + comparison.currentOnlyColumns.length;
}

function comparedColumnCount(comparison: DatasetComparison): number {
  return comparison.sharedColumns.length + comparison.comparedOnlyColumns.length;
}

/**
 * The size of a join by the comparison key. The key columns appear once; the
 * other columns of both files are kept. With repeated keys a row can match
 * several, so the row count is not known before joining.
 */
export function joinEstimate(comparison: DatasetComparison, joinType: DatasetJoinType): CombinationEstimate {
  const columnCount = currentColumnCount(comparison) + comparedColumnCount(comparison) - comparison.keyColumns.length;
  if (comparison.duplicateKeyCount > 0) return { rowCount: null, columnCount };
  const rowCount = joinType === "inner"
    ? comparison.matchedKeyCount
    : joinType === "left"
      ? comparison.currentRowCount
      : comparison.currentRowCount + comparison.comparedOnlyKeyCount;
  return { rowCount, columnCount };
}

/** Consolidating appends the rows of new keys, or every row without a key. */
export function consolidationEstimate(comparison: DatasetComparison): CombinationEstimate & { addedRowCount: number } {
  const addedRowCount = comparison.keyColumns.length > 0 ? comparison.comparedOnlyKeyCount : comparison.comparedRowCount;
  return {
    addedRowCount,
    rowCount: comparison.currentRowCount + addedRowCount,
    columnCount: currentColumnCount(comparison),
  };
}

/** The result line after a combination, as Preparar shows it with «Deshacer». */
export function combinationResultMessage(
  mutation: ReviewMutationKind,
  dataset: { rowCount: number; columnCount: number },
  comparedFileName: string | null,
  joinType: DatasetJoinType,
): string {
  const other = comparedFileName ? `«${comparedFileName}»` : "el otro archivo";
  const size = `quedan ${plural(dataset.rowCount, "fila", "filas")} y ${plural(dataset.columnCount, "columna", "columnas")}`;
  if (mutation === "join") return `Unido con ${other}, ${joinTypeLabel(joinType).toLowerCase()}: ${size}.`;
  const fromOther = comparedFileName ? `de «${comparedFileName}»` : "del otro archivo";
  if (mutation === "consolidate") return `Filas nuevas ${fromOther} añadidas: ${size}.`;
  return `Conflictos con ${other} resueltos: ${size}.`;
}
