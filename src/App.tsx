import "./shell/shell.css";

import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useState } from "react";

import { SetupDialog } from "./doctor/SetupDialog";
import { HomeScreen, type HomeTab } from "./home/HomeScreen";
import { BrandKitsScreen } from "./brand/BrandKitsScreen";
import { useBrandKits } from "./brand/useBrandKits";
import { ReferencesScreen } from "./references/ReferencesScreen";
import { useReferences } from "./references/useReferences";
import { useSetup } from "./doctor/useSetup";
import { api, errorMessage } from "./lib/ipc";
import type { ProjectSummary } from "./lib/types";
import { useMenu } from "./lib/useMenu";
import { NewProjectDialog } from "./projects/NewProjectDialog";
import { pickFolder } from "./projects/pickFolder";
import { useRecentProjects } from "./projects/useRecentProjects";
import { CommandPalette, type PaletteCommand } from "./shell/CommandPalette";
import { Rail, type View } from "./shell/Rail";
import { SettingsView } from "./shell/SettingsView";
import { useActiveJobs } from "./shell/useActiveJobs";
import { ShortcutsDialog } from "./shortcuts/ShortcutsDialog";
import { Workspace, type PaneVisibility } from "./workspace/Workspace";

const APP_NAME = "RIROTE VISION";

export function App() {
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [view, setView] = useState<View>("home");
  const [dialog, setDialog] = useState<"new" | "setup" | "shortcuts" | "palette" | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  /** The first request for a project Home just made; sent once the assistant is ready. */
  const [brief, setBrief] = useState<{ path: string; text: string } | null>(null);
  const [homeTab, setHomeTab] = useState<HomeTab>("make");
  const [referenceId, setReferenceId] = useState<string | null>(null);
  const references = useReferences();
  const kits = useBrandKits();
  const [panes, setPanes] = useState<PaneVisibility>({ sidebar: true, assistant: true });
  const recent = useRecentProjects();
  const setup = useSetup();
  const activeJobs = useActiveJobs();
  const reloadRecent = recent.reload;

  const navigate = useCallback(
    (next: View) => {
      if (next === "editor" && !project) return;
      setView(next);
    },
    [project],
  );

  const enter = useCallback(
    (opened: ProjectSummary) => {
      setProject(opened);
      setView("editor");
      setDialog(null);
      setNotice(null);
      void reloadRecent();
    },
    [reloadRecent],
  );

  /** A project made from Home or a reference, whose first request is waiting. */
  const startWithBrief = useCallback(
    (made: ProjectSummary, text: string) => {
      setBrief({ path: made.path, text });
      enter(made);
    },
    [enter],
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
    setView("home");
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
      case "view.palette":
        return setDialog((current) => (current === "palette" ? null : current ?? "palette"));
      case "view.home":
        return navigate("home");
      case "view.editor":
        return navigate("editor");
      case "app.settings":
        return navigate("settings");
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

  const commands = useMemo<PaletteCommand[]>(() => {
    const list: PaletteCommand[] = [
      { id: "new", label: "New video", icon: "plus", keys: "⌘N", run: () => setDialog("new") },
      { id: "open", label: "Open project folder…", icon: "folder", keys: "⌘O", run: () => void openFromDisk() },
      { id: "home", label: "Go to Home", icon: "home", keys: "⌘1", run: () => navigate("home") },
      { id: "references", label: "References", icon: "reference", run: () => navigate("references") },
      {
        id: "study",
        label: "Study a reference video…",
        icon: "reference",
        run: () => {
          setReferenceId(null);
          setHomeTab("reference");
          navigate("home");
        },
      },
      { id: "brands", label: "Brand kits", icon: "brand", run: () => navigate("brands") },
      { id: "settings", label: "Settings", icon: "settings", keys: "⌘,", run: () => navigate("settings") },
      { id: "setup", label: "Check setup", icon: "check", run: () => setDialog("setup") },
      { id: "shortcuts", label: "Keyboard shortcuts", icon: "command", keys: "⌘/", run: () => setDialog("shortcuts") },
    ];
    if (project) {
      list.splice(
        2,
        0,
        { id: "editor", label: `Go to editor: ${project.name}`, icon: "editor", keys: "⌘2", run: () => navigate("editor") },
        { id: "reveal", label: "Show project in Finder", icon: "reveal", keys: "⇧⌘R", run: reveal },
        { id: "close", label: "Close project", icon: "close", keys: "⇧⌘W", run: closeProject },
      );
    }
    return list;
  }, [project, navigate, openFromDisk, reveal, closeProject]);

  return (
    <div className="shell">
      <Rail
        view={view}
        hasProject={project !== null}
        activeJobs={activeJobs}
        onNavigate={navigate}
        onNew={() => setDialog("new")}
        onPalette={() => setDialog("palette")}
      />
      <div className="shell__main">
        {project ? (
          <div className="shell__screen" hidden={view !== "editor"}>
            <Workspace
              key={project.path}
              project={project}
              active={view === "editor"}
              panes={panes}
              notice={notice}
              onDismissNotice={() => setNotice(null)}
              onTogglePane={togglePane}
              onCheckSetup={() => setDialog("setup")}
              onReveal={reveal}
              brief={brief?.path === project.path ? brief.text : null}
              kits={kits.state}
              onManageBrands={() => setView("brands")}
              onBriefSent={() => setBrief(null)}
            />
          </div>
        ) : null}
        {view === "home" ? (
          <HomeScreen
            recent={recent.state}
            setup={setup.report}
            notice={notice}
            onDismissNotice={() => setNotice(null)}
            onStarted={startWithBrief}
            onOpen={openFromDisk}
            onOpenRecent={(chosen) => void openPath(chosen.path)}
            onForget={(chosen) => {
              recent.forget(chosen.path).catch((cause) => setNotice(errorMessage(cause)));
            }}
            onCheckSetup={() => setDialog("setup")}
            onRetryRecent={reloadRecent}
            tab={homeTab}
            onTab={setHomeTab}
            library={references.state}
            kits={kits.state}
            referenceId={referenceId}
            onSelectReference={setReferenceId}
          />
        ) : null}
        {view === "references" ? (
          <ReferencesScreen
            library={references.state}
            selectedId={referenceId}
            onSelect={setReferenceId}
            onStarted={startWithBrief}
            onAdd={() => {
              setReferenceId(null);
              setHomeTab("reference");
              setView("home");
            }}
          />
        ) : null}
        {view === "brands" ? <BrandKitsScreen kits={kits} /> : null}
        {view === "settings" ? <SettingsView setup={setup.report} onCheckSetup={() => setDialog("setup")} /> : null}
      </div>

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
      {dialog === "palette" ? (
        <CommandPalette
          commands={commands}
          recent={recent.state.status === "ready" ? recent.state.projects : []}
          onOpenProject={(chosen) => void openPath(chosen.path)}
          onClose={() => setDialog(null)}
        />
      ) : null}
    </div>
  );
}
