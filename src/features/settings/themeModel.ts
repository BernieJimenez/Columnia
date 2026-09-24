export type ThemePreference = "system" | "light" | "dark" | "paper" | "ocean" | "slate";

export const THEME_STORAGE_KEY = "columnia.theme";

const THEME_PREFERENCES: readonly ThemePreference[] = [
  "system",
  "light",
  "dark",
  "paper",
  "ocean",
  "slate",
];

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

function systemPrefersDark(): boolean {
  try {
    return typeof window !== "undefined"
      && typeof window.matchMedia === "function"
      && window.matchMedia("(prefers-color-scheme: dark)").matches;
  } catch {
    return false;
  }
}

/** "system" becomes light or dark; every other theme keeps its name. */
export function resolveThemePreference(
  preference: ThemePreference,
  prefersDark: boolean = systemPrefersDark(),
): Exclude<ThemePreference, "system"> {
  if (preference !== "system") return preference;
  return prefersDark ? "dark" : "light";
}

export function applyThemePreference(
  preference: ThemePreference,
  root: HTMLElement | undefined = typeof document === "undefined" ? undefined : document.documentElement,
  prefersDark: boolean = systemPrefersDark(),
): void {
  if (!root) return;

  root.dataset.theme = preference;
  // Styles key on the resolved theme, so "Sistema" paints exactly like the
  // theme it resolves to and no rule depends on the OS media query (T10-11).
  root.dataset.resolvedTheme = resolveThemePreference(preference, prefersDark);
  root.style.colorScheme = preference === "system"
    ? "light dark"
    : preference === "dark" ? "dark" : "light";
}
