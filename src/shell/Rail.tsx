import type { IconName } from "../ui/Icon";
import { Icon } from "../ui/Icon";

export type View = "home" | "editor" | "references" | "brands" | "settings";

interface RailProps {
  view: View;
  /** The editor is only reachable while a project is open. */
  hasProject: boolean;
  /** Jobs queued or running in the open project. */
  activeJobs: number;
  onNavigate: (view: View) => void;
  onNew: () => void;
  onPalette: () => void;
}

interface Destination {
  view: View;
  icon: IconName;
  label: string;
  keys?: string;
}

const MAIN: Destination[] = [
  { view: "home", icon: "home", label: "Home", keys: "⌘1" },
  { view: "editor", icon: "editor", label: "Editor", keys: "⌘2" },
  { view: "references", icon: "reference", label: "References" },
  { view: "brands", icon: "brand", label: "Brand kits" },
];

/** The column of destinations down the window's left edge. */
export function Rail(props: RailProps) {
  const item = (destination: Destination) => {
    const disabled = destination.view === "editor" && !props.hasProject;
    const label = destination.keys ? `${destination.label} (${destination.keys})` : destination.label;
    return (
      <button
        key={destination.view}
        type="button"
        className="rail__item"
        aria-label={destination.label}
        title={disabled ? "Open a project to edit it" : label}
        aria-current={props.view === destination.view ? "page" : undefined}
        disabled={disabled}
        onClick={() => props.onNavigate(destination.view)}
      >
        <Icon name={destination.icon} size={18} />
      </button>
    );
  };

  return (
    <nav className="rail" aria-label="Navigation">
      <div className="rail__drag" data-tauri-drag-region />
      <span className="rail__mark" aria-hidden="true">
        RV
      </span>
      <button type="button" className="rail__item" aria-label="Search projects and commands" title="Search (⌘K)" onClick={props.onPalette}>
        <Icon name="search" size={18} />
      </button>
      <button type="button" className="rail__item" aria-label="New video" title="New video (⌘N)" onClick={props.onNew}>
        <Icon name="plus" size={18} />
      </button>
      <span className="rail__rule" aria-hidden="true" />
      {MAIN.map(item)}
      <span className="rail__spacer" />
      {props.activeJobs > 0 ? (
        <span className="rail__jobs" role="status" title={`${props.activeJobs} job(s) running`}>
          <Icon name="jobs" size={18} />
          <span className="rail__badge">{props.activeJobs}</span>
        </span>
      ) : null}
      {item({ view: "settings", icon: "settings", label: "Settings", keys: "⌘," })}
    </nav>
  );
}
