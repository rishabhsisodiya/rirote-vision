import type { ProjectSummary } from "../lib/types";
import { Button, IconButton } from "../ui/Button";

interface TitleBarProps {
  project: ProjectSummary;
  sidebarShown: boolean;
  assistantShown: boolean;
  onToggleSidebar: () => void;
  onToggleAssistant: () => void;
  onExport: () => void;
  onClose: () => void;
}

/** The window's top bar. Empty parts of it drag the window. */
export function TitleBar(props: TitleBarProps) {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar__lead">
        <IconButton
          icon="sidebar"
          label={props.sidebarShown ? "Hide project sidebar" : "Show project sidebar"}
          pressed={props.sidebarShown}
          onClick={props.onToggleSidebar}
        />
        <Button variant="ghost" onClick={props.onClose}>
          Projects
        </Button>
      </div>
      <h1 className="titlebar__title" data-tauri-drag-region>
        {props.project.name}
      </h1>
      <div className="titlebar__trail">
        <Button variant="primary" icon="export" onClick={props.onExport}>
          Export
        </Button>
        <IconButton
          icon="assistant"
          label={props.assistantShown ? "Hide assistant" : "Show assistant"}
          pressed={props.assistantShown}
          onClick={props.onToggleAssistant}
        />
      </div>
    </header>
  );
}
