export type ReviewTab = "diagnosis" | "preview";

interface ReviewTabListProps {
  activeTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
}

const tabs: ReadonlyArray<{ id: ReviewTab; label: string }> = [
  { id: "diagnosis", label: "Diagnóstico" },
  { id: "preview", label: "Vista previa" },
];

export function ReviewTabList({ activeTab, onTabChange }: ReviewTabListProps) {
  function activate(tab: ReviewTab) {
    onTabChange(tab);
    document.getElementById(`review-${tab}-tab`)?.focus();
  }

  return (
    <div className="stage-tabs" role="tablist" aria-label="Vistas de revisión">
      {tabs.map((tab, index) => (
        <button
          key={tab.id}
          id={`review-${tab.id}-tab`}
          type="button"
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={`review-${tab.id}-panel`}
          tabIndex={activeTab === tab.id ? 0 : -1}
          className={activeTab === tab.id ? "stage-tab--active" : undefined}
          onClick={() => onTabChange(tab.id)}
          onKeyDown={(event) => {
            if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const direction = event.key === "ArrowRight" ? 1 : -1;
            const target = event.key === "Home"
              ? tabs[0]
              : event.key === "End"
                ? tabs.at(-1)
                : tabs[(index + direction + tabs.length) % tabs.length];
            if (target) activate(target.id);
          }}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
