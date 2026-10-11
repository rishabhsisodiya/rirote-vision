import { memo, useEffect, useRef, useState, type PointerEvent } from "react";

import { formatDuration } from "../lib/format";
import type { Clip, EditOp } from "../lib/types";
import {
  clipName,
  dragSpan,
  placementFor,
  sameFamily,
  snapTargets,
  type DragMode,
  type Lane,
  type Span,
} from "./model";

/** A press becomes a drag after this many pixels. */
const DRAG_START_PX = 3;
/** How close, in pixels, an edge must come to a target to snap to it. */
const SNAP_PX = 7;

/** Where a lane is on screen when a drag begins. */
interface LaneBox {
  track: number;
  kind: string;
  top: number;
  bottom: number;
}

interface Drag {
  clip: Clip;
  mode: DragMode;
  originX: number;
  originY: number;
  /** Null until the pointer has travelled far enough to count as a drag. */
  span: Span | null;
  /** The lane the clip is over, and how far down the screen that is from its own. */
  track: number;
  shift: number;
}

interface TimelineLanesProps {
  lanes: Lane[];
  clips: Clip[];
  duration: number;
  pxPerSecond: number;
  selected: string | null;
  /** Edits are not accepted while one is being made. */
  locked: boolean;
  /** Read when a drag begins, so the playhead is a snap target. */
  playhead: () => number;
  onSelect: (clip: string | null) => void;
  onSeek: (seconds: number) => void;
  onEdit: (op: EditOp) => void;
}

/** The tracks and their clips. Clips are dragged to move and by their edges to trim. */
export const TimelineLanes = memo(function TimelineLanes(props: TimelineLanesProps) {
  const { lanes, clips, duration, pxPerSecond, selected, locked, onSelect, onSeek, onEdit } = props;
  const [drag, setDrag] = useState<Drag | null>(null);
  const targets = useRef<number[]>([]);
  const boxes = useRef<LaneBox[]>([]);
  const container = useRef<HTMLDivElement>(null);

  // Escape abandons a drag in progress.
  useEffect(() => {
    if (!drag) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setDrag(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drag]);

  const begin = (event: PointerEvent<HTMLElement>, clip: Clip, mode: DragMode) => {
    event.stopPropagation();
    onSelect(clip.ref);
    if (locked || !clip.editable || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    targets.current = snapTargets(clips, clip.ref, props.playhead(), duration);
    boxes.current = [...(container.current?.querySelectorAll<HTMLElement>(".lane") ?? [])].map((lane) => {
      const { top, bottom } = lane.getBoundingClientRect();
      return { track: Number(lane.dataset.track), kind: lane.dataset.kind ?? "", top, bottom };
    });
    setDrag({ clip, mode, originX: event.clientX, originY: event.clientY, span: null, track: clip.track, shift: 0 });
  };

  const move = (event: PointerEvent<HTMLElement>) => {
    if (!drag || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
    const travelled = event.clientX - drag.originX;
    const risen = event.clientY - drag.originY;
    if (!drag.span && Math.abs(travelled) < DRAG_START_PX && Math.abs(risen) < DRAG_START_PX) return;
    const span = dragSpan(drag.clip, drag.mode, travelled / pxPerSecond, {
      targets: targets.current,
      threshold: SNAP_PX / pxPerSecond,
      duration,
    });
    // A clip being moved follows the pointer onto another lane of its own
    // kind; over anything else it stays on the lane it came from.
    const home = boxes.current.find((box) => box.track === drag.clip.track);
    const over =
      drag.mode === "move"
        ? boxes.current.find(
            (box) => event.clientY >= box.top && event.clientY < box.bottom && sameFamily(box.kind, drag.clip.kind),
          )
        : undefined;
    const lane = over ?? home;
    setDrag({ ...drag, span, track: lane?.track ?? drag.clip.track, shift: lane && home ? lane.top - home.top : 0 });
  };

  const end = () => {
    if (!drag) return;
    const op = drag.span ? placementFor(drag.clip, drag.mode, drag.span, drag.track) : null;
    setDrag(null);
    if (op) onEdit(op);
  };

  return (
    <div ref={container}>
      {lanes.map((lane) => (
        <div
          key={lane.track}
          className={`lane ${drag?.span && drag.track === lane.track && drag.track !== drag.clip.track ? "lane--target" : ""}`}
          data-track={lane.track}
          data-kind={lane.kind}
        >
          <div className="lane__label">{lane.label}</div>
          <div
            className="lane__track"
            style={{ width: duration * pxPerSecond }}
            onPointerDown={(event) => {
              const { left } = event.currentTarget.getBoundingClientRect();
              onSelect(null);
              onSeek((event.clientX - left) / pxPerSecond);
            }}
          >
            {lane.clips.map((clip) => {
              const dragging = drag?.clip.ref === clip.ref && drag.span ? drag : null;
              const dragged = dragging?.span ?? null;
              const span = dragged ?? clip;
              const name = clipName(clip);
              const classes = [
                "clip",
                `clip--${clip.kind}`,
                clip.ref === selected ? "clip--selected" : "",
                clip.editable ? "" : "clip--locked",
                dragged ? "clip--dragging" : "",
              ].join(" ");
              return (
                <div
                  key={clip.ref}
                  className={classes}
                  style={{
                    left: span.start * pxPerSecond,
                    width: (span.end - span.start) * pxPerSecond,
                    transform: dragging?.shift ? `translateY(${dragging.shift}px)` : undefined,
                  }}
                  role="button"
                  tabIndex={0}
                  aria-pressed={clip.ref === selected}
                  aria-label={`${name}, ${lane.label}, ${formatDuration(span.start)} to ${formatDuration(span.end)}${clip.editable ? "" : ", not editable here"}`}
                  title={clip.src ? `${name} — ${clip.src}` : name}
                  onPointerDown={(event) => begin(event, clip, "move")}
                  onPointerMove={move}
                  onPointerUp={end}
                  onPointerCancel={() => setDrag(null)}
                  onFocus={() => onSelect(clip.ref)}
                >
                  {clip.editable ? (
                    <span
                      className="clip__edge clip__edge--start"
                      onPointerDown={(event) => begin(event, clip, "start")}
                      onPointerMove={move}
                      onPointerUp={end}
                      onPointerCancel={() => setDrag(null)}
                    />
                  ) : null}
                  <span className="clip__name">{name}</span>
                  {clip.editable ? (
                    <span
                      className="clip__edge clip__edge--end"
                      onPointerDown={(event) => begin(event, clip, "end")}
                      onPointerMove={move}
                      onPointerUp={end}
                      onPointerCancel={() => setDrag(null)}
                    />
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
});
