/**
 * The audit column «Activar auditoría por fila» adds. Its empty cells mark
 * unchanged rows, not missing data, so summaries leave it out; one constant
 * keeps every screen agreeing on that (FUN-49).
 */
export const ROW_AUDIT_COLUMN = "_cambios";

export function isRowAuditColumn(name: string): boolean {
  return name === ROW_AUDIT_COLUMN;
}
