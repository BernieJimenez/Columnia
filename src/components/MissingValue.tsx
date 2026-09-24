/**
 * A missing cell. Sighted users see a styled "null"; screen readers hear
 * "valor ausente", so it is not confused with the literal text "null", which
 * Columnia itself treats as a sentinel to normalize.
 */
export function MissingValue() {
  return (
    <span className="null-value">
      <span aria-hidden="true">null</span>
      <span className="visually-hidden">valor ausente</span>
    </span>
  );
}
