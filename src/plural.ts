/**
 * «1 fila», «2 filas», «0 filas» (QA-08, TXT-05): a count with the singular
 * or plural form of what it counts.
 */
export function plural(count: number, one: string, many: string): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}
