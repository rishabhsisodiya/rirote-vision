import { memo, type PointerEvent } from "react";

import { formatRulerTime, rulerStep, rulerTicks } from "./model";

interface TimelineRulerProps {
  duration: number;
  pxPerSecond: number;
  onSeek: (seconds: number) => void;
}

/** The time scale. Pressing or dragging on it moves the playhead. */
export const TimelineRuler = memo(function TimelineRuler({ duration, pxPerSecond, onSeek }: TimelineRulerProps) {
  const step = rulerStep(pxPerSecond);

  const seekTo = (event: PointerEvent<HTMLDivElement>) => {
    const { left } = event.currentTarget.getBoundingClientRect();
    onSeek((event.clientX - left) / pxPerSecond);
  };

  return (
    <div
      className="ruler"
      style={{ width: duration * pxPerSecond }}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        seekTo(event);
      }}
      onPointerMove={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) seekTo(event);
      }}
    >
      {rulerTicks(duration, step).map((tick) => (
        <span key={tick} className="ruler__tick" style={{ left: tick * pxPerSecond }}>
          {formatRulerTime(tick, step)}
        </span>
      ))}
    </div>
  );
});
