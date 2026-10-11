// Every keyboard shortcut the editor has, for the sheet that lists them.
// The keys are handled where they act (the menu, the player, the timeline);
// this list is what people are shown, so the two are changed together.

export interface Shortcut {
  keys: string[];
  does: string;
}

export interface ShortcutGroup {
  title: string;
  shortcuts: Shortcut[];
}

export const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: "Playback",
    shortcuts: [
      { keys: ["Space", "K"], does: "Play or pause" },
      { keys: ["L"], does: "Play faster" },
      { keys: ["J"], does: "Play slower" },
      { keys: ["←", "→"], does: "Back or forward one frame" },
      { keys: ["⇧←", "⇧→"], does: "Back or forward one second" },
      { keys: ["↑", "↓"], does: "Previous or next cut" },
      { keys: ["Home", "End"], does: "Start or end of the film" },
    ],
  },
  {
    title: "Editing",
    shortcuts: [
      { keys: ["S", "⌘B"], does: "Split the selected clip at the playhead" },
      { keys: ["⌘D"], does: "Duplicate the selected clip" },
      { keys: ["⌫"], does: "Delete the selected clip" },
      { keys: [",", "."], does: "Nudge the selected clip one frame" },
      { keys: ["⇧,", "⇧."], does: "Nudge the selected clip ten frames" },
      { keys: ["⌘Z", "⇧⌘Z"], does: "Undo or redo" },
      { keys: ["Esc"], does: "Deselect, or abandon a drag" },
    ],
  },
  {
    title: "Timeline view",
    shortcuts: [
      { keys: ["⌘=", "⌘-"], does: "Zoom in or out" },
      { keys: ["⇧Z"], does: "Fit the whole timeline" },
    ],
  },
  {
    title: "Project and window",
    shortcuts: [
      { keys: ["⌘N"], does: "New project" },
      { keys: ["⌘O"], does: "Open project" },
      { keys: ["⌘E"], does: "Export" },
      { keys: ["⇧⌘R"], does: "Show the project in Finder" },
      { keys: ["⇧⌘W"], does: "Close the project" },
      { keys: ["⌘0"], does: "Show or hide the project sidebar" },
      { keys: ["⌥⌘0"], does: "Show or hide the assistant" },
      { keys: ["⌘/"], does: "Show this list" },
    ],
  },
  {
    title: "Assistant",
    shortcuts: [
      { keys: ["Return"], does: "Send the request" },
      { keys: ["⇧Return"], does: "New line" },
    ],
  },
];
