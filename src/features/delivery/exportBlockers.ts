export interface ExportBlockerFlags {
  busy: boolean;
  validationError: boolean;
  /** 1-based numbers of the rules the last validation found broken; empty when it passed or is stale. */
  failedRuleNumbers?: readonly number[];
  missingSqlServerKey: boolean;
  needsPersonalDataConfirmation: boolean;
  needsUnvalidatedConfirmation: boolean;
  databaseNotReady: boolean;
  /** «Reemplazar» is chosen and its own confirmation is not ticked. */
  needsReplaceConfirmation?: boolean;
  exceedsExcel: boolean;
}

function ruleList(numbers: readonly number[]): string {
  if (numbers.length === 1) return `la regla ${numbers[0]}`;
  return `las reglas ${numbers.slice(0, -1).join(", ")} y ${numbers.at(-1)}`;
}

// One phrase per condition that disables the main export button; the button's aria-disabled derives from this list.
export function exportBlockers(flags: ExportBlockerFlags): string[] {
  const failed = flags.failedRuleNumbers ?? [];
  return [
    flags.busy && "espera a que termine la operación en curso",
    flags.validationError && "corrige la regla marcada en el contrato",
    failed.length > 0 && `corrige los datos en Preparar o ajusta ${ruleList(failed)}`,
    flags.missingSqlServerKey && "elige al menos una columna clave para actualizar por clave",
    flags.needsPersonalDataConfirmation && "elige una protección de datos personales o confirma que los exportas sin proteger",
    flags.needsUnvalidatedConfirmation && "confirma que exportas sin validar",
    flags.databaseNotReady && "analiza la compatibilidad con el destino",
    flags.needsReplaceConfirmation && "confirma que se borrará el contenido actual de la tabla",
    flags.exceedsExcel && "elige otro formato: el dataset no cabe en Excel",
  ].filter((blocker): blocker is string => typeof blocker === "string");
}
