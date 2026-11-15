import { useEffect, useEffectEvent, useRef, useState } from "react";

import { openLastExport, openLastExportInPowerBi } from "../../bridge";
import type { SavedRecipe } from "../../bridge";
import { formatFileSize } from "./DatasetMetrics";
import type { DeliveryContractState, DeliveryExportState } from "./deliveryContract";
import { bundleFileNames, listFileNames } from "./deliveryModel";
import { plural } from "../../plural";
import { errorMessage } from "../../bridge/errors";

type ExportResult = Extract<DeliveryExportState, { kind: "success" }>["result"];

interface DeliveryResultProps {
  result: ExportResult;
  /** «Exportar y abrir en Power BI» was used for this export. */
  autoOpenPowerBi: boolean;
  contract: DeliveryContractState;
  recipeDraft: SavedRecipe | null;
  preparationChanges: string[];
  approvedQualitySummary: string | null;
  /**
   * False once the destination or format changed after this export: the
   * result stays as the previous one, without taking focus or the primary action.
   */
  current?: boolean;
  /** The export was asked for just now (not a result found when the stage mounted). */
  takeFocus?: boolean;
  /** A database delivery: «servidor · base · esquema.tabla», as chosen when it was sent. */
  databaseLocation?: string | null;
  /** The table policy the database write was asked with: the result says what happened to the table. */
  databasePolicy?: "create_only" | "append" | "replace" | null;
  /** With «Reemplazar», the rows the analysis found in the table and the write deleted, when known. */
  replacedRowCount?: number | null;
}

function isDatabaseResult(result: ExportResult): boolean {
  return result.format === "PostgreSQL" || result.format === "MySQL" || result.format === "SQL Server";
}

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * «Copia lista»: where the copy is, its rows, its quality and its protection,
 * with «Abrir carpeta» and «Abrir en Power BI». On arrival it takes the focus
 * and scrolls into view; its heading is described by one sentence, so a
 * screen reader hears that sentence once instead of a live region.
 */
export function DeliveryResult({
  result,
  autoOpenPowerBi,
  contract,
  recipeDraft,
  preparationChanges,
  approvedQualitySummary,
  current = true,
  takeFocus = true,
  databaseLocation = null,
  databasePolicy = null,
  replacedRowCount = null,
}: DeliveryResultProps) {
  const [openOutputState, setOpenOutputState] = useState<"idle" | "working" | "opened" | "error">("idle");
  const [powerBiState, setPowerBiState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "opened"; quotedLineBreaks: boolean }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const headingRef = useRef<HTMLHeadingElement>(null);
  const database = isDatabaseResult(result);

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
      setPowerBiState({ kind: "error", message: errorMessage(error) });
    }
  }

  const openPowerBiOnce = useEffectEvent(() => {
    void openInPowerBi();
  });
  useEffect(() => {
    if (autoOpenPowerBi) openPowerBiOnce();
  }, [autoOpenPowerBi]);

  // A new result takes the focus and comes into view (WCAG 2.4.3).
  const announce = useEffectEvent((arrived: ExportResult) => {
    if (!current || !takeFocus || arrived !== result) return;
    const heading = headingRef.current;
    if (!heading) return;
    heading.focus({ preventScroll: true });
    heading.scrollIntoView?.({ block: "start", behavior: prefersReducedMotion() ? "auto" : "smooth" });
  });
  useEffect(() => {
    announce(result);
  }, [result]);

  const rows = result.rowCount != null ? plural(result.rowCount, "fila", "filas") : null;
  const title = !current
    ? database ? "Escritura anterior" : "Copia anterior"
    : database ? "Tabla escrita" : "Copia lista";
  const where = database
    ? databaseLocation ?? result.fileName
    : result.folderName != null ? `${result.fileName} en la carpeta ${result.folderName}` : result.fileName;
  const tableOutcome = databasePolicy === "replace"
    ? ` Se reemplazó la tabla anterior.${replacedRowCount === null || replacedRowCount === 0
      ? ""
      : replacedRowCount === 1
        ? " Se borró la única fila anterior."
        : ` Se borraron las ${plural(replacedRowCount, "fila", "filas")} anteriores.`}`
    : databasePolicy === "append"
      ? ` Se ${result.rowCount === 1 ? "añadió 1 fila" : rows ? `añadieron ${rows}` : "añadieron las filas"} a la tabla existente.`
      : databasePolicy === "create_only" ? " Se creó la tabla." : "";
  const sentence = !current
    ? "Se creó antes de cambiar el destino o el formato; la entrega actual aún no se ha hecho."
    : database
      ? `${result.rowCount === 1 ? "La fila quedó" : rows ? `Las ${rows} quedaron` : "Los datos quedaron"} en ${where}.${tableOutcome}`
      : `Se guardó ${where}. El dataset preparado sigue disponible en Columnia.`;
  const protectedCells = result.formulaProtectedCellCount ?? 0;
  const actionClass = current ? "primary-action" : "secondary-action";

  return (
    <section
      className="delivery-result"
      data-current={current || undefined}
      aria-labelledby="delivery-result-title"
    >
      <div className="delivery-result__heading">
        <div>
          <h3 id="delivery-result-title" ref={headingRef} tabIndex={-1} aria-describedby="delivery-result-sentence">
            {title}
          </h3>
          <p id="delivery-result-sentence">{sentence}</p>
        </div>
        {!database && (
          <div className="delivery-result__actions">
            <button
              type="button"
              className={actionClass}
              onClick={() => void revealLastExport()}
              disabled={openOutputState === "working"}
            >
              {openOutputState === "working" ? "Abriendo carpeta…" : "Abrir carpeta"}
            </button>
            {(result.format === "CSV" || result.format === "Excel" || result.format === "Parquet") && (
              <button
                type="button"
                className="secondary-action"
                onClick={() => void openInPowerBi()}
                disabled={powerBiState.kind === "working"}
              >
                {/* UX-09: distinct from «Exportar y abrir en Power BI». */}
                {powerBiState.kind === "working" ? "Abriendo Power BI…" : "Abrir esta copia en Power BI"}
              </button>
            )}
          </div>
        )}
      </div>
      <dl className="delivery-result__facts">
        <div>
          <dt>{database ? "Destino" : "Archivo"}</dt>
          <dd>{database ? where : `${where} · ${result.format} · ${formatFileSize(result.fileSizeBytes)}`}</dd>
        </div>
        {rows && (
          <div>
            <dt>Filas</dt>
            <dd>
              {rows}
              {protectedCells > 0 && (protectedCells === 1
                ? " · 1 celda empezaba como una fórmula y se guardó como texto"
                : ` · ${protectedCells.toLocaleString("es-ES")} celdas empezaban como una fórmula y se guardaron como texto`)}
            </dd>
          </div>
        )}
        <div>
          <dt>Calidad</dt>
          <dd>{approvedQualitySummary ?? "sin validar: salida confirmada sin reglas de calidad"}</dd>
        </div>
        <div>
          <dt>Protección</dt>
          <dd>{result.protectedColumnCount > 0
            ? `${plural(result.protectedColumnCount, "columna", "columnas")}: ${result.protectedColumns?.join(", ")}`
            : "no aplicada"}</dd>
        </div>
      </dl>
      {preparationChanges.length > 0 && (
        <details className="delivery-result__changes">
          <summary>Ver los cambios de Preparar incluidos ({preparationChanges.length.toLocaleString("es-ES")})</summary>
          <ol>{preparationChanges.map((change, index) => <li key={`${index}-${change}`}>{change}</li>)}</ol>
        </details>
      )}
      {(result.replacedControlCellCount ?? 0) > 0 && (
        <p className="notice notice--warning">
          {result.replacedControlCellCount === 1
            ? "1 celda tenía caracteres de control que Excel no admite; se cambiaron por «�»."
            : `${result.replacedControlCellCount.toLocaleString("es-ES")} celdas tenían caracteres de control que Excel no admite; se cambiaron por «�».`}
        </p>
      )}
      {result.format === "Paquete Columnia" && (
        <p className="delivery-result__note">
          Incluye {listFileNames(bundleFileNames({ recipe: recipeDraft !== null, quality: contract.kind === "with_contract" }))}.
        </p>
      )}
      {openOutputState === "opened" && (
        <p className="notice notice--success" role="status">Carpeta de exportación abierta.</p>
      )}
      {openOutputState === "error" && (
        <p className="notice notice--error" role="alert">No se pudo abrir la carpeta de exportación.</p>
      )}
      {powerBiState.kind === "opened" && (
        <p className="notice notice--success" role="status">
          Power BI Desktop se abre con esta copia. Para abrirla se guardó a su lado un archivo de conexión .pbids con el mismo nombre; puedes borrarlo después.
        </p>
      )}
      {powerBiState.kind === "opened" && powerBiState.quotedLineBreaks && (
        <p className="notice notice--warning" role="note">
          Algunas celdas tienen saltos de línea. En Power BI, en el paso Origen, elige tener en cuenta los saltos entre comillas, o exporta a Excel o Parquet.
        </p>
      )}
      {powerBiState.kind === "error" && (
        <p className="notice notice--error" role="alert">{powerBiState.message}</p>
      )}
    </section>
  );
}
