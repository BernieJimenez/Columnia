import assert from "node:assert/strict";
import test from "node:test";

import { changelogSectionNotes } from "./changelog-notes.mjs";

const changelog = [
  "# Changelog",
  "",
  "## [Unreleased]",
  "",
  "### Corregido",
  "",
  "- Aún sin publicar.",
  "",
  "## [1.26.0] - 2026-10-01",
  "",
  "### Añadido",
  "",
  "- Lo de la 1.26.0.",
  "",
  "## [1.2.6] - 2026-01-01",
  "",
  "- Una versión que casaría con un punto sin escapar.",
  "",
].join("\n");

test("toma exactamente la sección de la versión, no [Unreleased] (OPS-12)", () => {
  assert.equal(changelogSectionNotes(changelog, "1.26.0"), "Añadido\n\n- Lo de la 1.26.0.");
  assert.equal(changelogSectionNotes(changelog, "1.2.6"), "- Una versión que casaría con un punto sin escapar.");
  assert.equal(changelogSectionNotes(changelog, "1.26"), null);
  assert.equal(changelogSectionNotes(changelog, "9.9.9"), null);
});
