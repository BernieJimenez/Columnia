import { existsSync, readFileSync } from "node:fs";
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

describe("Tauri desktop security boundary", () => {
  it("keeps production CSP local-only and blocks embedded remote content", () => {
    const config = JSON.parse(
      readFileSync(resolve(projectRoot, "src-tauri/tauri.conf.json"), "utf8"),
    ) as { app: { security: { csp: Record<string, string> } } };
    const csp = config.app.security.csp;

    expect(csp["default-src"]).toBe("'self'");
    expect(csp["script-src"]).toBe("'self'");
    expect(csp["object-src"]).toBe("'none'");
    expect(csp["frame-src"]).toBe("'none'");
    expect(csp["connect-src"]).not.toMatch(/https:|wss:/);
  });

  it("grants the main window no filesystem, shell, network or opener capability", () => {
    const capability = JSON.parse(
      readFileSync(resolve(projectRoot, "src-tauri/capabilities/main.json"), "utf8"),
    ) as { windows: string[]; permissions: string[] };

    expect(capability.windows).toEqual(["main"]);
    expect(capability.permissions).toEqual(["core:default"]);
    expect(capability.permissions.join(" ")).not.toMatch(/fs:|shell:|http:|opener:/);
  });
});
