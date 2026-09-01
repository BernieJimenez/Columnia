import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");

type DependencyMap = Record<string, string>;

type PackageManifest = {
  name: string;
  version: string;
  dependencies?: DependencyMap;
  devDependencies?: DependencyMap;
};

type PackageLock = PackageManifest & {
  packages: Record<string, PackageManifest>;
};

function readJson<T>(relativePath: string): T {
  return JSON.parse(readFileSync(resolve(projectRoot, relativePath), "utf8")) as T;
}

function readCargoPackageVersion(relativePath: string, packageName: string): string | undefined {
  const contents = readFileSync(resolve(projectRoot, relativePath), "utf8");
  const packageBlocks = contents.split(/^\[\[package\]\]\s*$/m).slice(1);

  for (const block of packageBlocks) {
    const name = block.match(/^name = "([^"]+)"$/m)?.[1];
    if (name === packageName) {
      return block.match(/^version = "([^"]+)"$/m)?.[1];
    }
  }

  return undefined;
}

describe("project version", () => {
  it("stays synchronized across npm, Cargo and Tauri", () => {
    const packageVersion = readJson<PackageManifest>("package.json").version;
    const packageLockVersion = readJson<PackageLock>("package-lock.json").version;
    const tauriVersion = readJson<{ version: string }>("src-tauri/tauri.conf.json").version;
    const cargoManifest = readFileSync(resolve(projectRoot, "src-tauri/Cargo.toml"), "utf8");
    const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];

    expect(packageVersion).toBe("0.129.0");
    expect(packageLockVersion).toBe(packageVersion);
    expect(tauriVersion).toBe(packageVersion);
    expect(cargoVersion).toBe(packageVersion);
  });

  it("keeps the npm lockfile root reproducible from package.json", () => {
    const manifest = readJson<PackageManifest>("package.json");
    const lockfile = readJson<PackageLock>("package-lock.json");
    const lockfileRoot = lockfile.packages[""];

    expect(lockfileRoot, "package-lock.json must contain the root package at packages['']").toBeDefined();
    expect(lockfile.version, "package-lock.json top-level version must match package.json").toBe(
      manifest.version,
    );
    expect(lockfileRoot.version, "package-lock.json root version must match package.json").toBe(
      manifest.version,
    );
    expect(
      lockfileRoot.dependencies ?? {},
      "package-lock.json root dependencies must exactly match package.json",
    ).toEqual(manifest.dependencies ?? {});
    expect(
      lockfileRoot.devDependencies ?? {},
      "package-lock.json root devDependencies must exactly match package.json",
    ).toEqual(manifest.devDependencies ?? {});
  });

  it("keeps the local Cargo package version synchronized with Cargo.toml", () => {
    const cargoManifest = readFileSync(resolve(projectRoot, "src-tauri/Cargo.toml"), "utf8");
    const packageName = cargoManifest.match(/^name = "([^"]+)"$/m)?.[1];
    const manifestVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];

    expect(packageName, "Cargo.toml [package] must declare a name").toBeDefined();
    expect(manifestVersion, "Cargo.toml [package] must declare a version").toBeDefined();

    const lockfileVersion = readCargoPackageVersion(
      "src-tauri/Cargo.lock",
      packageName as string,
    );

    expect(
      lockfileVersion,
      `Cargo.lock must contain the local package ${packageName as string}`,
    ).toBe(manifestVersion);
  });
});
