import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), a[href], input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), summary, [tabindex]:not([tabindex="-1"])';

function isAvailableForKeyboard(element: HTMLElement, panel: HTMLElement): boolean {
  if (element.matches(":disabled")) return false;
  if (element.tagName === "SUMMARY") {
    const parent = element.parentElement;
    const firstSummary = parent?.tagName === "DETAILS"
      ? Array.from(parent.children).find((child) => child.tagName === "SUMMARY")
      : null;
    if (firstSummary !== element) return false;
  }

  let current: HTMLElement | null = element;
  while (current) {
    if (current.hidden || current.hasAttribute("inert") || current.getAttribute("aria-hidden") === "true") {
      return false;
    }
    if (current.tagName === "DETAILS" && !current.hasAttribute("open")) {
      const summary = Array.from(current.children).find((child) => child.tagName === "SUMMARY");
      if (!summary?.contains(element)) return false;
    }

    const style = window.getComputedStyle(current);
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
      return false;
    }
    if (current === panel) break;
    current = current.parentElement;
  }

  return true;
}

function getFocusableElements(panel: HTMLElement): HTMLElement[] {
  return Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => isAvailableForKeyboard(element, panel))
    .sort((left, right) => {
      const position = left.compareDocumentPosition(right);
      if (position & Node.DOCUMENT_POSITION_FOLLOWING) return -1;
      if (position & Node.DOCUMENT_POSITION_PRECEDING) return 1;
      return 0;
    });
}

interface ModalDialogProps {
  role: "dialog" | "alertdialog";
  labelledBy: string;
  describedBy?: string;
  onDismiss: () => void;
  children: ReactNode;
}

export function ModalDialog({
  role,
  labelledBy,
  describedBy,
  onDismiss,
  children,
}: ModalDialogProps) {
  const panelRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const panel = panelRef.current;
    if (panel && !panel.open) {
      if (typeof panel.showModal === "function") {
        panel.showModal();
      } else {
        // Keep component tests and older embedded browsers usable while the
        // production WebView uses the native modal behavior.
        panel.setAttribute("open", "");
      }
    }
    const firstFocusable = panel ? getFocusableElements(panel)[0] : undefined;
    (firstFocusable ?? panel)?.focus();

    return () => {
      if (panel?.open) {
        if (typeof panel.close === "function") {
          panel.close();
        } else {
          panel.removeAttribute("open");
        }
      }
      previouslyFocused?.focus();
    };
  }, []);

  function keepFocusInside(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onDismiss();
      return;
    }
    if (event.key !== "Tab") return;

    const panel = panelRef.current;
    if (!panel) return;
    const focusable = getFocusableElements(panel);
    const first = focusable[0];
    const last = focusable.at(-1);
    if (!first || !last) {
      event.preventDefault();
      panel.focus();
      return;
    }

    const active = event.target instanceof HTMLElement ? event.target : document.activeElement;
    const activeIndex = active instanceof HTMLElement ? focusable.indexOf(active) : -1;
    const nextIndex = activeIndex < 0
      ? 0
      : (activeIndex + (event.shiftKey ? -1 : 1) + focusable.length) % focusable.length;
    event.preventDefault();
    focusable[nextIndex].focus();
  }

  return (
    <dialog
      ref={panelRef}
      className="sheet-dialog"
      role={role}
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      tabIndex={-1}
      onKeyDown={keepFocusInside}
      onCancel={(event) => event.preventDefault()}
    >
      <div className="sheet-dialog__panel">{children}</div>
    </dialog>
  );
}
