// The levels of trust a project's assistant can be given. The wording here
// is what people choose from, so it says plainly what each level allows.

import type { TrustLevel } from "../lib/types";

export interface TrustOption {
  value: TrustLevel;
  label: string;
  /** What runs without asking. */
  allows: string;
  /** What still asks, or the warning for the level that asks nothing. */
  asks: string;
}

export const TRUST_OPTIONS: TrustOption[] = [
  {
    value: "ask",
    label: "Ask every time",
    allows: "Reads and edits files in this project.",
    asks: "Asks before every command.",
  },
  {
    value: "trusted",
    label: "Trusted video work",
    allows:
      "Also creates, copies, moves and deletes files in this project, and runs the video engine, this project's own scripts, ffmpeg and ffprobe.",
    asks: "Still asks before anything outside this folder, downloads, installs and any other program.",
  },
  {
    value: "full",
    label: "Full trust",
    allows: "Runs anything without asking.",
    asks: "Nothing is asked. A mistaken command cannot be stopped before it runs.",
  },
];

/** The level offered first to someone who has not chosen yet. */
export const DEFAULT_TRUST: TrustLevel = "trusted";

export function trustLabel(level: TrustLevel): string {
  return TRUST_OPTIONS.find((option) => option.value === level)?.label ?? level;
}
