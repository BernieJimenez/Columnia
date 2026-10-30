import type { ColumnProfile, DatasetColumn } from "../bridge";
import { isNumericType } from "../dataTypes";
import { formatDataType, formatNumber, formatPercent } from "../format";
import { plural } from "../plural";

/** The quality signals of a column that deserve a word in its header. */
export function columnProfileSignals(profile: ColumnProfile): string[] {
  const signals: string[] = [];
  if (profile.invalidTypeCount) signals.push(`${profile.invalidTypeCount.toLocaleString()} con otro tipo`);
  if (profile.untrimmedCount) signals.push(`${profile.untrimmedCount.toLocaleString()} con espacios`);
  if (profile.sentinelCount) signals.push(plural(profile.sentinelCount, "marcador de vacío", "marcadores de vacío"));
  if (profile.encodingIssueCount) signals.push(`${profile.encodingIssueCount.toLocaleString()} con acentos mal leídos`);
  return signals;
}

function rangeValue(value: string, numeric: boolean): string {
  if (!numeric) return value;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? formatNumber(parsed, 2) : value;
}

function MiniHistogram({ buckets }: { buckets: NonNullable<ColumnProfile["histogram"]> }) {
  const highest = Math.max(...buckets.map((bucket) => bucket.count), 1);
  const width = 100 / buckets.length;
  return (
    <svg className="col-profile__histogram" viewBox="0 0 100 20" preserveAspectRatio="none" aria-hidden="true">
      {buckets.map((bucket, index) => {
        const height = bucket.count === 0 ? 0 : Math.max((bucket.count / highest) * 20, 1.5);
        return <rect key={index} x={index * width + 0.5} y={20 - height} width={Math.max(width - 1, 0.5)} height={height} />;
      })}
    </svg>
  );
}

interface ColumnProfileHeaderProps {
  column: DatasetColumn;
  /** The profile of this column; without it the header shows name and type only. */
  profile?: ColumnProfile;
  rowCount: number;
}

/**
 * A table header that profiles its column at a glance: how full it is, how
 * many distinct values it has, its range and distribution, and any quality
 * signal. The figures come from the whole dataset, not from the visible page.
 */
export function ColumnProfileHeader({ column, profile, rowCount }: ColumnProfileHeaderProps) {
  const type = formatDataType(column.dataType);
  if (!profile) {
    return (
      <>
        <span>{column.name}</span>
        <small>{type}</small>
      </>
    );
  }

  const numeric = isNumericType(column.dataType);
  const filled = Math.min(Math.max(profile.completenessPercentage, 0), 100);
  const allDistinct = rowCount > 1 && profile.nullCount === 0 && profile.uniqueCount === rowCount;
  const distinct = allDistinct
    ? "todos distintos"
    : plural(profile.uniqueCount, "valor distinto", "valores distintos");
  const hasRange = profile.minimum !== null && profile.maximum !== null && profile.minimum !== profile.maximum;
  const signals = columnProfileSignals(profile);

  return (
    <>
      <span>{column.name}</span>
      <small>{type}</small>
      <span className="col-profile">
        <span className="col-profile__fill" aria-hidden="true">
          <span style={{ width: `${filled}%` }} />
        </span>
        <span className="col-profile__line">{filled >= 100 ? "sin vacíos" : `${formatPercent(filled)} con valor`} · {distinct}</span>
        {/* An identifier spreads evenly by construction: its histogram says nothing. */}
        {!allDistinct && profile.histogram && profile.histogram.length > 1 && <MiniHistogram buckets={profile.histogram} />}
        {hasRange && (
          <span className="col-profile__line col-profile__range">
            {rangeValue(profile.minimum ?? "", numeric)} – {rangeValue(profile.maximum ?? "", numeric)}
          </span>
        )}
        {signals.length > 0 && <span className="col-profile__signal">{signals.join(" · ")}</span>}
      </span>
    </>
  );
}
