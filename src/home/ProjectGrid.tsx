import { useEffect, useState } from "react";

import { api } from "../lib/ipc";
import { formatOpened, shortPath } from "../lib/format";
import type { RecentProject } from "../lib/types";
import type { RecentState } from "../projects/useRecentProjects";
import { Button, IconButton } from "../ui/Button";
import { Spinner } from "../ui/Spinner";

interface ProjectGridProps {
  state: RecentState;
  onOpen: (project: RecentProject) => void;
  onForget: (project: RecentProject) => void;
  onOpenFolder: () => void;
  onRetry: () => void;
}

/** Recent projects as cards with a still from each film. */
export function ProjectGrid({ state, onOpen, onForget, onOpenFolder, onRetry }: ProjectGridProps) {
  return (
    <section className="home__projects" aria-labelledby="projects-heading">
      <header className="home__panel-head">
        <h2 id="projects-heading">Your projects</h2>
        <Button icon="folder" onClick={onOpenFolder}>
          Open Folder…
        </Button>
      </header>
      {state.status === "loading" ? (
        <p className="home__state">
          <Spinner label="Loading projects" />
        </p>
      ) : state.status === "error" ? (
        <div className="home__state">
          <p>The project list could not be read. {state.message}</p>
          <Button onClick={onRetry}>Try Again</Button>
        </div>
      ) : state.projects.length === 0 ? (
        <p className="home__state">
          No projects yet. Describe a video on the left, or open a HyperFrames folder you already have.
        </p>
      ) : (
        <ul className="project-grid">
          {state.projects.map((project) => (
            <li key={project.path} className="project-card">
              <button
                type="button"
                className="project-card__open"
                disabled={project.missing}
                title={project.missing ? "This folder has been moved or deleted" : `Open ${project.name}`}
                onClick={() => onOpen(project)}
              >
                <Poster project={project} />
                <span className="project-card__name">{project.name}</span>
                <span className="project-card__meta">
                  {project.missing ? (
                    <span className="project-card__missing">Folder not found</span>
                  ) : (
                    <>
                      <span>{formatOpened(project.openedAt)}</span>
                      <span className="project-card__path">{shortPath(project.path)}</span>
                    </>
                  )}
                </span>
              </button>
              <IconButton
                className="project-card__forget"
                icon="close"
                label={`Remove ${project.name} from this list`}
                onClick={() => onForget(project)}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

type PosterState = { status: "loading" } | { status: "ready"; url: string } | { status: "none" };

function Poster({ project }: { project: RecentProject }) {
  const [poster, setPoster] = useState<PosterState>(project.missing ? { status: "none" } : { status: "loading" });

  useEffect(() => {
    if (project.missing) return;
    let current = true;
    api.projectPoster(project.path).then(
      (url) => current && setPoster({ status: "ready", url }),
      () => current && setPoster({ status: "none" }),
    );
    return () => {
      current = false;
    };
  }, [project.path, project.missing]);

  return (
    <span className="project-card__poster" aria-hidden="true">
      {poster.status === "ready" ? <img src={poster.url} alt="" /> : null}
      {poster.status === "loading" ? <Spinner /> : null}
      {poster.status === "none" ? <span className="project-card__initial">{project.name.slice(0, 1)}</span> : null}
    </span>
  );
}
