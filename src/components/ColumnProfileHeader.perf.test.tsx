import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ColumnProfile, DatasetColumn } from "../bridge";
import { ColumnProfileHeader } from "./ColumnProfileHeader";

const profile = (index: number): ColumnProfile => ({
  name: `c${index}`,
  nullCount: 2,
  completenessPercentage: 99.5,
  uniqueCount: 40,
  minimum: "1.5",
  maximum: "1234.25",
  histogram: null,
  invalidTypeCount: 3,
  untrimmedCount: 1200,
  encodingIssueCount: 0,
} as unknown as ColumnProfile);

const columns: DatasetColumn[] = Array.from({ length: 200 }, (_, i) => ({ name: `c${i}`, dataType: "Float64" }));
const profiles = columns.map((_, i) => profile(i));

function Panel() {
  return (
    <table><thead><tr>
      {columns.map((column, i) => <th key={column.name}><ColumnProfileHeader column={column} profile={profiles[i]} rowCount={1000} /></th>)}
    </tr></thead></table>
  );
}

describe("coste de formato de ColumnProfileHeader (AUD-042)", () => {
  afterEach(() => vi.restoreAllMocks());

  it("no construye ningún Intl.NumberFormat al renderizar 200 columnas una vez cargado", () => {
    const { unmount } = render(<Panel />);
    unmount();
    const spy = vi.spyOn(Intl, "NumberFormat");
    const view = render(<Panel />);
    view.rerender(<Panel />);
    expect(spy).not.toHaveBeenCalled();
  });
});
