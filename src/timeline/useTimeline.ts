import { useCallback, useEffect, useRef, useState } from "react";

import { api, AppError, errorMessage, logToApp } from "../lib/ipc";
import type { EditOp, TimelineState } from "../lib/types";

export type TimelineView =
  | { status: "loading" }
  | { status: "error"; message: string; needsSetup: boolean }
  | ({ status: "ready" } & TimelineState);

/**
 * A project's timeline and the edits made to it. `signature` changes when the
 * project's files change, from this timeline or from anywhere else, and the
 * clips are read again.
 */
export function useTimeline(projectPath: string, signature: string | null) {
  const [view, setView] = useState<TimelineView>({ status: "loading" });
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  /** Only the newest request may set the view. */
  const latest = useRef(0);

  const show = useCallback((request: number, state: TimelineState) => {
    if (request === latest.current) setView({ status: "ready", ...state });
  }, []);

  const load = useCallback(async () => {
    const request = ++latest.current;
    try {
      const state = await api.readTimeline(projectPath);
      show(request, state);
      logToApp(`timeline read: ${state.timeline.clips.length} clips`);
    } catch (cause) {
      logToApp(`timeline error: ${errorMessage(cause)}`);
      if (request !== latest.current) return;
      setView({
        status: "error",
        message: errorMessage(cause),
        needsSetup: cause instanceof AppError && cause.code === "tool_missing",
      });
    }
  }, [projectPath, show]);

  useEffect(() => {
    void load();
  }, [load, signature]);

  /** Runs one change; a refusal leaves the clips as they were and says why. */
  const change = useCallback(
    async (action: () => Promise<TimelineState>) => {
      const request = ++latest.current;
      setBusy(true);
      setNotice(null);
      try {
        show(request, await action());
      } catch (cause) {
        setNotice(errorMessage(cause));
        void load();
      } finally {
        setBusy(false);
      }
    },
    [load, show],
  );

  const edit = useCallback(
    (op: EditOp) => change(() => api.editTimeline(projectPath, op)),
    [change, projectPath],
  );
  const undo = useCallback(() => change(() => api.undoTimeline(projectPath)), [change, projectPath]);
  const redo = useCallback(() => change(() => api.redoTimeline(projectPath)), [change, projectPath]);
  const dismissNotice = useCallback(() => setNotice(null), []);

  return { view, busy, notice, dismissNotice, reload: load, edit, undo, redo };
}

export type TimelineControls = ReturnType<typeof useTimeline>;
