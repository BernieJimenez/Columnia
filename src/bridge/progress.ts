import { Channel } from "@tauri-apps/api/core";
import type { OperationProgress } from "./contracts";

export type ProgressHandler = (progress: OperationProgress) => void;

export function progressChannel(onProgress?: ProgressHandler): Channel<OperationProgress> {
  return new Channel<OperationProgress>((progress) => onProgress?.(progress));
}
