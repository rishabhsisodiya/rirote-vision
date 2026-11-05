import type { ProjectSummary } from "../lib/types";
import { Button, IconButton } from "../ui/Button";

interface TitleBarProps {
  project: ProjectSummary;
  sidebarShown: boolean;
  assistantShown: boolean;
  /** The assistant is changing the project: the tally light is on. */
  onAir: boolean;
  /** What it is doing, in a few words, for the tally's tooltip. */
  onAirDetail: string | null;
  onToggleSidebar: () => void;
  onToggleAssistant: () => void;
  onExport: () => void;
}

/** The window's top bar. Empty parts of it drag the window. */
export function TitleBar(props: TitleBarProps) {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar__lead">
        <IconButton
          icon="assistant"
          label={props.assistantShown ? "Hide assistant (⌥⌘0)" : "Show assistant (⌥⌘0)"}
          pressed={props.assistantShown}
          onClick={props.onToggleAssistant}
        />
        {props.onAir ? (
          <span className="tally" role="status" title={props.onAirDetail ?? "The assistant is editing"}>
            On air
          </span>
        ) : null}
      </div>
      <h1 className="titlebar__title" data-tauri-drag-region>
        {props.project.name}
      </h1>
      <div className="titlebar__trail">
        <Button variant="primary" icon="export" onClick={props.onExport}>
          Export
        </Button>
        <IconButton
          icon="sidebar"
          label={props.sidebarShown ? "Hide inspector (⌘0)" : "Show inspector (⌘0)"}
          pressed={props.sidebarShown}
          onClick={props.onToggleSidebar}
        />
      </div>
    </header>
  );
}
