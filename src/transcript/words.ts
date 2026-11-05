// A clip's words on the timeline, and the stretches to cut for an edit made
// in the text. Free of React so it can be tested.

import type { Clip } from "../lib/types.ts";

export interface FileWord {
  text: string;
  /** Seconds into the media file. */
  start: number;
  end: number;
}

export interface TimedWord {
  /** Position in the transcript file. */
  index: number;
  text: string;
  /** Seconds on the timeline. */
  start: number;
  end: number;
  filler: boolean;
}

const FILLERS = new Set(["um", "umm", "uh", "uhh", "uhm", "erm", "er", "ah", "hmm", "mm", "mhm"]);

export function isFiller(text: string): boolean {
  return FILLERS.has(text.toLowerCase().replace(/[^\p{L}]/gu, ""));
}

/** The words the clip plays, placed on the timeline. */
export function clipWords(clip: Clip, words: FileWord[]): TimedWord[] {
  const rate = clip.rate ?? 1;
  const from = clip.mediaStart ?? 0;
  const to = from + (clip.end - clip.start) * rate;
  const at = (seconds: number) => clip.start + (seconds - from) / rate;
  return words
    .map((word, index) => ({ word, index }))
    .filter(({ word }) => word.end > from + 0.01 && word.start < to - 0.01)
    .map(({ word, index }) => ({
      index,
      text: word.text,
      start: Math.max(clip.start, at(word.start)),
      end: Math.min(clip.end, at(word.end)),
      filler: isFiller(word.text),
    }));
}

export type Range = [number, number];

/** The stretches that remove `chosen`, joining words that sit side by side. */
export function rangesForWords(words: TimedWord[], chosen: Set<number>): Range[] {
  const ranges: Range[] = [];
  let open: Range | null = null;
  for (const word of words) {
    if (chosen.has(word.index)) {
      // Running into the next word's start takes the breath after a word too.
      if (open) open[1] = word.end;
      else open = [word.start, word.end];
    } else if (open) {
      open[1] = Math.min(word.start, open[1] + 0.15);
      ranges.push(open);
      open = null;
    }
  }
  if (open) ranges.push(open);
  return ranges;
}

export interface Pause {
  /** Timeline seconds. */
  start: number;
  end: number;
  /** The word the pause follows. */
  after: number;
}

/** Gaps between words at least `min` seconds long. */
export function pauses(words: TimedWord[], min: number): Pause[] {
  const found: Pause[] = [];
  for (let i = 1; i < words.length; i += 1) {
    const before = words[i - 1]!;
    const gap = words[i]!.start - before.end;
    if (gap >= min) found.push({ start: before.end, end: words[i]!.start, after: before.index });
  }
  return found;
}

/** Shortens each pause to `keep` seconds, keeping a little air on both sides. */
export function shortenRanges(found: Pause[], keep: number): Range[] {
  return found
    .filter((pause) => pause.end - pause.start > keep + 0.05)
    .map((pause) => [pause.start + keep / 2, pause.end - keep / 2] as Range);
}

/** Seconds a set of cuts removes. */
export function removedSeconds(ranges: Range[]): number {
  return ranges.reduce((total, [from, to]) => total + Math.max(0, to - from), 0);
}
