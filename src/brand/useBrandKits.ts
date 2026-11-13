import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { BrandKit } from "../lib/types";

export type BrandKitsState = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; kits: BrandKit[] };

/** The brand kit library. */
export function useBrandKits() {
  const [state, setState] = useState<BrandKitsState>({ status: "loading" });
  const reload = useCallback(async () => {
    try {
      setState({ status: "ready", kits: await api.listBrandKits() });
    } catch (cause) {
      setState({ status: "error", message: errorMessage(cause) });
    }
  }, []);
  useEffect(() => {
    void reload();
  }, [reload]);
  return { state, reload };
}

export type BrandKitsControls = ReturnType<typeof useBrandKits>;

/** A first prompt (Make, Script, Reference) with the chosen kit added. */
export function withKitBrief(brief: string, kitBrief: string): string {
  return `${brief}\n\nUse this brand throughout:\n${kitBrief}`;
}

/** A chat message with a kit picked from the composer's Brand kit menu attached. */
export function withAttachedKit(prompt: string, kitBrief: string): string {
  return [
    prompt,
    "",
    "Use the brand kit below for this. Its logo, photos, cut-outs and 3D avatars are already copied into the project; use each where its note says.",
    kitBrief,
  ].join("\n");
}

/** What the assistant is asked when a project takes on a kit. */
export function restyleBrief(kitBrief: string): string {
  return [
    "Restyle this video to the brand kit below. Use its colours for backgrounds, text and accents, its fonts for headings and body text, and its logo where a logo belongs (usually the closing card). Use the kit's photos, cut-outs and 3D avatars where their notes say, replacing other brands' images. Keep the timing and the words as they are, except a different brand's name or website, which become this kit's.",
    "",
    kitBrief,
    "",
    "Check the result with a snapshot before you finish.",
  ].join("\n");
}
