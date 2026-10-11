// What can be done with a media file, and how the assistant is asked to
// place one on the timeline. Free of React so it can be tested.

import type { MediaItem } from "../lib/types";

const CUTOUT_IMAGES = ["png", "jpg", "jpeg", "webp"];

function extension(path: string): string {
  return path.slice(path.lastIndexOf(".") + 1).toLowerCase();
}

export function canTranscribe(item: MediaItem): boolean {
  return item.kind === "audio" || item.kind === "video";
}

export function canRemoveBackground(item: MediaItem): boolean {
  return item.kind === "video" || (item.kind === "image" && CUTOUT_IMAGES.includes(extension(item.path)));
}

/**
 * The request that puts a file on the timeline. The engine has no command
 * for adding a clip, so the assistant writes it into the composition.
 */
export function placementRequest(item: MediaItem): string {
  if (item.kind === "captions") {
    return `Add captions to the video from the file "${item.path}", timed as the file says and styled to suit this composition.`;
  }
  const what = item.kind === "audio" ? "sound" : item.kind;
  return `Add the ${what} file "${item.path}" to the timeline as a new clip starting at the playhead, on a track where it overlaps nothing.`;
}
