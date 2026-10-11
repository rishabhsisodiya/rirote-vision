// What a music bed can be. Free of React so it can be tested. The limits
// mirror `music.rs` and `music_bed.py`, which decide.

import type { MusicStyle } from "../lib/types";

export const MUSIC_STYLES: { value: MusicStyle; label: string; detail: string }[] = [
  { value: "calm", label: "Calm", detail: "Slow and sparse. Sits quietly under a voice." },
  { value: "steady", label: "Steady", detail: "Warm, with an even pulse." },
  { value: "bright", label: "Bright", detail: "Lighter and quicker, with more movement." },
  { value: "cinematic", label: "Cinematic build", detail: "Rises from silence to one impact, then holds." },
];

export const MIN_SECONDS = 3;
export const MAX_SECONDS = 600;
const MIN_BEFORE_IMPACT = 3;
const MIN_AFTER_IMPACT = 2;
const FALLBACK_SECONDS = 30;

/** The length offered at first: the film's, rounded up to a whole second. */
export function defaultSeconds(filmSeconds: number | null): number {
  if (filmSeconds === null || !Number.isFinite(filmSeconds) || filmSeconds <= 0) return FALLBACK_SECONDS;
  return Math.min(MAX_SECONDS, Math.max(MIN_SECONDS, Math.ceil(filmSeconds)));
}

/** A sensible impact for a cinematic bed of this length: about two thirds in. */
export function defaultImpact(seconds: number): number {
  const wanted = Math.round((seconds * 2) / 3);
  return Math.min(Math.max(MIN_BEFORE_IMPACT, seconds - MIN_AFTER_IMPACT), Math.max(MIN_BEFORE_IMPACT, wanted));
}

/** Why these choices will not do, or null when they will. */
export function musicProblem(style: MusicStyle, seconds: number, impactAt: number | null): string | null {
  if (!Number.isFinite(seconds) || seconds < MIN_SECONDS || seconds > MAX_SECONDS) {
    return `Choose a length from ${MIN_SECONDS} to ${MAX_SECONDS} seconds.`;
  }
  if (style !== "cinematic") return null;
  if (seconds < MIN_BEFORE_IMPACT + MIN_AFTER_IMPACT) {
    return `A cinematic build needs at least ${MIN_BEFORE_IMPACT + MIN_AFTER_IMPACT} seconds.`;
  }
  if (impactAt === null || !Number.isFinite(impactAt)) return "Say at which second the impact lands.";
  if (impactAt < MIN_BEFORE_IMPACT || impactAt > seconds - MIN_AFTER_IMPACT) {
    return `Put the impact between second ${MIN_BEFORE_IMPACT} and second ${seconds - MIN_AFTER_IMPACT}.`;
  }
  return null;
}
