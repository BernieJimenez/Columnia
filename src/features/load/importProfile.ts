import type {
  DatasetPreview,
  DatasetSourceInspection,
  ImportDateConvention,
  ImportNumberConvention,
  ImportProfile,
  ImportProfileMismatch,
  SpreadsheetHeaderMode,
} from "../../bridge";

export const IMPORT_PROFILE_MISMATCH_PREFIX = "__columnia_import_profile_mismatch__:";

export type ImportProfileApplicability =
  | { kind: "applicable"; sheetId: string | null; headerMode: SpreadsheetHeaderMode | null }
  | { kind: "format_mismatch" }
  | { kind: "sheet_missing" };

export function importProfileApplicability(
  profile: ImportProfile,
  source: DatasetSourceInspection,
): ImportProfileApplicability {
  if (profile.version !== 1 || profile.format !== source.format) {
    return { kind: "format_mismatch" };
  }
  if (source.format === "csv" || source.format === "tsv") {
    if (profile.sheetName || !profile.headerMode) return { kind: "format_mismatch" };
    return { kind: "applicable", sheetId: null, headerMode: profile.headerMode };
  }
  if (source.format !== "excel") {
    return profile.sheetName || profile.headerMode
      ? { kind: "format_mismatch" }
      : { kind: "applicable", sheetId: null, headerMode: null };
  }
  if (!profile.sheetName || !profile.headerMode) return { kind: "format_mismatch" };
  const sheet = source.sheets.find((candidate) => candidate.name === profile.sheetName);
  return sheet
    ? { kind: "applicable", sheetId: sheet.id, headerMode: profile.headerMode }
    : { kind: "sheet_missing" };
}

export function createImportProfile(
  source: DatasetSourceInspection,
  dataset: DatasetPreview,
  options: { sheetId: string | null; headerMode: SpreadsheetHeaderMode | null },
  conventions?: Pick<ImportProfile, "dateConvention" | "numberConvention"> | null,
): ImportProfile {
  const sheetName = source.format === "excel"
    ? source.sheets.find((sheet) => sheet.id === options.sheetId)?.name
    : undefined;
  const headerMode = source.format === "excel" || source.format === "csv" || source.format === "tsv"
    ? options.headerMode ?? "firstRow"
    : undefined;
  return {
    version: 1,
    format: source.format,
    ...(sheetName ? { sheetName } : {}),
    ...(headerMode ? { headerMode } : {}),
    dateConvention: conventions?.dateConvention ?? "unresolved",
    numberConvention: conventions?.numberConvention ?? "unresolved",
    schema: dataset.columns.map(({ name, dataType }) => ({ name, dataType })),
  };
}

export function parseImportProfileMismatch(error: unknown): ImportProfileMismatch | null {
  const message = error instanceof Error ? error.message : String(error);
  if (!message.startsWith(IMPORT_PROFILE_MISMATCH_PREFIX)) return null;
  try {
    const parsed: unknown = JSON.parse(message.slice(IMPORT_PROFILE_MISMATCH_PREFIX.length));
    if (!parsed || typeof parsed !== "object") return null;
    const mismatch = parsed as Partial<ImportProfileMismatch>;
    if (
      mismatch.code !== "importProfileSchemaMismatch" ||
      !Array.isArray(mismatch.missingColumns) ||
      !Array.isArray(mismatch.addedColumns) ||
      !Array.isArray(mismatch.changedTypes) ||
      !mismatch.missingColumns.every((value) => typeof value === "string") ||
      !mismatch.addedColumns.every((value) => typeof value === "string") ||
      !mismatch.changedTypes.every((value) =>
        value && typeof value.column === "string" &&
        typeof value.expected === "string" && typeof value.actual === "string")
    ) {
      return null;
    }
    return mismatch as ImportProfileMismatch;
  } catch {
    return null;
  }
}

export const DATE_CONVENTIONS: Array<{ value: ImportDateConvention; label: string }> = [
  { value: "unresolved", label: "Sin definir" },
  { value: "iso8601", label: "ISO 8601" },
  { value: "ymd", label: "Año-mes-día" },
  { value: "dmy", label: "Día-mes-año" },
  { value: "mdy", label: "Mes-día-año" },
];

export const NUMBER_CONVENTIONS: Array<{ value: ImportNumberConvention; label: string }> = [
  { value: "unresolved", label: "Sin definir" },
  { value: "dotDecimalCommaGrouping", label: "Decimal punto · miles coma" },
  { value: "commaDecimalDotGrouping", label: "Decimal coma · miles punto" },
  { value: "dotDecimalSpaceGrouping", label: "Decimal punto · miles espacio" },
  { value: "commaDecimalSpaceGrouping", label: "Decimal coma · miles espacio" },
  { value: "integer", label: "Números enteros" },
];
