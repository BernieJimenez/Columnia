import { readdirSync, readFileSync, existsSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");
const fixturesRoot = join(projectRoot, "fixtures");

function readProjectFile(relativePath: string): string {
  return readFileSync(join(projectRoot, relativePath), "utf8");
}

function fixtureFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolutePath = join(directory, entry.name);
    if (entry.isDirectory()) {
      return fixtureFiles(absolutePath);
    }
    if (entry.name === "README.md" || entry.name === "manifest.json") {
      return [];
    }
    return [relative(fixturesRoot, absolutePath).replaceAll("\\", "/")];
  });
}

describe("repository governance", () => {
  it("publishes the license and durable governance documents", () => {
    const packageManifest = JSON.parse(readProjectFile("package.json")) as { license?: string };
    const cargoManifest = readProjectFile("src-tauri/Cargo.toml");

    expect(packageManifest.license).toBe("MIT");
    expect(cargoManifest).toMatch(/^license = "MIT"$/m);
    expect(readProjectFile("LICENSE")).toContain("MIT License");
    for (const path of [
      "CONTRIBUTING.md",
      "docs/README.md",
      "docs/adr/0001-contratos-del-repositorio.md",
      "docs/reference/repository-governance.md",
      "docs/reference/dependency-audit.md",
      "docs/reference/fixtures-policy.md",
    ]) {
      expect(existsSync(join(projectRoot, path)), `${path} must exist`).toBe(true);
    }
  });

  it("keeps the fixture tree deterministic and manifest-backed", () => {
    const manifest = JSON.parse(readProjectFile("fixtures/manifest.json")) as {
      version: number;
      policy: string;
      files: string[];
    };
    const actualFiles = fixtureFiles(fixturesRoot).sort();
    const listedFiles = [...manifest.files].sort();

    expect(manifest.version).toBe(1);
    expect(manifest.policy).toBe("synthetic-only-no-pii");
    expect(listedFiles).toEqual(actualFiles);
    expect(actualFiles.some((file) => /(^|\/)(\.env|.*\.(key|pem|p12|pfx|sqlite|db))$/i.test(file))).toBe(false);

    for (const file of actualFiles) {
      const contents = readProjectFile(`fixtures/${file}`);
      expect(contents).not.toMatch(/-----BEGIN [A-Z ]*PRIVATE KEY-----/);
    }
  });
});
