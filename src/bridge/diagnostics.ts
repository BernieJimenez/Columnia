import { invoke } from "@tauri-apps/api/core";

import type { DiagnosticReport } from "./diagnostics-contracts";

/** Abre un selector nativo local y devuelve null cuando la persona lo cancela. */
export function saveDiagnosticReport(report: DiagnosticReport): Promise<void | null> {
  return invoke<void | null>("save_diagnostic_report", { report });
}
