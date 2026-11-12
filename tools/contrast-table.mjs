// UI-02: the contrast of each theme, computed from the tokens in src/styles.css
// (their only source, plus the base sidebar rule of src/workflow-styles.css) and written into DESIGN.md between two markers.
//   node tools/contrast-table.mjs          rewrites the table in DESIGN.md
//   node tools/contrast-table.mjs --check  fails if DESIGN.md is out of date
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const START = "<!-- contrast-table:start -->";
const END = "<!-- contrast-table:end -->";

/** Text and edge colours against the surfaces they sit on, with their WCAG minimum. */
const PAIRS = [
  ["text-primary", "canvas", 4.5],
  ["text-primary", "surface", 4.5],
  ["text-secondary", "surface", 4.5],
  ["accent", "surface", 4.5],
  ["danger", "surface", 4.5],
  ["control-border", "surface", 3],
  ["control-border", "canvas", 3],
];

/**
 * What the sidebar paints, measured only on the sidebar: the focus ring against its
 * surface (WCAG 1.4.11) and the text and accent of the rail, on the rail itself and
 * on the highlighted current step (`--surface` inside `.app-sidebar`).
 */
const SIDEBAR_PAIRS = [
  ["text-primary", "sidebar-surface", 4.5],
  ["text-secondary", "sidebar-surface", 4.5],
  ["accent", "sidebar-surface", 4.5],
  ["text-primary", "surface", 4.5],
  ["text-secondary", "surface", 4.5],
  ["focus-ring", "sidebar-surface", 3],
];
const SIDEBAR_THEMES = ["light", "dark", "paper", "ocean", "slate"];

function luminance(hex) {
  const channels = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255)
    .map((value) => (value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

export function contrastRatio(left, right) {
  const [high, low] = [luminance(left), luminance(right)].sort((a, b) => b - a);
  return (high + 0.05) / (low + 0.05);
}

/** Every theme block of the stylesheet that defines colour tokens. */
export function themeBlocks(css) {
  return [...css.matchAll(/(?:^|\n)\s*(:root(?:\[data-[a-z-]+="[a-z]+"\])?(?: \.app-sidebar)?)\s*\{([^}]*)\}/g)]
    .map(([, selector, body]) => ({
      selector,
      tokens: Object.fromEntries([...body.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/gi)].map(([, name, value]) => [name, value.toLowerCase()])),
    }))
    .filter(({ tokens }) => tokens["control-border"]);
}

function tokensOf(body) {
  return Object.fromEntries([...body.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/gi)].map(([, name, value]) => [name, value.toLowerCase()]));
}

/**
 * What the sidebar really paints in each theme: the base `.app-sidebar` rule of
 * workflow-styles.css (every sidebar is dark) overlaid with the theme's own
 * `:root[data-theme] .app-sidebar` block of styles.css, if it has one.
 */
export function sidebarBlocks(css, workflowCss) {
  const base = tokensOf(workflowCss.match(/(?:^|\n)\.app-sidebar\s*\{([^}]*)\}/)?.[1] ?? "");
  return SIDEBAR_THEMES.map((theme) => {
    const own = css.match(new RegExp(String.raw`(?:^|\n)\s*:root\[data-theme="${theme}"\] \.app-sidebar\s*\{([^}]*)\}`))?.[1] ?? "";
    return { selector: `.app-sidebar (${theme})`, tokens: { ...base, ...tokensOf(own) } };
  });
}

/** The Markdown table: one row per theme and pair that both colours define. */
export function contrastTable(css, workflowCss = "") {
  const rows = ["| Tema | Color | Fondo | Contraste | Mínimo |", "| --- | --- | --- | ---: | ---: |"];
  for (const { selector, tokens } of themeBlocks(css)) {
    for (const [foreground, background, minimum] of PAIRS) {
      if (!tokens[foreground] || !tokens[background]) continue;
      const ratio = contrastRatio(tokens[foreground], tokens[background]);
      rows.push(`| \`${selector}\` | \`--${foreground}\` ${tokens[foreground]} | \`--${background}\` ${tokens[background]} | ${ratio.toFixed(2)}:1 | ${minimum}:1 |`);
    }
  }
  for (const { selector, tokens } of sidebarBlocks(css, workflowCss)) {
    for (const [foreground, background, minimum] of SIDEBAR_PAIRS) {
      if (!tokens[foreground] || !tokens[background]) continue;
      const ratio = contrastRatio(tokens[foreground], tokens[background]);
      rows.push(`| \`${selector}\` | \`--${foreground}\` ${tokens[foreground]} | \`--${background}\` ${tokens[background]} | ${ratio.toFixed(2)}:1 | ${minimum}:1 |`);
    }
  }
  return rows.join("\n");
}

export function withContrastTable(design, css, workflowCss = "") {
  const start = design.indexOf(START);
  const end = design.indexOf(END);
  if (start < 0 || end < start) throw new Error(`DESIGN.md no tiene los marcadores ${START} y ${END}.`);
  return `${design.slice(0, start + START.length)}\n${contrastTable(css, workflowCss)}\n${design.slice(end)}`;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const css = readFileSync(join(projectRoot, "src", "styles.css"), "utf8");
  const workflowCss = readFileSync(join(projectRoot, "src", "workflow-styles.css"), "utf8");
  const designPath = join(projectRoot, "DESIGN.md");
  const design = readFileSync(designPath, "utf8").replace(/\r\n/g, "\n");
  const updated = withContrastTable(design, css, workflowCss);
  if (process.argv.includes("--check")) {
    if (updated !== design) {
      console.error("La tabla de contraste de DESIGN.md no coincide con src/styles.css. Ejecuta node tools/contrast-table.mjs.");
      process.exitCode = 1;
    } else {
      console.log("Tabla de contraste de DESIGN.md al día.");
    }
  } else {
    writeFileSync(designPath, updated, "utf8");
    console.log("Tabla de contraste de DESIGN.md actualizada.");
  }
}
