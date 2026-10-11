import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

export type MenuCommand =
  | "project.new"
  | "project.open"
  | "project.close"
  | "project.export"
  | "project.reveal"
  | "setup.check"
  | "edit.undo"
  | "edit.redo"
  | "clip.split"
  | "clip.duplicate"
  | "clip.delete"
  | "help.shortcuts"
  | "view.sidebar"
  | "view.assistant";

/** Runs `handler` when an item is chosen from the menu bar. */
export function useMenu(handler: (command: MenuCommand) => void): void {
  const latest = useRef(handler);
  latest.current = handler;

  useEffect(() => {
    const stop = listen<MenuCommand>("menu", (event) => latest.current(event.payload));
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);
}
