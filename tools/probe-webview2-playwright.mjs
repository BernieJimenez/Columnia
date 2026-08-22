import { chromium } from "@playwright/test";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;

if (!Number.isInteger(port) || port < 1024 || port > 65535) {
  console.error(JSON.stringify({ status: "failed", error: "Invalid --port." }));
  process.exit(1);
}

const endpoint = `http://127.0.0.1:${port}`;
let browser;

try {
  browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const pages = browser.contexts().flatMap((context) => context.pages());
  const pageEvidence = [];

  for (const page of pages) {
    let appReady = false;
    let projectsReady = false;
    try {
      await page.waitForSelector("#app-title", { state: "attached", timeout: 10_000 });
      appReady = true;
    } catch {
      // A CDP endpoint can appear before Vite/React finishes mounting.
    }
    if (appReady) {
      try {
        await page.waitForSelector("#projects-title", { state: "attached", timeout: 5_000 });
        projectsReady = true;
      } catch {
        // ProjectsPanel may be unavailable when the native runtime is still loading.
      }
    }
    pageEvidence.push(
      await page.evaluate(() => ({
        url: window.location.href,
        title: document.title,
        readyState: document.readyState,
        hasAppTitle: Boolean(document.querySelector("#app-title")),
        hasProjectsTitle: Boolean(document.querySelector("#projects-title")),
        interactiveElementCount: document.querySelectorAll("button, input, select, textarea, a").length,
      })),
    );
    pageEvidence.at(-1).appReady = appReady;
    pageEvidence.at(-1).projectsReady = projectsReady;
  }

  const domReady = pageEvidence.some((page) => page.appReady);

  console.log(
    JSON.stringify({
      status: domReady ? "passed" : "not_ready",
      endpoint,
      contextCount: browser.contexts().length,
      pageCount: pages.length,
      pages: pageEvidence,
    }),
  );
} catch (error) {
  console.error(
    JSON.stringify({
      status: "failed",
      endpoint,
      error: error instanceof Error ? error.message : String(error),
    }),
  );
  process.exitCode = 1;
} finally {
  if (browser) {
    await browser.close();
  }
}
