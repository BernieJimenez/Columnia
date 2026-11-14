import { describe, expect, it } from "vitest";

import type { ColumnProfile, DatasetProfile } from "../../bridge";
import { columnFixScopes } from "./columnFixes";

function column(overrides: Partial<ColumnProfile> & { name: string }): ColumnProfile {
  return {
    dataType: "String", nullCount: 0, completenessPercentage: 100, uniqueCount: 10, minimum: null, maximum: null,
    mean: null, emptyCount: 0, minimumLength: 1, maximumLength: 10, averageLength: 5, suggestedType: null,
    typeMatchPercentage: null, invalidTypeCount: null, sentinelCount: 0, encodingIssueCount: 0, privacySignal: null,
    standardDeviation: null, firstQuartile: null, median: null, thirdQuartile: null, outlierCount: null, histogram: null,
    dateOrder: null, dateHasTime: null, untrimmedCount: 0, ...overrides,
  };
}

describe("columnFixScopes", () => {
  it("«Desconocido» no toca números, fechas ni sí/no guardados como texto, que se ofrecen para convertir", () => {
    const profile: DatasetProfile = {
      rowCount: 100, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0,
      columns: [
        column({ name: "cliente", nullCount: 5, completenessPercentage: 95 }),
        column({ name: "importe", nullCount: 5, completenessPercentage: 95, suggestedType: "decimal", typeMatchPercentage: 97 }),
        column({ name: "alta", nullCount: 5, completenessPercentage: 95, suggestedType: "date", typeMatchPercentage: 100 }),
        column({ name: "activo", nullCount: 5, completenessPercentage: 95, suggestedType: "boolean", typeMatchPercentage: 100 }),
      ],
    };
    expect(columnFixScopes(profile).fillUnknown.map((item) => item.name)).toEqual(["cliente"]);
  });
});
