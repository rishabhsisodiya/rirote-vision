import { useCallback, useEffect, useState } from "react";

import { api, AppError, errorMessage } from "../lib/ipc";
import type { ProjectInfo } from "../lib/types";

export type ProjectInfoState =
  | { status: "loading" }
  | { status: "error"; message: string; needsSetup: boolean }
  | { status: "ready"; info: ProjectInfo };

/** What the engine reports about the open project's composition. */
export function useProjectInfo(path: string) {
  const [state, setState] = useState<ProjectInfoState>({ status: "loading" });

  const load = useCallback(async () => {
    // A refresh keeps showing the last figures until the new ones arrive.
    setState((current) => (current.status === "ready" ? current : { status: "loading" }));
    try {
      setState({ status: "ready", info: await api.projectInfo(path) });
    } catch (cause) {
      setState({
        status: "error",
        message: errorMessage(cause),
        needsSetup: cause instanceof AppError && cause.code === "tool_missing",
      });
    }
  }, [path]);

  useEffect(() => {
    void load();
  }, [load]);

  return { state, reload: load };
}
