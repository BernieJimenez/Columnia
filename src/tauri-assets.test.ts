import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "..");

type ContentSecurityPolicy = Record<string, string>;

type TauriConfig = {
  build: { devUrl: string };
  app: { security: { csp: ContentSecurityPolicy; devCsp: ContentSecurityPolicy } };
};

type TauriCapability = {
  windows: string[];
  permissions: string[];
};

function readJson<T>(relativePath: string): T {
  return JSON.parse(readFileSync(resolve(projectRoot, relativePath), "utf8")) as T;
}

function cspSources(csp: ContentSecurityPolicy, directive: string): Set<string> {
  return new Set((csp[directive] ?? "").trim().split(/\s+/).filter(Boolean));
}

function sorted(values: Iterable<string>): string[] {
  return [...values].sort();
}

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

describe("Tauri desktop security boundary", () => {
  it("keeps production CSP restrictive and free of remote web origins", () => {
    const config = readJson<TauriConfig>("src-tauri/tauri.conf.json");
    const csp = config.app.security.csp;
    const serializedCsp = Object.values(csp).join(" ");
    const forbiddenRemoteSources = serializedCsp
      .split(/\s+/)
      .filter(
        (source) =>
          source === "*" ||
          /^(?:https?|wss?):$/i.test(source) ||
          (/^(?:https?|wss?):\/\//i.test(source) && source !== "http://ipc.localhost"),
      );

    expect(sorted(cspSources(csp, "default-src")), "production default-src must remain self-only").toEqual([
      "'self'",
    ]);
    expect(sorted(cspSources(csp, "script-src")), "production scripts must remain self-only").toEqual([
      "'self'",
    ]);
    expect(sorted(cspSources(csp, "object-src")), "production must disable object embedding").toEqual([
      "'none'",
    ]);
    expect(sorted(cspSources(csp, "frame-src")), "production must disable frame embedding").toEqual([
      "'none'",
    ]);
    expect(sorted(cspSources(csp, "base-uri")), "production base URIs must remain self-only").toEqual([
      "'self'",
    ]);
    expect(
      sorted(cspSources(csp, "connect-src")),
      "production connections must remain limited to the Tauri IPC transport",
    ).toEqual(["'self'", "http://ipc.localhost", "ipc:"]);
    expect(
      forbiddenRemoteSources,
      "production CSP must not allow wildcards or remote web/socket origins",
    ).toEqual([]);
    expect(serializedCsp, "production CSP must not enable unsafe script evaluation").not.toMatch(
      /'unsafe-eval'/i,
    );
  });

  it("limits development CSP exceptions to the configured loopback server", () => {
    const config = readJson<TauriConfig>("src-tauri/tauri.conf.json");
    const { csp, devCsp } = config.app.security;
    const devUrl = new URL(config.build.devUrl);

    expect(devUrl.protocol, "Tauri development URL must use local HTTP").toBe("http:");
    expect(devUrl.hostname, "Tauri development URL must remain on numeric loopback").toBe("127.0.0.1");

    const productionDirectives = Object.keys(csp).sort();
    expect(Object.keys(devCsp).sort(), "development CSP must keep the production directive set").toEqual(
      productionDirectives,
    );

    for (const directive of productionDirectives.filter((name) => name !== "connect-src")) {
      expect(
        sorted(cspSources(devCsp, directive)),
        `development ${directive} must not be broader than production`,
      ).toEqual(sorted(cspSources(csp, directive)));
    }

    const expectedDevelopmentConnections = new Set(cspSources(csp, "connect-src"));
    expectedDevelopmentConnections.add(devUrl.origin);
    expectedDevelopmentConnections.add(`ws://${devUrl.host}`);
    expect(
      sorted(cspSources(devCsp, "connect-src")),
      "development connections may add only the configured loopback HTTP and WebSocket origins",
    ).toEqual(sorted(expectedDevelopmentConnections));
  });

  it("grants the main window no filesystem, shell, network or opener capability", () => {
    const capability = readJson<TauriCapability>("src-tauri/capabilities/main.json");

    expect(sorted(capability.windows), "main capability must target only the main window").toEqual([
      "main",
    ]);
    expect(sorted(capability.permissions), "main capability must retain only Tauri core defaults").toEqual([
      "core:default",
    ]);
    expect(
      capability.permissions,
      "main capability must not include filesystem, shell, HTTP or opener permissions",
    ).not.toEqual(expect.arrayContaining([expect.stringMatching(/^(?:fs|shell|http|opener):/i)]));
  });
});
