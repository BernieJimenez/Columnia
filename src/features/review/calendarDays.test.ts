import { describe, expect, it } from "vitest";

import { contiguousCalendarDays } from "./ReviewCharts";

describe("contiguousCalendarDays (FUN-52)", () => {
  it("rellena los días ausentes entre el primero y el último", () => {
    const days = contiguousCalendarDays([
      { period: "2024-01-01", rowCount: 3, percentage: 60 },
      { period: "2024-01-10", rowCount: 2, percentage: 40 },
    ]);
    expect(days).toHaveLength(10);
    expect(days[0]).toMatchObject({ period: "2024-01-01", rowCount: 3 });
    expect(days[4]).toEqual({ period: "2024-01-05", rowCount: 0, percentage: 0 });
    expect(days[9]).toMatchObject({ period: "2024-01-10", rowCount: 2 });
  });

  it("deja al final un periodo que no es un día y no rellena rangos enormes", () => {
    const days = contiguousCalendarDays([
      { period: "2024-01-01", rowCount: 1, percentage: 50 },
      { period: "2024-01-02", rowCount: 1, percentage: 50 },
      { period: "Otros días", rowCount: 5, percentage: 0 },
    ]);
    expect(days.map((day) => day.period)).toEqual(["2024-01-01", "2024-01-02", "Otros días"]);
    expect(contiguousCalendarDays([
      { period: "2000-01-01", rowCount: 1, percentage: 50 },
      { period: "2024-01-01", rowCount: 1, percentage: 50 },
    ])).toHaveLength(2);
  });
});
