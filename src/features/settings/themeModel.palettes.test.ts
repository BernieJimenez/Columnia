import { describe, expect, it } from "vitest";

import { applyThemePreference, readThemePreference, THEME_STORAGE_KEY } from "./themeModel";

function storageWith(value: string): Storage {
  return {
    getItem: (key) => key === THEME_STORAGE_KEY ? value : null,
    setItem: () => undefined,
    removeItem: () => undefined,
    clear: () => undefined,
    key: () => null,
    length: 1,
  };
}

describe("paletas adicionales del modelo de temas", () => {
  it.each(["paper", "ocean", "slate"] as const)("acepta y aplica %s", (preference) => {
    const root = document.createElement("html");

    expect(readThemePreference(storageWith(preference))).toBe(preference);
    applyThemePreference(preference, root);

    expect(root.dataset.theme).toBe(preference);
    expect(root.style.colorScheme).toBe("light");
  });
});
