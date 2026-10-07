/**
 * OPS-12: the notes of exactly `## [version]` in a Keep a Changelog file,
 * without its `###` markers, or null when that section does not exist. The
 * first `##` section is usually `[Unreleased]`, not the version published.
 */
export function changelogSectionNotes(changelog, version) {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const heading = new RegExp(`^##\\s+\\[${escaped}\\](?:\\s.*)?$`, "m");
  const match = heading.exec(changelog);
  if (!match) return null;
  const rest = changelog.slice(match.index + match[0].length);
  const next = rest.search(/^##\s/m);
  return (next < 0 ? rest : rest.slice(0, next)).replace(/^###\s+/gm, "").trim();
}
