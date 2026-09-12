import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src", "workflow-styles.css"), "utf8");

describe("estilos de los espacios de trabajo", () => {
  it("permite envolver los estados sin convertirlos en tarjetas o controles", () => {
    expect(styles).toMatch(/\.workspace-list li\s*\{[^}]*min-width:\s*0[^}]*overflow-wrap:\s*anywhere/s);
    expect(styles).toMatch(/\.workspace-list li\s*\{[^}]*grid-template-columns:\s*minmax\(0,/s);
    expect(styles).toMatch(/\.workspace-list li\[aria-current="true"\] strong/);
  });

  it("incluye el nuevo bloque en la disposición compacta y al 200 %", () => {
    expect(styles).toMatch(/html\[data-columnia-zoom="2"\] \.workspace-list\s*\{[^}]*grid-area:\s*workspaces/s);
    expect(styles).toMatch(/html\[data-columnia-zoom="2"\] \.sidebar__tools\s*\{[^}]*grid-area:\s*utilities/s);
    expect(styles).toMatch(/@media \(max-width: 900px\)[\s\S]*\.sidebar\s*\{[^}]*workspaces workspaces/s);
    expect(styles).toMatch(/@media \(max-width: 700px\)[\s\S]*\.sidebar\s*\{[^}]*workspaces workspaces/s);
    expect(styles).toMatch(/@media \(max-width: 420px\)[\s\S]*\.workspace-list ul\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/s);
  });

  it("permite desplazar el sidebar cuando la ventana no tiene altura suficiente", () => {
    expect(styles).toMatch(/@media \(min-width: 901px\) and \(max-height: 900px\)[\s\S]*\.sidebar\s*\{[^}]*overflow-y:\s*auto/s);
  });
});
