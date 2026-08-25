import { describe, expect, it } from "vitest";

import {
  applyThemePreference,
  readThemePreference,
  THEME_STORAGE_KEY,
  writeThemePreference,
} from "./themeModel";

function storageWith(value?: string): Storage {
  const values = new Map<string, string>();
  if (value !== undefined) values.set(THEME_STORAGE_KEY, value);

  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, nextValue) => values.set(key, nextValue),
    removeItem: (key) => values.delete(key),
    clear: () => values.clear(),
    key: (index) => [...values.keys()][index] ?? null,
    get length() { return values.size; },
  };
}

describe("modelo de temas", () => {
  it("lee solo preferencias soportadas y vuelve a sistema ante valores inválidos", () => {
    expect(readThemePreference(storageWith("dark"))).toBe("dark");
    expect(readThemePreference(storageWith("light"))).toBe("light");
    expect(readThemePreference(storageWith("solarized"))).toBe("system");
    expect(readThemePreference(storageWith())).toBe("system");
  });

  it("persiste la preferencia sin depender del storage global", () => {
    const storage = storageWith();

    writeThemePreference("dark", storage);

    expect(readThemePreference(storage)).toBe("dark");
  });

  it("aplica el atributo de tema y el color scheme al documento", () => {
    const root = document.createElement("html");

    applyThemePreference("dark", root);
    expect(root.dataset.theme).toBe("dark");
    expect(root.style.colorScheme).toBe("dark");

    applyThemePreference("system", root);
    expect(root.dataset.theme).toBe("system");
    expect(root.style.colorScheme).toBe("light dark");
  });
});
