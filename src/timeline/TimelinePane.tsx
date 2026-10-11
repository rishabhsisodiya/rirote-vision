import "./timeline.css";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { EditOp } from "../lib/types";
import { useMenu } from "../lib/useMenu";
import type { PlayerControls } from "../preview/usePlayer";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { ClipInspector } from "./ClipInspector";
import { buildLanes, clampScale, editPoints, fitScale, MIN_CLIP, neighbourPoint, newTrack } from "./model";
import { Playhead } from "./Playhead";
import { TimelineLanes } from "./TimelineLanes";
import { TimelineRuler } from "./TimelineRuler";
import { TimelineToolbar } from "./TimelineToolbar";
import type { TimelineControls } from "./useTimeline";

/** Width of the lane labels, in pixels. Matches --lane-label in timeline.css. */
const LABEL_WIDTH = 112;
/** Room kept after the last second so the end is easy to grab. */
const TAIL = 24;
/** One nudge is a frame at this rate. */
const NUDGE_FPS = 30;

/** True when a text field or dialog should get the key or command instead. */
function typing(): boolean {
  const focus = document.activeElement;
  if (!(focus instanceof HTMLElement)) return false;
  if (focus.closest("dialog")) return true;
  return (
    (focus instanceof HTMLInputElement && focus.type !== "range") ||
    focus instanceof HTMLTextAreaElement ||
    focus.isContentEditable
  );
}

interface TimelinePaneProps {
  timeline: TimelineControls;
  player: PlayerControls;
  /** The selected clip's reference. Owned by the workspace: the assistant is told about it too. */
  selected: string | null;
  onSelect: (clip: string | null) => void;
  /** The assistant is changing the project; hand edits wait until it is done. */
  assistantBusy: boolean;
  onCheckSetup: () => void;
}

/** The project's tracks and clips, with the tools that edit them. */
export function TimelinePane(props: TimelinePaneProps) {
  const { timeline, player, selected, onSelect: setSelected, onCheckSetup } = props;
  const { view, notice } = timeline;
  const busy = timeline.busy || props.assistantBusy;
  const { currentTime, playing } = player.playback;
  const scroller = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  /** Pixels per second; null fits the whole timeline in view. */
  const [zoom, setZoom] = useState<number | null>(null);

  const ready = view.status === "ready" ? view : null;
  const clips = ready?.timeline.clips;
  const duration = ready?.timeline.duration ?? 0;
  const lanes = useMemo(() => buildLanes(clips ?? []), [clips]);
  const selectedClip = clips?.find((clip) => clip.ref === selected) ?? null;

  // The scroller exists only once the clips are shown.
  const shown = !!ready;
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setWidth(element.clientWidth));
    observer.observe(element);
    setWidth(element.clientWidth);
    return () => observer.disconnect();
  }, [shown]);

  // A clip that an edit or an outside change removed is no longer selected.
  useEffect(() => {
    if (selected && clips && !clips.some((clip) => clip.ref === selected)) setSelected(null);
  }, [clips, selected, setSelected]);

  const fit = fitScale(width - LABEL_WIDTH - TAIL, duration);
  const pxPerSecond = zoom === null ? fit : clampScale(zoom, fit);

  // Keep the playhead in view while the film plays.
  useEffect(() => {
    const element = scroller.current;
    if (!element || !playing) return;
    const x = currentTime * pxPerSecond;
    const visible = element.clientWidth - LABEL_WIDTH;
    if (x < element.scrollLeft || x > element.scrollLeft + visible - 40) {
      element.scrollLeft = Math.max(0, x - 40);
    }
  }, [currentTime, playing, pxPerSecond]);

  const time = useRef(currentTime);
  time.current = currentTime;

  // Zooming keeps the playhead on screen instead of leaving it behind.
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const x = time.current * pxPerSecond;
    const visible = element.clientWidth - LABEL_WIDTH;
    if (x < element.scrollLeft || x > element.scrollLeft + visible) {
      element.scrollLeft = Math.max(0, x - visible / 2);
    }
  }, [pxPerSecond]);
  const playhead = useCallback(() => time.current, []);

  const { edit, undo, redo } = timeline;
  const { seek } = player;
  const act = useCallback(
    (make: (clip: string) => EditOp) => {
      if (selectedClip?.editable && !busy) void edit(make(selectedClip.ref));
    },
    [selectedClip, busy, edit],
  );
  const split = useCallback(
    () => act((clip) => ({ type: "split", clip, time: time.current })),
    [act],
  );
  const duplicate = useCallback(() => act((clip) => ({ type: "duplicate", clip })), [act]);
  const remove = useCallback(() => act((clip) => ({ type: "delete", clip })), [act]);
  const nudge = useCallback(
    (frames: number) => {
      if (!selectedClip) return;
      act((clip) => ({ type: "move", clip, start: Math.max(0, selectedClip.start + frames / NUDGE_FPS) }));
    },
    [act, selectedClip],
  );

  useMenu((command) => {
    if (!ready || typing()) return;
    if (command === "edit.undo" && ready.canUndo && !busy) void undo();
    if (command === "edit.redo" && ready.canRedo && !busy) void redo();
    if (command === "clip.split") split();
    if (command === "clip.duplicate") duplicate();
    if (command === "clip.delete") remove();
  });

  const points = useMemo(() => editPoints(clips ?? [], duration), [clips, duration]);
  const zoomBy = useCallback(
    (factor: number) => setZoom((current) => clampScale((current ?? fit) * factor, fit)),
    [fit],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (typing()) return;
      if (event.metaKey && !event.altKey && !event.ctrlKey) {
        if (event.key === "=" || event.key === "+") zoomBy(1.5);
        else if (event.key === "-") zoomBy(1 / 1.5);
        else return;
        event.preventDefault();
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const frames = event.shiftKey ? 10 : 1;
      if (event.key === "Backspace" || event.key === "Delete") remove();
      else if (event.key === "s" || event.key === "S") split();
      else if (event.key === "Z") setZoom(null);
      else if (event.key === "ArrowUp") seek(neighbourPoint(points, time.current, -1));
      else if (event.key === "ArrowDown") seek(neighbourPoint(points, time.current, 1));
      else if (event.key === ",") nudge(-frames);
      else if (event.key === ".") nudge(frames);
      else if (event.key === "<") nudge(-10);
      else if (event.key === ">") nudge(10);
      else if (event.key === "Escape") setSelected(null);
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [remove, nudge, split, seek, zoomBy, points, setSelected]);

  if (view.status === "loading") {
    return (
      <div className="timeline__state" role="status">
        <Spinner />
        <p>Reading the timeline</p>
      </div>
    );
  }
  if (view.status === "error") {
    return (
      <div className="timeline__state">
        <p>{view.message}</p>
        {view.needsSetup ? (
          <Button onClick={onCheckSetup}>Check Setup</Button>
        ) : (
          <Button onClick={() => void timeline.reload()}>Try Again</Button>
        )}
      </div>
    );
  }

  const canSplit =
    !!selectedClip &&
    currentTime - selectedClip.start >= MIN_CLIP &&
    selectedClip.end - currentTime >= MIN_CLIP;

  return (
    <div className="timeline">
      <TimelineToolbar
        selected={selectedClip}
        canUndo={view.canUndo}
        canRedo={view.canRedo}
        canSplit={canSplit}
        busy={busy}
        assistantBusy={props.assistantBusy}
        zoomed={zoom !== null && pxPerSecond > fit}
        onUndo={() => void undo()}
        onRedo={() => void redo()}
        onSplit={split}
        onDuplicate={duplicate}
        onDelete={remove}
        onZoom={zoomBy}
        onFit={() => setZoom(null)}
      />
      {selectedClip?.editable ? (
        <ClipInspector
          clip={selectedClip}
          duration={duration}
          freeTrack={newTrack(view.timeline.clips)}
          disabled={busy}
          onEdit={edit}
        />
      ) : null}
      {notice ? <Notice message={notice} onDismiss={timeline.dismissNotice} /> : null}

      {lanes.length === 0 ? (
        <div className="timeline__state">
          <p>This composition has no clips yet.</p>
        </div>
      ) : (
        <div className="timeline__scroll" ref={scroller}>
          <div className="timeline__content" style={{ width: LABEL_WIDTH + duration * pxPerSecond + TAIL }}>
            <div className="timeline__ruler-row">
              <div className="lane__label lane__label--corner" />
              <TimelineRuler duration={duration} pxPerSecond={pxPerSecond} onSeek={seek} />
            </div>
            <TimelineLanes
              lanes={lanes}
              clips={view.timeline.clips}
              duration={duration}
              pxPerSecond={pxPerSecond}
              selected={selected}
              locked={busy}
              playhead={playhead}
              onSelect={setSelected}
              onSeek={seek}
              onEdit={edit}
            />
            <Playhead
              time={currentTime}
              playing={playing}
              duration={duration}
              pxPerSecond={pxPerSecond}
              offset={LABEL_WIDTH}
            />
          </div>
        </div>
      )}
    </div>
  );
}
