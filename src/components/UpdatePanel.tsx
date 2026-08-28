import { useRef, useState } from "react";

import {
  cancelUpdateDownload,
  checkForUpdate,
  downloadUpdate,
  installUpdate,
  type UpdateInfo,
  type UpdaterProgress,
} from "../bridge";

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "none" }
  | { kind: "available"; info: UpdateInfo }
  | { kind: "downloading"; info: UpdateInfo; downloadedBytes: number; contentLength: number | null; cancelRequested: boolean }
  | { kind: "ready"; info: UpdateInfo; downloadedBytes: number; contentLength: number | null }
  | { kind: "installing"; info: UpdateInfo }
  | { kind: "installed"; info: UpdateInfo }
  | { kind: "error"; message: string };

interface UpdatePanelProps {
  enabled: boolean;
  currentVersion: string | null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatBytes(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "tamaño no informado";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unitIndex]}`;
}

function updateProgress(current: UpdateState, progress: UpdaterProgress): UpdateState {
  if (current.kind !== "downloading") return current;
  return {
    ...current,
    downloadedBytes: progress.downloadedBytes,
    contentLength: progress.contentLength ?? current.contentLength,
  };
}

export function UpdatePanel({ enabled, currentVersion }: UpdatePanelProps) {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const updateRef = useRef<UpdateInfo | null>(null);
  const busy = state.kind === "checking" || state.kind === "downloading" || state.kind === "installing";

  const handleCheck = async () => {
    if (!enabled || busy) return;
    setState({ kind: "checking" });
    updateRef.current = null;
    try {
      const info = await checkForUpdate();
      if (info === null) {
        setState({ kind: "none" });
        return;
      }
      updateRef.current = info;
      setState({ kind: "available", info });
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  };

  const handleDownload = async () => {
    const info = updateRef.current;
    if (!enabled || !info || busy) return;
    let lastProgress: UpdaterProgress | null = null;
    setState({
      kind: "downloading",
      info,
      downloadedBytes: 0,
      contentLength: info.sizeBytes,
      cancelRequested: false,
    });
    try {
      await downloadUpdate((progress) => {
        lastProgress = progress;
        setState((current) => updateProgress(current, progress));
      });
      const finalProgress = lastProgress as UpdaterProgress | null;
      setState({
        kind: "ready",
        info,
        downloadedBytes: finalProgress?.downloadedBytes ?? info.sizeBytes ?? 0,
        contentLength: finalProgress?.contentLength ?? info.sizeBytes,
      });
    } catch (error) {
      const message = errorMessage(error);
      setState(message.toLowerCase().includes("cancel")
        ? { kind: "idle" }
        : { kind: "error", message });
    }
  };

  const handleCancel = () => {
    if (state.kind !== "downloading" || state.cancelRequested) return;
    setState({ ...state, cancelRequested: true });
    void cancelUpdateDownload().catch((error) => {
      setState({ kind: "error", message: errorMessage(error) });
    });
  };

  const handleInstall = async () => {
    if (state.kind !== "ready" || busy) return;
    const info = state.info;
    setState({ kind: "installing", info });
    try {
      await installUpdate();
      setState({ kind: "installed", info });
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  };

  const downloadPercent = (state.kind === "downloading" || state.kind === "ready") && state.contentLength && state.contentLength > 0
    ? Math.min(100, (state.downloadedBytes / state.contentLength) * 100)
    : null;

  return (
    <section className="update-panel" aria-labelledby="update-panel-heading">
      <div className="update-panel__heading">
        <h2 id="update-panel-heading">Actualizaciones</h2>
        <p>Solo se comprueban después de una acción explícita y se validan con firma.</p>
      </div>
      {!enabled && (
        <p className="update-panel__status" role="status">
          El updater no está configurado para esta compilación.
        </p>
      )}
      {enabled && currentVersion && (
        <p className="update-panel__version">Versión instalada: {currentVersion}</p>
      )}
      <div className="update-panel__actions">
        <button type="button" disabled={!enabled || busy} onClick={() => void handleCheck()}>
          {state.kind === "checking" ? "Comprobando…" : "Buscar actualizaciones"}
        </button>
        {state.kind === "available" && (
          <button type="button" disabled={busy} onClick={() => void handleDownload()}>
            Descargar actualización
          </button>
        )}
        {state.kind === "downloading" && (
          <button type="button" disabled={state.cancelRequested} onClick={handleCancel}>
            {state.cancelRequested ? "Cancelando…" : "Cancelar descarga"}
          </button>
        )}
        {state.kind === "ready" && (
          <button type="button" disabled={busy} onClick={() => void handleInstall()}>
            Instalar y reiniciar
          </button>
        )}
      </div>
      {(state.kind === "available" || state.kind === "downloading" || state.kind === "ready" || state.kind === "installing" || state.kind === "installed") && (
        <div className="update-panel__details" aria-live="polite">
          <p><strong>Disponible: {state.info.version}</strong></p>
          <p>Tamaño: {formatBytes(state.info.sizeBytes)}</p>
          {state.info.notes && <p className="update-panel__notes">{state.info.notes}</p>}
        </div>
      )}
      {(state.kind === "downloading" || state.kind === "ready") && (
        <div className="update-panel__progress" aria-live="polite">
          {downloadPercent === null ? (
            <p>Descargados: {formatBytes(state.downloadedBytes)}</p>
          ) : (
            <>
              <progress max={100} value={downloadPercent} aria-label="Progreso de actualización" />
              <p>{Math.round(downloadPercent)}% · {formatBytes(state.downloadedBytes)} de {formatBytes(state.contentLength)}</p>
            </>
          )}
        </div>
      )}
      {state.kind === "none" && <p className="update-panel__status" role="status">No hay actualizaciones disponibles.</p>}
      {state.kind === "installed" && <p className="update-panel__status" role="status">La instalación se inició; la aplicación se reiniciará según la plataforma.</p>}
      {state.kind === "error" && <p className="update-panel__status update-panel__status--error" role="alert">{state.message}</p>}
    </section>
  );
}
