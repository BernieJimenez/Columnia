import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");

function readJson(relativePath: string): { version: string } {
  return JSON.parse(readFileSync(resolve(projectRoot, relativePath), "utf8")) as {
    version: string;
  };
}

describe("project version", () => {
  it("stays synchronized across npm, Cargo and Tauri", () => {
    const packageVersion = readJson("package.json").version;
    const packageLockVersion = readJson("package-lock.json").version;
    const tauriVersion = readJson("src-tauri/tauri.conf.json").version;
    const cargoManifest = readFileSync(resolve(projectRoot, "src-tauri/Cargo.toml"), "utf8");
    const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];

    expect(packageVersion).toBe("0.11.0");
    expect(packageLockVersion).toBe(packageVersion);
    expect(tauriVersion).toBe(packageVersion);
    expect(cargoVersion).toBe(packageVersion);
  });
});
