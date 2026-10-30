import type { DatasetProfile } from "../../bridge";
import type { ProposalItem, ProposalItemId } from "../prepare/proposalModel";
import { formatPercent } from "../../format";

/**
 * What a problem does to an analysis, from most to least serious. It is a
 * consequence the person can check, not an arbitrary score:
 * - results: counts, sums and averages come out wrong;
 * - matching: equal values stop matching in filters, groups and joins;
 * - format: values are right but stored as text;
 * - optional: a fix that invents values, offered but never chosen for you.
 */
type ProposalImpactLevel = "results" | "matching" | "format" | "optional";

interface ProposalImpact {
  level: ProposalImpactLevel;
  label: string;
  /** How much of the dataset it touches, when the profile says so. */
  detail: string | null;
}

const LEVEL_BY_ITEM: Record<ProposalItemId, ProposalImpactLevel> = {
  duplicates: "results",
  sentinels: "results",
  trim: "matching",
  types: "format",
  dates: "format",
  impute: "optional",
};

const LEVEL_ORDER: ProposalImpactLevel[] = ["results", "matching", "format", "optional"];

export function proposalImpact(item: ProposalItem, profile: DatasetProfile): ProposalImpact {
  const level = LEVEL_BY_ITEM[item.id];
  switch (item.id) {
    case "duplicates":
      return {
        level,
        label: "Altera conteos y sumas",
        detail: profile.duplicatePercentage > 0 ? `${formatPercent(profile.duplicatePercentage)} de las filas` : null,
      };
    case "sentinels":
      return { level, label: "Altera conteos y promedios: cuentan como valores reales", detail: null };
    case "trim":
      return { level, label: "Afecta a filtros, agrupaciones y uniones", detail: null };
    case "types":
    case "dates":
      return { level, label: "Mejora el formato; no cambia ningún valor", detail: null };
    case "impute":
      return { level, label: "Opcional: rellena huecos con valores estimados", detail: null };
  }
}

/** The items from most to least serious; equal ones keep the proposal order. */
export function byImpact<T extends ProposalItem>(items: T[]): T[] {
  return items
    .map((item, index) => ({ item, index, rank: LEVEL_ORDER.indexOf(LEVEL_BY_ITEM[item.id]) }))
    .sort((left, right) => left.rank - right.rank || left.index - right.index)
    .map(({ item }) => item);
}
