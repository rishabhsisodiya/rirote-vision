import { formatDuration, shortPath } from "../lib/format";
import type { ReactNode } from "react";

import type { ProjectSummary } from "../lib/types";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import type { ProjectInfoState } from "./useProjectInfo";

interface ProjectSidebarProps {
  project: ProjectSummary;
  info: ProjectInfoState;
  onRetry: () => void;
  onCheckSetup: () => void;
  onReveal: () => void;
  /** The media bin, which fills what is left of the sidebar. */
  children: ReactNode;
}

export function ProjectSidebar(props: ProjectSidebarProps) {
  const { project, info, onRetry, onCheckSetup, onReveal } = props;
  return (
    <aside className="pane sidebar" aria-label="Project">
      <section className="sidebar__section">
        <h2 className="pane__heading">Project</h2>
        <p className="sidebar__name">{project.name}</p>
        <p className="sidebar__path" title={project.path}>
          {shortPath(project.path)}
        </p>
        <Button variant="ghost" icon="reveal" onClick={onReveal}>
          Show in Finder
        </Button>
      </section>

      <section className="sidebar__section">
        <h2 className="pane__heading">Composition</h2>
        <Composition info={info} onRetry={onRetry} onCheckSetup={onCheckSetup} />
      </section>

      <section className="sidebar__section sidebar__section--fill">{props.children}</section>
    </aside>
  );
}

interface CompositionProps {
  info: ProjectInfoState;
  onRetry: () => void;
  onCheckSetup: () => void;
}

function Composition({ info, onRetry, onCheckSetup }: CompositionProps) {
  if (info.status === "loading") {
    return (
      <p className="sidebar__state">
        <Spinner label="Reading the composition" />
      </p>
    );
  }
  if (info.status === "error") {
    return (
      <div className="sidebar__state">
        <p>{info.message}</p>
        {info.needsSetup ? (
          <Button onClick={onCheckSetup}>Check Setup</Button>
        ) : (
          <Button onClick={onRetry}>Try Again</Button>
        )}
      </div>
    );
  }
  const { width, height, duration, tracks, elements } = info.info;
  return (
    <dl className="facts">
      <dt>Frame</dt>
      <dd>
        {width} × {height}
      </dd>
      <dt>Duration</dt>
      <dd>{formatDuration(duration)}</dd>
      <dt>Tracks</dt>
      <dd>{tracks}</dd>
      <dt>Clips</dt>
      <dd>{elements}</dd>
    </dl>
  );
}
