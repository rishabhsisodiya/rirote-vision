import { useEffect, useState } from "react";

import { api } from "../lib/ipc";
import type { Clip, MediaPeaks, MediaStill } from "../lib/types";

export interface MediaVisual {
  peaks?: MediaPeaks;
  stills?: MediaStill[];
}

/**
 * Waveforms and stills for the media the timeline shows, fetched once per
 * file and again when the project's files change. Failures leave a clip plain.
 */
export function useMediaVisuals(projectPath: string, clips: Clip[] | undefined, signature: string | null) {
  const [visuals, setVisuals] = useState<Map<string, MediaVisual>>(new Map());
  const wanted = (clips ?? [])
    .filter((clip) => clip.media && clip.src)
    .map((clip) => `${clip.kind}|${clip.src}`)
    .filter((key, index, all) => all.indexOf(key) === index)
    .sort()
    .join("\n");

  useEffect(() => {
    let current = true;
    const fetchAll = async () => {
      for (const entry of wanted.split("\n").filter(Boolean)) {
        const [kind, src] = entry.split("|") as [string, string];
        const visual: MediaVisual = {};
        // One file at a time: each read starts ffmpeg.
        visual.peaks = await api.mediaPeaks(projectPath, src).catch(() => undefined);
        if (kind === "video") visual.stills = await api.mediaStills(projectPath, src).catch(() => undefined);
        if (!current) return;
        setVisuals((before) => new Map(before).set(src, visual));
      }
    };
    void fetchAll();
    return () => {
      current = false;
    };
  }, [projectPath, wanted, signature]);

  return visuals;
}
