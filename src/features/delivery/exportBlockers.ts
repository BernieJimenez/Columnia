export interface ExportBlockerFlags {
  busy: boolean;
  validationError: boolean;
  missingSqlServerKey: boolean;
  needsPersonalDataConfirmation: boolean;
  needsUnvalidatedConfirmation: boolean;
  databaseNotReady: boolean;
  exceedsExcel: boolean;
}

// One phrase per condition that disables the main export button; the button's aria-disabled derives from this list.
export function exportBlockers(flags: ExportBlockerFlags): string[] {
  return [
    flags.busy && "espera a que termine la operación en curso",
    flags.validationError && "corrige la regla marcada en el contrato",
    flags.missingSqlServerKey && "elige al menos una columna clave para actualizar por clave",
    flags.needsPersonalDataConfirmation && "elige una protección de datos personales o confirma que los exportas sin proteger",
    flags.needsUnvalidatedConfirmation && "confirma que exportas sin validar",
    flags.databaseNotReady && "analiza la compatibilidad con el destino",
    flags.exceedsExcel && "elige otro formato: el dataset no cabe en Excel",
  ].filter((blocker): blocker is string => typeof blocker === "string");
}
