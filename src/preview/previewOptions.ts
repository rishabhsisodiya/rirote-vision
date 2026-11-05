// How the picture is shown: the backdrop behind it and the safe-zone guides.
// The person's choice is kept per Mac, not per project. Free of React.

export type Backdrop = "dark" | "grey" | "white" | "checker";
export type Guides = "off" | "action" | "social";

export interface PreviewOptions {
  backdrop: Backdrop;
  guides: Guides;
}

export const DEFAULT_OPTIONS: PreviewOptions = { backdrop: "dark", guides: "off" };

const KEY = "rirote-vision.preview-options";
const BACKDROPS: Backdrop[] = ["dark", "grey", "white", "checker"];
const GUIDES: Guides[] = ["off", "action", "social"];

export function readOptions(storage: Pick<Storage, "getItem"> | undefined): PreviewOptions {
  try {
    const saved = JSON.parse(storage?.getItem(KEY) ?? "{}") as Partial<PreviewOptions>;
    return {
      backdrop: BACKDROPS.includes(saved.backdrop as Backdrop) ? (saved.backdrop as Backdrop) : DEFAULT_OPTIONS.backdrop,
      guides: GUIDES.includes(saved.guides as Guides) ? (saved.guides as Guides) : DEFAULT_OPTIONS.guides,
    };
  } catch {
    return DEFAULT_OPTIONS;
  }
}

export function saveOptions(storage: Pick<Storage, "setItem"> | undefined, options: PreviewOptions): void {
  try {
    storage?.setItem(KEY, JSON.stringify(options));
  } catch {
    // A preference that cannot be kept is not worth an error.
  }
}

/** A rectangle as fractions of the frame. */
export interface Zone {
  label: string;
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * The guides for a frame. "action" is the broadcast 5 % margin; "social" adds
 * where a vertical feed puts its own buttons and caption, so key content stays
 * clear of them (top bar, the right-hand button column, the bottom caption).
 */
export function guideZones(guides: Guides, width: number, height: number): { safe: Zone | null; covered: Zone[] } {
  if (guides === "off" || width <= 0 || height <= 0) return { safe: null, covered: [] };
  const safe: Zone = { label: "Action safe", left: 0.05, top: 0.05, width: 0.9, height: 0.9 };
  if (guides === "action" || height <= width) return { safe, covered: [] };
  return {
    safe: { label: "Keep key content here", left: 0.06, top: 0.12, width: 0.76, height: 0.66 },
    covered: [
      { label: "App bar", left: 0, top: 0, width: 1, height: 0.1 },
      { label: "Buttons", left: 0.84, top: 0.35, width: 0.16, height: 0.45 },
      { label: "Caption and account", left: 0, top: 0.8, width: 1, height: 0.2 },
    ],
  };
}
