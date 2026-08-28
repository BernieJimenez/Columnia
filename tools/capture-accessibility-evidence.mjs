import { chromium } from "@playwright/test";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const port = 4174;
const timestamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const evidenceRelativePath = `.local/validation/accessibility-visual/${timestamp}`;
const evidenceDirectory = join(projectRoot, evidenceRelativePath.replaceAll("/", "\\"));
const summaryPath = join(evidenceDirectory, "summary.json");
const baseUrl = `http://127.0.0.1:${port}`;
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";

const cases = [
  {
    name: "desktop",
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: 1,
    forcedColors: "none",
  },
  {
    name: "mobile",
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 1,
    forcedColors: "none",
  },
  {
    name: "zoom-125",
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: 1,
    zoom: 1.25,
    forcedColors: "none",
  },
  {
    name: "zoom-200",
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: 1,
    zoom: 2,
    forcedColors: "none",
  },
  {
    name: "forced-colors",
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: 1,
    forcedColors: "active",
  },
];

function run(command, args) {
  const windows = process.platform === "win32";
  const processCommand = windows ? (process.env.ComSpec ?? "cmd.exe") : command;
  const processArgs = windows
    ? ["/d", "/s", "/c", [command, ...args].join(" ")]
    : args;
  return new Promise((resolveProcess, rejectProcess) => {
    const child = spawn(processCommand, processArgs, {
      cwd: projectRoot,
      stdio: "inherit",
      windowsHide: true,
    });
    child.once("error", rejectProcess);
    child.once("exit", (code, signal) => {
      if (code === 0) resolveProcess();
      else rejectProcess(new Error(`${command} terminó con código ${code ?? "desconocido"}${signal ? ` (${signal})` : ""}.`));
    });
  });
}

async function waitForServer(server) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (server.exitCode !== null) throw new Error("El preview terminó antes de quedar disponible.");
    try {
      const response = await fetch(baseUrl);
      if (response.ok) return;
    } catch {
      // El servidor todavía está iniciando.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw new Error(`El preview no respondió en ${baseUrl}.`);
}

async function inspectShell(page, expectedForcedColors) {
  await page.keyboard.press("Tab");
  return page.evaluate((forcedColors) => {
    const active = document.activeElement;
    const activeStyle = active instanceof HTMLElement ? getComputedStyle(active) : null;
    const targets = [...document.querySelectorAll("button:not(:disabled), a[href], input, select, textarea")]
      .map((element) => {
        const bounds = element.getBoundingClientRect();
        return { width: bounds.width, height: bounds.height };
      });
    const minTarget = targets.reduce(
      (minimum, target) => Math.min(minimum, target.width, target.height),
      Number.POSITIVE_INFINITY,
    );
    const viewportWidth = window.innerWidth;
    const documentWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
    return {
      forcedColors: window.matchMedia("(forced-colors: active)").matches,
      forcedColorsExpected: forcedColors === "active",
      landmarks: {
        main: document.querySelectorAll("main").length,
        navigation: document.querySelectorAll("nav").length,
        complementary: document.querySelectorAll("aside").length,
      },
      focus: {
        tagName: active?.tagName ?? null,
        outlineStyle: activeStyle?.outlineStyle ?? null,
        outlineWidth: activeStyle?.outlineWidth ?? null,
      },
      targetCount: targets.length,
      minimumTargetSize: Number.isFinite(minTarget) ? Number(minTarget.toFixed(2)) : null,
      viewportWidth,
      documentWidth,
      noHorizontalOverflow: documentWidth <= viewportWidth + 1,
    };
  }, expectedForcedColors);
}

let server;
let browser;
const capturedCases = [];
let status = "failed";
let error = null;

try {
  await mkdir(evidenceDirectory, { recursive: true });
  await run(npmCommand, ["run", "build"]);
  server = spawn(process.execPath, [
    "node_modules/vite/bin/vite.js",
    "preview",
    "--host",
    "127.0.0.1",
    "--port",
    String(port),
  ], {
    cwd: projectRoot,
    stdio: "ignore",
    windowsHide: true,
  });
  await waitForServer(server);
  browser = await chromium.launch({
    channel: process.platform === "win32" ? "msedge" : undefined,
    headless: true,
    args: ["--no-proxy-server", "--disable-gpu", "--disable-dev-shm-usage"],
  });

  for (const captureCase of cases) {
    const context = await browser.newContext({
      viewport: captureCase.viewport,
      deviceScaleFactor: captureCase.deviceScaleFactor,
    });
    try {
      const page = await context.newPage();
      await page.emulateMedia({ reducedMotion: "reduce", forcedColors: captureCase.forcedColors });
      await page.goto(baseUrl, { waitUntil: "networkidle" });
      await page.evaluate((zoom) => {
        document.documentElement.style.zoom = String(zoom);
        document.documentElement.dataset.columniaZoom = String(zoom);
      }, captureCase.zoom ?? 1);
      const inspection = await inspectShell(page, captureCase.forcedColors);
      const screenshotName = `${captureCase.name}.png`;
      const screenshotPath = join(evidenceDirectory, screenshotName);
      await page.screenshot({ path: screenshotPath, fullPage: true });
      const screenshotSha256 = createHash("sha256")
        .update(await readFile(screenshotPath))
        .digest("hex");
      const valid = inspection.forcedColors === inspection.forcedColorsExpected
        && inspection.landmarks.main === 1
        && inspection.landmarks.navigation === 1
        && inspection.landmarks.complementary === 1
        && inspection.focus.outlineStyle !== "none"
        && inspection.focus.outlineWidth !== "0px"
        && inspection.targetCount > 0
        && inspection.minimumTargetSize >= 24
        && inspection.noHorizontalOverflow;
      capturedCases.push({
        name: captureCase.name,
        viewport: captureCase.viewport,
        deviceScaleFactor: captureCase.deviceScaleFactor,
        zoom: captureCase.zoom ?? 1,
        forcedColors: captureCase.forcedColors,
        screenshot: relative(projectRoot, screenshotPath).replaceAll("\\", "/"),
        screenshotSha256,
        valid,
        inspection,
      });
      if (!valid) throw new Error(`El caso ${captureCase.name} no cumple el contrato visual/accesible.`);
    } finally {
      await context.close();
    }
  }
  status = "passed";
} catch (captureError) {
  error = captureError instanceof Error ? captureError.message : String(captureError);
} finally {
  if (browser) await browser.close();
  if (server) server.kill();
  await mkdir(dirname(summaryPath), { recursive: true });
  await writeFile(summaryPath, `${JSON.stringify({
    schemaVersion: 1,
    captureVersion: 2,
    status,
    generatedAt: new Date().toISOString(),
    baseUrl,
    cases: capturedCases,
    evidenceDirectory: evidenceRelativePath,
    error,
  }, null, 2)}\n`, "utf8");
}

if (status !== "passed") {
  console.error(`Evidencia de accesibilidad visual falló: ${error ?? "error desconocido"}. Evidencia: ${evidenceRelativePath}`);
  process.exitCode = 1;
} else {
  console.log(`Evidencia de accesibilidad visual aprobada: ${evidenceRelativePath}`);
}
