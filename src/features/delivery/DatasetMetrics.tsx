import type { DatasetPreview } from "../../bridge";
import { formatBytes } from "../../format";

export function formatFileSize(bytes: number): string {
  return formatBytes(bytes);
}

export function DatasetMetrics({ dataset }: { dataset: DatasetPreview }) {
  return (
    <dl className="metrics" aria-label="Resumen del dataset">
      <div><dt>Filas</dt><dd>{dataset.rowCount.toLocaleString()}</dd></div>
      <div><dt>Columnas</dt><dd>{dataset.columnCount.toLocaleString()}</dd></div>
      <div><dt>Tamaño</dt><dd>{formatFileSize(dataset.fileSizeBytes)}</dd></div>
    </dl>
  );
}
