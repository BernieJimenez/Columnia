import { useCallback, useRef } from "react";

/**
 * AUD-058: publishes the real height of the top bar as `--topbar-height` on
 * the root element, so the scroll padding and the sticky side panel do not
 * rely on a guessed 88 px (a long file name makes the bar taller).
 * Use the returned callback as the `ref` of the bar; it disconnects on unmount.
 */
export function useTopbarHeight(): (element: HTMLElement | null) => void {
  const observerRef = useRef<ResizeObserver | null>(null);
  return useCallback((element: HTMLElement | null) => {
    observerRef.current?.disconnect();
    observerRef.current = null;
    const root = document.documentElement;
    if (!element) {
      root.style.removeProperty("--topbar-height");
      return;
    }
    const publish = () => {
      const height = Math.ceil(element.getBoundingClientRect().height);
      if (height > 0) root.style.setProperty("--topbar-height", `${height}px`);
    };
    publish();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(publish);
    observer.observe(element);
    observerRef.current = observer;
  }, []);
}
