import { MissingValue } from "./MissingValue";

const MARKER = "·";

/**
 * A text cell that keeps leading and trailing spaces visible. HTML collapses
 * them, which hid exactly the problem the trim correction fixes.
 */
export function CellText({ value }: { value: string }) {
  const core = value.trim();
  if (core === value) return <>{value}</>;
  if (core === "") {
    return (
      <span className="cell-text--padded" title={`${value.length} espacios`}>
        <span className="whitespace-marker" aria-hidden="true">{MARKER.repeat(value.length)}</span>
        <span className="visually-hidden">{`${value.length} espacios`}</span>
      </span>
    );
  }
  const leading = value.length - value.trimStart().length;
  const trailing = value.length - value.trimEnd().length;
  return (
    <span className="cell-text--padded" title="Tiene espacios al inicio o al final">
      {leading > 0 && <span className="whitespace-marker" aria-hidden="true">{MARKER.repeat(leading)}</span>}
      {core}
      {trailing > 0 && <span className="whitespace-marker" aria-hidden="true">{MARKER.repeat(trailing)}</span>}
      <span className="visually-hidden"> (con espacios al inicio o al final)</span>
    </span>
  );
}

/** Renders a preview cell: missing values and padded text stay distinguishable. */
export function renderCellValue(value: string | null | undefined) {
  return value === null || value === undefined ? <MissingValue /> : <CellText value={value} />;
}
