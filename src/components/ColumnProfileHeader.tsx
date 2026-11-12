import { memo } from "react";
import type { ColumnProfile, DatasetColumn } from "../bridge";
import { isIntegerType, isNumericType } from "../dataTypes";
import { formatDataType, formatNumber, formatPercent } from "../format";
import { plural } from "../plural";

/** The quality signals of a column that deserve a word in its header. */
export function columnProfileSignals(profile: ColumnProfile): string[] {
  const signals: string[] = [];
  if (profile.invalidTypeCount) signals.push(`${formatNumber(profile.invalidTypeCount)} con otro tipo`);
  if (profile.untrimmedCount) signals.push(`${formatNumber(profile.untrimmedCount)} con espacios`);
  if (profile.sentinelCount) signals.push(plural(profile.sentinelCount, "marcador de vacío", "marcadores de vacío"));
  if (profile.encodingIssueCount) signals.push(`${formatNumber(profile.encodingIssueCount)} con acentos mal leídos`);
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

/**
 * The spoken shape of a histogram: the span it covers and its fullest bin.
 * It lives in the profile block, so the header's description carries it.
 */
export function histogramDescription(buckets: NonNullable<ColumnProfile["histogram"]>): string | null {
  const first = buckets[0];
  const last = buckets[buckets.length - 1];
  if (!first || !last) return null;
  const populated = buckets.reduce((best, bucket) => (bucket.count > best.count ? bucket : best), first);
  if (populated.count <= 0) return null;
  const edge = (value: number) => formatNumber(value, 2);
  return `Distribución: de ${edge(first.lower)} a ${edge(last.upper)}, la mayoría entre ${edge(populated.lower)} y ${edge(populated.upper)}.`;
}

interface ColumnProfileHeaderProps {
  column: DatasetColumn;
  /** Id for the profile block, so the `<th>` can point `aria-describedby` at it. */
  profileId?: string;
  /** The profile of this column; without it the header shows name and type only. */
  profile?: ColumnProfile;
  /** The profile is still on its way: reserve the block's height so the table does not jump when it lands. */
  pending?: boolean;
  rowCount: number;
}

/**
 * A table header that profiles its column at a glance: how full it is, how
 * many distinct values it has, its range and distribution, and any quality
 * signal. The figures come from the whole dataset, not from the visible page.
 */
export const ColumnProfileHeader = memo(function ColumnProfileHeader({ column, profile, profileId, pending = false, rowCount }: ColumnProfileHeaderProps) {
  const type = formatDataType(column.dataType);
  if (!profile) {
    return (
      <>
        <span>{column.name}</span>
        <small>{type}</small>
        {pending && (
          // Decorative: same footprint as the final block, no text and no id, so nothing points at it.
          <span className="col-profile col-profile--pending" aria-hidden="true">
            <span className="col-profile__fill" />
          </span>
        )}
      </>
    );
  }

  const numeric = isNumericType(column.dataType);
  const filled = Math.min(Math.max(profile.completenessPercentage, 0), 100);
  const allDistinct = rowCount > 1 && profile.nullCount === 0 && profile.uniqueCount === rowCount;
  const identifierLike = allDistinct && !(numeric && !isIntegerType(column.dataType));
  const distinct = allDistinct
    ? "todos distintos"
    : plural(profile.uniqueCount, "valor distinto", "valores distintos");
  const hasRange = profile.minimum !== null && profile.maximum !== null && profile.minimum !== profile.maximum;
  const signals = columnProfileSignals(profile);
  const showHistogram = !identifierLike && !!profile.histogram && profile.histogram.length > 1;
  const histogramText = showHistogram && profile.histogram ? histogramDescription(profile.histogram) : null;

  return (
    <>
      <span>{column.name}</span>
      <small>{type}</small>
      <span className="col-profile" id={profileId}>
        <span className="col-profile__fill" aria-hidden="true">
          <span style={{ width: `${filled}%` }} />
        </span>
        <span className="col-profile__line">{filled >= 100 ? "sin vacíos" : `${formatPercent(filled)} con valor`} · {distinct}</span>
        {/* An identifier spreads evenly by construction: its histogram says nothing.
            A continuous number (float or decimal, e.g. an amount) is all-distinct
            without being an identifier, so it keeps its histogram. */}
        {showHistogram && profile.histogram && <MiniHistogram buckets={profile.histogram} />}
        {histogramText && <span className="visually-hidden">{histogramText}</span>}
        {hasRange && (
          <span className="col-profile__line col-profile__range">
            {rangeValue(profile.minimum ?? "", numeric)} – {rangeValue(profile.maximum ?? "", numeric)}
          </span>
        )}
        {signals.length > 0 && <span className="col-profile__signal">{signals.join(" · ")}</span>}
      </span>
    </>
  );
});
