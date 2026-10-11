import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";

import { SetupDialog } from "./doctor/SetupDialog";
import { useSetup } from "./doctor/useSetup";
import { api, errorMessage } from "./lib/ipc";
import type { ProjectSummary } from "./lib/types";
import { useMenu } from "./lib/useMenu";
import { NewProjectDialog } from "./projects/NewProjectDialog";
import { pickFolder } from "./projects/pickFolder";
import { useRecentProjects } from "./projects/useRecentProjects";
import { WelcomeScreen } from "./projects/WelcomeScreen";
import { ShortcutsDialog } from "./shortcuts/ShortcutsDialog";
import { Workspace, type PaneVisibility } from "./workspace/Workspace";

const APP_NAME = "RIROTE VISION";

export function App() {
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [dialog, setDialog] = useState<"new" | "setup" | "shortcuts" | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [panes, setPanes] = useState<PaneVisibility>({ sidebar: true, assistant: true });
  const recent = useRecentProjects();
  const setup = useSetup();
  const reloadRecent = recent.reload;

  const enter = useCallback(
    (opened: ProjectSummary) => {
      setProject(opened);
      setDialog(null);
      setNotice(null);
      void reloadRecent();
    },
    [reloadRecent],
  );

  const openPath = useCallback(
    async (path: string) => {
      try {
        enter(await api.openProject(path));
      } catch (cause) {
        setNotice(errorMessage(cause));
        void reloadRecent();
      }
    },
    [enter, reloadRecent],
  );

  const openFromDisk = useCallback(async () => {
    try {
      const folder = await pickFolder("Open a HyperFrames project");
      if (folder) await openPath(folder);
    } catch (cause) {
      setNotice(errorMessage(cause));
    }
  }, [openPath]);

  const closeProject = useCallback(() => {
    setProject(null);
    setNotice(null);
    void reloadRecent();
  }, [reloadRecent]);

  const reveal = useCallback(() => {
    if (!project) return;
    api.revealProject(project.path).catch((cause) => setNotice(errorMessage(cause)));
  }, [project]);

  const togglePane = useCallback((pane: keyof PaneVisibility) => {
    setPanes((current) => ({ ...current, [pane]: !current[pane] }));
  }, []);

  useMenu((command) => {
    switch (command) {
      case "project.new":
        return setDialog("new");
      case "project.open":
        return void openFromDisk();
      case "project.close":
        return closeProject();
      case "project.reveal":
        return reveal();
      case "setup.check":
        return setDialog("setup");
      case "help.shortcuts":
        return setDialog("shortcuts");
      case "edit.undo":
      case "edit.redo":
        // In a text field these are the field's own undo and redo; the
        // timeline handles them everywhere else.
        if (
          document.activeElement instanceof HTMLInputElement ||
          document.activeElement instanceof HTMLTextAreaElement
        ) {
          document.execCommand(command === "edit.undo" ? "undo" : "redo");
        }
        return;
      case "view.sidebar":
        return togglePane("sidebar");
      case "view.assistant":
        return togglePane("assistant");
    }
  });

  // A folder given on the command line opens straight away.
  useEffect(() => {
    api
      .launchProject()
      .then((path) => (path ? openPath(path) : undefined))
      .catch((cause) => setNotice(errorMessage(cause)));
  }, [openPath]);

  useEffect(() => {
    const title = project ? `${project.name} — ${APP_NAME}` : APP_NAME;
    void getCurrentWindow().setTitle(title);
  }, [project]);

  return (
    <>
      {project ? (
        <Workspace
          key={project.path}
          project={project}
          panes={panes}
          notice={notice}
          onDismissNotice={() => setNotice(null)}
          onTogglePane={togglePane}
          onClose={closeProject}
          onCheckSetup={() => setDialog("setup")}
          onReveal={reveal}
        />
      ) : (
        <WelcomeScreen
          recent={recent.state}
          setup={setup.report}
          notice={notice}
          onDismissNotice={() => setNotice(null)}
          onNew={() => setDialog("new")}
          onOpen={openFromDisk}
          onOpenRecent={(chosen) => void openPath(chosen.path)}
          onForget={(chosen) => {
            recent.forget(chosen.path).catch((cause) => setNotice(errorMessage(cause)));
          }}
          onCheckSetup={() => setDialog("setup")}
          onRetryRecent={reloadRecent}
        />
      )}

      {dialog === "new" ? (
        <NewProjectDialog onClose={() => setDialog(null)} onCreated={enter} />
      ) : null}
      {dialog === "shortcuts" ? <ShortcutsDialog onClose={() => setDialog(null)} /> : null}
      {dialog === "setup" ? (
        <SetupDialog
          setup={setup}
          onCheck={setup.check}
          onInstall={setup.install}
          onClose={() => setDialog(null)}
        />
      ) : null}
    </>
  );
}
