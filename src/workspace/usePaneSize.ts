import { useCallback, useState } from "react";

/** A pane's size in pixels, remembered between launches. */
export function usePaneSize(name: string, fallback: number, min: number, max: number) {
  const key = `rirote-vision.pane.${name}`;
  const [size, setSize] = useState(() => {
    const saved = Number(localStorage.getItem(key));
    return Number.isFinite(saved) && saved >= min && saved <= max ? saved : fallback;
  });

  const resize = useCallback(
    (next: number) => {
      const clamped = Math.round(Math.min(max, Math.max(min, next)));
      setSize(clamped);
      localStorage.setItem(key, String(clamped));
    },
    [key, min, max],
  );

  const reset = useCallback(() => {
    setSize(fallback);
    localStorage.removeItem(key);
  }, [key, fallback]);

  return { size, min, max, resize, reset };
}

export type PaneSize = ReturnType<typeof usePaneSize>;
