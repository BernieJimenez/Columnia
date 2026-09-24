// Builds the one-click Preparar proposal from the current profile. Every item
// maps to an option of apply_safe_corrections, so the whole proposal is
// published (and undone) as one revision. Nothing here applies changes: the
// user always confirms with a click.
import type { ColumnProfile, DatasetPreview, DatasetProfile, SafeCorrectionOptions } from "../../bridge";
import { formatDecimal } from "../../format";

export type ProposalItemId = "sentinels" | "trim" | "duplicates" | "impute";

export interface ProposalExample {
  column: string;
  before: string | null;
  after: string;
}

export interface ProposalItem {
  id: ProposalItemId;
  title: string;
  hint: string;
  examples: ProposalExample[];
}

export type ProposalSelection = Record<ProposalItemId, boolean>;

const ROW_AUDIT_COLUMN = "_cambios";
const MAX_EXAMPLES = 3;

function plural(count: number, one: string, many: string): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

function isNumericType(dataType: string): boolean {
  return /^(u?int|float)\d+$/i.test(dataType.trim());
}

/** Mirrors impute_missing_values_in_frame: numbers need an observed value, text a repeated one. */
function isImputable(column: ColumnProfile, rowCount: number): boolean {
  if (column.name === ROW_AUDIT_COLUMN || column.nullCount === 0) return false;
  const observed = rowCount - column.nullCount;
  if (isNumericType(column.dataType)) return observed > 0;
  if (column.dataType === "String") return column.uniqueCount < observed;
  return false;
}

function trimExamples(dataset: DatasetPreview): ProposalExample[] {
  const examples: ProposalExample[] = [];
  const textColumns = dataset.columns
    .map((column, index) => ({ column, index }))
    .filter(({ column }) => column.dataType === "String" && column.name !== ROW_AUDIT_COLUMN);
  for (const row of dataset.rows) {
    for (const { column, index } of textColumns) {
      const value = row[index];
      if (value === null || value === undefined) continue;
      const trimmed = value.trim();
      if (trimmed !== value && trimmed !== "") {
        examples.push({ column: column.name, before: value, after: trimmed });
        if (examples.length === MAX_EXAMPLES) return examples;
      }
    }
  }
  return examples;
}

export function buildPrepareProposal(profile: DatasetProfile, dataset: DatasetPreview): ProposalItem[] {
  const items: ProposalItem[] = [];
  const columns = profile.columns.filter((column) => column.name !== ROW_AUDIT_COLUMN);

  const sentinelColumns = columns.filter((column) => (column.sentinelCount ?? 0) > 0);
  if (sentinelColumns.length > 0) {
    const total = sentinelColumns.reduce((sum, column) => sum + (column.sentinelCount ?? 0), 0);
    items.push({
      id: "sentinels",
      title: `Convertir ${plural(total, "marcador", "marcadores")} de «sin dato» en ${plural(sentinelColumns.length, "columna", "columnas")}`,
      hint: "Textos que significan «sin dato» pasan a ser vacíos reales.",
      examples: [],
    });
  }

  if (dataset.columns.some((column) => column.dataType === "String" && column.name !== ROW_AUDIT_COLUMN)) {
    items.push({
      id: "trim",
      title: "Recortar espacios al inicio y al final del texto",
      hint: "Solo cambian las celdas con espacios sobrantes.",
      examples: trimExamples(dataset),
    });
  }

  if (profile.duplicateRowCount > 0) {
    items.push({
      id: "duplicates",
      title: `Quitar ${plural(profile.duplicateRowCount, "fila duplicada", "filas duplicadas")}`,
      hint: "Se conserva la primera de cada grupo.",
      examples: [],
    });
  }

  const imputable = columns.filter((column) => isImputable(column, profile.rowCount));
  if (imputable.length > 0) {
    const total = imputable.reduce((sum, column) => sum + column.nullCount, 0);
    items.push({
      id: "impute",
      title: `Rellenar ${plural(total, "valor vacío", "valores vacíos")} en ${plural(imputable.length, "columna", "columnas")}`,
      hint: "Mediana en números y valor más frecuente en texto.",
      examples: imputable.slice(0, MAX_EXAMPLES).map((column) => ({
        column: column.name,
        before: null,
        after: isNumericType(column.dataType) && column.median !== null
          ? `mediana (${formatDecimal(column.median, Number.isInteger(column.median) ? 0 : 2)})`
          : "valor más frecuente",
      })),
    });
  }

  return items;
}

export function defaultProposalSelection(items: ProposalItem[]): ProposalSelection {
  const present = new Set(items.map((item) => item.id));
  return {
    sentinels: present.has("sentinels"),
    trim: present.has("trim"),
    duplicates: present.has("duplicates"),
    impute: present.has("impute"),
  };
}

export function selectedProposalCount(items: ProposalItem[], selection: ProposalSelection): number {
  return items.filter((item) => selection[item.id]).length;
}

export function proposalOptions(
  items: ProposalItem[],
  selection: ProposalSelection,
  normalizeColumnNames = false,
): SafeCorrectionOptions {
  const on = (id: ProposalItemId) => items.some((item) => item.id === id) && selection[id];
  return {
    trimText: on("trim"),
    normalizeSentinels: on("sentinels"),
    normalizeColumnNames,
    removeDuplicates: on("duplicates"),
    imputeMissing: on("impute"),
  };
}

export function applyLabel(count: number): string {
  return count === 1 ? "Aplicar 1 cambio" : `Aplicar ${count} cambios`;
}
