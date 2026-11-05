import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Reference } from "../lib/types";

export type ReferencesState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; references: Reference[] };

/** The reference library, kept current as studies move along. */
export function useReferences() {
  const [state, setState] = useState<ReferencesState>({ status: "loading" });

  const reload = useCallback(async () => {
    try {
      setState({ status: "ready", references: await api.listReferences() });
    } catch (cause) {
      setState({ status: "error", message: errorMessage(cause) });
    }
  }, []);

  useEffect(() => {
    void reload();
    const stop = listen<Reference[]>("references", ({ payload }) => setState({ status: "ready", references: payload }));
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, [reload]);

  return { state, reload };
}

export type ReferencesControls = ReturnType<typeof useReferences>;
