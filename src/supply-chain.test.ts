import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");
const npmRegistryHost = "registry.npmjs.org";
const cratesIoSource = "registry+https://github.com/rust-lang/crates.io-index";

type NpmLockPackage = {
  version?: string;
  resolved?: string;
  integrity?: string;
  link?: boolean;
};

type NpmLockfile = {
  packages: Record<string, NpmLockPackage>;
};

type CargoPackage = {
  name?: string;
  version?: string;
  source?: string;
  checksum?: string;
};

function readProjectFile(relativePath: string): string {
  return readFileSync(resolve(projectRoot, relativePath), "utf8");
}

function npmPackageName(packagePath: string): string {
  const marker = "node_modules/";
  const markerIndex = packagePath.lastIndexOf(marker);
  return markerIndex === -1 ? packagePath : packagePath.slice(markerIndex + marker.length);
}

function validStrongIntegrity(integrity: string | undefined): boolean {
  if (!integrity) {
    return false;
  }

  const digests = integrity.trim().split(/\s+/);
  return (
    digests.length > 0 &&
    digests.every((digest) => {
      const match = digest.match(/^(sha256|sha512)-([A-Za-z0-9+/]+={0,2})$/);
      if (!match) {
        return false;
      }

      const expectedBytes = match[1] === "sha512" ? 64 : 32;
      return Buffer.from(match[2], "base64").byteLength === expectedBytes;
    })
  );
}

function cargoField(block: string, field: string): string | undefined {
  return block.match(new RegExp(`^${field} = "([^"]+)"$`, "m"))?.[1];
}

function cargoPackages(): CargoPackage[] {
  return readProjectFile("src-tauri/Cargo.lock")
    .split(/^\[\[package\]\]\s*$/m)
    .slice(1)
    .map((block) => ({
      name: cargoField(block, "name"),
      version: cargoField(block, "version"),
      source: cargoField(block, "source"),
      checksum: cargoField(block, "checksum"),
    }));
}

function cargoManifestIdentity(): { name: string | undefined; version: string | undefined } {
  const lines = readProjectFile("src-tauri/Cargo.toml").split(/\r?\n/);
  const packageHeader = lines.findIndex((line) => line.trim() === "[package]");
  const packageLines = lines
    .slice(packageHeader + 1)
    .slice(0, lines.slice(packageHeader + 1).findIndex((line) => /^\[.+\]$/.test(line.trim())));
  const packageBlock = packageLines.join("\n");
  return {
    name: cargoField(packageBlock, "name"),
    version: cargoField(packageBlock, "version"),
  };
}

function contradictoryIdentities(
  entries: Array<{ identity: string; fingerprint: string }>,
): string[] {
  const fingerprints = new Map<string, Set<string>>();
  for (const { identity, fingerprint } of entries) {
    const known = fingerprints.get(identity) ?? new Set<string>();
    known.add(fingerprint);
    fingerprints.set(identity, known);
  }

  return [...fingerprints]
    .filter(([, values]) => values.size > 1)
    .map(([identity]) => identity)
    .sort();
}

describe("offline supply-chain lockfile integrity", () => {
  it("mantiene una política cargo-deny explícita y razonada para excepciones upstream", () => {
    const policy = readProjectFile("src-tauri/deny.toml");

    expect(policy).toContain("[advisories]");
    expect(policy).toContain("RUSTSEC-2026-0194");
    expect(policy).toContain("reason =");
    expect(policy).toContain("[licenses]");
    expect(policy).toContain("[sources]");
  });

  it("conserva el inventario generado de avisos y la política de privacidad local", () => {
    const notices = readProjectFile("THIRD_PARTY_NOTICES.md");
    const networkPolicy = readProjectFile("docs/reference/network-privacy.md");

    expect(notices).toContain("# Third-party notices");
    expect(notices).toContain("| Ecosistema | Paquete | Versión | Licencia | Fuente |");
    expect(notices).toMatch(/Total: \d+ dependencias de terceros\./);
    expect(networkPolicy).toContain("npm run network:check");
    expect(networkPolicy).toMatch(/no\s+inicia conexiones de red/);
  });

  it("pins every npm dependency to the HTTPS npm registry with a strong integrity digest", () => {
    const lockfile = JSON.parse(readProjectFile("package-lock.json")) as NpmLockfile;
    const violations: string[] = [];

    for (const [packagePath, dependency] of Object.entries(lockfile.packages)) {
      if (packagePath === "") {
        continue;
      }

      const identity = `${npmPackageName(packagePath)}@${dependency.version ?? "missing-version"}`;
      if (dependency.link || !dependency.resolved) {
        violations.push(`${identity}: must resolve directly from the npm registry`);
        continue;
      }

      let source: URL;
      try {
        source = new URL(dependency.resolved);
      } catch {
        violations.push(`${identity}: has an invalid resolved URL`);
        continue;
      }

      if (source.protocol !== "https:" || source.hostname !== npmRegistryHost) {
        violations.push(`${identity}: source must be https://${npmRegistryHost}`);
      }
      if (!validStrongIntegrity(dependency.integrity)) {
        violations.push(`${identity}: integrity must contain only valid sha512/sha256 SRI digests`);
      }
    }

    expect(violations, "npm lockfile contains untrusted or weakly pinned dependencies").toEqual([]);
  });

  it("has no contradictory npm identities", () => {
    const lockfile = JSON.parse(readProjectFile("package-lock.json")) as NpmLockfile;
    const entries = Object.entries(lockfile.packages)
      .filter(([packagePath, dependency]) => packagePath !== "" && dependency.version)
      .map(([packagePath, dependency]) => ({
        identity: `${npmPackageName(packagePath)}@${dependency.version as string}`,
        fingerprint: `${dependency.resolved ?? "no-source"}|${dependency.integrity ?? "no-integrity"}`,
      }));

    expect(
      contradictoryIdentities(entries),
      "the same npm name/version must not resolve to conflicting sources or digests",
    ).toEqual([]);
  });

  it("pins every registry crate to crates.io with a checksum and permits only the local package otherwise", () => {
    const localPackage = cargoManifestIdentity();
    const violations: string[] = [];

    expect(localPackage.name, "Cargo.toml [package] must declare a name").toBeDefined();
    expect(localPackage.version, "Cargo.toml [package] must declare a version").toBeDefined();

    for (const dependency of cargoPackages()) {
      const identity = `${dependency.name ?? "missing-name"}@${dependency.version ?? "missing-version"}`;
      if (!dependency.name || !dependency.version) {
        violations.push(`${identity}: package block must declare name and version`);
        continue;
      }

      if (!dependency.source) {
        if (dependency.name !== localPackage.name || dependency.version !== localPackage.version) {
          violations.push(`${identity}: source-less package is not the local Cargo package`);
        }
        if (dependency.checksum) {
          violations.push(`${identity}: local package must not carry a registry checksum`);
        }
        continue;
      }

      if (dependency.source !== cratesIoSource) {
        violations.push(`${identity}: source must be the official crates.io registry (git sources forbidden)`);
      }
      if (!/^[a-f0-9]{64}$/i.test(dependency.checksum ?? "")) {
        violations.push(`${identity}: registry crate must have a 64-character SHA-256 checksum`);
      }
    }

    expect(violations, "Cargo.lock contains untrusted or incompletely pinned packages").toEqual([]);
  });

  it("has no contradictory Cargo identities", () => {
    const entries = cargoPackages()
      .filter((dependency) => dependency.name && dependency.version)
      .map((dependency) => ({
        identity: `${dependency.name as string}@${dependency.version as string}`,
        fingerprint: `${dependency.source ?? "local"}|${dependency.checksum ?? "no-checksum"}`,
      }));

    expect(
      contradictoryIdentities(entries),
      "the same Cargo name/version must not resolve to conflicting sources or checksums",
    ).toEqual([]);
  });
});
