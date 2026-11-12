import { useEffect, type RefObject } from "react";

const FOCUSABLE = "a[href], button, input, select, textarea, summary, [tabindex]:not([tabindex='-1'])";

/**
 * AUD-063: a floating `<details>` panel closes with Esc and with a click
 * outside it. Esc returns focus to the panel's `summary`; a click outside only
 * does so when it landed on something that cannot take focus and the focus was
 * still inside the panel, so it never steals focus from the control clicked.
 */
export function useDismissablePanel(
  open: boolean,
  onClose: () => void,
  detailsRef: RefObject<HTMLDetailsElement | null>,
): void {
  useEffect(() => {
    if (!open) return;
    const focusSummary = () => detailsRef.current?.querySelector("summary")?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      onClose();
      focusSummary();
    };
    const onClick = (event: MouseEvent) => {
      const details = detailsRef.current;
      const target = event.target;
      if (!details || !(target instanceof Element) || details.contains(target)) return;
      const focusWasInside = details.contains(document.activeElement);
      onClose();
      if (focusWasInside && !target.closest(FOCUSABLE)) focusSummary();
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("click", onClick);
    };
  }, [open, onClose, detailsRef]);
}
