import { useState, type ReactNode } from "react";

export interface SideTab {
  id: string;
  label: string;
  /** A count worth seeing before the tab is opened, such as open comments. */
  badge?: number;
  content: ReactNode;
}

const KEY = "rirote-vision.side-tab";

function remembered(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

interface SidePanelProps {
  tabs: SideTab[];
  /** A tab to bring forward, e.g. Inspect when a clip is chosen. */
  focus?: string | null;
}

/** The editor's right-hand panel: Inspect, Media, Project and the rest, one at a time. */
export function SidePanel({ tabs, focus }: SidePanelProps) {
  const [chosen, setChosen] = useState<string>(() => remembered() ?? tabs[0]?.id ?? "");
  const [lastFocus, setLastFocus] = useState(focus ?? null);
  // A new focus request wins over the remembered tab, once.
  if ((focus ?? null) !== lastFocus) {
    setLastFocus(focus ?? null);
    if (focus) setChosen(focus);
  }
  const current = tabs.find((tab) => tab.id === chosen) ?? tabs[0];
  const choose = (id: string) => {
    setChosen(id);
    try {
      localStorage.setItem(KEY, id);
    } catch {
      // Remembering the tab is a convenience only.
    }
  };

  return (
    <aside className="pane sidebar side" aria-label="Inspector">
      <div className="side__tabs" role="tablist">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={`side-tab-${tab.id}`}
            aria-selected={tab.id === current?.id}
            aria-controls="side-panel"
            onClick={() => choose(tab.id)}
          >
            {tab.label}
            {tab.badge ? <span className="side__badge">{tab.badge}</span> : null}
          </button>
        ))}
      </div>
      <div className="side__body" role="tabpanel" id="side-panel" aria-labelledby={`side-tab-${current?.id}`}>
        {current?.content}
      </div>
    </aside>
  );
}
