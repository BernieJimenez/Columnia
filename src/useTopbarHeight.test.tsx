import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTopbarHeight } from "./useTopbarHeight";

function Bar() {
  const ref = useTopbarHeight();
  return <header ref={ref}>barra</header>;
}

describe("useTopbarHeight (AUD-058)", () => {
  let callback: (() => void) | null = null;
  const observe = vi.fn();
  const disconnect = vi.fn();
  let height = 120;

  beforeEach(() => {
    callback = null;
    observe.mockClear();
    disconnect.mockClear();
    height = 120;
    vi.stubGlobal("ResizeObserver", class {
      constructor(cb: () => void) { callback = cb; }
      observe = observe;
      disconnect = disconnect;
      unobserve = vi.fn();
    });
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      () => ({ height }) as DOMRect,
    );
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.documentElement.style.removeProperty("--topbar-height");
  });

  it("fija --topbar-height, la actualiza al cambiar y se desconecta al desmontar", () => {
    const { unmount } = render(<Bar />);
    const root = document.documentElement;
    expect(root.style.getPropertyValue("--topbar-height")).toBe("120px");
    expect(observe).toHaveBeenCalledTimes(1);
    height = 143.2;
    callback?.();
    expect(root.style.getPropertyValue("--topbar-height")).toBe("144px");
    unmount();
    expect(disconnect).toHaveBeenCalled();
    expect(root.style.getPropertyValue("--topbar-height")).toBe("");
  });
});
