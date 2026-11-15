// «Corregir columna por columna»: one row per column with something to fix,
// each with a single recommended action. Most engine actions work on every
// column they apply to at once; such an action sits in the row only when this
// column is the only one it touches, otherwise in «En varias columnas».
import type { ColumnProfile, DatasetProfile } from "../../bridge";
import { formatDecimal } from "../../format";
import { plural } from "../../plural";
import { isBooleanType, isDateType, isDatetimeType, isNumericType, isTextType } from "../../dataTypes";
import { isRowAuditColumn } from "../../rowAudit";
import { dateConversionCandidates, numericConversionCandidates } from "./prepareModel";
import { isCategoryColumn, looksLikeIdentifier } from "./proposalModel";

/** The share of nulls from which a column is offered for removal (as the engine). */
export const REMOVAL_NULL_PERCENTAGE = 80;

/** Engine actions that work on every column they apply to. */
export type SharedActionId =
  | "removeEmpty"
  | "removeHighNull"
  | "removeConstant"
  | "castNumeric"
  | "parseDates"
  | "nullifyInvalid"
  | "maskPersonal"
  | "removePersonal"
  | "removeIdentifier"
  | "capOutliers"
  | "imputeOutliers"
  | "dropOutliers"
  | "fixEncoding"
  | "normalizeBooleans"
  | "fillUnknown";

/** An action on one column only: the median or the most frequent value. */
export interface ColumnFill {
  kind: "fill";
  column: string;
  method: "median" | "mode";
  /** Nulls it fills: the «sin dato» markers stay as text until converted. */
  count: number;
  /** The median, written as it will be stored; null for the most frequent value. */
  value: string | null;
}

export type FixAction = { kind: "shared"; id: SharedActionId } | ColumnFill;

/** The groups of «Corregir columna por columna», most serious first. */
type ColumnFixGroup = "removal" | "types" | "personal" | "outliers" | "text" | "fill" | "identifier";

export const COLUMN_FIX_GROUPS: ReadonlyArray<{ id: ColumnFixGroup; title: string }> = [
  { id: "removal", title: "Columnas sin datos útiles" },
  { id: "types", title: "Tipos que no cuadran" },
  { id: "personal", title: "Datos personales" },
  { id: "outliers", title: "Valores atípicos" },
  { id: "text", title: "Texto" },
  { id: "fill", title: "Huecos que rellenar" },
  { id: "identifier", title: "Identificadores" },
];

interface ColumnFixRow {
  column: ColumnProfile;
  typeLabel: string;
  /** What happens to the column, figures of every row. */
  issues: string[];
  recommended: FixAction | null;
  /** What the row needs said besides its action: why, or what to do elsewhere. */
  notes: string[];
  alternatives: FixAction[];
  group: ColumnFixGroup;
}

type FixScopes = Record<SharedActionId, ColumnProfile[]>;

interface ColumnFixPlan {
  rows: ColumnFixRow[];
  scopes: FixScopes;
}

const PERSONAL_LABELS = {
  email: "correo electrónico",
  phone: "teléfono",
  address: "dirección",
  name: "nombre de persona",
} as const;

function isPersonal(column: ColumnProfile): boolean {
  return column.privacySignal !== null && column.privacySignal in PERSONAL_LABELS;
}

function columnTypeLabel(column: ColumnProfile): string {
  if (isTextType(column.dataType)) return "Texto";
  if (isBooleanType(column.dataType)) return "Sí/no";
  if (isDatetimeType(column.dataType)) return "Fecha y hora";
  if (isDateType(column.dataType)) return "Fecha";
  if (isNumericType(column.dataType)) return "Número";
  return column.dataType;
}

/** «83 %», never more than the real share: 79,6 % is not «80 %». */
function share(count: number, rowCount: number): string {
  if (rowCount === 0) return "0 %";
  const percent = (count * 100) / rowCount;
  if (percent > 0 && percent < 1) return "menos del 1 %";
  return `${Math.floor(percent).toLocaleString("es-ES")} %`;
}

function suggestedWord(column: ColumnProfile): string {
  switch (column.suggestedType) {
    case "integer":
    case "decimal":
      return "número";
    case "date":
      return "fecha";
    case "boolean":
      return "sí/no";
    default:
      return "el tipo sugerido";
  }
}

function countOf(value: number | null | undefined): number {
  return value ?? 0;
}

export function columnFixScopes(profile: DatasetProfile): FixScopes {
  const rows = profile.rowCount;
  const columns = profile.columns.filter((column) => !isRowAuditColumn(column.name));
  const tooEmpty = (column: ColumnProfile) => rows > 0 && column.nullCount * 100 >= rows * REMOVAL_NULL_PERCENTAGE;
  return {
    removeEmpty: columns.filter((column) => rows > 0 && column.nullCount === rows),
    removeHighNull: columns.filter((column) => column.nullCount > 0 && column.nullCount < rows && tooEmpty(column)),
    // FUN-71: one value in every row, no empty cell (as the engine).
    removeConstant: columns.filter((column) => rows > 1 && column.uniqueCount <= 1 && column.nullCount === 0),
    castNumeric: numericConversionCandidates(profile),
    parseDates: dateConversionCandidates(profile),
    nullifyInvalid: columns.filter((column) => countOf(column.invalidTypeCount) > 0),
    maskPersonal: columns.filter(isPersonal),
    removePersonal: columns.filter(isPersonal),
    removeIdentifier: profile.columns.filter((column) => column.privacySignal === "identifier"),
    capOutliers: columns.filter((column) => countOf(column.outlierCount) > 0),
    imputeOutliers: columns.filter((column) => countOf(column.outlierCount) > 0),
    dropOutliers: columns.filter((column) => countOf(column.outlierCount) > 0),
    fixEncoding: columns.filter((column) => countOf(column.encodingIssueCount) > 0),
    normalizeBooleans: columns.filter(
      (column) => isTextType(column.dataType) && column.suggestedType === "boolean" && countOf(column.typeMatchPercentage) >= 90,
    ),
    // The engine fills every text column with nulls except personal data,
    // columns offered for removal and numbers, dates or yes/no stored as text
    // (offered for conversion): the confirmation names exactly these.
    fillUnknown: columns.filter(
      (column) => isTextType(column.dataType) && column.nullCount > 0 && !column.privacySignal && !tooEmpty(column)
        && column.suggestedType === null,
    ),
  };
}

/** The fill of this column alone, when its median or most frequent value means something. */
function columnFill(column: ColumnProfile, rowCount: number): ColumnFill | null {
  if (column.nullCount === 0 || column.nullCount * 100 >= rowCount * REMOVAL_NULL_PERCENTAGE) return null;
  // FUN-47: personal data, keys and dates are never filled with a made-up value.
  if (column.privacySignal || looksLikeIdentifier(column.name) || column.suggestedType === "date") return null;
  if (isNumericType(column.dataType) && column.median !== null) {
    const value = formatDecimal(column.median, Number.isInteger(column.median) ? 0 : 2);
    return { kind: "fill", column: column.name, method: "median", count: column.nullCount, value };
  }
  if (isCategoryColumn(column, rowCount)) {
    return { kind: "fill", column: column.name, method: "mode", count: column.nullCount, value: null };
  }
  return null;
}

/** Which group a recommended action puts its row in. */
const ACTION_GROUP: Record<SharedActionId, ColumnFixGroup> = {
  removeEmpty: "removal",
  removeHighNull: "removal",
  removeConstant: "removal",
  castNumeric: "types",
  parseDates: "types",
  nullifyInvalid: "types",
  maskPersonal: "personal",
  removePersonal: "personal",
  removeIdentifier: "identifier",
  capOutliers: "outliers",
  imputeOutliers: "outliers",
  dropOutliers: "outliers",
  fixEncoding: "text",
  normalizeBooleans: "text",
  fillUnknown: "fill",
};

/** Within a group, the more serious action first (an empty column before a constant one). */
const ACTION_RANK: SharedActionId[] = [
  "removeEmpty", "removeHighNull", "removeConstant", "nullifyInvalid", "castNumeric", "parseDates",
  "maskPersonal", "removePersonal", "capOutliers", "imputeOutliers", "dropOutliers", "fixEncoding",
  "normalizeBooleans", "fillUnknown", "removeIdentifier",
];

function actionRank(action: FixAction | null): number {
  if (action === null) return ACTION_RANK.length + 1;
  return action.kind === "fill" ? ACTION_RANK.length : ACTION_RANK.indexOf(action.id);
}

/** Text whose dates read both ways (01/02/2024): the engine never converts it without an answer. */
function hasAmbiguousDates(column: ColumnProfile): boolean {
  return isTextType(column.dataType) && column.suggestedType === "date" && column.dateOrder === "ambiguous";
}

/**
 * `available` says whether the app offers a shared action (some callbacks are
 * optional): an action it does not offer gives way to the next one.
 */
export function buildColumnFixes(
  profile: DatasetProfile,
  available: (id: SharedActionId) => boolean = () => true,
): ColumnFixPlan {
  const scopes = columnFixScopes(profile);
  const rowCount = profile.rowCount;
  const inScope = (id: SharedActionId, column: ColumnProfile) => scopes[id].some((candidate) => candidate.name === column.name);
  const shared = (id: SharedActionId): FixAction => ({ kind: "shared", id });
  const offered = (action: FixAction) => action.kind === "fill" || available(action.id);

  const rows = profile.columns.filter((column) => !isRowAuditColumn(column.name)).flatMap((column): ColumnFixRow[] => {
    const issues: string[] = [];
    const sentinels = countOf(column.sentinelCount);
    const invalid = countOf(column.invalidTypeCount);
    const empty = inScope("removeEmpty", column);
    const ambiguousDates = hasAmbiguousDates(column);
    const identifier = column.privacySignal === "identifier";
    if (empty) issues.push(`100 % nulos: no tiene ningún dato (${plural(rowCount, "fila", "filas")})`);
    else if (column.nullCount > 0) issues.push(`${share(column.nullCount, rowCount)} nulos (${plural(column.nullCount, "fila", "filas")})`);
    if (sentinels > 0) issues.push(plural(sentinels, "marcador «sin dato»", "marcadores «sin dato»"));
    if (inScope("removeConstant", column)) issues.push("el mismo valor en todas las filas");
    if (invalid > 0) issues.push(`${plural(invalid, "valor incompatible", "valores incompatibles")} con ${suggestedWord(column)}`);
    else if (inScope("castNumeric", column)) issues.push("texto con solo números");
    else if (inScope("parseDates", column) || ambiguousDates) issues.push("texto con fechas");
    if (isPersonal(column)) issues.push(`posible ${PERSONAL_LABELS[column.privacySignal as keyof typeof PERSONAL_LABELS]}`);
    if (countOf(column.outlierCount) > 0) {
      issues.push(`${plural(countOf(column.outlierCount), "valor", "valores")} fuera del rango normal, lejos de la mitad central de sus datos (regla IQR)`);
    }
    if (countOf(column.encodingIssueCount) > 0) {
      issues.push(`${plural(countOf(column.encodingIssueCount), "valor", "valores")} con acentos mal leídos («Ã¡» en lugar de «á»)`);
    }
    if (inScope("normalizeBooleans", column)) issues.push("sí/no escrito de varias formas");
    if (issues.length === 0 && !identifier) return [];

    const fill = columnFill(column, rowCount);
    const unknown = inScope("fillUnknown", column) && !looksLikeIdentifier(column.name) && column.suggestedType !== "date"
      ? shared("fillUnknown")
      : null;
    // Filling, in the order of preference: the column's own value, then «Desconocido».
    const fills = [fill, unknown].filter((action): action is FixAction => action !== null);

    // Every action that applies, in order of priority: the first is the one
    // recommendation; the rest fold under «Otras opciones». A column to remove
    // gets no other option: it is never filled, converted or treated.
    const removal = empty
      ? shared("removeEmpty")
      : inScope("removeHighNull", column)
        ? shared("removeHighNull")
        : inScope("removeConstant", column) ? shared("removeConstant") : null;
    // A key is a note, never a recommendation: removing it only sits under
    // «Otras opciones».
    const candidates: FixAction[] = (removal ? [removal] : [
      ...(inScope("castNumeric", column) ? [shared("castNumeric")] : []),
      ...(inScope("parseDates", column) ? [shared("parseDates")] : []),
      ...(invalid > 0 ? [shared("nullifyInvalid")] : []),
      ...(isPersonal(column) ? [shared("maskPersonal"), shared("removePersonal")] : []),
      ...(countOf(column.outlierCount) > 0 ? [shared("capOutliers"), shared("imputeOutliers"), shared("dropOutliers")] : []),
      ...(countOf(column.encodingIssueCount) > 0 ? [shared("fixEncoding")] : []),
      ...(inScope("normalizeBooleans", column) ? [shared("normalizeBooleans")] : []),
      ...fills,
    ]).filter(offered);
    const recommended = candidates[0] ?? null;
    const identifierRemoval = !removal && identifier && offered(shared("removeIdentifier")) ? [shared("removeIdentifier")] : [];
    const alternatives = [...candidates.slice(1), ...identifierRemoval];

    const notes: string[] = [];
    if (ambiguousDates && !removal) {
      notes.push(column.privacySignal
        ? "Sus fechas se leen de dos formas (día/mes o mes/día): conviértela en «Transformar columnas» con el formato que le corresponda."
        : "Sus fechas se leen de dos formas (día/mes o mes/día): elige el orden en la propuesta de arriba.");
    }
    if (identifier && !removal) notes.push("Parece un identificador: suele ser la clave de la tabla.");
    if (recommended?.kind === "shared" && recommended.id === "maskPersonal") {
      notes.push("Ocultar sus valores protege la privacidad si vas a entregar los datos; no es obligatorio.");
    }
    if (recommended === null && (column.nullCount > 0 || sentinels > 0)) {
      notes.push(sentinels > 0
        ? "Los marcadores «sin dato» se convierten en nulos con la propuesta de arriba; después se pueden rellenar aquí."
        : "Nada de esta sección cambia esta columna sin inventar valores: sus nulos se conservan.");
    }
    const group: ColumnFixGroup = recommended !== null
      ? recommended.kind === "fill" ? "fill" : ACTION_GROUP[recommended.id]
      : ambiguousDates ? "types" : identifier ? "identifier" : "fill";
    return [{ column, typeLabel: columnTypeLabel(column), issues, recommended, notes, alternatives, group }];
  });

  // Most serious first: by group, then by action; ties keep the profile order.
  const groupIndex = (row: ColumnFixRow) => COLUMN_FIX_GROUPS.findIndex((group) => group.id === row.group);
  const sorted = rows
    .map((row, index) => ({ row, index }))
    .sort((a, b) => groupIndex(a.row) - groupIndex(b.row)
      || actionRank(a.row.recommended) - actionRank(b.row.recommended)
      || a.index - b.index)
    .map(({ row }) => row);

  return { rows: sorted, scopes };
}

/** The columns a shared action touches, or the one column of a fill. */
export function actionColumns(action: FixAction, scopes: FixScopes): string[] {
  return action.kind === "fill" ? [action.column] : scopes[action.id].map((column) => column.name);
}

/** «x, y y z». */
export function listNames(names: string[]): string {
  return names.length <= 1 ? names.join("") : `${names.slice(0, -1).join(", ")} y ${names.at(-1)}`;
}

const sum = (columns: ColumnProfile[], pick: (column: ColumnProfile) => number | null | undefined) =>
  columns.reduce((total, column) => total + countOf(pick(column)), 0);

/**
 * The button of an action: verb and object, with its figure. Actions that
 * change data in a way that needs a second look end in «…» and ask first.
 */
export function actionLabel(action: FixAction, scopes: FixScopes): string {
  if (action.kind === "fill") {
    return action.method === "median"
      ? `Rellenar ${plural(action.count, "nulo", "nulos")} de «${action.column}» con la mediana (${action.value ?? ""})…`
      : `Rellenar ${plural(action.count, "nulo", "nulos")} de «${action.column}» con el valor más frecuente…`;
  }
  const columns = scopes[action.id];
  const one = columns.length === 1 ? columns[0]?.name ?? "" : null;
  const where = one === null ? ` en ${plural(columns.length, "columna", "columnas")}` : ` de «${one}»`;
  const outliers = sum(columns, (column) => column.outlierCount);
  const cols = plural(columns.length, "columna", "columnas");
  // A conversion asks first when some value would be left null (see conversionNullEstimate).
  const asks = columns.some((column) => countOf(column.invalidTypeCount) > 0) ? "…" : "";
  switch (action.id) {
    case "removeEmpty":
      return one !== null ? `Eliminar columna «${one}»` : `Eliminar ${plural(columns.length, "columna vacía", "columnas vacías")}`;
    case "removeHighNull":
      return one !== null
        ? `Eliminar columna «${one}»`
        : `Eliminar ${cols} con ${REMOVAL_NULL_PERCENTAGE} % o más de nulos`;
    case "removeConstant":
      return one !== null ? `Eliminar columna «${one}»` : `Eliminar ${plural(columns.length, "columna constante", "columnas constantes")}`;
    case "castNumeric":
      return one !== null ? `Convertir «${one}» a número${asks}` : `Convertir ${cols} a número${asks}`;
    case "parseDates":
      return one !== null ? `Convertir «${one}» a fecha${asks}` : `Convertir ${cols} a fecha${asks}`;
    case "nullifyInvalid":
      return `Dejar como nulos ${plural(sum(columns, (column) => column.invalidTypeCount), "valor incompatible", "valores incompatibles")}${where}…`;
    case "maskPersonal":
      return one !== null ? `Ocultar los valores de «${one}»…` : `Ocultar valores personales en ${cols}…`;
    case "removePersonal":
      return one !== null ? `Eliminar columna «${one}»…` : `Eliminar ${cols} de datos personales…`;
    case "removeIdentifier":
      return one !== null ? `Eliminar columna identificadora «${one}»…` : `Eliminar ${cols} identificadoras…`;
    case "capOutliers":
      return `Limitar ${plural(outliers, "valor atípico", "valores atípicos")}${where} al rango normal…`;
    case "imputeOutliers":
      return `Reemplazar ${plural(outliers, "valor atípico", "valores atípicos")}${where} por la mediana…`;
    case "dropOutliers":
      return `Eliminar hasta ${plural(outliers, "fila", "filas")} con valores atípicos${where}…`;
    case "fixEncoding":
      return `Corregir acentos en ${plural(sum(columns, (column) => column.encodingIssueCount), "valor", "valores")}${where}`;
    case "normalizeBooleans":
      return one !== null ? `Unificar sí/no en «${one}»` : `Unificar sí/no en ${cols}`;
    case "fillUnknown":
      return `Completar ${plural(sum(columns, (column) => column.nullCount), "nulo", "nulos")}${where} como «Desconocido»…`;
  }
}

/** What a shared action does, one sentence, for «En varias columnas». */
export function actionEffect(id: SharedActionId): string {
  switch (id) {
    case "removeEmpty":
      return "Elimina las columnas sin ningún dato; se conserva al menos una columna.";
    case "removeHighNull":
      return `Elimina las columnas con ${REMOVAL_NULL_PERCENTAGE} % o más de nulos; nunca se rellenan.`;
    case "removeConstant":
      return "Elimina las columnas con el mismo valor en todas las filas; se conserva al menos una columna.";
    case "castNumeric":
      return "Pasa a número el texto que solo tiene números; los códigos con ceros iniciales no se tocan y, si algún valor quedaría nulo, se pide confirmación.";
    case "parseDates":
      return "Pasa a fecha el texto con un formato de fecha claro; las columnas ambiguas no se tocan y, si algún valor quedaría nulo, se pide confirmación.";
    case "nullifyInvalid":
      return "Deja como nulos los valores que no encajan con el tipo que tiene al menos el 90 % de la columna.";
    case "maskPersonal":
    case "removePersonal":
      return "Correo, teléfono, dirección y nombre detectados por el encabezado: ocultar sus valores protege la privacidad al entregar los datos; no es obligatorio.";
    case "removeIdentifier":
      return "Elimina las columnas que el encabezado señala como identificadores; suelen ser la clave de la tabla.";
    case "capOutliers":
    case "imputeOutliers":
    case "dropOutliers":
      return "Valores fuera del rango normal de su columna, lejos de la mitad central de sus datos (regla IQR); cada opción pide confirmación.";
    case "fixEncoding":
      return "Repara acentos y signos mal leídos, como «Ã©» por «é»; solo cuando la reparación es inequívoca.";
    case "normalizeBooleans":
      return "Escribe «sí»/«no», «yes»/«no» y «true»/«false» siempre como «true» y «false»; lo que no reconoce queda igual.";
    case "fillUnknown":
      return "Escribe «Desconocido» en los nulos de texto; es un valor inventado y pide confirmación.";
  }
}

/** For «En varias columnas»: one entry per kind of change, its alternatives folded under it. */
interface SharedEntry {
  primary: SharedActionId;
  alternatives: SharedActionId[];
}

const ENTRY_ALTERNATIVES: Partial<Record<SharedActionId, SharedActionId[]>> = {
  maskPersonal: ["removePersonal"],
  capOutliers: ["imputeOutliers", "dropOutliers"],
};
const FOLDED = new Set<SharedActionId>(["removePersonal", "imputeOutliers", "dropOutliers"]);

/**
 * The shared actions that touch more than one column and that some row
 * offers: they cannot sit in a single row.
 */
export function sharedEntries(plan: ColumnFixPlan): SharedEntry[] {
  const offered = new Set<SharedActionId>();
  for (const row of plan.rows) {
    for (const action of [row.recommended, ...row.alternatives]) {
      if (action?.kind === "shared") offered.add(action.id);
    }
  }
  const order: SharedActionId[] = [
    "removeEmpty", "removeHighNull", "removeConstant", "castNumeric", "parseDates", "nullifyInvalid",
    "maskPersonal", "removeIdentifier", "capOutliers", "fixEncoding", "normalizeBooleans", "fillUnknown",
  ];
  return order
    .filter((id) => !FOLDED.has(id) && offered.has(id) && plan.scopes[id].length > 1)
    .map((id) => ({ primary: id, alternatives: (ENTRY_ALTERNATIVES[id] ?? []).filter((alt) => offered.has(alt)) }));
}
