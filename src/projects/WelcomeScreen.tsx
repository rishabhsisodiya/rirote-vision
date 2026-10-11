import "./welcome.css";

import { formatOpened, shortPath } from "../lib/format";
import type { RecentProject, SetupReport } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import type { RecentState } from "./useRecentProjects";

interface WelcomeScreenProps {
  recent: RecentState;
  /** Null while the first check is still running. */
  setup: SetupReport | null;
  notice: string | null;
  onDismissNotice: () => void;
  onNew: () => void;
  onOpen: () => void;
  onOpenRecent: (project: RecentProject) => void;
  onForget: (project: RecentProject) => void;
  onCheckSetup: () => void;
  onRetryRecent: () => void;
}

export function WelcomeScreen(props: WelcomeScreenProps) {
  const { recent, setup, notice } = props;
  return (
    <main className="welcome">
      <div className="welcome__drag" data-tauri-drag-region />
      <div className="welcome__column">
        <header className="welcome__brand">
          <h1 className="wordmark">
            RIROTE <span>VISION</span>
          </h1>
          <p>Describe the video. Refine it on the timeline. Export it.</p>
        </header>

        <div className="welcome__actions">
          <Button variant="primary" icon="plus" onClick={props.onNew}>
            New Project
          </Button>
          <Button icon="folder" onClick={props.onOpen}>
            Open Project
          </Button>
        </div>

        {notice ? <Notice message={notice} onDismiss={props.onDismissNotice} /> : null}
        {setup && !setup.ready ? <SetupBanner setup={setup} onCheck={props.onCheckSetup} /> : null}

        <section className="recent" aria-labelledby="recent-heading">
          <h2 id="recent-heading">Recent projects</h2>
          <RecentList
            state={recent}
            onOpen={props.onOpenRecent}
            onForget={props.onForget}
            onRetry={props.onRetryRecent}
          />
        </section>
      </div>
    </main>
  );
}

function SetupBanner({ setup, onCheck }: { setup: SetupReport; onCheck: () => void }) {
  const open = setup.checks.filter((check) => check.required && check.status !== "ok");
  return (
    <div className="setup-banner" role="status">
      <Icon name="alert" />
      <p>
        <strong>Setup is not finished.</strong> {open.map((check) => check.label).join(", ")}{" "}
        {open.length === 1 ? "needs" : "need"} attention before you can edit or export.
      </p>
      <Button onClick={onCheck}>Check Setup</Button>
    </div>
  );
}

interface RecentListProps {
  state: RecentState;
  onOpen: (project: RecentProject) => void;
  onForget: (project: RecentProject) => void;
  onRetry: () => void;
}

function RecentList({ state, onOpen, onForget, onRetry }: RecentListProps) {
  if (state.status === "loading") {
    return (
      <p className="recent__state">
        <Spinner label="Loading recent projects" />
      </p>
    );
  }
  if (state.status === "error") {
    return (
      <div className="recent__state">
        <p>The recent projects list could not be read. {state.message}</p>
        <Button onClick={onRetry}>Try Again</Button>
      </div>
    );
  }
  if (state.projects.length === 0) {
    return (
      <p className="recent__state">
        Nothing here yet. Create a project, or open a HyperFrames folder you already have.
      </p>
    );
  }
  return (
    <ul className="recent__list">
      {state.projects.map((project) => (
        <li key={project.path} className="recent__row">
          <button
            type="button"
            className="recent__open"
            disabled={project.missing}
            onClick={() => onOpen(project)}
          >
            <span className="recent__name">{project.name}</span>
            <span className="recent__path">{shortPath(project.path)}</span>
            <span className="recent__when">
              {project.missing ? "Folder not found" : formatOpened(project.openedAt)}
            </span>
          </button>
          <IconButton
            icon="close"
            label={`Remove ${project.name} from recent projects`}
            onClick={() => onForget(project)}
          />
        </li>
      ))}
    </ul>
  );
}
