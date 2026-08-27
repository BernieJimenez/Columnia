import { describe, expect, it } from "vitest";

import {
  hasPerformanceProfilePreference,
  isPerformanceProfile,
  PERFORMANCE_STORAGE_KEY,
  readPerformanceProfile,
  writePerformanceProfile,
} from "./performanceModel";

function storageWith(value?: string): Storage {
  const values = new Map<string, string>();
  if (value !== undefined) values.set(PERFORMANCE_STORAGE_KEY, value);

  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, nextValue) => values.set(key, nextValue),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => [...values.keys()][index] ?? null,
    get length() { return values.size; },
  };
}

describe("modelo de rendimiento", () => {
  it("acepta solo perfiles soportados y conserva equilibrado como valor seguro", () => {
    expect(isPerformanceProfile("maximum")).toBe(true);
    expect(isPerformanceProfile("turbo")).toBe(false);
    expect(readPerformanceProfile(storageWith("maximum"))).toBe("maximum");
    expect(readPerformanceProfile(storageWith("turbo"))).toBe("balanced");
    expect(readPerformanceProfile(storageWith())).toBe("balanced");
  });

  it("persiste el perfil sin depender del storage global", () => {
    const storage = storageWith();

    writePerformanceProfile("conservative", storage);

    expect(readPerformanceProfile(storage)).toBe("conservative");
    expect(hasPerformanceProfilePreference(storage)).toBe(true);
    expect(hasPerformanceProfilePreference(storageWith())).toBe(false);
  });
});
