import type { OperationProgress } from "../bridge";

interface OperationProgressViewProps {
  progress: OperationProgress;
  cancellation:
    | { kind: "available"; onCancel: () => void }
    | { kind: "requested" };
}

export function OperationProgressView({
  progress,
  cancellation,
}: OperationProgressViewProps) {
  return (
    <div className="operation-progress" role="status" aria-live="polite" aria-atomic="true">
      <div>
        <span>{progress.stage}</span>
        <strong>{progress.percent}%</strong>
      </div>
      <progress
        aria-label={`Progreso: ${progress.stage}`}
        max={100}
        value={progress.percent}
      />
      <button
        type="button"
        onClick={cancellation.kind === "available" ? cancellation.onCancel : undefined}
        disabled={cancellation.kind === "requested"}
      >
        {cancellation.kind === "requested" ? "Cancelando…" : "Cancelar"}
      </button>
    </div>
  );
}
