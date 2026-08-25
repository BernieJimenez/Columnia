export type ThemePreference = "system" | "light" | "dark";

export const THEME_STORAGE_KEY = "columnia.theme";

const THEME_PREFERENCES: readonly ThemePreference[] = ["system", "light", "dark"];

export function isThemePreference(value: string | null | undefined): value is ThemePreference {
  return value != null && THEME_PREFERENCES.includes(value as ThemePreference);
}

function defaultStorage(): Storage | undefined {
  if (typeof window === "undefined") return undefined;

  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

export function readThemePreference(storage: Storage | undefined = defaultStorage()): ThemePreference {
  try {
    const stored = storage?.getItem(THEME_STORAGE_KEY);
    return isThemePreference(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

export function writeThemePreference(
  preference: ThemePreference,
  storage: Storage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, preference);
  } catch {
    // A restricted storage context must not make the interface unusable.
  }
}

export function applyThemePreference(
  preference: ThemePreference,
  root: HTMLElement | undefined = typeof document === "undefined" ? undefined : document.documentElement,
): void {
  if (!root) return;

  root.dataset.theme = preference;
  root.style.colorScheme = preference === "system" ? "light dark" : preference;
}
