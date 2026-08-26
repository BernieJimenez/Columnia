import type { DatasetPreview } from "../../bridge";

export function formatFileSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return unitIndex === 0
    ? `${value} ${units[unitIndex]}`
    : `${value.toFixed(1)} ${units[unitIndex]}`;
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
