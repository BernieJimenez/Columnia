/** Which background or foreground operations keep the app busy (names only, no data). */
export interface BusyFlags {
  dataset: boolean;
  page: boolean;
  profile: boolean;
  prepare: boolean;
  qualityGate: boolean;
  exportRunning: boolean;
  comparison: boolean;
  join: boolean;
  reviewMutation: boolean;
  /** Detail of the file selection in progress (e.g. «inspecting»), or null when there is none. */
  selection: string | null;
  project: boolean;
  autosave: boolean;
}

// Order matters: the first active reason is the one shown to the user.
const REASONS: ReadonlyArray<{ code: string; active: (flags: BusyFlags) => boolean; phrase: string }> = [
  { code: "dataset", active: (f) => f.dataset, phrase: "la carga del dataset" },
  { code: "page", active: (f) => f.page, phrase: "el cambio de página" },
  { code: "profile", active: (f) => f.profile, phrase: "el análisis de calidad" },
  { code: "prepare", active: (f) => f.prepare, phrase: "la aplicación de cambios" },
  { code: "quality-gate", active: (f) => f.qualityGate, phrase: "la comprobación de calidad" },
  { code: "export", active: (f) => f.exportRunning, phrase: "la exportación" },
  { code: "comparison", active: (f) => f.comparison, phrase: "la comparación" },
  { code: "join", active: (f) => f.join, phrase: "el cruce de datasets" },
  { code: "review-mutation", active: (f) => f.reviewMutation, phrase: "la modificación en revisión" },
  { code: "selection", active: (f) => f.selection !== null, phrase: "la selección del archivo" },
  { code: "project", active: (f) => f.project, phrase: "la operación del proyecto" },
  { code: "autosave", active: (f) => f.autosave, phrase: "el guardado automático" },
];

/** Comma-separated names for `data-busy-reasons` (native probes). */
export function busyReasonCodes(flags: BusyFlags): string {
  return REASONS.filter((reason) => reason.active(flags))
    .map((reason) => reason.code === "selection" ? `selection:${flags.selection}` : reason.code)
    .join(",");
}

/** What the app is waiting for, as a noun phrase («el análisis de calidad»), or null when idle. */
export function busyReasonPhrase(flags: BusyFlags): string | null {
  return REASONS.find((reason) => reason.active(flags))?.phrase ?? null;
}

/** One sentence that tells why controls are disabled, or "" when nothing is running. */
export function busyReasonMessage(flags: BusyFlags): string {
  const phrase = busyReasonPhrase(flags);
  return phrase ? `Disponible al terminar ${phrase}.` : "";
}
