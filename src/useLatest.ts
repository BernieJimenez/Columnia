import { useLayoutEffect, useRef, type RefObject } from "react";

/**
 * COD-06: a ref that always holds the last rendered `value`, for async work
 * that must compare against the current inputs. It is updated after the
 * render commits, never while rendering.
 */
export function useLatest<T>(value: T): RefObject<T> {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  });
  return ref;
}
