import type { DatasetFormat } from "../../bridge";

export const RECENT_DATASETS_STORAGE_KEY = "columnia.recent-datasets";
export const MAX_RECENT_DATASETS = 5;

const DATASET_FORMATS: readonly DatasetFormat[] = ["csv", "tsv", "json", "parquet", "excel"];

export interface RecentDataset {
  id: string;
  fileName: string;
  format: DatasetFormat;
  lastOpenedAt: number;
}

export interface RecentDatasetInput {
  fileName: string;
  format: DatasetFormat;
}

function defaultStorage(): Storage | undefined {
  if (typeof window === "undefined") return undefined;

  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

function isDatasetFormat(value: unknown): value is DatasetFormat {
  return typeof value === "string" && DATASET_FORMATS.includes(value as DatasetFormat);
}

/** Keeps the visible label while making it impossible to persist a path. */
export function normalizeRecentFileName(value: unknown): string {
  if (typeof value !== "string") return "";
  const normalized = value.replace(/[\\/]+/g, "/").trim();
  const name = normalized.slice(normalized.lastIndexOf("/") + 1).trim();
  return name.slice(0, 160);
}

function isOpaqueId(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9][A-Za-z0-9._:-]{0,95}$/.test(value);
}

function createOpaqueId(): string {
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return crypto.randomUUID();
    }
  } catch {
    // Use the local fallback when the runtime does not expose Web Crypto.
  }
  return `recent-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function sanitizeRecentDataset(value: unknown): RecentDataset | null {
  if (!value || typeof value !== "object") return null;
  const candidate = value as Partial<RecentDataset>;
  const fileName = normalizeRecentFileName(candidate.fileName);
  if (!fileName || !isOpaqueId(candidate.id) || !isDatasetFormat(candidate.format)) return null;
  if (typeof candidate.lastOpenedAt !== "number" || !Number.isFinite(candidate.lastOpenedAt)) return null;
  return {
    id: candidate.id,
    fileName,
    format: candidate.format,
    lastOpenedAt: Math.max(0, Math.trunc(candidate.lastOpenedAt)),
  };
}

export function readRecentDatasets(
  storage: Storage | undefined = defaultStorage(),
): RecentDataset[] {
  try {
    const raw = storage?.getItem(RECENT_DATASETS_STORAGE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map(sanitizeRecentDataset)
      .filter((item): item is RecentDataset => item !== null)
      .sort((first, second) => second.lastOpenedAt - first.lastOpenedAt)
      .slice(0, MAX_RECENT_DATASETS);
  } catch {
    return [];
  }
}

export function writeRecentDatasets(
  items: readonly RecentDataset[],
  storage: Storage | undefined = defaultStorage(),
): void {
  try {
    const sanitized = items
      .map(sanitizeRecentDataset)
      .filter((item): item is RecentDataset => item !== null)
      .sort((first, second) => second.lastOpenedAt - first.lastOpenedAt)
      .slice(0, MAX_RECENT_DATASETS);
    storage?.setItem(RECENT_DATASETS_STORAGE_KEY, JSON.stringify(sanitized));
  } catch {
    // A restricted storage context must not make the interface unusable.
  }
}

export function rememberRecentDataset(
  current: readonly RecentDataset[],
  input: RecentDatasetInput,
  now = Date.now(),
): RecentDataset[] {
  const fileName = normalizeRecentFileName(input.fileName);
  if (!fileName || !isDatasetFormat(input.format)) return [...current].slice(0, MAX_RECENT_DATASETS);

  const normalizedKey = `${fileName.toLocaleLowerCase()}\u0000${input.format}`;
  const existing = current.find((item) =>
    `${item.fileName.toLocaleLowerCase()}\u0000${item.format}` === normalizedKey,
  );
  const next: RecentDataset = {
    id: existing?.id ?? createOpaqueId(),
    fileName,
    format: input.format,
    lastOpenedAt: Math.max(0, Math.trunc(now)),
  };
  return [next, ...current.filter((item) => item.id !== existing?.id &&
    `${item.fileName.toLocaleLowerCase()}\u0000${item.format}` !== normalizedKey)].slice(0, MAX_RECENT_DATASETS);
}

export function removeRecentDataset(
  current: readonly RecentDataset[],
  id: string,
): RecentDataset[] {
  return current.filter((item) => item.id !== id);
}

export function formatRecentDatasetFormat(format: DatasetFormat): string {
  return format === "excel" ? "Excel" : format.toUpperCase();
}

export function formatRecentDatasetDate(timestamp: number): string {
  try {
    return new Intl.DateTimeFormat("es", { day: "numeric", month: "short" }).format(new Date(timestamp));
  } catch {
    return "Fecha no disponible";
  }
}
