// Builds the one-click Preparar proposal from the current profile. Every item
// maps to an option of apply_safe_corrections, so the whole proposal is
// published (and undone) as one revision. Nothing here applies changes: the
// user always confirms with a click.
import type {
  ColumnProfile,
  DatasetPreview,
  DatasetProfile,
  DateColumnPlan,
  SafeCorrectionOptions,
  SafeCorrectionsPreview,
} from "../../bridge";
import { formatDecimal } from "../../format";
import { isNumericType, isTextType } from "../../dataTypes";

export type ProposalItemId = "sentinels" | "trim" | "types" | "dates" | "duplicates" | "impute";

export type DateOrder = DateColumnPlan["order"];

/** A column the proposal types as a date; `order` is null while its values are ambiguous. */
export interface ProposalDateColumn {
  name: string;
  order: DateOrder | null;
  /** A real value of the column, to show before and after. */
  sample: string | null;
}

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
  /** Only for "impute" and "types": the exact columns sent to the engine. */
  columns?: ProposalColumn[];
  /** Only for "dates". */
  dateColumns?: ProposalDateColumn[];
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

/**
 * A key word first or last names a key: «CustomerID», «vm_id», «InvoiceNo» in
 * English order, and «id_cliente», «codigo_postal», «num_factura» in Spanish
 * order (FUN-22).
 */
export function looksLikeIdentifier(name: string): boolean {
  const words = name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .toLowerCase()
    .split(/[^a-z0-9áéíóúñ]+/)
    .filter(Boolean);
  const first = words[0];
  const last = words.at(-1);
  // «no» only names a key at the end («InvoiceNo»); first it is usually a
  // negation («no_contesta»).
  return (last !== undefined && IDENTIFIER_WORDS.has(last))
    || (first !== undefined && first !== "no" && IDENTIFIER_WORDS.has(first));
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

/**
 * A text column the engine can type without losing anything (RV18 / FUN-07):
 * every value is a number and it is not a key or personal data. Rust checks
 * it again (and skips leading-zero codes) before converting.
 */
function isTypeable(column: ColumnProfile): boolean {
  return (
    column.name !== ROW_AUDIT_COLUMN
    && isTextType(column.dataType)
    && (column.suggestedType === "integer" || column.suggestedType === "decimal")
    && column.invalidTypeCount === 0
    && !column.privacySignal
    && !looksLikeIdentifier(column.name)
  );
}

/** Text columns whose every value is a date (RV18); keys and personal data stay as they are. */
function isDateable(column: ColumnProfile): boolean {
  return (
    column.name !== ROW_AUDIT_COLUMN
    && isTextType(column.dataType)
    && column.dateOrder !== null
    && column.dateOrder !== undefined
    && !column.privacySignal
  );
}

function datesTitle(columnCount: number): string {
  return `Convertir ${plural(columnCount, "columna", "columnas")} a fecha`;
}

function sampleValue(dataset: DatasetPreview, name: string): string | null {
  const index = dataset.columns.findIndex((column) => column.name === name);
  if (index < 0) return null;
  for (const row of dataset.rows) {
    const value = row[index];
    if (value !== null && value !== undefined && value.trim() !== "") return value;
  }
  return null;
}

/** How a date value reads once typed, in an order nobody can misread: 2010-12-01 08:26. */
export function dateExample(value: string, order: DateOrder): string | null {
  const [datePart, timePart] = value.trim().split(/[T ]/, 2);
  const separator = datePart.match(/[/.-]/)?.[0];
  if (!separator) return null;
  const parts = datePart.split(separator);
  if (parts.length !== 3) return null;
  const [year, month, day] = order === "iso"
    ? [parts[0], parts[1], parts[2]]
    : order === "dmy" ? [parts[2], parts[1], parts[0]] : [parts[2], parts[0], parts[1]];
  const pad = (text: string) => text.padStart(2, "0");
  // Same rule as the engine for two-digit years (Excel's): 00-29 is 20xx.
  const fullYear = year.length === 2 ? `${Number(year) < 30 ? "20" : "19"}${year}` : year;
  const date = `${fullYear}-${pad(month)}-${pad(day)}`;
  if (!timePart) return date;
  const [hours, minutes] = timePart.split(":");
  return `${date} ${pad(hours)}:${pad(minutes ?? "00")}`;
}

/** The date columns that will be sent, resolving ambiguous ones with the person's answer. */
export function resolvedDateColumns(item: ProposalItem, ambiguousOrder: DateOrder | null): DateColumnPlan[] {
  return (item.dateColumns ?? []).flatMap((column) => {
    const order = column.order ?? ambiguousOrder;
    return order ? [{ column: column.name, order }] : [];
  });
}

/**
 * The dates title counts the columns that will be converted; columns whose
 * order is still unknown are named apart, so the figure matches the result
 * (FUN-24).
 */
export function datesItemTitle(item: ProposalItem, ambiguousOrder: DateOrder | null): string {
  const resolved = resolvedDateColumns(item, ambiguousOrder).length;
  const pending = (item.dateColumns ?? []).length - resolved;
  if (pending === 0) return datesTitle(resolved);
  if (resolved === 0) {
    return `Convertir ${plural(pending, "columna", "columnas")} a fecha: elige cómo se lee${pending === 1 ? "" : "n"}`;
  }
  return `${datesTitle(resolved)} (${plural(pending, "columna necesita", "columnas necesitan")} que elijas el orden)`;
}

export function hasAmbiguousDates(item: ProposalItem): boolean {
  return (item.dateColumns ?? []).some((column) => column.order === null);
}

/** Before and after for each date column, once its order is known. */
export function dateExamples(item: ProposalItem, ambiguousOrder: DateOrder | null): ProposalExample[] {
  return (item.dateColumns ?? []).flatMap((column) => {
    const order = column.order ?? ambiguousOrder;
    const after = order && column.sample ? dateExample(column.sample, order) : null;
    return after && column.sample ? [{ column: column.name, before: column.sample, after }] : [];
  });
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

  // With the profile's count the item says how many cells change and is left
  // out when none would (UX-01); older cached profiles have no count.
  const untrimmed = columns.filter((column) => isTextType(column.dataType)).map((column) => column.untrimmedCount);
  const untrimmedTotal = untrimmed.length > 0 && untrimmed.every((count) => typeof count === "number")
    ? untrimmed.reduce<number>((sum, count) => sum + (count ?? 0), 0)
    : null;
  if (
    dataset.columns.some((column) => isTextType(column.dataType) && column.name !== ROW_AUDIT_COLUMN)
    && untrimmedTotal !== 0
  ) {
    items.push({
      id: "trim",
      title: untrimmedTotal === null
        ? "Recortar espacios al inicio y al final del texto"
        : `Recortar espacios en ${plural(untrimmedTotal, "celda", "celdas")}`,
      hint: "Solo cambian las celdas con espacios sobrantes.",
      examples: trimExamples(dataset),
    });
  }

  const typeable = columns.filter(isTypeable);
  if (typeable.length > 0) {
    items.push({
      id: "types",
      title: `Convertir ${plural(typeable.length, "columna", "columnas")} a número`,
      hint: "Todos sus valores son números; así se suman, ordenan y exportan como números. Los identificadores no se tocan.",
      // A column whose only non-numbers are «sin dato» markers types once
      // those markers become gaps, so it depends on that change.
      columns: typeable.map((column) => ({ name: column.name, missing: 0, sentinels: column.sentinelCount ?? 0 })),
      examples: typeable.slice(0, MAX_EXAMPLES).map((column) => ({
        column: column.name,
        before: "Texto",
        after: column.suggestedType === "integer" ? "Entero" : "Decimal",
      })),
    });
  }

  const dated = columns.filter(isDateable);
  if (dated.length > 0) {
    const dateColumns = dated.map((column) => ({
      name: column.name,
      order: column.dateOrder === "ambiguous" ? null : (column.dateOrder ?? null),
      sample: sampleValue(dataset, column.name),
    }));
    items.push({
      id: "dates",
      title: datesTitle(dateColumns.length),
      hint: "Todos sus valores son fechas; así se ordenan y filtran como fechas.",
      dateColumns,
      examples: [],
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
    // Typing changes no value, only how it is stored.
    types: present.has("types"),
    // Dates whose order is known change no value either; ambiguous ones wait for an answer.
    dates: items.some((item) => item.id === "dates" && resolvedDateColumns(item, null).length > 0),
    duplicates: present.has("duplicates"),
    // Filling gaps invents values: the person opts in explicitly.
    impute: false,
  };
}

/**
 * The columns «Convertir a número» types with this selection: one that holds
 * «sin dato» markers only types when those markers are converted first.
 */
export function typedColumns(item: ProposalItem, selection: ProposalSelection): string[] {
  return (item.columns ?? [])
    .filter((column) => selection.sentinels || column.sentinels === 0)
    .map((column) => column.name);
}

/**
 * The title shown for an item. Imputation counts come from the engine's
 * simulation of the whole selected chain when there is one (RV17 / FUN-06);
 * otherwise they are estimated from the profile and the current selection.
 */
export function proposalItemTitle(
  item: ProposalItem,
  selection: ProposalSelection,
  preview?: SafeCorrectionsPreview | null,
): string {
  if (item.id === "types" && item.columns) {
    const count = typedColumns(item, selection).length;
    return count === 0
      ? "Convertir a número: requiere convertir los marcadores «sin dato»"
      : `Convertir ${plural(count, "columna", "columnas")} a número`;
  }
  // Trimming spaces can make more rows identical: the engine's simulation
  // counts the duplicates that will really go (FUN-12).
  if (item.id === "duplicates" && preview) {
    return preview.removedRowCount === 0
      ? "Quitar filas duplicadas: no queda ninguna tras los demás cambios"
      : `Quitar ${plural(preview.removedRowCount, "fila duplicada", "filas duplicadas")}`;
  }
  if (item.id !== "impute" || !item.columns) return item.title;
  if (preview) {
    return preview.imputedCellCount === 0
      ? "Rellenar valores vacíos: no queda ninguno tras los demás cambios"
      : imputeTitle(preview.imputedCellCount, preview.imputations.length);
  }
  const total = item.columns.reduce((sum, column) => sum + filledCells(column, selection.sentinels), 0);
  return imputeTitle(total, item.columns.length);
}

/** One row per filled column: the exact value it receives and in how many cells (RV17 / FUN-03). */
export function imputationExamples(preview: SafeCorrectionsPreview): ProposalExample[] {
  return preview.imputations.map((fill) => ({
    column: fill.column,
    before: null,
    after: `${fill.value} · ${plural(fill.cellCount, "celda", "celdas")}`,
  }));
}

export function selectedProposalCount(items: ProposalItem[], selection: ProposalSelection): number {
  // «Convertir a número» with every column waiting on the markers changes nothing.
  return items.filter((item) => selection[item.id]
    && !(item.id === "types" && item.columns && typedColumns(item, selection).length === 0)).length;
}

export function proposalOptions(
  items: ProposalItem[],
  selection: ProposalSelection,
  normalizeColumnNames = false,
  ambiguousDateOrder: DateOrder | null = null,
): SafeCorrectionOptions {
  const on = (id: ProposalItemId) => items.some((item) => item.id === id) && selection[id];
  const datesItem = items.find((item) => item.id === "dates");
  const dateColumns = on("dates") && datesItem ? resolvedDateColumns(datesItem, ambiguousDateOrder) : [];
  return {
    trimText: on("trim"),
    normalizeSentinels: on("sentinels"),
    normalizeColumnNames,
    removeDuplicates: on("duplicates"),
    imputeMissing: on("impute"),
    ...(on("types") ? { castColumns: typedColumnsOf(items, selection) } : {}),
    ...(dateColumns.length > 0 ? { dateColumns } : {}),
    ...(on("impute") ? { imputeColumns: items.find((item) => item.id === "impute")?.columns?.map((column) => column.name) ?? [] } : {}),
  };
}

function typedColumnsOf(items: ProposalItem[], selection: ProposalSelection): string[] {
  const item = items.find((candidate) => candidate.id === "types");
  return item ? typedColumns(item, selection) : [];
}

export function applyLabel(count: number): string {
  return count === 1 ? "Aplicar 1 cambio" : `Aplicar ${count} cambios`;
}
