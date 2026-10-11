// What an export can be, worked out from the composition. Free of React so
// it can be tested.

import type { ExportFormat, ExportQuality, ProjectInfo } from "../lib/types";

export const FORMATS: { value: ExportFormat; label: string; detail: string }[] = [
  { value: "mp4", label: "MP4", detail: "H.264. Plays everywhere; the one to post." },
  { value: "mov", label: "MOV", detail: "ProRes 4444 with transparency. Large; for other editors." },
  { value: "webm", label: "WebM", detail: "With transparency. For the web and overlays." },
  { value: "gif", label: "GIF", detail: "No sound. Best kept short and at 15 frames a second." },
];

export const QUALITIES: { value: ExportQuality; label: string; detail: string }[] = [
  { value: "draft", label: "Draft", detail: "Fast, for checking the cut." },
  { value: "standard", label: "Standard", detail: "For posting." },
  { value: "delivery", label: "Delivery", detail: "Highest quality; larger and slower." },
];

export const FRAME_RATES = [24, 25, 30, 50, 60];

const SHAPES: Record<string, { name: string; fourK: string; fourKSize: string }> = {
  "1920x1080": { name: "Landscape", fourK: "landscape-4k", fourKSize: "3840 × 2160" },
  "1080x1920": { name: "Portrait", fourK: "portrait-4k", fourKSize: "2160 × 3840" },
  "1080x1080": { name: "Square", fourK: "square-4k", fourKSize: "2160 × 2160" },
};

export interface SizeOption {
  /** Passed to the engine; null keeps the composition's own size. */
  value: string | null;
  label: string;
}

/**
 * The sizes a composition can be exported at: its own, and a 4K version
 * when it has one of the standard shapes. Changing the shape itself is a
 * re-layout, which is an edit, not an export setting.
 */
export function sizeOptions(info: Pick<ProjectInfo, "width" | "height"> | null): SizeOption[] {
  if (!info) return [{ value: null, label: "Composition size" }];
  const own = { value: null, label: `Composition size · ${info.width} × ${info.height}` };
  const shape = SHAPES[`${info.width}x${info.height}`];
  return shape ? [own, { value: shape.fourK, label: `4K ${shape.name} · ${shape.fourKSize}` }] : [own];
}

/** A file name made from the project's name: what the backend will accept. */
export function defaultName(projectName: string): string {
  const cleaned = projectName
    .replace(/[^\p{L}\p{N} _().-]+/gu, " ")
    .replace(/\s+/g, " ")
    .replace(/^[.\s-]+/, "")
    .replace(/[.\s]+$/, "")
    .slice(0, 80)
    .trim();
  return cleaned || "export";
}

/** Why a typed name will not do, or null when it will. Mirrors `file_name` in render.rs. */
export function nameProblem(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Give the file a name.";
  if (/^[.-]/.test(trimmed) || /[^\p{L}\p{N} _().-]/u.test(trimmed)) {
    return "Use letters, numbers, spaces, hyphens or underscores.";
  }
  return trimmed.length > 80 ? "Keep the name to 80 characters." : null;
}
