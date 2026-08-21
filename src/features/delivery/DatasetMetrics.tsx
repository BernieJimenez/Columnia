import type { DatasetPreview } from "../../bridge";

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
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
