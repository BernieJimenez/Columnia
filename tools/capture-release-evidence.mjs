import { chromium } from "@playwright/test";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const baselinePath = join(projectRoot, "fixtures", "accessibility", "release-evidence-baseline-v1.json");
const portArgumentIndex = process.argv.indexOf("--port");
const outputArgumentIndex = process.argv.indexOf("--output");
const projectVersionArgumentIndex = process.argv.indexOf("--project-version");
const binaryPathArgumentIndex = process.argv.indexOf("--binary-path");
const binaryShaArgumentIndex = process.argv.indexOf("--binary-sha256");
const binarySizeArgumentIndex = process.argv.indexOf("--binary-size");
const fixturePathArgumentIndex = process.argv.indexOf("--fixture-path");
const fixtureShaArgumentIndex = process.argv.indexOf("--fixture-sha256");
const fixtureSizeArgumentIndex = process.argv.indexOf("--fixture-size");
const port = Number(portArgumentIndex >= 0 ? process.argv[portArgumentIndex + 1] : 9222);
const outputPath = outputArgumentIndex >= 0 ? resolve(projectRoot, process.argv[outputArgumentIndex + 1]) : null;
const projectVersion = projectVersionArgumentIndex >= 0 ? process.argv[projectVersionArgumentIndex + 1] : null;
const binaryPath = binaryPathArgumentIndex >= 0 ? process.argv[binaryPathArgumentIndex + 1] : null;
const binarySha256 = binaryShaArgumentIndex >= 0 ? process.argv[binaryShaArgumentIndex + 1] : null;
const binarySizeBytes = Number(binarySizeArgumentIndex >= 0 ? process.argv[binarySizeArgumentIndex + 1] : NaN);
const fixturePath = fixturePathArgumentIndex >= 0 ? process.argv[fixturePathArgumentIndex + 1] : null;
const fixtureSha256 = fixtureShaArgumentIndex >= 0 ? process.argv[fixtureShaArgumentIndex + 1] : null;
const fixtureSizeBytes = Number(fixtureSizeArgumentIndex >= 0 ? process.argv[fixtureSizeArgumentIndex + 1] : NaN);
const endpoint = `http://127.0.0.1:${port}`;
const captureTimeoutMs = 20_000;

if (!Number.isInteger(port) || port < 1024 || port > 65535 || !outputPath || !projectVersion || !binaryPath || !binarySha256 || !fixturePath || !fixtureSha256) {
  console.error(JSON.stringify({ status: "failed", error: "Faltan argumentos de captura release." }));
  process.exit(1);
}

const baseline = JSON.parse(await readFile(baselinePath, "utf8"));
const captureCases = baseline.requiredCases;
let browser;
let status = "failed";
let error = null;
const capturedCases = [];

function sleep(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

function relativePath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

async function findShellPage() {
  const deadline = Date.now() + captureTimeoutMs;
  while (Date.now() < deadline) {
    const pages = browser.contexts().flatMap((context) => context.pages());
    for (const page of pages) {
      try {
        if (await page.locator("#app-title").count()) return page;
      } catch {
        // WebView2 may expose a provisional page while the shell commits.
      }
    }
    await sleep(250);
  }
  throw new Error("El binario release no expuso el shell de Columnia dentro del timeout.");
}

async function inspectShell(page, expectedForcedColors) {
  await page.bringToFront();
  const focusable = page.locator("a[href], button:not(:disabled), input, select, textarea").first();
  await focusable.focus();
  await page.keyboard.press("Tab");
  return page.evaluate((forcedColors) => {
    const active = document.activeElement;
    const activeStyle = active instanceof HTMLElement ? getComputedStyle(active) : null;
    const targets = [...document.querySelectorAll("button:not(:disabled), a[href], input, select, textarea")]
      .map((element) => {
        const bounds = element.getBoundingClientRect();
        return { width: bounds.width, height: bounds.height };
      });
    const minimumTargetSize = targets.reduce(
      (minimum, target) => Math.min(minimum, target.width, target.height),
      Number.POSITIVE_INFINITY,
    );
    const viewportWidth = window.innerWidth;
    const documentWidth = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
    return {
      forcedColors: window.matchMedia("(forced-colors: active)").matches,
      forcedColorsExpected: forcedColors === "active",
      landmarks: {
        main: document.querySelectorAll("main, [role='main']").length,
        navigation: document.querySelectorAll("nav, [role='navigation']").length,
        complementary: document.querySelectorAll("aside, [role='complementary']").length,
      },
      focus: {
        tagName: active?.tagName ?? null,
        outlineStyle: activeStyle?.outlineStyle ?? null,
        outlineWidth: activeStyle?.outlineWidth ?? null,
      },
      targetCount: targets.length,
      minimumTargetSize: Number.isFinite(minimumTargetSize) ? Number(minimumTargetSize.toFixed(2)) : null,
      viewportWidth,
      documentWidth,
      noHorizontalOverflow: documentWidth <= viewportWidth + 1,
    };
  }, expectedForcedColors);
}

try {
  browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const page = await findShellPage();
  const context = page.context();
  const client = await context.newCDPSession(page);

  for (const captureCase of captureCases) {
    await client.send("Emulation.setDeviceMetricsOverride", {
      width: captureCase.viewport.width,
      height: captureCase.viewport.height,
      deviceScaleFactor: captureCase.deviceScaleFactor,
      mobile: false,
      screenWidth: captureCase.viewport.width,
      screenHeight: captureCase.viewport.height,
    });
    await client.send("Emulation.setEmulatedMedia", {
      features: [{ name: "forced-colors", value: captureCase.forcedColors }],
    });
    await page.waitForTimeout(250);
    const inspection = await inspectShell(page, captureCase.forcedColors);
    const screenshotPath = join(dirname(outputPath), `${captureCase.name}.png`);
    await page.screenshot({ path: screenshotPath, fullPage: true });
    const screenshotSha256 = createHash("sha256").update(await readFile(screenshotPath)).digest("hex");
    const valid = inspection.forcedColors === inspection.forcedColorsExpected
      && inspection.landmarks.main === 1
      && inspection.landmarks.navigation === 1
      && inspection.landmarks.complementary === 1
      && inspection.focus.outlineStyle !== "none"
      && inspection.focus.outlineWidth !== "0px"
      && inspection.targetCount > 0
      && inspection.minimumTargetSize >= baseline.contract.minimumTargetSize
      && inspection.noHorizontalOverflow;
    capturedCases.push({
      name: captureCase.name,
      viewport: captureCase.viewport,
      deviceScaleFactor: captureCase.deviceScaleFactor,
      forcedColors: captureCase.forcedColors,
      screenshot: relativePath(screenshotPath),
      screenshotSha256,
      valid,
      inspection,
    });
    if (!valid) throw new Error(`El caso release ${captureCase.name} no cumple el contrato visual.`);
  }
  await client.send("Emulation.clearDeviceMetricsOverride");
  await client.send("Emulation.setEmulatedMedia", { features: [] });
  status = "passed";
} catch (captureError) {
  error = captureError instanceof Error ? captureError.message : String(captureError);
} finally {
  if (browser) await browser.close();
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify({
    schemaVersion: 1,
    captureVersion: 1,
    status,
    source: "tauri-release-binary",
    generatedAt: new Date().toISOString(),
    projectVersion,
    endpoint,
    binary: {
      path: binaryPath,
      sizeBytes: binarySizeBytes,
      sha256: binarySha256,
    },
    fixture: {
      path: fixturePath,
      sizeBytes: fixtureSizeBytes,
      sha256: fixtureSha256,
    },
    cases: capturedCases,
    evidenceDirectory: relativePath(dirname(outputPath)),
    error,
  }, null, 2)}\n`, "utf8");
}

if (status !== "passed") {
  console.error(`Evidencia release falló: ${error ?? "error desconocido"}. Sumario: ${relativePath(outputPath)}`);
  process.exitCode = 1;
} else {
  console.log(`Evidencia release aprobada: ${relativePath(dirname(outputPath))}`);
}
