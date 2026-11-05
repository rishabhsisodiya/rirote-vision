// Requests the editor hands the assistant for jobs the engine has no command for.

import type { Clip } from "../lib/types.ts";

/** Swap a clip's media for another file, keeping its place and length. */
export function replaceBrief(clip: Clip, file: string): string {
  return [
    `Replace the media of ${clip.ref} (now ${clip.src ?? "unknown"}) with ${file}.`,
    `Keep the clip where it is (${clip.start.toFixed(2)}–${clip.end.toFixed(2)} s on track ${clip.track}), with its volume, speed and any look.`,
    "If the new file is shorter than the clip, end the clip where the file ends and tell me. Check the result with a snapshot.",
  ].join("\n");
}

/** Pick the best short pieces of a long recording. */
export function momentsBrief(clip: Clip, transcriptFile: string, count = 3): string {
  return [
    `Find the ${count} strongest moments in ${clip.ref} (${clip.src ?? "its media"}) for short social clips, using the words and timings in ${transcriptFile}.`,
    "Each moment should stand on its own: 15 to 45 seconds, starting on a strong first line and ending on a complete thought.",
    "List them with their start and end times in the file and one line on why each works. Do not change the project yet; I will choose which to cut.",
  ].join("\n");
}
