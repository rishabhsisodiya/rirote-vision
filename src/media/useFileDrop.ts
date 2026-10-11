import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

/**
 * Files dragged from Finder onto the window. Returns true while a drag is
 * over the window; `onDrop` gets the dropped files' paths.
 */
export function useFileDrop(onDrop: (paths: string[]) => void): boolean {
  const [over, setOver] = useState(false);
  const latest = useRef(onDrop);
  latest.current = onDrop;

  useEffect(() => {
    const stop = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") setOver(true);
      else setOver(false);
      if (payload.type === "drop" && payload.paths.length > 0) latest.current(payload.paths);
    });
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);

  return over;
}
