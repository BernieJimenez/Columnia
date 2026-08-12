import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");

describe("Tauri desktop assets", () => {
  it.each([
    "src-tauri/icons/icon.ico",
    "src-tauri/icons/icon.icns",
    "src-tauri/icons/icon.png",
    "src-tauri/icons/32x32.png",
    "src-tauri/icons/128x128.png",
    "src-tauri/icons/128x128@2x.png",
  ])("includes the required cross-platform icon %s", (relativePath) => {
    expect(existsSync(resolve(projectRoot, relativePath))).toBe(true);
  });
});

