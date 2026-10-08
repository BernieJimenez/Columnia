import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";

/**
 * QA-56: the axe-core rules for WCAG 2.2 A/AA on the current page, as one line
 * per violation (rule, impact and the first affected node) so a failure says
 * where to look. Colour contrast is measured by theme-contrast.spec.ts with
 * the app's own rule for SVG text, so axe leaves it out.
 */
export async function axeViolations(page: Page, context: string): Promise<string[]> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .disableRules(["color-contrast"])
    .analyze();
  return results.violations.map((violation) => {
    const target = violation.nodes[0]?.target.join(" ") ?? "";
    return `${context}: ${violation.id} (${violation.impact ?? "sin impacto"}) en ${target} — ${violation.help}`;
  });
}
