import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const shell = readFileSync(join(process.cwd(), "src", "shell.css"), "utf8");

describe("estilos del shell", () => {
  it("la barra lateral se vuelve una franja sobre el trabajo en la vista compacta y al 200 % (UX-02)", () => {
    expect(shell).toMatch(/@media \(max-width: 900px\)[\s\S]*\.app-shell\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/);
    expect(shell).toMatch(/@media \(max-width: 900px\)[\s\S]*\.app-sidebar\s*\{[^}]*position:\s*static/);
    expect(shell).toMatch(/html\[data-columnia-zoom="2"\] \.app-sidebar\s*\{[^}]*height:\s*auto/);
    expect(shell).toMatch(/html\[data-columnia-zoom="2"\] \.app-sidebar \.sidebar__tools\s*\{[^}]*grid-area:\s*tools/);
  });

  it("los nombres de etapa pueden partirse sin crear scroll horizontal", () => {
    expect(shell).toMatch(/\.flow-nav__copy strong\s*\{[^}]*overflow-wrap:\s*anywhere/);
    expect(shell).toMatch(/\.dataset-bar__name\s*\{[^}]*text-overflow:\s*ellipsis/);
  });

  it("la barra superior solo queda fija si hay altura, y el foco no se esconde debajo", () => {
    expect(shell).toMatch(/@media \(min-width: 901px\) and \(min-height: 701px\)\s*\{\s*html\s*\{\s*scroll-padding-top:/);
  });

  it("el estado nunca depende solo del color del punto", () => {
    expect(shell).toMatch(/\.engine-status::before,\s*\.dataset-save__status::before\s*\{/);
    expect(shell).toContain("/* The dot is the visual cue; the text always names the state. */");
  });
});
