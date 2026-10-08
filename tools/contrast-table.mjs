// UI-02: the contrast of each theme, computed from the tokens in src/styles.css
// (their only source) and written into DESIGN.md between two markers.
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
  return [...css.matchAll(/(?:^|\n)\s*(:root(?:\[data-[a-z-]+="[a-z]+"\])?(?: \.sidebar)?)\s*\{([^}]*)\}/g)]
    .map(([, selector, body]) => ({
      selector,
      tokens: Object.fromEntries([...body.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/gi)].map(([, name, value]) => [name, value.toLowerCase()])),
    }))
    .filter(({ tokens }) => tokens["control-border"]);
}

/** The Markdown table: one row per theme and pair that both colours define. */
export function contrastTable(css) {
  const rows = ["| Tema | Color | Fondo | Contraste | Mínimo |", "| --- | --- | --- | ---: | ---: |"];
  for (const { selector, tokens } of themeBlocks(css)) {
    for (const [foreground, background, minimum] of PAIRS) {
      if (!tokens[foreground] || !tokens[background]) continue;
      const ratio = contrastRatio(tokens[foreground], tokens[background]);
      rows.push(`| \`${selector}\` | \`--${foreground}\` ${tokens[foreground]} | \`--${background}\` ${tokens[background]} | ${ratio.toFixed(2)}:1 | ${minimum}:1 |`);
    }
  }
  return rows.join("\n");
}

export function withContrastTable(design, css) {
  const start = design.indexOf(START);
  const end = design.indexOf(END);
  if (start < 0 || end < start) throw new Error(`DESIGN.md no tiene los marcadores ${START} y ${END}.`);
  return `${design.slice(0, start + START.length)}\n${contrastTable(css)}\n${design.slice(end)}`;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const css = readFileSync(join(projectRoot, "src", "styles.css"), "utf8");
  const designPath = join(projectRoot, "DESIGN.md");
  const design = readFileSync(designPath, "utf8").replace(/\r\n/g, "\n");
  const updated = withContrastTable(design, css);
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
