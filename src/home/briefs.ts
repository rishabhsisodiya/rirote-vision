// What Home hands the assistant when it starts a new video, and the name the
// new project folder gets. Kept free of React so it can be tested.

import type { Resolution } from "../lib/types.ts";

const SHAPES: Record<Resolution, string> = {
  landscape: "landscape 16:9 (1920×1080)",
  portrait: "portrait 9:16 (1080×1920)",
  square: "square 1:1 (1080×1080)",
};

const FILLER = new Set(["a", "an", "the", "make", "create", "build", "please", "me", "for", "of", "with", "and", "to", "my", "our", "video", "second", "seconds"]);

/** A short project name from what was typed: its first few telling words. */
export function projectNameFrom(text: string, fallback = "New video"): string {
  const words = text
    .replace(/https?:\/\/(www\.)?/g, "")
    .replace(/[^\p{L}\p{N}\s.-]/gu, " ")
    .split(/\s+/)
    .map((word) => word.replace(/^[.-]+|[.-]+$/g, ""))
    .filter((word) => word && !FILLER.has(word.toLowerCase()) && !/^\d+$/.test(word));
  const name = words.slice(0, 4).join(" ").slice(0, 48).trim();
  return name || fallback;
}

/** The name to try after `name` is taken: "Launch" → "Launch 2" → "Launch 3". */
export function nextName(name: string): string {
  const match = /^(.*) (\d+)$/.exec(name);
  return match ? `${match[1]} ${Number(match[2]) + 1}` : `${name} 2`;
}

export function makeBrief(request: string, shape: Resolution): string {
  return [
    "Make this video from scratch in this project.",
    "",
    request.trim(),
    "",
    `The project is ${SHAPES[shape]}. Plan the scenes first, then build them with a music bed and sound effects timed to the scenes, then check the result with a snapshot before you finish.`,
  ].join("\n");
}

export function scriptBrief(script: string, shape: Resolution, voice: string | null): string {
  const narration = voice
    ? `Narrate it: make a voiceover of the script with \`hyperframes tts\` using the voice "${voice}", place it on the timeline, and time each scene to its words.`
    : "There is no narration: put the words on screen, and give each scene time to be read.";
  return [
    "Turn this script into a video in this project. Keep the script's words exactly as written.",
    "",
    "--- script ---",
    script.trim(),
    "--- end of script ---",
    "",
    narration,
    `The project is ${SHAPES[shape]}. Plan the scenes from the script first, then build them with a music bed and sound effects timed to the scenes, then check the result with a snapshot before you finish.`,
  ].join("\n");
}
