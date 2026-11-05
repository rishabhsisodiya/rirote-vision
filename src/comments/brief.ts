// What the assistant is asked when review notes are handed to it.

import type { Comment } from "../lib/types.ts";

function where(comment: Comment): string {
  if (comment.x === null || comment.y === null) return "";
  const across = comment.x < 0.33 ? "left" : comment.x > 0.66 ? "right" : "centre";
  const down = comment.y < 0.33 ? "top" : comment.y > 0.66 ? "bottom" : "middle";
  const spot = down === "middle" && across === "centre" ? "the centre" : `the ${down} ${across}`.replace("middle centre", "centre");
  return ` near ${spot} of the frame (${Math.round(comment.x * 100)}% across, ${Math.round(comment.y * 100)}% down)`;
}

/** One request covering every open note, in time order. */
export function fixBrief(notes: Comment[]): string {
  const open = notes.filter((note) => !note.resolved).sort((a, b) => a.time - b.time);
  const lines = open.map((note, index) => `${index + 1}. At ${note.time.toFixed(2)} s${where(note)}: ${note.text}`);
  return [
    open.length === 1 ? "Fix this review note on the video:" : `Fix these ${open.length} review notes on the video:`,
    "",
    ...lines,
    "",
    "Look at each moment with a snapshot before and after you change it. Change only what the notes ask for.",
  ].join("\n");
}
