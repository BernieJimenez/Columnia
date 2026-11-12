import type { DatabaseImportResult, ImportOrigin } from "../../bridge";
import { plural } from "../../plural";

/**
 * AUD-061: what the import left out. Only a partial import (the limit was
 * reached and the table had more rows) or one that cut or replaced texts
 * needs to be remembered; a complete one leaves nothing to say.
 */
export function importOriginFromResult(result: DatabaseImportResult, table: string): ImportOrigin | null {
  if (!result.moreRowsAvailable && result.truncatedColumns.length === 0 && result.replacedColumns.length === 0) {
    return null;
  }
  return {
    table,
    rowsImported: result.rowsImported,
    partial: result.moreRowsAvailable,
    truncatedColumns: result.truncatedColumns,
    replacedColumns: result.replacedColumns,
  };
}

function columnList(names: readonly string[]): string {
  return names.join(", ");
}

/** «primeras 100.000 filas de dbo.Ventas»; only said when the table had more rows than were read. */
function importOriginRows(origin: ImportOrigin): string | null {
  return origin.partial
    ? `primeras ${plural(origin.rowsImported, "fila", "filas")} de ${origin.table}`
    : null;
}

/** The lines Entregar adds to its summary, in the order they are read. */
export function importOriginSummaryItems(
  origin: ImportOrigin | null | undefined,
): Array<{ label: string; value: string }> {
  if (!origin) return [];
  const items: Array<{ label: string; value: string }> = [];
  const rows = importOriginRows(origin);
  if (rows) items.push({ label: "Origen", value: rows });
  if (origin.truncatedColumns.length > 0) {
    items.push({ label: "Columnas con textos recortados", value: columnList(origin.truncatedColumns) });
  }
  if (origin.replacedColumns.length > 0) {
    items.push({ label: "Columnas con caracteres sustituidos", value: columnList(origin.replacedColumns) });
  }
  return items;
}

/** The short notice of the top bar, with the long form for its tooltip; null when there is nothing to warn. */
export function importOriginNotice(origin: ImportOrigin | null | undefined): { text: string; title: string } | null {
  if (!origin) return null;
  const rows = importOriginRows(origin);
  const items = importOriginSummaryItems(origin);
  if (items.length === 0) return null;
  const title = items.map((item) => `${item.label}: ${item.value}`).join(". ");
  if (rows) return { text: `Parte de la tabla: ${rows}`, title };
  return { text: "Textos modificados en la importación", title };
}
