import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";

import { ReviewTabList, type ReviewTab } from "./ReviewTabList";

afterEach(cleanup);

function ReviewTabsHarness() {
  const [activeTab, setActiveTab] = useState<ReviewTab>("diagnosis");
  return (
    <>
      <ReviewTabList activeTab={activeTab} onTabChange={setActiveTab} />
      <div
        id={`review-${activeTab}-panel`}
        role="tabpanel"
        aria-labelledby={`review-${activeTab}-tab`}
      >
        Panel activo
      </div>
    </>
  );
}

describe("ReviewTabList", () => {
  it("expone relaciones ARIA y navegación circular con flechas, Home y End", () => {
    render(<ReviewTabsHarness />);
    const diagnosis = screen.getByRole("tab", { name: "Diagnóstico" });
    const preview = screen.getByRole("tab", { name: "Vista previa" });

    expect(diagnosis).toHaveAttribute("aria-selected", "true");
    expect(diagnosis).toHaveAttribute("aria-controls", "review-diagnosis-panel");
    expect(diagnosis).toHaveAttribute("tabindex", "0");
    expect(preview).toHaveAttribute("tabindex", "-1");

    diagnosis.focus();
    fireEvent.keyDown(diagnosis, { key: "ArrowLeft" });
    expect(preview).toHaveFocus();
    expect(preview).toHaveAttribute("aria-selected", "true");

    fireEvent.keyDown(preview, { key: "Home" });
    expect(diagnosis).toHaveFocus();
    fireEvent.keyDown(diagnosis, { key: "End" });
    expect(preview).toHaveFocus();
    expect(screen.getByRole("tabpanel", { name: "Vista previa" })).toBeInTheDocument();
  });
});
