import { open } from "@tauri-apps/plugin-dialog";

/** Asks for one folder. Resolves to null when the person cancels. */
export async function pickFolder(title: string, startAt?: string): Promise<string | null> {
  const chosen = await open({ directory: true, multiple: false, title, defaultPath: startAt });
  return typeof chosen === "string" ? chosen : null;
}

const MEDIA_EXTENSIONS = [
  "mp4", "mov", "webm", "mkv", "m4v",
  "png", "jpg", "jpeg", "webp", "gif", "svg", "avif",
  "wav", "mp3", "m4a", "aac", "ogg", "flac",
  "srt", "vtt",
];

/** Asks for media files. Resolves to an empty list when the person cancels. */
export async function pickFiles(title: string): Promise<string[]> {
  const chosen = await open({
    multiple: true,
    title,
    filters: [{ name: "Media", extensions: MEDIA_EXTENSIONS }],
  });
  if (!chosen) return [];
  return Array.isArray(chosen) ? chosen : [chosen];
}

