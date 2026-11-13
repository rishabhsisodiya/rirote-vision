import type { IconName } from "../ui/Icon";
import { Icon } from "../ui/Icon";
import { LogoMark } from "./LogoMark";

export type View = "home" | "editor" | "references" | "brands" | "settings";

interface RailProps {
  view: View;
  /** The editor is only reachable while a project is open. */
  hasProject: boolean;
  /** Jobs queued or running in the open project. */
  activeJobs: number;
  /** The assistant or a job is working: the logo plays. */
  working: boolean;
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

/** The sidebar down the window's left edge: icon and name for each place. */
export function Rail(props: RailProps) {
  const item = (destination: Destination) => {
    const disabled = destination.view === "editor" && !props.hasProject;
    return (
      <button
        key={destination.view}
        type="button"
        className="rail__item"
        aria-label={destination.label}
        title={disabled ? "Open a project to edit it" : destination.keys ? `${destination.label} (${destination.keys})` : destination.label}
        aria-current={props.view === destination.view ? "page" : undefined}
        disabled={disabled}
        onClick={() => props.onNavigate(destination.view)}
      >
        <RailLabel icon={destination.icon} label={destination.label} keys={destination.keys} />
      </button>
    );
  };

  return (
    <nav className="rail" aria-label="Navigation">
      <div className="rail__drag" data-tauri-drag-region />
      <span className="rail__brand" aria-hidden="true">
        <LogoMark working={props.working} />
        <span className="rail__name">
          RI<em>RO</em>TE VISION
        </span>
      </span>
      <button type="button" className="rail__item" aria-label="Search projects and commands" title="Search (⌘K)" onClick={props.onPalette}>
        <RailLabel icon="search" label="Search" keys="⌘K" />
      </button>
      <button type="button" className="rail__item" aria-label="New video" title="New video (⌘N)" onClick={props.onNew}>
        <RailLabel icon="plus" label="New video" keys="⌘N" />
      </button>
      <span className="rail__rule" aria-hidden="true" />
      {MAIN.map(item)}
      <span className="rail__spacer" />
      {props.activeJobs > 0 ? (
        <span className="rail__jobs" role="status" title={`${props.activeJobs} job(s) running`}>
          <Icon name="jobs" size={18} />
          <span className="rail__label">Jobs running</span>
          <span className="rail__badge">{props.activeJobs}</span>
        </span>
      ) : null}
      {item({ view: "settings", icon: "settings", label: "Settings", keys: "⌘," })}
    </nav>
  );
}

/** One row's icon, name and shortcut. The name and shortcut hide when the rail is narrow. */
function RailLabel({ icon, label, keys }: { icon: IconName; label: string; keys?: string }) {
  return (
    <>
      <Icon name={icon} size={18} />
      <span className="rail__label">{label}</span>
      {keys ? <kbd className="rail__keys">{keys}</kbd> : null}
    </>
  );
}
