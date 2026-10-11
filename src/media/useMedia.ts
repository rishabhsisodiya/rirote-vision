import { useCallback, useEffect, useRef, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { MediaItem } from "../lib/types";

export type MediaState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; items: MediaItem[] };

/**
 * The project's media files. `revision` changes when the project's files
 * change, and the list is read again.
 */
export function useMedia(projectPath: string, revision: string | null) {
  const [state, setState] = useState<MediaState>({ status: "loading" });
  const [importing, setImporting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const latest = useRef(0);

  const reload = useCallback(async () => {
    const request = ++latest.current;
    try {
      const items = await api.listMedia(projectPath);
      if (request === latest.current) setState({ status: "ready", items });
    } catch (cause) {
      if (request === latest.current) setState({ status: "error", message: errorMessage(cause) });
    }
  }, [projectPath]);

  useEffect(() => {
    void reload();
  }, [reload, revision]);

  /** Copies files into the project. Resolves to the paths of the copies. */
  const importFiles = useCallback(
    async (sources: string[]): Promise<string[]> => {
      if (sources.length === 0) return [];
      setImporting(true);
      setNotice(null);
      try {
        const added = await api.importMedia(projectPath, sources);
        await reload();
        return added.map((item) => item.path);
      } catch (cause) {
        setNotice(errorMessage(cause));
        return [];
      } finally {
        setImporting(false);
      }
    },
    [projectPath, reload],
  );

  const reveal = useCallback(
    (file: string) => {
      api.revealMedia(projectPath, file).catch((cause) => setNotice(errorMessage(cause)));
    },
    [projectPath],
  );

  const dismissNotice = useCallback(() => setNotice(null), []);
  return { state, importing, notice, setNotice, dismissNotice, reload, importFiles, reveal };
}

export type MediaControls = ReturnType<typeof useMedia>;
