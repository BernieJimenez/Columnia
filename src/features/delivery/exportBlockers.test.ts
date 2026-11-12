import { describe, expect, it } from "vitest";
import { exportBlockers, type ExportBlockerFlags } from "./exportBlockers";

const KEYS: (keyof ExportBlockerFlags)[] = [
  "busy",
  "validationError",
  "missingSqlServerKey",
  "needsPersonalDataConfirmation",
  "needsUnvalidatedConfirmation",
  "databaseNotReady",
  "exceedsExcel",
];

function flagsFor(mask: number): ExportBlockerFlags {
  return Object.fromEntries(KEYS.map((key, index) => [key, (mask & (1 << index)) !== 0])) as unknown as ExportBlockerFlags;
}

describe("exportBlockers", () => {
  it("no da ninguna frase si no hay bloqueos", () => {
    expect(exportBlockers(flagsFor(0))).toEqual([]);
  });

  it("da una frase propia, no vacía y distinta por cada condición", () => {
    const phrases = KEYS.map((_, index) => exportBlockers(flagsFor(1 << index)));
    for (const list of phrases) {
      expect(list).toHaveLength(1);
      expect(list[0].trim()).not.toBe("");
    }
    expect(new Set(phrases.map((list) => list[0])).size).toBe(KEYS.length);
  });

  it("en todas las combinaciones hay tantas frases como condiciones activas", () => {
    for (let mask = 0; mask < 1 << KEYS.length; mask += 1) {
      const active = KEYS.filter((_, index) => (mask & (1 << index)) !== 0);
      expect(exportBlockers(flagsFor(mask))).toHaveLength(active.length);
    }
  });
});
