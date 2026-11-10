import { plural } from "../plural";
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
      <span className="cell-text--padded" title={plural(value.length, "espacio", "espacios")}>
        <span className="whitespace-marker" aria-hidden="true">{MARKER.repeat(value.length)}</span>
        <span className="visually-hidden">{plural(value.length, "espacio", "espacios")}</span>
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

/** Longer texts are clipped in the grid; the full value is in `title` and a focus expands it. */
const LONG_CELL_THRESHOLD = 36;

/** Renders a preview cell: missing values and padded text stay distinguishable. */
export function renderCellValue(value: string | null | undefined) {
  if (value === null || value === undefined) return <MissingValue />;
  if (value.length <= LONG_CELL_THRESHOLD) return <CellText value={value} />;
  return (
    <span className="cell-clip" tabIndex={0} title={value}>
      <CellText value={value} />
    </span>
  );
}
