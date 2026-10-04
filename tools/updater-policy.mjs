// Rules shared by the updater gates (SEG-04): a published or local manifest
// must announce the version being released (no downgrade), point to an
// allowed HTTPS host and stay within a size the gate is willing to download.

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** Largest installer a gate downloads: the release is far below it. */
export const MAX_ARTIFACT_BYTES = 1024 * 1024 * 1024;
/** Largest manifest a gate reads. */
export const MAX_MANIFEST_BYTES = 1024 * 1024;

/** The version this checkout releases (package.json). */
export function localVersion() {
  return JSON.parse(readFileSync(resolve(projectRoot, "package.json"), "utf8")).version;
}

/** Hosts of the updater endpoints declared in tauri.conf.json. */
export function configuredUpdaterHosts() {
  const config = JSON.parse(readFileSync(resolve(projectRoot, "src-tauri/tauri.conf.json"), "utf8"));
  return (config.plugins?.updater?.endpoints ?? [])
    .map((endpoint) => {
      try {
        return new URL(endpoint).host;
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

function normalizeVersion(value) {
  return String(value ?? "").trim().replace(/^v/, "");
}

/** Problems of a manifest platform entry against the release policy. */
export function updaterPolicyProblems({ manifestVersion, expectedVersion, artifactUrl, allowedHosts, sizeBytes }) {
  const problems = [];
  if (normalizeVersion(manifestVersion) !== normalizeVersion(expectedVersion)) {
    problems.push(`El manifiesto declara ${manifestVersion}, pero esta versión publica ${expectedVersion}.`);
  }
  let url;
  try {
    url = new URL(artifactUrl);
  } catch {
    problems.push("La URL del artefacto no es una URL absoluta válida.");
  }
  if (url) {
    if (url.protocol !== "https:") problems.push("La URL del artefacto no usa HTTPS.");
    if (allowedHosts.length === 0) {
      problems.push("No hay hosts permitidos para el updater: declara el endpoint en tauri.conf.json o pasa --allowed-host.");
    } else if (!allowedHosts.includes(url.host)) {
      problems.push(`El artefacto se aloja en ${url.host}, que no es un host permitido (${allowedHosts.join(", ")}).`);
    }
  }
  if (!Number.isInteger(sizeBytes) || sizeBytes <= 0 || sizeBytes > MAX_ARTIFACT_BYTES) {
    problems.push("El tamaño declarado del artefacto no es válido o supera el límite del gate.");
  }
  return problems;
}

/** The body of `response`, refusing more than `maximumBytes`. */
export async function readLimitedBody(response, maximumBytes, label) {
  const declared = Number(response.headers?.get?.("content-length"));
  if (Number.isFinite(declared) && declared > maximumBytes) {
    throw new Error(`${label} declara ${declared} bytes y supera el límite de ${maximumBytes}.`);
  }
  const chunks = [];
  let total = 0;
  for await (const chunk of response.body) {
    total += chunk.length;
    if (total > maximumBytes) throw new Error(`${label} supera el límite de ${maximumBytes} bytes.`);
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}
