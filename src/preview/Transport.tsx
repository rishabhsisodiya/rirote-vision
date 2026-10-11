import { formatDuration } from "../lib/format";
import { IconButton } from "../ui/Button";
import type { PlayerControls } from "./usePlayer";

/** Play, scrub and step controls under the picture. */
export function Transport({ player }: { player: PlayerControls }) {
  const { playback, seek, toggle, step, setMuted, setLooping, setRate } = player;
  const { ready, playing, currentTime, duration, muted, looping, rate } = playback;

  return (
    <div className="transport" role="group" aria-label="Playback">
      <div className="transport__buttons">
        <IconButton icon="toStart" label="Go to start (Home)" disabled={!ready} onClick={() => seek(0)} />
        <IconButton icon="stepBack" label="Back one frame (←)" disabled={!ready} onClick={() => step(-1)} />
        <IconButton
          className="transport__play"
          icon={playing ? "pause" : "play"}
          label={playing ? "Pause (Space)" : "Play (Space)"}
          disabled={!ready}
          onClick={toggle}
        />
        <IconButton icon="stepForward" label="Forward one frame (→)" disabled={!ready} onClick={() => step(1)} />
        <IconButton icon="toEnd" label="Go to end (End)" disabled={!ready} onClick={() => seek(duration)} />
      </div>

      <input
        className="transport__scrubber"
        type="range"
        min={0}
        max={duration || 1}
        step={0.01}
        value={Math.min(currentTime, duration || 1)}
        disabled={!ready}
        aria-label="Playhead"
        aria-valuetext={`${formatDuration(currentTime)} of ${formatDuration(duration)}`}
        style={{ "--played": `${duration ? (currentTime / duration) * 100 : 0}%` } as React.CSSProperties}
        onChange={(event) => seek(Number(event.target.value))}
      />

      <p className="transport__time" aria-hidden="true">
        <span>{formatDuration(currentTime)}</span>
        <span className="transport__total"> / {formatDuration(duration)}</span>
      </p>

      <div className="transport__buttons">
        <button
          type="button"
          className={`transport__rate ${rate === 1 ? "" : "transport__rate--changed"}`}
          disabled={!ready}
          title="Playback speed. L is faster, J is slower; press to return to normal."
          aria-label={`Playback speed ${rate} times. Press to return to normal speed.`}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setRate(1)}
        >
          {rate}×
        </button>
        <IconButton
          icon="loop"
          label={looping ? "Stop looping" : "Loop playback"}
          pressed={looping}
          disabled={!ready}
          onClick={() => setLooping(!looping)}
        />
        <IconButton
          icon={muted ? "muted" : "sound"}
          label={muted ? "Turn sound on" : "Mute"}
          pressed={muted}
          disabled={!ready}
          onClick={() => setMuted(!muted)}
        />
      </div>
    </div>
  );
}
