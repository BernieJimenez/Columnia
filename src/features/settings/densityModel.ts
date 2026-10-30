/**
 * How much fits on screen. «Compacta» tightens tables, headings and spacing so
 * more rows and columns are visible; buttons keep their 44 px target.
 */
export type DensityPreference = "comfortable" | "compact";

export const DENSITY_STORAGE_KEY = "columnia.density";

function defaultStorage(): Storage | undefined {
  if (typeof window === "undefined") return undefined;
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

export function readDensityPreference(storage: Storage | undefined = defaultStorage()): DensityPreference {
  try {
    return storage?.getItem(DENSITY_STORAGE_KEY) === "compact" ? "compact" : "comfortable";
  } catch {
    return "comfortable";
  }
}

export function writeDensityPreference(
  preference: DensityPreference,
  storage: Storage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(DENSITY_STORAGE_KEY, preference);
  } catch {
    // A restricted storage context must not make the interface unusable.
  }
}

export function applyDensityPreference(
  preference: DensityPreference,
  root: HTMLElement | undefined = typeof document === "undefined" ? undefined : document.documentElement,
): void {
  if (!root) return;
  root.dataset.density = preference;
}
