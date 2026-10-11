import { useEffect } from "react";

import type { PlayerControls } from "./usePlayer";

/** True when the key belongs to a field or control that uses it itself. */
function claimedByFocus(target: EventTarget | null, key: string): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.closest("dialog")) return true;
  if (target instanceof HTMLInputElement) {
    return target.type !== "range" || key !== " ";
  }
  if (target instanceof HTMLTextAreaElement || target.isContentEditable) return true;
  return key === " " && target instanceof HTMLButtonElement;
}

/**
 * Space or K plays or pauses; L and J make playback faster and slower; the
 * arrows step a frame (a second with Shift); Home and End jump.
 */
export function usePlaybackKeys(player: PlayerControls): void {
  const { toggle, step, seek, shiftRate, playback } = player;
  const { duration } = playback;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (claimedByFocus(event.target, event.key)) return;
      const frames = event.shiftKey ? 30 : 1;
      switch (event.key) {
        case " ":
        case "k":
        case "K":
          toggle();
          break;
        case "l":
        case "L":
          shiftRate(1);
          break;
        case "j":
        case "J":
          shiftRate(-1);
          break;
        case "ArrowLeft":
          step(-frames);
          break;
        case "ArrowRight":
          step(frames);
          break;
        case "Home":
          seek(0);
          break;
        case "End":
          seek(duration);
          break;
        default:
          return;
      }
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle, step, seek, shiftRate, duration]);
}
