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

/** What the assistant is asked when a project takes on a kit. */
export function restyleBrief(kitBrief: string): string {
  return [
    "Restyle this video to the brand kit below. Use its colours for backgrounds, text and accents, its fonts for headings and body text, and its logo where a logo belongs (usually the closing card). Keep the timing and the words as they are.",
    "",
    kitBrief,
    "",
    "Check the result with a snapshot before you finish.",
  ].join("\n");
}
