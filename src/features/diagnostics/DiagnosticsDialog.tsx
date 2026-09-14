import { useState } from "react";

import { saveDiagnosticReport } from "../../bridge/diagnostics";
import { ModalDialog } from "../../components/ModalDialog";
import {
  bucketDatasetMetrics,
  createDiagnosticReport,
  DIAGNOSTIC_ERROR_CODES,
  DIAGNOSTIC_PHASES,
  serializeDiagnosticReport,
  type DatasetMetricInput,
  type DiagnosticErrorCode,
  type DiagnosticPhase,
  type DiagnosticReport,
  type DiagnosticStatus,
} from "./diagnosticsModel";

const PHASE_LABELS: Record<DiagnosticPhase, string> = {
  load: "Cargar",
  review: "Revisar",
  prepare: "Preparar",
  deliver: "Entregar",
};

const STATUS_LABELS: Record<DiagnosticStatus, string> = {
  ready: "La aplicación responde con normalidad",
  working: "Una operación está en curso",
  issue_reported: "Ocurrió un problema",
  no_dataset: "No hay un dataset activo",
};

const ERROR_LABELS: Record<DiagnosticErrorCode, string> = {
  DATASET_LOAD_FAILED: "Carga de dataset",
  DATASET_PROFILE_FAILED: "Análisis de calidad",
  DATASET_REVIEW_FAILED: "Revisión o comparación",
  TRANSFORM_APPLY_FAILED: "Aplicación de transformación",
  LOCAL_EXPORT_FAILED: "Exportación de archivo local",
  DATABASE_EXPORT_FAILED: "Exportación a una base de datos",
  PROJECT_SAVE_FAILED: "Guardado o apertura de proyecto",
  UNKNOWN_OPERATION_FAILED: "Otra operación",
};

interface DiagnosticsDialogProps {
  appVersion: string | null;
  activePhase: DiagnosticPhase;
  datasetMetrics: DatasetMetricInput | null;
  onDismiss: () => void;
}

export function DiagnosticsDialog({
  appVersion,
  activePhase,
  datasetMetrics,
  onDismiss,
}: DiagnosticsDialogProps) {
  const [phase, setPhase] = useState<DiagnosticPhase>(activePhase);
  const [status, setStatus] = useState<DiagnosticStatus | "">("");
  const [errorCodes, setErrorCodes] = useState<DiagnosticErrorCode[]>([]);
  const [includeMetrics, setIncludeMetrics] = useState(false);
  const [preview, setPreview] = useState<DiagnosticReport | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  function toggleErrorCode(code: DiagnosticErrorCode, checked: boolean) {
    setErrorCodes((current) => {
      if (checked) {
        return current.length >= 5 ? current : [...current, code];
      }
      return current.filter((currentCode) => currentCode !== code);
    });
  }

  function buildPreview() {
    if (!appVersion || !status) return;
    try {
      const report = createDiagnosticReport({
        appVersion,
        phase,
        status,
        errorCodes,
        metrics: includeMetrics ? bucketDatasetMetrics(datasetMetrics) : null,
      });
      setPreview(report);
      setMessage(null);
    } catch {
      setMessage("Revisa el estado y los códigos elegidos.");
    }
  }

  async function savePreviewLocally() {
    if (!preview || saving) return;
    setSaving(true);
    setMessage(null);
    try {
      const saved = await saveDiagnosticReport(preview);
      setMessage(saved === null
        ? "Guardado cancelado. No se creó ningún archivo."
        : "Diagnóstico guardado localmente.");
    } catch {
      setMessage("No se pudo guardar el diagnóstico local. Selecciona otra ubicación o revisa sus permisos.");
    } finally {
      setSaving(false);
    }
  }

  const issueNeedsCode = status === "issue_reported" && errorCodes.length === 0;
  const stateNeedsNoCode = status !== "issue_reported" && errorCodes.length > 0;
  const canBuildPreview = Boolean(appVersion && status) && !issueNeedsCode && !stateNeedsNoCode;

  return (
    <ModalDialog
      role="dialog"
      labelledBy="diagnostics-title"
      describedBy="diagnostics-description"
      onDismiss={() => {
        if (!saving) onDismiss();
      }}
    >
      <h2 id="diagnostics-title">Diagnóstico local revisable</h2>
      <p id="diagnostics-description">
        Elige manualmente qué contexto agregado incluir. Columnia no recopila errores en segundo plano ni envía este informe.
      </p>

      {!preview ? (
        <div className="diagnostics-form">
          <p className="diagnostics-privacy-note">
            El contrato solo permite versión, etapa, estado, códigos de una lista y métricas por rangos. No admite mensajes libres,
            rutas, consultas, nombres, valores, datos personales, credenciales, trazas ni identificadores del dispositivo.
          </p>

          <label>
            Versión de Columnia
            <input aria-label="Versión de Columnia" readOnly value={appVersion ?? "No disponible en esta sesión"} />
          </label>

          <label>
            Etapa relacionada
            <select value={phase} onChange={(event) => setPhase(event.target.value as DiagnosticPhase)}>
              {DIAGNOSTIC_PHASES.map((item) => <option key={item} value={item}>{PHASE_LABELS[item]}</option>)}
            </select>
          </label>

          <label>
            Estado que quieres informar
            <select value={status} onChange={(event) => setStatus(event.target.value as DiagnosticStatus | "")}>
              <option value="">Elige un estado</option>
              {(Object.keys(STATUS_LABELS) as DiagnosticStatus[]).map((item) => (
                <option key={item} value={item}>{STATUS_LABELS[item]}</option>
              ))}
            </select>
          </label>

          <fieldset className="diagnostics-code-list">
            <legend>Códigos del problema (opcional; máximo cinco)</legend>
            {DIAGNOSTIC_ERROR_CODES.map((code) => (
              <label key={code}>
                <input
                  type="checkbox"
                  checked={errorCodes.includes(code)}
                  disabled={!errorCodes.includes(code) && errorCodes.length >= 5}
                  onChange={(event) => toggleErrorCode(code, event.currentTarget.checked)}
                />
                {ERROR_LABELS[code]}
              </label>
            ))}
          </fieldset>
          {issueNeedsCode && <p role="status">Elige al menos un código para el estado de problema.</p>}
          {stateNeedsNoCode && <p role="status">Quita los códigos o selecciona “Ocurrió un problema”.</p>}

          <label className="diagnostics-metrics-choice">
            <input
              type="checkbox"
              checked={includeMetrics}
              onChange={(event) => setIncludeMetrics(event.currentTarget.checked)}
            />
            Incluir rangos agregados de filas, columnas y tamaño del archivo de origen
          </label>
          <p className="diagnostics-metrics-note">
            Las métricas son opcionales y se limitan a rangos amplios; nunca contienen conteos exactos, nombres ni valores.
          </p>

          {message && <p role="status">{message}</p>}
          <div className="sheet-dialog__actions">
            <button type="button" onClick={onDismiss}>Cancelar</button>
            <button type="button" className="primary-action" disabled={!canBuildPreview} onClick={buildPreview}>
              Crear vista previa
            </button>
          </div>
        </div>
      ) : (
        <div className="diagnostics-preview">
          <p className="diagnostics-privacy-note">
            Revisa el contenido. Solo “Guardar archivo local” abre el selector nativo; cancelar ese selector no guarda nada.
          </p>
          <pre aria-label="Contenido exacto del diagnóstico">{serializeDiagnosticReport(preview)}</pre>
          {message && <p role="status">{message}</p>}
          <div className="sheet-dialog__actions">
            <button type="button" disabled={saving} onClick={() => { setPreview(null); setMessage(null); }}>
              Volver a editar
            </button>
            <button type="button" disabled={saving} onClick={onDismiss}>Cancelar diagnóstico</button>
            <button type="button" className="primary-action" disabled={saving} onClick={() => void savePreviewLocally()}>
              {saving ? "Guardando…" : "Guardar archivo local"}
            </button>
          </div>
        </div>
      )}
    </ModalDialog>
  );
}
