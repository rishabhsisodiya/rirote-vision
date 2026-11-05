// Where a clip's media falls under it: which slice of a waveform and which
// stills to draw. Free of React so it can be tested.

import type { ChatMessage, Clip, MediaPeaks, MediaStill } from "../lib/types.ts";

/** Seconds into the media file at a point `offset` seconds into the clip. */
export function sourceTime(clip: Clip, offset: number): number {
  return (clip.mediaStart ?? 0) + offset * (clip.rate ?? 1);
}

/**
 * Bar heights (0–1) for a waveform `width` pixels wide drawn under the clip,
 * one bar every `gap` pixels.
 */
export function waveBars(clip: Clip, peaks: MediaPeaks, pxPerSecond: number, width: number, gap = 3): number[] {
  if (pxPerSecond <= 0 || width <= 0 || peaks.step <= 0) return [];
  const bars: number[] = [];
  for (let x = 0; x < width; x += gap) {
    const from = sourceTime(clip, x / pxPerSecond);
    const to = sourceTime(clip, (x + gap) / pxPerSecond);
    const first = Math.floor(from / peaks.step);
    const last = Math.max(first, Math.ceil(to / peaks.step) - 1);
    let loudest = 0;
    for (let index = first; index <= last && index < peaks.values.length; index += 1) {
      if (index >= 0) loudest = Math.max(loudest, peaks.values[index] ?? 0);
    }
    bars.push(loudest / 255);
  }
  return bars;
}

/** The stills that fall inside the clip, with where each starts in pixels. */
export function placedStills(clip: Clip, stills: MediaStill[], pxPerSecond: number): { still: MediaStill; left: number }[] {
  const length = clip.end - clip.start;
  const rate = clip.rate ?? 1;
  const from = clip.mediaStart ?? 0;
  const to = from + length * rate;
  return stills
    .filter((still) => still.time >= from - 1 && still.time <= to)
    .map((still) => ({ still, left: Math.max(0, ((still.time - from) / rate) * pxPerSecond - 32) }));
}

/** What the assistant is asked to do with a block just installed from the catalogue. */
export function placementBrief(installed: { name: string; snippet: string }, title: string, at: number, after: Clip | null): string {
  const where = after
    ? `right after ${after.ref} ends, at ${after.end.toFixed(2)} s`
    : `at the playhead, ${at.toFixed(2)} s`;
  return [
    `I added “${title}” (${installed.name}) from the HyperFrames catalogue; its files are already in the project.`,
    `Place it on the timeline ${where}, on a free track above what is there, using this snippet as the starting point:`,
    "",
    installed.snippet,
    "",
    "If it is a transition, line it up across the cut so it covers the change from one shot to the next. Keep everything else where it is, then check the result with a snapshot.",
  ].join("\n");
}

const WRITING_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);

/**
 * The clips the assistant is changing right now: those whose file is the
 * subject of a write that has not finished. Edits to the root composition are
 * not pinned on any one clip.
 */
export function liveClips(messages: ChatMessage[], clips: Clip[], busy: boolean): Set<string> {
  const live = new Set<string>();
  if (!busy) return live;
  const files = messages
    .filter((message) => message.role === "tool" && WRITING_TOOLS.has(message.tool ?? "") && message.ok === undefined)
    .map((message) => message.text.trim().replace(/^\.\//, ""))
    .filter((file) => file && !/(^|\/)index\.html$/.test(file));
  for (const clip of clips) {
    if (clip.src && files.some((file) => file.endsWith(clip.src!) || clip.src!.endsWith(file))) live.add(clip.ref);
  }
  return live;
}

/** What the assistant is asked after a caption style is installed. */
export function captionsBrief(installed: { name: string; snippet: string }, title: string, clip: Clip, transcriptFile: string): string {
  return [
    `I added the caption style “${title}” (${installed.name}) from the HyperFrames catalogue; its files are already in the project.`,
    `Caption ${clip.ref} (${clip.src ?? "its media"}, ${clip.start.toFixed(2)}–${clip.end.toFixed(2)} s on the timeline) in that style, using the word timings in ${transcriptFile}.`,
    "The transcript's times are seconds into the media file; the clip plays from " +
      `${(clip.mediaStart ?? 0).toFixed(2)} s of its file at ${(clip.rate ?? 1).toFixed(2)}× speed, so map the times onto the timeline accordingly.`,
    "Put the captions on one track of their own, inside the safe area, and check the result with a snapshot.",
    "",
    installed.snippet,
  ].join("\n");
}
