import { describe, expect, it } from "vitest";

import type { DatasetProfile } from "../../bridge";
import type { ProposalItem, ProposalItemId } from "../prepare/proposalModel";
import { byImpact, proposalImpact } from "./proposalImpact";

const item = (id: ProposalItemId): ProposalItem => ({ id, title: id, hint: "", examples: [] });
const profile = { rowCount: 1000, duplicateRowCount: 4, nearDuplicateRowCount: 0, duplicatePercentage: 0.4, columns: [] } satisfies DatasetProfile;

describe("proposalImpact", () => {
  it("pone primero lo que altera los resultados y al final lo opcional", () => {
    const ordered = byImpact(["types", "impute", "trim", "dates", "duplicates", "sentinels"].map((id) => item(id as ProposalItemId)));
    expect(ordered.map(({ id }) => id)).toEqual(["duplicates", "sentinels", "trim", "types", "dates", "impute"]);
  });

  it("explica cada consecuencia con texto y dice cuánto toca cuando se sabe", () => {
    expect(proposalImpact(item("duplicates"), profile)).toEqual({
      level: "results",
      label: "Altera conteos y sumas",
      detail: expect.stringMatching(/^0[,.]4\s?% de las filas$/),
    });
    expect(proposalImpact(item("trim"), profile)).toMatchObject({ level: "matching", detail: null });
    expect(proposalImpact(item("types"), profile).label).toBe("Mejora el formato; no cambia ningún valor");
    expect(proposalImpact(item("impute"), profile).level).toBe("optional");
  });
});
