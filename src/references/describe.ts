// Words for a reference's progress and figures. Free of React so it can be tested.

import type { Reference, ReferenceStep } from "../lib/types.ts";

const DOING: Record<ReferenceStep["name"], string> = {
  download: "Getting the video",
  shots: "Finding the cuts",
  keyframes: "Taking stills",
  transcribe: "Listening to the words",
  analyze: "Studying the format",
};

/** One line on where a study is: what it is doing, why it stopped, or that it is ready. */
export function progressLine(reference: Reference): string {
  switch (reference.status) {
    case "ready":
      return "Studied";
    case "queued":
      return "Waiting for the study before it";
    case "running": {
      const step = reference.steps.find((item) => item.state === "running");
      const done = reference.steps.filter((item) => item.state === "done").length;
      return step ? `${DOING[step.name]}… (${done + 1} of ${reference.steps.length})` : "Starting…";
    }
    case "failed": {
      const step = reference.steps.find((item) => item.state === "failed");
      if (!step) return "Stopped before it finished";
      const reason = step.error?.split("\n")[0] ?? "it stopped";
      return reason === "Stopped." ? "Stopped" : `${DOING[step.name]} failed: ${reason}`;
    }
  }
}

/** 66 → "1:06". */
export function clock(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return "–";
  const whole = Math.round(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = String(whole % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

const PLATFORMS: Record<Reference["platform"], string> = {
  youtube: "YouTube",
  instagram: "Instagram",
  tiktok: "TikTok",
  x: "X",
  web: "Web",
  file: "File",
};

export function platformName(platform: Reference["platform"]): string {
  return PLATFORMS[platform];
}
