export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  updaterConfigured?: boolean;
}

export interface UpdateInfo {
  currentVersion: string;
  version: string;
  notes: string | null;
  date: string | null;
  sizeBytes: number | null;
}

export interface UpdaterProgress {
  phase: "started" | "progress" | "finished" | "cancelled";
  downloadedBytes: number;
  contentLength: number | null;
}

export interface ResourceUsage {
  processCpuPercentage: number;
  systemCpuPercentage: number;
  logicalCpuCount: number;
  processMemoryBytes: number;
  systemMemoryUsedBytes: number;
  systemMemoryTotalBytes: number;
  /** Available RAM is optional for compatibility with older desktop builds. */
  systemMemoryAvailableBytes?: number;
  /** GPU values are null when the runtime has no GPU probe. */
  gpu?: GpuUsage;
}

export type PerformanceProfile = "conservative" | "balanced" | "maximum";

export interface PerformanceSettings {
  requestedProfile: PerformanceProfile;
  activeProfile: PerformanceProfile | null;
  requestedThreads: number;
  activeThreads: number | null;
  applied: boolean;
  locked: boolean;
  reason: string | null;
}

export interface GpuUsage {
  status: "available" | "unavailable";
  usagePercentage: number | null;
  memoryUsedBytes: number | null;
  memoryTotalBytes: number | null;
  reason: string | null;
}
