import "./workspace.css";

import { useEffect, useMemo, useRef, useState } from "react";

import { ChatPane } from "../chat/ChatPane";
import { ExportDialog } from "../export/ExportDialog";
import { useChat } from "../chat/useChat";
import { api } from "../lib/ipc";
import type { EditorContext, MediaItem } from "../lib/types";
import { useMenu } from "../lib/useMenu";
import { MediaBin } from "../media/MediaBin";
import { MusicDialog } from "../media/MusicDialog";
import { placementRequest } from "../media/mediaActions";
import { useFileDrop } from "../media/useFileDrop";
import { useJobs } from "../media/useJobs";
import { useMedia } from "../media/useMedia";
import { VoiceoverDialog } from "../media/VoiceoverDialog";
import { pickFiles } from "../projects/pickFolder";
import { clipName } from "../timeline/model";

import type { ProjectSummary } from "../lib/types";
import { PreviewPane } from "../preview/PreviewPane";
import { usePlaybackKeys } from "../preview/usePlaybackKeys";
import { usePlayer } from "../preview/usePlayer";
import { usePreview } from "../preview/usePreview";
import { TimelinePane } from "../timeline/TimelinePane";
import { useTimeline } from "../timeline/useTimeline";
import { Notice } from "../ui/Notice";
import { Divider } from "./Divider";
import { ProjectSidebar } from "./ProjectSidebar";
import { TitleBar } from "./TitleBar";
import { usePaneSize } from "./usePaneSize";
import { useProjectInfo } from "./useProjectInfo";

export interface PaneVisibility {
  sidebar: boolean;
  assistant: boolean;
}

interface WorkspaceProps {
  project: ProjectSummary;
  panes: PaneVisibility;
  notice: string | null;
  onDismissNotice: () => void;
  onTogglePane: (pane: keyof PaneVisibility) => void;
  onClose: () => void;
  onCheckSetup: () => void;
  onReveal: () => void;
}

/** The editor: project on the left, picture and timeline in the middle, assistant on the right. */
export function Workspace(props: WorkspaceProps) {
  const { project, panes } = props;
  const info = useProjectInfo(project.path);
  const preview = usePreview(project.path);

  // One player serves the picture, the transport and the timeline's playhead.
  const session = preview.state.status === "ready" ? preview.state : null;
  const signature = session?.signature ?? null;
  const player = usePlayer(session?.session.url ?? null, signature ?? "");
  usePlaybackKeys(player);

  // Started with --autoplay: play once the film has loaded (see launch_autoplay).
  const { toggle } = player;
  const loaded = player.playback.ready;
  const autoplayed = useRef(false);
  useEffect(() => {
    if (!loaded || autoplayed.current) return;
    autoplayed.current = true;
    api.launchAutoplay().then((wanted) => wanted && window.setTimeout(toggle, 1500)).catch(() => {});
  }, [loaded, toggle]);
  const timeline = useTimeline(project.path, signature);
  const chat = useChat(project.path);
  const [selected, setSelected] = useState<string | null>(null);

  // What the assistant is told with each request: where the playhead is and
  // which clip is selected.
  const clips = timeline.view.status === "ready" ? timeline.view.timeline.clips : null;
  const selectedClip = clips?.find((clip) => clip.ref === selected) ?? null;
  const playhead = player.playback.ready ? player.playback.currentTime : null;
  const context = useMemo<EditorContext>(
    () => ({
      playhead,
      selected: selectedClip
        ? { ref: selectedClip.ref, start: selectedClip.start, end: selectedClip.end }
        : null,
    }),
    [playhead, selectedClip],
  );

  // Media: the files in the project, the jobs that make more, and import.
  const media = useMedia(project.path, signature);
  const reloadMedia = media.reload;
  const jobs = useJobs(project.path, () => void reloadMedia());
  const [voiceover, setVoiceover] = useState(false);
  const [music, setMusic] = useState(false);
  const [exporting, setExporting] = useState(false);
  useMenu((command) => {
    if (command === "project.export") setExporting(true);
  });
  const { importFiles } = media;
  const dropping = useFileDrop((paths) => void importFiles(paths));
  const importFromDisk = async () => {
    const chosen = await pickFiles("Import media into the project");
    await importFiles(chosen);
  };

  // The engine cannot add a clip by command, so the assistant is asked to.
  const { send } = chat;
  const place = (item: MediaItem) => {
    if (!panes.assistant) props.onTogglePane("assistant");
    void send(placementRequest(item), context);
  };

  // A request for permission must be seen, even with the assistant hidden.
  const waiting = chat.state.approvals.length > 0;
  const { onTogglePane } = props;
  useEffect(() => {
    if (waiting && !panes.assistant) onTogglePane("assistant");
  }, [waiting, panes.assistant, onTogglePane]);

  // The composition's figures follow the files, as the picture does.
  const reloadInfo = info.reload;
  useEffect(() => {
    if (signature) void reloadInfo();
  }, [signature, reloadInfo]);

  // Pane sizes are the user's to set, and are remembered.
  const sidebarSize = usePaneSize("sidebar", 264, 200, 440);
  const assistantSize = usePaneSize("assistant", 360, 280, 620);
  const timelineSize = usePaneSize("timeline", 320, 160, 640);
  const sizes = {
    "--sidebar-size": `${sidebarSize.size}px`,
    "--assistant-size": `${assistantSize.size}px`,
    "--timeline-size": `${timelineSize.size}px`,
  } as React.CSSProperties;

  const layout = [
    "workspace",
    panes.sidebar ? "" : "workspace--no-sidebar",
    panes.assistant ? "" : "workspace--no-assistant",
  ].join(" ");

  return (
    <div className={layout} style={sizes}>
      <TitleBar
        project={project}
        sidebarShown={panes.sidebar}
        assistantShown={panes.assistant}
        onToggleSidebar={() => props.onTogglePane("sidebar")}
        onToggleAssistant={() => props.onTogglePane("assistant")}
        onExport={() => setExporting(true)}
        onClose={props.onClose}
      />

      {panes.sidebar ? (
        <ProjectSidebar
          project={project}
          info={info.state}
          onRetry={info.reload}
          onCheckSetup={props.onCheckSetup}
          onReveal={props.onReveal}
        >
          <MediaBin
            media={media}
            jobs={jobs}
            filesUrl={session?.session.url ?? null}
            assistantBusy={chat.state.busy}
            onImport={() => void importFromDisk()}
            onVoiceover={() => setVoiceover(true)}
            onMusic={() => setMusic(true)}
            onPlace={place}
          />
        </ProjectSidebar>
      ) : null}

      <main className="stage">
        {props.notice ? <Notice message={props.notice} onDismiss={props.onDismissNotice} /> : null}
        <section className="pane stage__preview" aria-label="Preview">
          <PreviewPane
            preview={preview.state}
            player={player}
            onRestart={preview.restart}
            onCheckSetup={props.onCheckSetup}
          />
        </section>
        <Divider label="Timeline height" orientation="horizontal" pane={timelineSize} grows={-1} />
        <section className="pane stage__timeline" aria-label="Timeline">
          <TimelinePane
            timeline={timeline}
            player={player}
            selected={selected}
            onSelect={setSelected}
            assistantBusy={chat.state.busy}
            onCheckSetup={props.onCheckSetup}
          />
        </section>
      </main>

      {panes.assistant ? (
        <aside className="pane assistant" aria-label="Assistant">
          <ChatPane
            chat={chat}
            context={context}
            selectedName={selectedClip ? clipName(selectedClip) : null}
          />
        </aside>
      ) : null}

      {panes.sidebar ? (
        <Divider
          className="divider--sidebar"
          label="Sidebar width"
          orientation="vertical"
          pane={sidebarSize}
          grows={1}
        />
      ) : null}
      {panes.assistant ? (
        <Divider
          className="divider--assistant"
          label="Assistant width"
          orientation="vertical"
          pane={assistantSize}
          grows={-1}
        />
      ) : null}

      {voiceover ? (
        <VoiceoverDialog
          onCreate={(text, voice) => jobs.start({ type: "voiceover", text, voice })}
          onClose={() => setVoiceover(false)}
        />
      ) : null}
      {music ? (
        <MusicDialog
          filmSeconds={info.state.status === "ready" ? info.state.info.duration : null}
          onCreate={(style, seconds, impactAt) => jobs.start({ type: "music", style, seconds, impactAt })}
          onClose={() => setMusic(false)}
        />
      ) : null}
      {exporting ? (
        <ExportDialog
          project={project}
          info={info.state.status === "ready" ? info.state.info : null}
          jobs={jobs}
          onClose={() => setExporting(false)}
        />
      ) : null}
      {dropping ? <div className="drop-overlay">Drop to import into the project</div> : null}
    </div>
  );
}
