interface QualitySnapshotProps {
  rowCount: number;
  columnCount: number;
  nullCount: number;
  duplicateCount: number;
  duplicatePercentage: number;
  invalidTypeCount: number;
}

function clampPercentage(value: number): number {
  return Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0));
}

export function QualitySnapshot({
  rowCount,
  columnCount,
  nullCount,
  duplicateCount,
  duplicatePercentage,
  invalidTypeCount,
}: QualitySnapshotProps) {
  const totalCells = Math.max(0, rowCount * columnCount);
  const nullPercentage = totalCells === 0 ? 0 : clampPercentage((nullCount / totalCells) * 100);
  const invalidTypePercentage = totalCells === 0
    ? 0
    : clampPercentage((invalidTypeCount / totalCells) * 100);
  const completeness = clampPercentage(100 - nullPercentage);
  const signals = [
    { label: "Celdas sin valor", count: nullCount, percentage: nullPercentage, tone: "warning" },
    { label: "Filas duplicadas", count: duplicateCount, percentage: clampPercentage(duplicatePercentage), tone: "danger" },
    { label: "Valores incompatibles", count: invalidTypeCount, percentage: invalidTypePercentage, tone: "info" },
  ] as const;

  return (
    <section className="quality-snapshot" aria-label="Vista rápida de calidad">
      <div className="quality-snapshot__completeness">
        <div>
          <strong>Completitud global</strong>
          <span>{completeness.toFixed(1)}%</span>
        </div>
        <div
          className="quality-snapshot__track"
          role="progressbar"
          aria-label="Completitud global"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Number(completeness.toFixed(1))}
        >
          <span style={{ width: `${completeness}%` }} />
        </div>
      </div>
      <ul className="quality-snapshot__signals" aria-label="Señales de calidad">
        {signals.map((signal) => (
          <li key={signal.label}>
            <span className={`quality-snapshot__dot quality-snapshot__dot--${signal.tone}`} aria-hidden="true" />
            <span>{signal.label}</span>
            <strong>{signal.count.toLocaleString()} · {signal.percentage.toFixed(1)}%</strong>
          </li>
        ))}
      </ul>
    </section>
  );
}
