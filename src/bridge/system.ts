import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  UpdateInfo,
  UpdaterProgress,
  ResourceUsage,
  PerformanceProfile,
  PerformanceSettings,
} from "./contracts";

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export function checkForUpdate(): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("check_for_update");
}

export function downloadUpdate(onProgress?: (progress: UpdaterProgress) => void): Promise<void> {
  const channel = new Channel<UpdaterProgress>((progress) => onProgress?.(progress));
  return invoke<void>("download_update", { onProgress: channel });
}

export function cancelUpdateDownload(): Promise<void> {
  return invoke<void>("cancel_update_download");
}

export function installUpdate(): Promise<void> {
  return invoke<void>("install_update");
}

export function getResourceUsage(): Promise<ResourceUsage> {
  return invoke<ResourceUsage>("get_resource_usage");
}

export function getPerformanceSettings(): Promise<PerformanceSettings> {
  return invoke<PerformanceSettings>("get_performance_settings");
}

export function setPerformanceProfile(profile: PerformanceProfile): Promise<PerformanceSettings> {
  return invoke<PerformanceSettings>("set_performance_profile", { profile });
}
