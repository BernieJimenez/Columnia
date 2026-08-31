import { useEffect, useState } from "react";

import type { OperationProgress } from "../bridge";

interface OperationProgressViewProps {
  progress: OperationProgress;
  cancellation:
    | { kind: "available"; onCancel: () => void }
    | { kind: "requested" };
}

const OPERATION_COPY: Record<
  OperationProgress["operation"],
  { eyebrow: string; title: string; description: string }
> = {
  load: {
    eyebrow: "Importación",
    title: "Cargando dataset",
    description: "Estamos preparando tus datos para que puedas revisarlos.",
  },
  profile: {
    eyebrow: "Diagnóstico",
    title: "Analizando calidad",
    description: "Estamos comprobando estructura, valores y consistencia.",
  },
  export: {
    eyebrow: "Entrega",
    title: "Exportando dataset",
    description: "Estamos escribiendo una copia validada en el formato elegido.",
  },
};

function formatElapsed(seconds: number) {
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes.toString().padStart(2, "0")}:${remainingSeconds.toString().padStart(2, "0")}`;
}

export function OperationProgressView({
  progress,
  cancellation,
}: OperationProgressViewProps) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const copy = OPERATION_COPY[progress.operation];
  const percent = Math.min(100, Math.max(0, progress.percent));
  const isCancelling = cancellation.kind === "requested";
  const titleId = `operation-progress-title-${progress.operation}`;
  const descriptionId = `operation-progress-description-${progress.operation}`;

  useEffect(() => {
    setElapsedSeconds(0);
    if (isCancelling) return undefined;
    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      setElapsedSeconds(Math.max(0, Math.floor((Date.now() - startedAt) / 1000)));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [progress.operation, isCancelling]);

  return (
    <section
      className={`operation-progress${isCancelling ? " operation-progress--cancelling" : ""}`}
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-busy={!isCancelling}
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
    >
      <header className="operation-progress__header">
        <div className="operation-progress__title-block">
          <span className="operation-progress__eyebrow">{copy.eyebrow}</span>
          <h3 id={titleId}>{copy.title}</h3>
        </div>
        <span className="operation-progress__state">
          <span className="operation-progress__state-dot" aria-hidden="true" />
          {isCancelling ? "Cancelación solicitada" : "En curso"}
        </span>
      </header>

      <p id={descriptionId} className="operation-progress__description">
        {copy.description}
      </p>

      <div className="operation-progress__current">
        <div className="operation-progress__stage">
          <span>Etapa actual</span>
          <strong>{progress.stage}</strong>
        </div>
        <strong className="operation-progress__percent">{percent}%</strong>
      </div>

      <progress
        aria-label={`Progreso: ${progress.stage}`}
        max={100}
        value={percent}
      />

      <div className="operation-progress__scale" aria-hidden="true">
        <span>Inicio</span>
        <span>Completado</span>
      </div>

      <footer className="operation-progress__footer">
        <p>
          {isCancelling
            ? "Terminando la operación actual…"
            : "El avance se actualiza automáticamente. Los datasets grandes pueden tardar varios minutos."}
        </p>
        <span className="operation-progress__elapsed" aria-label={`Tiempo transcurrido: ${formatElapsed(elapsedSeconds)}`}>
          {formatElapsed(elapsedSeconds)}
        </span>
        <button
          type="button"
          onClick={cancellation.kind === "available" ? cancellation.onCancel : undefined}
          disabled={isCancelling}
        >
          {isCancelling ? "Cancelando…" : "Cancelar"}
        </button>
      </footer>
    </section>
  );
}
