import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import { api, AppError, errorMessage } from "../lib/ipc";
import type { PreviewChanged, PreviewSession } from "../lib/types";

export type PreviewState =
  | { status: "starting" }
  | { status: "error"; message: string; needsSetup: boolean }
  | { status: "ready"; session: PreviewSession; signature: string };

/**
 * Runs the preview server for one project while the calling component is on
 * screen, and follows the project's changes on disk through `signature`.
 */
export function usePreview(projectPath: string) {
  const [state, setState] = useState<PreviewState>({ status: "starting" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let current = true;
    setState({ status: "starting" });

    // "Start again" asks for a new server; a first start shares a running one.
    const started = api.startPreview(projectPath, attempt > 0);
    started
      .then((session) => {
        if (current) setState({ status: "ready", session, signature: session.signature });
      })
      .catch((cause) => {
        if (!current) return;
        setState({
          status: "error",
          message: errorMessage(cause),
          needsSetup: cause instanceof AppError && cause.code === "tool_missing",
        });
      });

    const changed = listen<PreviewChanged>("preview:changed", ({ payload }) => {
      if (payload.projectPath !== projectPath) return;
      setState((now) => (now.status === "ready" ? { ...now, signature: payload.signature } : now));
    });
    const stopped = listen<{ projectPath: string }>("preview:stopped", ({ payload }) => {
      if (payload.projectPath !== projectPath) return;
      setState({
        status: "error",
        message: "The preview server stopped unexpectedly.",
        needsSetup: false,
      });
    });

    return () => {
      current = false;
      void changed.then((unlisten) => unlisten());
      void stopped.then((unlisten) => unlisten());
      // Give back exactly the hold this view took, whenever it arrives.
      started.then((session) => api.releasePreview(session.lease)).catch(() => {});
    };
  }, [projectPath, attempt]);

  const restart = useCallback(() => setAttempt((count) => count + 1), []);
  return { state, restart };
}
