import { useEffect, useEffectEvent, useState } from "react";

import { openLastExport, openLastExportInPowerBi } from "../../bridge";
import type { SavedRecipe } from "../../bridge";
import { formatFileSize } from "./DatasetMetrics";
import type { DeliveryContractState, DeliveryExportState } from "./deliveryModel";

type ExportResult = Extract<DeliveryExportState, { kind: "success" }>["result"];

interface DeliveryResultProps {
  result: ExportResult;
  /** «Exportar y abrir en Power BI» was used for this export. */
  autoOpenPowerBi: boolean;
  contract: DeliveryContractState;
  recipeDraft: SavedRecipe | null;
  preparationChanges: string[];
  approvedQualitySummary: string | null;
}

/** «Copia lista»: what was delivered, with «Abrir carpeta» and «Abrir en Power BI». */
export function DeliveryResult({
  result,
  autoOpenPowerBi,
  contract,
  recipeDraft,
  preparationChanges,
  approvedQualitySummary,
}: DeliveryResultProps) {
  const [openOutputState, setOpenOutputState] = useState<"idle" | "working" | "opened" | "error">("idle");
  const [powerBiState, setPowerBiState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "opened"; quotedLineBreaks: boolean }
    | { kind: "error"; message: string }
  >({ kind: "idle" });

  async function revealLastExport() {
    if (openOutputState === "working") return;
    setOpenOutputState("working");
    try {
      await openLastExport();
      setOpenOutputState("opened");
    } catch {
      setOpenOutputState("error");
    }
  }

  async function openInPowerBi() {
    if (powerBiState.kind === "working") return;
    setPowerBiState({ kind: "working" });
    try {
      const quotedLineBreaks = await openLastExportInPowerBi();
      setPowerBiState({ kind: "opened", quotedLineBreaks });
    } catch (error: unknown) {
      setPowerBiState({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  const openPowerBiOnce = useEffectEvent(() => {
    void openInPowerBi();
  });
  useEffect(() => {
    if (autoOpenPowerBi) openPowerBiOnce();
  }, [autoOpenPowerBi]);

  return (
        <section className="delivery-result" aria-labelledby="delivery-result-title" aria-live="polite">
          <div className="delivery-result__heading">
            <div>
              <p className="step">Entrega completada</p>
              <h3 id="delivery-result-title">Copia lista</h3>
              <p>La salida se publicó correctamente. El dataset preparado sigue disponible en Columnia.</p>
            </div>
            {result.format !== "PostgreSQL"
              && result.format !== "MySQL"
              && result.format !== "SQL Server" && (
              <div className="delivery-result__actions">
                <button
                  type="button"
                  className="primary-action"
                  onClick={() => void revealLastExport()}
                  disabled={openOutputState === "working"}
                >
                  {openOutputState === "working" ? "Abriendo carpeta…" : "Abrir carpeta"}
                </button>
                {(result.format === "CSV" || result.format === "Excel") && (
                  <button
                    type="button"
                    className="secondary-action"
                    onClick={() => void openInPowerBi()}
                    disabled={powerBiState.kind === "working"}
                  >
                    {powerBiState.kind === "working" ? "Abriendo Power BI…" : "Abrir en Power BI"}
                  </button>
                )}
              </div>
            )}
          </div>
          <dl className="delivery-result__facts">
            <div>
              <dt>{result.format === "PostgreSQL" || result.format === "MySQL" || result.format === "SQL Server" ? "Destino" : "Archivo"}</dt>
              <dd>{result.fileName}</dd>
            </div>
            <div>
              <dt>Formato</dt>
              <dd>{result.format}</dd>
            </div>
            {result.format !== "PostgreSQL"
              && result.format !== "MySQL"
              && result.format !== "SQL Server" && (
              <div>
                <dt>Tamaño</dt>
                <dd>{formatFileSize(result.fileSizeBytes)}</dd>
              </div>
            )}
            <div>
              <dt>Calidad</dt>
              <dd>{approvedQualitySummary ?? "Salida confirmada sin reglas de calidad"}</dd>
            </div>
            <div>
              <dt>Preparación incluida</dt>
              <dd>{preparationChanges.length > 0
                ? `${preparationChanges.length.toLocaleString()} ${preparationChanges.length === 1 ? "cambio" : "cambios"} del historial activo`
                : "Dataset activo sin cambios registrados en el historial"}</dd>
            </div>
            <div>
              <dt>Protección adicional</dt>
              <dd>{result.protectedColumnCount > 0
                ? `${result.protectedColumnCount.toLocaleString()} columnas: ${result.protectedColumns?.join(", ")}`
                : "No aplicada"}</dd>
            </div>
          </dl>
          {preparationChanges.length > 0 && (
            <details className="delivery-result__changes">
              <summary>Ver cambios incluidos ({preparationChanges.length.toLocaleString()})</summary>
              <ol>{preparationChanges.map((change, index) => <li key={`${index}-${change}`}>{change}</li>)}</ol>
            </details>
          )}
          {(result.replacedControlCellCount ?? 0) > 0 && (
            <p className="notice notice--warning">
              {result.replacedControlCellCount === 1
                ? "1 celda tenía caracteres de control que Excel no admite; se cambiaron por «\uFFFD»."
                : `${result.replacedControlCellCount.toLocaleString()} celdas tenían caracteres de control que Excel no admite; se cambiaron por «\uFFFD».`}
            </p>
          )}
          {result.format === "Paquete Columnia" && (
            <p className="delivery-result__note">
              Incluye dataset.csv, dictionary.json, delivery-summary.md y manifest.json{recipeDraft ? "; recipe.json incluye la receta validada" : ""}, además del reporte de calidad cuando hay reglas aprobadas.
            </p>
          )}
          {contract.kind === "without_contract" && (
            <p className="notice notice--warning">Esta copia no incluye una validación de calidad.</p>
          )}
          {openOutputState === "opened" && (
            <p className="notice notice--success" role="status">Carpeta de exportación abierta.</p>
          )}
          {openOutputState === "error" && (
            <p className="notice notice--error" role="alert">No se pudo abrir la carpeta de exportación.</p>
          )}
          {powerBiState.kind === "opened" && (
            <p className="notice notice--success" role="status">Power BI Desktop se abre con esta copia.</p>
          )}
          {powerBiState.kind === "opened" && powerBiState.quotedLineBreaks && (
            <p className="notice notice--warning" role="note">
              Algunas celdas tienen saltos de línea. En Power BI, en el paso Origen, elige tener en cuenta los saltos entre comillas, o exporta a Excel.
            </p>
          )}
          {powerBiState.kind === "error" && (
            <p className="notice notice--error" role="alert">{powerBiState.message}</p>
          )}
        </section>
  );
}
