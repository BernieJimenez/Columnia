import type { PerformanceProfile } from "../../bridge";

export const PERFORMANCE_STORAGE_KEY = "columnia.performance-profile";

const PERFORMANCE_PROFILES: readonly PerformanceProfile[] = [
  "conservative",
  "balanced",
  "maximum",
];

export function isPerformanceProfile(
  value: string | null | undefined,
): value is PerformanceProfile {
  return value != null && PERFORMANCE_PROFILES.includes(value as PerformanceProfile);
}

function defaultStorage(): Storage | undefined {
  if (typeof window === "undefined") return undefined;

  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

export function readPerformanceProfile(
  storage: Storage | undefined = defaultStorage(),
): PerformanceProfile {
  try {
    const stored = storage?.getItem(PERFORMANCE_STORAGE_KEY);
    return isPerformanceProfile(stored) ? stored : "balanced";
  } catch {
    return "balanced";
  }
}

export function hasPerformanceProfilePreference(
  storage: Storage | undefined = defaultStorage(),
): boolean {
  try {
    return storage?.getItem(PERFORMANCE_STORAGE_KEY) !== null;
  } catch {
    return false;
  }
}

export function writePerformanceProfile(
  profile: PerformanceProfile,
  storage: Storage | undefined = defaultStorage(),
): void {
  try {
    storage?.setItem(PERFORMANCE_STORAGE_KEY, profile);
  } catch {
    // A restricted storage context must not make the interface unusable.
  }
}
