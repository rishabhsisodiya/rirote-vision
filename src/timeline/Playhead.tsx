import { useEffect, useRef } from "react";

interface PlayheadProps {
  /** The last time the player reported, in seconds. */
  time: number;
  playing: boolean;
  duration: number;
  pxPerSecond: number;
  /** Width of the lane labels the playhead starts after. */
  offset: number;
}

/**
 * How far the line may run ahead of the last time the player reported. The
 * player reports about every 100 ms, so a longer silence means the film has
 * stopped moving and the line must stop with it.
 */
const MAX_LEAD_SECONDS = 0.25;

/**
 * The line marking the current frame. The player reports its time about ten
 * times a second; while playing, the line is moved every frame from the last
 * report so it glides instead of stepping.
 */
export function Playhead({ time, playing, duration, pxPerSecond, offset }: PlayheadProps) {
  const line = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const place = (seconds: number) => {
      const at = Math.min(Math.max(seconds, 0), duration);
      if (line.current) line.current.style.transform = `translateX(${offset + at * pxPerSecond}px)`;
    };
    place(time);
    if (!playing) return;

    const reportedAt = performance.now();
    let frame = requestAnimationFrame(function advance(now) {
      place(time + Math.min(MAX_LEAD_SECONDS, (now - reportedAt) / 1000));
      frame = requestAnimationFrame(advance);
    });
    return () => cancelAnimationFrame(frame);
  }, [time, playing, duration, pxPerSecond, offset]);

  return <div ref={line} className="playhead" aria-hidden="true" />;
}
