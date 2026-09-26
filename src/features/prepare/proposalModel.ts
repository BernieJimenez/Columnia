// Builds the one-click Preparar proposal from the current profile. Every item
// maps to an option of apply_safe_corrections, so the whole proposal is
// published (and undone) as one revision. Nothing here applies changes: the
// user always confirms with a click.
import type { ColumnProfile, DatasetPreview, DatasetProfile, SafeCorrectionOptions } from "../../bridge";
import { formatDecimal } from "../../format";
import { isNumericType, isTextType } from "../../dataTypes";

export type ProposalItemId = "sentinels" | "trim" | "duplicates" | "impute";

export interface ProposalExample {
  column: string;
  before: string | null;
  after: string;
}

/** A column the imputation will fill: its gaps and the «sin dato» markers that become gaps. */
export interface ProposalColumn {
  name: string;
  missing: number;
  sentinels: number;
}

export interface ProposalItem {
  id: ProposalItemId;
  title: string;
  hint: string;
  examples: ProposalExample[];
  /** Only for "impute": the exact columns sent to the engine. */
  columns?: ProposalColumn[];
}

export type ProposalSelection = Record<ProposalItemId, boolean>;

const ROW_AUDIT_COLUMN = "_cambios";
const MAX_EXAMPLES = 3;
/** Filling more than this share of a column invents too much of it. */
const MAX_IMPUTED_SHARE = 0.05;
/** A text column is a category (and its mode meaningful) only with few, short values. */
const MAX_CATEGORY_VALUES = 50;
const MAX_CATEGORY_SHARE = 0.05;
const MAX_CATEGORY_LENGTH = 40;
const IDENTIFIER_WORDS = new Set(["id", "code", "codigo", "cod", "no", "nro", "num", "number", "key", "clave", "uuid", "ref", "sku"]);

function plural(count: number, one: string, many: string): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

/** «CustomerID», «vm_id», «InvoiceNo», «StockCode»: the last word names a key. */
function looksLikeIdentifier(name: string): boolean {
  const words = name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .toLowerCase()
    .split(/[^a-z0-9áéíóúñ]+/)
    .filter(Boolean);
  const last = words.at(-1);
  return last !== undefined && IDENTIFIER_WORDS.has(last);
}

/**
 * The one-click proposal only fills small gaps in measures and short
 * categories (RV17 / FUN-03). Filling an identifier, a person's name, a date
 * or free text with the median or the most frequent value fabricates data
 * that cannot be told apart from real values.
 */
function isImputable(column: ColumnProfile, rowCount: number): boolean {
  if (column.name === ROW_AUDIT_COLUMN) return false;
  const missing = column.nullCount + (column.sentinelCount ?? 0);
  if (missing === 0 || rowCount === 0 || missing / rowCount > MAX_IMPUTED_SHARE) return false;
  if (column.privacySignal || column.suggestedType === "date" || looksLikeIdentifier(column.name)) return false;
  const observed = rowCount - column.nullCount;
  if (isNumericType(column.dataType)) return observed > 0 && column.median !== null;
  if (!isTextType(column.dataType)) return false;
  return (
    column.uniqueCount < observed
    && column.uniqueCount <= MAX_CATEGORY_VALUES
    && column.uniqueCount / Math.max(observed, 1) <= MAX_CATEGORY_SHARE
    && (column.averageLength ?? 0) <= MAX_CATEGORY_LENGTH
  );
}

function imputeTitle(total: number, columnCount: number): string {
  return `Rellenar ${plural(total, "valor vacío", "valores vacíos")} en ${plural(columnCount, "columna", "columnas")}`;
}

function filledCells(column: ProposalColumn, includeSentinels: boolean): number {
  return column.missing + (includeSentinels ? column.sentinels : 0);
}

function trimExamples(dataset: DatasetPreview): ProposalExample[] {
  const examples: ProposalExample[] = [];
  const textColumns = dataset.columns
    .map((column, index) => ({ column, index }))
    .filter(({ column }) => isTextType(column.dataType) && column.name !== ROW_AUDIT_COLUMN);
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

  if (dataset.columns.some((column) => isTextType(column.dataType) && column.name !== ROW_AUDIT_COLUMN)) {
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
    const imputeColumns = imputable.map((column) => ({
      name: column.name,
      missing: column.nullCount,
      sentinels: column.sentinelCount ?? 0,
    }));
    const convertsSentinels = items.some((item) => item.id === "sentinels");
    const total = imputeColumns.reduce((sum, column) => sum + filledCells(column, convertsSentinels), 0);
    items.push({
      id: "impute",
      title: imputeTitle(total, imputeColumns.length),
      hint: "Solo huecos pequeños en números y categorías; nunca identificadores, nombres, fechas ni texto libre.",
      columns: imputeColumns,
      examples: imputable.slice(0, MAX_EXAMPLES).map((column, index) => {
        const cells = plural(filledCells(imputeColumns[index], convertsSentinels), "celda", "celdas");
        return {
          column: column.name,
          before: null,
          after: isNumericType(column.dataType) && column.median !== null
            ? `mediana (${formatDecimal(column.median, Number.isInteger(column.median) ? 0 : 2)}) · ${cells}`
            : `valor más frecuente · ${cells}`,
        };
      }),
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
    // Filling gaps invents values: the person opts in explicitly.
    impute: false,
  };
}

/** The title shown for an item; imputation counts follow the current selection (FUN-06). */
export function proposalItemTitle(item: ProposalItem, selection: ProposalSelection): string {
  if (item.id !== "impute" || !item.columns) return item.title;
  const total = item.columns.reduce((sum, column) => sum + filledCells(column, selection.sentinels), 0);
  return imputeTitle(total, item.columns.length);
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
    ...(on("impute") ? { imputeColumns: items.find((item) => item.id === "impute")?.columns?.map((column) => column.name) ?? [] } : {}),
  };
}

export function applyLabel(count: number): string {
  return count === 1 ? "Aplicar 1 cambio" : `Aplicar ${count} cambios`;
}
