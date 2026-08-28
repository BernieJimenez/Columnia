import { chromium } from "@playwright/test";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;
const probeTimeoutMs = 15_000;
const pollIntervalMs = 250;
const firstRenderBudgetMs = 3_000;

if (!Number.isInteger(port) || port < 1024 || port > 65535) {
  console.error(JSON.stringify({ status: "failed", error: "Invalid --port." }));
  process.exit(1);
}

const endpoint = `http://127.0.0.1:${port}`;
let browser;

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function isProvisionalUrl(url) {
  return /^about:blank(?:#.*)?$/i.test(url);
}

async function readPageShell(page) {
  const url = page.url();
  const provisional = isProvisionalUrl(url);
  try {
    return await page.evaluate(({ pageUrl, isProvisional }) => {
      const has = (selector) => Boolean(document.querySelector(selector));
      return {
        url: pageUrl,
        provisional: isProvisional,
        title: document.title,
        readyState: document.readyState,
        hasAppTitle: has("#app-title"),
        hasProjectsTitle: has("#projects-title"),
        interactiveElementCount: document.querySelectorAll("button, input, select, textarea, a").length,
      };
    }, { pageUrl: url, isProvisional: provisional });
  } catch (error) {
    return {
      url,
      provisional,
      title: null,
      readyState: null,
      hasAppTitle: false,
      hasProjectsTitle: false,
      interactiveElementCount: 0,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

async function readFirstRender(page) {
  return page.evaluate((budgetMs) => {
    const marks = performance
      .getEntriesByName("columnia:app-render")
      .filter((entry) => entry.entryType === "mark");
    const bootstrapMarks = performance
      .getEntriesByName("columnia:app-bootstrap")
      .filter((entry) => entry.entryType === "mark");
    const first = marks[0];
    const bootstrap = bootstrapMarks[0];
    const absoluteStartTime = first?.startTime ?? null;
    const startTime = first && bootstrap
      ? Math.max(0, first.startTime - bootstrap.startTime)
      : absoluteStartTime;
    return {
      markName: "columnia:app-render",
      found: Boolean(first),
      markCount: marks.length,
      bootstrapMarkName: "columnia:app-bootstrap",
      bootstrapFound: Boolean(bootstrap),
      bootstrapMarkCount: bootstrapMarks.length,
      bootstrapStartTime: bootstrap?.startTime ?? null,
      absoluteStartTime,
      startTime,
      withinBudget: typeof startTime === "number" && startTime < budgetMs,
      budgetMs,
    };
  }, firstRenderBudgetMs);
}

async function readLandmarks(page) {
  return page.evaluate(() => {
    const count = (selector) => document.querySelectorAll(selector).length;
    const counts = {
      main: count("main, [role='main']"),
      navigation: count("nav, [role='navigation']"),
      complementary: count("aside, [role='complementary']"),
      banner: count("body > header, [role='banner']"),
      contentInfo: count("body > footer, [role='contentinfo']"),
      region: count("[role='region']"),
    };
    return {
      ...counts,
      valid: counts.main === 1 && counts.navigation === 1 && counts.complementary === 1,
    };
  });
}

async function readKeyboardFocus(page) {
  try {
    await page.evaluate(() => {
      const active = document.activeElement;
      if (active instanceof HTMLElement) active.blur();
    });
    await page.keyboard.press("Tab");
    const afterTab = await page.evaluate(() => {
      const skipLink = document.querySelector("a.skip-link");
      const active = document.activeElement;
      const style = skipLink instanceof HTMLElement ? getComputedStyle(skipLink) : null;
      return {
        skipLinkFound: Boolean(skipLink),
        skipLinkFocused: skipLink === active,
        skipLinkVisible: Boolean(
          skipLink instanceof HTMLElement &&
          style &&
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          style.transform !== "none",
        ),
      };
    });

    let afterEnter = {
      mainFound: false,
      mainFocused: false,
    };
    if (afterTab.skipLinkFocused) {
      await page.keyboard.press("Enter");
      afterEnter = await page.evaluate(() => {
        const main = document.querySelector("main#main-content");
        return {
          mainFound: Boolean(main),
          mainFocused: main === document.activeElement,
        };
      });
    }

    return {
      tested: true,
      ...afterTab,
      ...afterEnter,
      valid: afterTab.skipLinkFocused && afterEnter.mainFocused,
    };
  } catch (error) {
    return {
      tested: true,
      skipLinkFound: false,
      skipLinkFocused: false,
      skipLinkVisible: false,
      mainFound: false,
      mainFocused: false,
      valid: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

async function inspectPage(page) {
  const shell = await readPageShell(page);
  if (!shell.hasAppTitle) {
    return {
      status: "not_ready",
      phase: shell.provisional ? "provisional_target" : "waiting_for_shell",
      ...shell,
    };
  }

  const firstRender = await readFirstRender(page);
  const landmarks = await readLandmarks(page);
  if (!firstRender.found) {
    return {
      status: "not_ready",
      phase: "waiting_for_first_render",
      ...shell,
      firstRender,
      landmarks,
    };
  }

  const focus = await readKeyboardFocus(page);
  const functionalStatus = landmarks.valid && focus.valid ? "passed" : "failed";
  const performanceStatus = firstRender.withinBudget ? "passed" : "failed";
  const status = functionalStatus === "passed" && performanceStatus === "passed" ? "passed" : "failed";
  return {
    status,
    phase: status === "passed" ? "ready" : functionalStatus === "failed" ? "shell_contract_failed" : "first_render_budget_failed",
    functionalStatus,
    performanceStatus,
    ...shell,
    firstRender,
    landmarks,
    focus,
  };
}

function snapshotResult(status, pages, extra = {}) {
  return {
    status,
    endpoint,
    contextCount: browser?.contexts().length ?? 0,
    pageCount: pages.length,
    pages,
    checks: {
      firstRenderMark: "columnia:app-render",
      firstRenderBudgetMs,
      landmarks: ["main", "navigation", "complementary"],
      keyboardFocus: ["a.skip-link", "main#main-content"],
    },
    ...extra,
  };
}

try {
  browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const deadline = Date.now() + probeTimeoutMs;
  let lastPages = [];
  let sawMountedApp = false;

  while (Date.now() < deadline) {
    const pages = browser.contexts().flatMap((context) => context.pages());
    const pageEvidence = [];
    for (const page of pages) {
      const evidence = await inspectPage(page);
      pageEvidence.push(evidence);
      sawMountedApp ||= evidence.hasAppTitle;
      if (evidence.status === "passed") {
        console.log(JSON.stringify(snapshotResult("passed", pageEvidence)));
        process.exitCode = 0;
        throw new Error("__probe_complete__");
      }
      if (evidence.status === "failed") {
        console.log(JSON.stringify(snapshotResult("failed", pageEvidence, {
          error: evidence.performanceStatus === "failed"
            ? "El primer render del shell nativo excede el presupuesto de 3 s."
            : "El shell nativo no cumple el contrato de landmarks o foco.",
        })));
        process.exitCode = 1;
        throw new Error("__probe_complete__");
      }
    }
    lastPages = pageEvidence;
    await sleep(pollIntervalMs);
  }

  const status = sawMountedApp ? "failed" : "not_ready";
  console.log(JSON.stringify(snapshotResult(status, lastPages, {
    phase: sawMountedApp ? "mounted_shell_timeout" : "provisional_target_timeout",
    error: sawMountedApp
      ? "El shell montó, pero no completó la métrica o los contratos antes del timeout."
      : "El endpoint CDP solo expuso targets provisionales o sin el shell de Columnia.",
  })));
  process.exitCode = status === "not_ready" ? 0 : 1;
} catch (error) {
  if (error instanceof Error && error.message === "__probe_complete__") {
    // The structured result was already emitted above.
  } else {
    console.error(JSON.stringify({
      status: "failed",
      endpoint,
      error: error instanceof Error ? error.message : String(error),
    }));
    process.exitCode = 1;
  }
} finally {
  if (browser) {
    await browser.close();
  }
}
