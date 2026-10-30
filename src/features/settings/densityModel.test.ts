import { describe, expect, it } from "vitest";

import { applyDensityPreference, DENSITY_STORAGE_KEY, readDensityPreference, writeDensityPreference } from "./densityModel";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() { return values.size; },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => { values.delete(key); },
    setItem: (key, value) => { values.set(key, value); },
  };
}

describe("densityModel", () => {
  it("usa la densidad normal salvo que se haya guardado «compacta»", () => {
    const storage = memoryStorage();
    expect(readDensityPreference(storage)).toBe("comfortable");
    storage.setItem(DENSITY_STORAGE_KEY, "otra");
    expect(readDensityPreference(storage)).toBe("comfortable");
    writeDensityPreference("compact", storage);
    expect(readDensityPreference(storage)).toBe("compact");
  });

  it("no rompe la interfaz si el almacenamiento falla", () => {
    const broken = { getItem: () => { throw new Error("bloqueado"); }, setItem: () => { throw new Error("bloqueado"); } } as unknown as Storage;
    expect(readDensityPreference(broken)).toBe("comfortable");
    expect(() => writeDensityPreference("compact", broken)).not.toThrow();
  });

  it("marca la raíz con la densidad elegida", () => {
    const root = document.createElement("html");
    applyDensityPreference("compact", root);
    expect(root.dataset.density).toBe("compact");
  });
});
