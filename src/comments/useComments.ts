import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Comment } from "../lib/types";

/** The project's review notes. Each change answers with the whole list. */
export function useComments(projectPath: string) {
  const [notes, setNotes] = useState<Comment[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listComments(projectPath).then(setNotes, (cause) => setError(errorMessage(cause)));
  }, [projectPath]);

  const act = useCallback(async (change: () => Promise<Comment[]>) => {
    try {
      setNotes(await change());
      setError(null);
      return true;
    } catch (cause) {
      setError(errorMessage(cause));
      return false;
    }
  }, []);

  return {
    notes,
    error,
    dismissError: () => setError(null),
    add: (time: number, x: number | null, y: number | null, text: string) => act(() => api.addComment(projectPath, time, x, y, text)),
    resolve: (id: string, resolved: boolean) => act(() => api.updateComment(projectPath, id, { resolved })),
    remove: (id: string) => act(() => api.deleteComment(projectPath, id)),
  };
}

export type CommentControls = ReturnType<typeof useComments>;
