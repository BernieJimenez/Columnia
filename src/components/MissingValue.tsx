/**
 * A missing cell. Sighted users see a styled "∅ nulo" (the glossary term for a
 * null cell); screen readers hear
 * "valor ausente", so it is not confused with the literal text "null", which
 * Columnia itself treats as a sentinel to normalize.
 */
export function MissingValue() {
  return (
    <span className="null-value" title="Sin valor">
      <span aria-hidden="true">∅ </span>
      <span aria-hidden="true">nulo</span>
      <span className="visually-hidden">valor ausente</span>
    </span>
  );
}
