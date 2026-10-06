import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

/**
 * SEG-08: `vite preview` (the server the E2E suite drives) answers with the
 * production CSP of tauri.conf.json, so every browser test runs under it.
 * Tauri adds the hash of each inline script itself; the preview does the same.
 */
function productionCspPreview(): Plugin {
  return {
    name: "columnia-production-csp-preview",
    configurePreviewServer(server) {
      const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
      const directives: Record<string, string> = config.app.security.csp;
      const html = readFileSync(`${server.config.build.outDir}/index.html`, "utf8");
      const hashes = [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)]
        .map((match) => `'sha256-${createHash("sha256").update(match[1]).digest("base64")}'`);
      const policy = Object.entries(directives)
        .map(([name, value]) => (name === "script-src" ? [name, value, ...hashes] : [name, value]).join(" "))
        .join("; ");
      server.middlewares.use((_request, response, next) => {
        response.setHeader("Content-Security-Policy", policy);
        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), productionCspPreview()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  // SEG-06: `TAURI_` would also expose TAURI_SIGNING_PRIVATE_KEY_PASSWORD to
  // the bundle; Tauri only needs TAURI_ENV_*.
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG ? false : "oxc",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
  },
});
