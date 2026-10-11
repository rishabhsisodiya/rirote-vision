// The timeline's arithmetic, free of React and the DOM so it can be tested.

import type { Clip, EditOp } from "../lib/types";

/** The shortest clip a drag may leave behind, in seconds. Matches timeline.rs. */
export const MIN_CLIP = 0.05;

export interface Lane {
  track: number;
  kind: string;
  label: string;
  clips: Clip[];
}

const KIND_LABEL: Record<string, string> = {
  video: "Video",
  graphics: "Graphics",
  captions: "Captions",
  audio: "Audio",
};

/**
 * One lane per track index. Picture lanes come first with the highest track
 * on top, then sound lanes, as an editor lays them out.
 */
export function buildLanes(clips: Clip[]): Lane[] {
  const byTrack = new Map<number, Clip[]>();
  for (const clip of clips) {
    const lane = byTrack.get(clip.track);
    if (lane) lane.push(clip);
    else byTrack.set(clip.track, [clip]);
  }
  const lanes = [...byTrack.entries()].map(([track, members]) => {
    const kind = members[0]?.kind ?? "graphics";
    return { track, kind, label: `${KIND_LABEL[kind] ?? "Track"} ${track}`, clips: members };
  });
  const isSound = (lane: Lane) => lane.kind === "audio";
  return lanes.sort((a, b) => {
    if (isSound(a) !== isSound(b)) return isSound(a) ? 1 : -1;
    return isSound(a) ? a.track - b.track : b.track - a.track;
  });
}

const STEPS = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600];
const MIN_LABEL_GAP = 72;

/** Seconds between labelled ruler marks, so labels never crowd. */
export function rulerStep(pxPerSecond: number): number {
  return STEPS.find((step) => step * pxPerSecond >= MIN_LABEL_GAP) ?? STEPS[STEPS.length - 1]!;
}

export function rulerTicks(duration: number, step: number): number[] {
  const ticks: number[] = [];
  for (let index = 0; index * step <= duration + 1e-9; index += 1) {
    ticks.push(Number((index * step).toFixed(3)));
  }
  return ticks;
}

/** 65 → "1:05"; with a sub-second step, 5.5 → "0:05.5". */
export function formatRulerTime(seconds: number, step: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds - minutes * 60;
  const text = step < 1 ? rest.toFixed(step < 0.5 ? 2 : 1) : String(Math.round(rest));
  const [whole, fraction] = text.split(".");
  return `${minutes}:${whole!.padStart(2, "0")}${fraction ? `.${fraction}` : ""}`;
}

/** The scale at which the whole timeline fits the given width. */
export function fitScale(width: number, duration: number): number {
  return Math.max(1, width) / Math.max(duration, 0.1);
}

/** Zoom never goes wider than fitting, nor past 40 times closer. */
export function clampScale(scale: number, fit: number): number {
  return Math.min(fit * 40, Math.max(fit, scale));
}

/** Times a dragged edge may snap to: the playhead, both ends and every other clip's edges. */
export function snapTargets(clips: Clip[], dragged: string, playhead: number, duration: number): number[] {
  const targets = [0, duration, playhead];
  for (const clip of clips) {
    if (clip.ref !== dragged) targets.push(clip.start, clip.end);
  }
  return targets;
}

/** The nearest target within `threshold` seconds, or null. */
export function nearest(time: number, targets: number[], threshold: number): number | null {
  let best: number | null = null;
  for (const target of targets) {
    const distance = Math.abs(target - time);
    if (distance <= threshold && (best === null || distance < Math.abs(best - time))) best = target;
  }
  return best;
}

export type DragMode = "move" | "start" | "end";

export interface Span {
  start: number;
  end: number;
}

interface DragOptions {
  targets: number[];
  /** Snap reach in seconds. */
  threshold: number;
  duration: number;
}

const clamp = (value: number, low: number, high: number) => Math.min(Math.max(value, low), high);

/** Where a clip sits after being dragged `delta` seconds by its body or an edge. */
export function dragSpan(clip: Span, mode: DragMode, delta: number, options: DragOptions): Span {
  const { targets, threshold, duration } = options;
  const length = clip.end - clip.start;

  if (mode === "start") {
    const wanted = clip.start + delta;
    const start = nearest(wanted, targets, threshold) ?? wanted;
    return { start: clamp(start, 0, clip.end - MIN_CLIP), end: clip.end };
  }
  if (mode === "end") {
    const wanted = clip.end + delta;
    const end = nearest(wanted, targets, threshold) ?? wanted;
    return { start: clip.start, end: clamp(end, clip.start + MIN_CLIP, Math.max(duration, clip.end)) };
  }

  // Moving: whichever edge is closer to a target pulls the clip onto it.
  const wanted = clip.start + delta;
  const byStart = nearest(wanted, targets, threshold);
  const byEnd = nearest(wanted + length, targets, threshold);
  let start = wanted;
  if (byStart !== null && (byEnd === null || Math.abs(byStart - wanted) <= Math.abs(byEnd - wanted - length))) {
    start = byStart;
  } else if (byEnd !== null) {
    start = byEnd - length;
  }
  start = clamp(start, 0, Math.max(0, duration - length));
  return { start, end: start + length };
}

const moved = (a: number, b: number) => Math.abs(a - b) >= 0.001;

/** The edit that turns `clip` into `span`, or null when nothing changed. */
export function editFor(clip: Clip, mode: DragMode, span: Span): EditOp | null {
  if (mode === "move") {
    return moved(clip.start, span.start) ? { type: "move", clip: clip.ref, start: span.start } : null;
  }
  if (mode === "start") {
    return moved(clip.start, span.start)
      ? { type: "trimStart", clip: clip.ref, start: span.start }
      : null;
  }
  return moved(clip.end, span.end) ? { type: "trimEnd", clip: clip.ref, end: span.end } : null;
}

/** The name shown on a clip: its label, else its id. */
export function clipName(clip: Clip): string {
  return clip.label ?? clip.id;
}

/** Picture lanes take picture clips and sound lanes take sound, so a lane keeps one meaning. */
export function sameFamily(a: string, b: string): boolean {
  return (a === "audio") === (b === "audio");
}

/** The edit for a clip dragged to `span`, possibly onto another track. */
export function placementFor(clip: Clip, mode: DragMode, span: Span, track: number): EditOp | null {
  if (mode === "move" && track !== clip.track) {
    return { type: "place", clip: clip.ref, start: span.start, track };
  }
  return editFor(clip, mode, span);
}

/** Every moment a cut falls on: the ends of the film and of each clip, in order. */
export function editPoints(clips: Clip[], duration: number): number[] {
  const points = new Set<number>([0, duration]);
  for (const clip of clips) {
    points.add(Number(clip.start.toFixed(3)));
    points.add(Number(clip.end.toFixed(3)));
  }
  return [...points].sort((a, b) => a - b);
}

/**
 * The nearest edit point before or after `time`; `time` itself when there is
 * none. The player parks on whole frames, so after stepping to a cut it
 * reports a time up to one frame before it: going forward, a cut that close
 * ahead is the one the playhead is already on.
 */
export function neighbourPoint(points: number[], time: number, direction: -1 | 1, frame = 1 / 30): number {
  if (direction === 1) return points.find((point) => point > time + frame + 0.001) ?? time;
  return [...points].reverse().find((point) => point < time - 0.001) ?? time;
}

/** A free track number above every track in use, for "new track". */
export function newTrack(clips: Clip[]): number {
  return clips.reduce((highest, clip) => Math.max(highest, clip.track), -1) + 1;
}

