import { formatPercent } from "../../format";
import { plural } from "../../plural";

interface QualitySnapshotProps {
  rowCount: number;
  columnCount: number;
  nullCount: number;
  /** «sin dato» markers the figure counts as values; the profile does not treat them as nulls. */
  markerCount?: number;
}

function clampPercentage(value: number): number {
  return Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0));
}

/**
 * The one figure the diagnosis list does not give: how complete the dataset is
 * overall. Plain text with the figure, not a progress bar (nothing is in
 * progress); nulls, duplicates and incompatible values are listed above.
 */
export function QualitySnapshot({ rowCount, columnCount, nullCount, markerCount = 0 }: QualitySnapshotProps) {
  const totalCells = Math.max(0, rowCount * columnCount);
  const nullPercentage = totalCells === 0 ? 0 : clampPercentage((nullCount / totalCells) * 100);
  const completeness = clampPercentage(100 - nullPercentage);

  return (
    <p className="quality-snapshot">
      <span>
        {formatPercent(completeness, 1)} de las celdas tienen valor
        {totalCells > 0 ? ` (${plural(nullCount, "nulo", "nulos")} en ${plural(totalCells, "celda", "celdas")})` : ""}
        {markerCount > 0
          ? `; incluye ${plural(markerCount, "marcador «sin dato»", "marcadores «sin dato»")} que cuentan como valor`
          : ""}
      </span>
    </p>
  );
}
