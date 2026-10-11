import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { RecentProject } from "../lib/types";

export type RecentState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; projects: RecentProject[] };

export function useRecentProjects() {
  const [state, setState] = useState<RecentState>({ status: "loading" });

  const reload = useCallback(async () => {
    try {
      setState({ status: "ready", projects: await api.recentProjects() });
    } catch (cause) {
      setState({ status: "error", message: errorMessage(cause) });
    }
  }, []);

  const forget = useCallback(
    async (path: string) => {
      await api.forgetProject(path);
      await reload();
    },
    [reload],
  );

  useEffect(() => {
    void reload();
  }, [reload]);

  return { state, reload, forget };
}
