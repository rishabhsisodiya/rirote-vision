import { useRef, type KeyboardEvent, type PointerEvent } from "react";

import type { PaneSize } from "./usePaneSize";

/** How far an arrow key moves a divider, in pixels. */
const KEY_STEP = 16;

interface DividerProps {
  label: string;
  /** "vertical" is a bar between side-by-side panes; "horizontal" one between stacked panes. */
  orientation: "vertical" | "horizontal";
  pane: PaneSize;
  /** 1 when dragging right or down grows the pane, -1 when it shrinks it. */
  grows: 1 | -1;
  className?: string;
}

/**
 * The bar between two panes. Drag it to resize, double-click it to put the
 * size back, or focus it and use the arrow keys.
 */
export function Divider({ label, orientation, pane, grows, className = "" }: DividerProps) {
  const start = useRef<{ at: number; size: number } | null>(null);
  const along = (event: PointerEvent) => (orientation === "vertical" ? event.clientX : event.clientY);

  const onKey = (event: KeyboardEvent) => {
    const forward = orientation === "vertical" ? "ArrowRight" : "ArrowDown";
    const back = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
    if (event.key !== forward && event.key !== back) return;
    event.preventDefault();
    event.stopPropagation();
    pane.resize(pane.size + (event.key === forward ? KEY_STEP : -KEY_STEP) * grows);
  };

  return (
    <div
      className={`divider divider--${orientation} ${className}`}
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuenow={pane.size}
      aria-valuemin={pane.min}
      aria-valuemax={pane.max}
      title={`${label}. Drag to resize; double-click to reset.`}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        start.current = { at: along(event), size: pane.size };
      }}
      onPointerMove={(event) => {
        if (start.current && event.currentTarget.hasPointerCapture(event.pointerId)) {
          pane.resize(start.current.size + (along(event) - start.current.at) * grows);
        }
      }}
      onPointerUp={() => (start.current = null)}
      onPointerCancel={() => (start.current = null)}
      onDoubleClick={pane.reset}
      onKeyDown={onKey}
    />
  );
}
