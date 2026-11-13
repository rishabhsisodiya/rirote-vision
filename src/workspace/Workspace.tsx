import "./workspace.css";

import { useEffect, useMemo, useRef, useState } from "react";

import { ChatPane } from "../chat/ChatPane";
import { ExportDialog } from "../export/ExportDialog";
import { useChat } from "../chat/useChat";
import { api } from "../lib/ipc";
import type { Clip, EditorContext, MediaItem } from "../lib/types";
import { useMenu } from "../lib/useMenu";
import { MediaBin } from "../media/MediaBin";
import { MusicDialog } from "../media/MusicDialog";
import { placementRequest } from "../media/mediaActions";
import { useFileDrop } from "../media/useFileDrop";
import { useJobs } from "../media/useJobs";
import { useMedia } from "../media/useMedia";
import { VoiceoverDialog } from "../media/VoiceoverDialog";
import { pickFiles } from "../projects/pickFolder";
import { ClipInspector } from "../timeline/ClipInspector";
import { clipName, newTrack } from "../timeline/model";
import { useMediaVisuals } from "../timeline/useMediaVisuals";
import { captionsBrief, liveClips, placementBrief } from "../timeline/visuals";
import { CatalogDialog } from "../timeline/CatalogDialog";
import { TranscriptPanel } from "../transcript/TranscriptPanel";
import { BrandPanel } from "../brand/BrandPanel";
import type { BrandKitsState } from "../brand/useBrandKits";
import { LookPicker } from "../timeline/LookPicker";
import { NotesPanel } from "../comments/NotesPanel";
import { useComments } from "../comments/useComments";
import { HistoryPanel } from "./HistoryPanel";
import { momentsBrief, replaceBrief } from "./requests";
import { ReplaceMedia } from "../timeline/ReplaceMedia";
import { SoundsDialog } from "../media/SoundsDialog";
import { formatDuration } from "../lib/format";

import type { ProjectSummary } from "../lib/types";
import { PreviewPane } from "../preview/PreviewPane";
import { usePlaybackKeys } from "../preview/usePlaybackKeys";
import { usePlayer } from "../preview/usePlayer";
import { usePreview } from "../preview/usePreview";
import { TimelinePane } from "../timeline/TimelinePane";
import { useTimeline } from "../timeline/useTimeline";
import { Notice } from "../ui/Notice";
import { Divider } from "./Divider";
import { ProjectDetails } from "./ProjectDetails";
import { SidePanel } from "./SidePanel";
import { TitleBar } from "./TitleBar";
import { usePaneSize } from "./usePaneSize";
import { useProjectInfo } from "./useProjectInfo";

export interface PaneVisibility {
  sidebar: boolean;
  assistant: boolean;
}

interface WorkspaceProps {
  project: ProjectSummary;
  /** False while another screen is shown: the editor stays loaded but quiet. */
  active: boolean;
  panes: PaneVisibility;
  notice: string | null;
  onDismissNotice: () => void;
  onTogglePane: (pane: keyof PaneVisibility) => void;
  onCheckSetup: () => void;
  onReveal: () => void;
  /** A first request to send as soon as the assistant can take it. */
  brief: string | null;
  onBriefSent: () => void;
  kits: BrandKitsState;
  onManageBrands: () => void;
  /** Told when the assistant starts and stops working. */
  onAssistantBusy: (busy: boolean) => void;
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
  usePlaybackKeys(player, props.active);

  // Leaving the editor stops the picture; coming back leaves it paused.
  const { toggle: togglePlayback } = player;
  const playingNow = player.playback.playing;
  useEffect(() => {
    if (!props.active && playingNow) togglePlayback();
  }, [props.active, playingNow, togglePlayback]);

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
  const [adding, setAdding] = useState(false);
  const [findingSounds, setFindingSounds] = useState(false);
  const comments = useComments(project.path);
  const openNotes = comments.notes.filter((note) => !note.resolved).length;
  const ask = (prompt: string) => {
    if (!panes.assistant) props.onTogglePane("assistant");
    void send(prompt, context);
  };
  const [captioning, setCaptioning] = useState<{ clip: Clip; file: string } | null>(null);
  const [bottom, setBottom] = useState<"timeline" | "transcript">("timeline");
  useMenu((command) => {
    if (command === "project.export" && props.active) setExporting(true);
  });
  const { importFiles } = media;
  const dropping = useFileDrop((paths) => void importFiles(paths), props.active);
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

  // A brief from Home goes out once the chat has loaded. A project with no
  // level chosen runs at full trust, so nothing is asked first.
  const { brief, onBriefSent } = props;
  const latestContext = useRef(context);
  latestContext.current = context;
  const { loaded: chatLoaded, trust, busy: assistantBusy } = chat.state;
  useEffect(() => {
    if (!brief || !chatLoaded || trust === null || assistantBusy) return;
    onBriefSent();
    void send(brief, latestContext.current);
  }, [brief, chatLoaded, trust, assistantBusy, send, onBriefSent]);

  // The sidebar logo plays while the assistant works; it stops when the project closes.
  const { onAssistantBusy } = props;
  useEffect(() => {
    onAssistantBusy(assistantBusy);
  }, [assistantBusy, onAssistantBusy]);
  useEffect(() => () => onAssistantBusy(false), [onAssistantBusy]);

  // A request for permission must be seen, even with the assistant hidden.
  const waiting = chat.state.approvals.length > 0 || brief !== null;
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
  const sidebarSize = usePaneSize("sidebar", 300, 240, 480);
  const assistantSize = usePaneSize("assistant", 360, 280, 620);
  const timelineSize = usePaneSize("timeline", 300, 160, 640);
  const [theater, setTheater] = useState(false);
  const sizes = {
    "--sidebar-size": `${sidebarSize.size}px`,
    "--assistant-size": `${assistantSize.size}px`,
    "--timeline-size": `${timelineSize.size}px`,
  } as React.CSSProperties;

  const layout = [
    "workspace",
    panes.sidebar ? "" : "workspace--no-sidebar",
    panes.assistant ? "" : "workspace--no-assistant",
    theater ? "workspace--theater" : "",
  ].join(" ");

  const timelineReady = timeline.view.status === "ready" ? timeline.view : null;
  const soundClips = useMemo(
    () => (timelineReady?.timeline.clips ?? []).filter((clip) => clip.media && clip.src && clip.volume !== null && clip.editable),
    [timelineReady],
  );
  const bottomTabs = (
    <div className="bottom-tabs" role="tablist" aria-label="Bottom panel">
      {(["timeline", "transcript"] as const).map((tab) => (
        <button key={tab} type="button" role="tab" aria-selected={bottom === tab} onClick={() => setBottom(tab)}>
          {tab}
        </button>
      ))}
    </div>
  );
  const visuals = useMediaVisuals(project.path, timelineReady?.timeline.clips, signature);
  const live = useMemo(
    () => liveClips(chat.state.messages, timelineReady?.timeline.clips ?? [], chat.state.busy),
    [chat.state.messages, timelineReady, chat.state.busy],
  );
  const frame = info.state.status === "ready" ? { width: info.state.info.width, height: info.state.info.height } : null;
  const inspect = selectedClip?.editable && timelineReady ? (
    <>
      <ClipInspector
        clip={selectedClip}
        duration={timelineReady.timeline.duration}
        freeTrack={newTrack(timelineReady.timeline.clips)}
        disabled={timeline.busy || chat.state.busy}
        onEdit={timeline.edit}
      />
      {selectedClip.kind === "video" && selectedClip.media ? (
        <LookPicker key={selectedClip.ref} projectPath={project.path} clip={selectedClip} disabled={chat.state.busy} />
      ) : null}
      {selectedClip.media ? (
        <ReplaceMedia
          key={`replace-${selectedClip.ref}`}
          clip={selectedClip}
          items={media.state.status === "ready" ? media.state.items : []}
          disabled={chat.state.busy}
          onReplace={(file) => ask(replaceBrief(selectedClip, file))}
        />
      ) : null}
    </>
  ) : (
    <p className="side__empty">
      {selectedClip ? "This clip is inside another composition; ask the assistant to change it." : "Select a clip on the timeline to see and change its timing, track and volume."}
    </p>
  );

  return (
    <div className={layout} style={sizes}>
      <TitleBar
        project={project}
        sidebarShown={panes.sidebar}
        assistantShown={panes.assistant}
        onToggleSidebar={() => props.onTogglePane("sidebar")}
        onToggleAssistant={() => props.onTogglePane("assistant")}
        onAir={chat.state.busy}
        onAirDetail={chat.state.busy ? "The assistant is editing the project" : null}
        onExport={() => setExporting(true)}
      />

      {panes.sidebar ? (
        <SidePanel
          focus={selectedClip ? "inspect" : null}
          tabs={[
            { id: "inspect", label: "Inspect", content: inspect },
            {
              id: "media",
              label: "Media",
              content: (
                <MediaBin
                  media={media}
                  jobs={jobs}
                  filesUrl={session?.session.url ?? null}
                  assistantBusy={chat.state.busy}
                  onImport={() => void importFromDisk()}
                  onVoiceover={() => setVoiceover(true)}
                  onMusic={() => setMusic(true)}
                  onPlace={place}
                  onSounds={() => setFindingSounds(true)}
                />
              ),
            },
            {
              id: "brand",
              label: "Brand",
              content: (
                <BrandPanel
                  projectPath={project.path}
                  kits={props.kits}
                  assistantBusy={chat.state.busy}
                  onAsk={(prompt) => {
                    if (!panes.assistant) props.onTogglePane("assistant");
                    void send(prompt, context);
                  }}
                  onManage={props.onManageBrands}
                />
              ),
            },
            {
              id: "notes",
              label: "Notes",
              badge: openNotes,
              content: <NotesPanel comments={comments} assistantBusy={chat.state.busy} onSeek={player.seek} onAsk={ask} />,
            },
            {
              id: "history",
              label: "History",
              content: <HistoryPanel projectPath={project.path} signature={signature} assistantBusy={chat.state.busy} />,
            },
            {
              id: "project",
              label: "Project",
              content: (
                <ProjectDetails
                  project={project}
                  info={info.state}
                  onRetry={info.reload}
                  onCheckSetup={props.onCheckSetup}
                  onReveal={props.onReveal}
                />
              ),
            },
          ]}
        />
      ) : null}

      <main className="stage">
        {props.notice ? <Notice message={props.notice} onDismiss={props.onDismissNotice} /> : null}
        <section className="pane stage__preview" aria-label="Preview">
          <PreviewPane
            preview={preview.state}
            player={player}
            frame={frame}
            theater={theater}
            onTheater={setTheater}
            onRestart={preview.restart}
            onCheckSetup={props.onCheckSetup}
            notes={comments.notes}
            onAddNote={(x, y, text) => comments.add(playhead ?? 0, x, y, text)}
          />
        </section>
      </main>

      <div className="workspace__bottom">
        <Divider label="Timeline height" orientation="horizontal" pane={timelineSize} grows={-1} />
        <section className="pane stage__timeline" aria-label={bottom === "timeline" ? "Timeline" : "Transcript"}>
          {bottom === "timeline" ? (
            <TimelinePane
              timeline={timeline}
              player={player}
              selected={selected}
              onSelect={setSelected}
              assistantBusy={chat.state.busy}
              onCheckSetup={props.onCheckSetup}
              active={props.active}
              visuals={visuals}
              live={live}
              onAdd={() => setAdding(true)}
              lead={bottomTabs}
            />
          ) : (
            <TranscriptPanel
              projectPath={project.path}
              clips={soundClips}
              selected={selectedClip && soundClips.includes(selectedClip) ? selectedClip : null}
              signature={signature}
              playhead={playhead ?? 0}
              busy={timeline.busy || chat.state.busy}
              jobs={jobs.jobs}
              lead={bottomTabs}
              onSeek={player.seek}
              onCut={(clip, ranges) => timeline.cut(clip, ranges)}
              onTranscribe={(src) => jobs.start({ type: "transcribe", source: src })}
              onCaptions={(clip, file) => setCaptioning({ clip, file })}
              onMoments={(clip, file) => ask(momentsBrief(clip, file))}
            />
          )}
        </section>
      </div>

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
          label="Inspector width"
          orientation="vertical"
          pane={sidebarSize}
          grows={-1}
        />
      ) : null}
      {panes.assistant ? (
        <Divider
          className="divider--assistant"
          label="Assistant width"
          orientation="vertical"
          pane={assistantSize}
          grows={1}
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
      {adding ? (
        <CatalogDialog
          projectPath={project.path}
          where={selectedClip ? `after ${clipName(selectedClip)}` : `at ${formatDuration(playhead ?? 0)}`}
          onClose={() => setAdding(false)}
          onInstalled={(installed, item) => {
            setAdding(false);
            if (!panes.assistant) props.onTogglePane("assistant");
            void send(placementBrief(installed, item.title || item.name, playhead ?? 0, selectedClip), context);
          }}
        />
      ) : null}
      {findingSounds ? (
        <SoundsDialog projectPath={project.path} onAdded={() => void reloadMedia()} onClose={() => setFindingSounds(false)} />
      ) : null}
      {captioning ? (
        <CatalogDialog
          projectPath={project.path}
          where={`captions to ${clipName(captioning.clip)}`}
          initialShelf="captions"
          onClose={() => setCaptioning(null)}
          onInstalled={(installed, item) => {
            const { clip, file } = captioning;
            setCaptioning(null);
            if (!panes.assistant) props.onTogglePane("assistant");
            void send(captionsBrief(installed, item.title || item.name, clip, file), context);
          }}
        />
      ) : null}
      {dropping ? <div className="drop-overlay">Drop to import into the project</div> : null}
    </div>
  );
}
