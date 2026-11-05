import { formatDuration } from "../lib/format";
import { IconButton } from "../ui/Button";
import type { Backdrop, Guides, PreviewOptions } from "./previewOptions";
import type { PlayerControls } from "./usePlayer";

interface TransportProps {
  player: PlayerControls;
  options: PreviewOptions;
  onOptions: (patch: Partial<PreviewOptions>) => void;
  theater: boolean;
  onTheater: (on: boolean) => void;
  /** A tall frame, where the social-feed guides apply. */
  tall: boolean;
  /** The next click on the picture adds a review note. */
  placing: boolean;
  onPlacing: (on: boolean) => void;
}

/** Play, scrub and step controls under the picture, and how the picture is shown. */
export function Transport({ player, options, onOptions, theater, onTheater, tall, placing, onPlacing }: TransportProps) {
  const { playback, seek, toggle, step, setMuted, setVolume, setLooping, setRate } = player;
  const { ready, playing, currentTime, duration, muted, looping, rate, volume } = playback;

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
        <input
          className="transport__scrubber transport__volume"
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={muted ? 0 : volume}
          disabled={!ready}
          aria-label="Preview volume"
          title="Preview volume (exports are not affected)"
          style={{ "--played": `${(muted ? 0 : volume) * 100}%` } as React.CSSProperties}
          onChange={(event) => {
            if (muted) setMuted(false);
            setVolume(Number(event.target.value));
          }}
        />
      </div>

      <div className="transport__buttons transport__view" role="group" aria-label="Preview options">
        <select
          className="transport__select"
          value={options.guides}
          aria-label="Safe-zone guides"
          title="Safe-zone guides"
          onChange={(event) => onOptions({ guides: event.target.value as Guides })}
        >
          <option value="off">No guides</option>
          <option value="action">Action safe</option>
          <option value="social" disabled={!tall}>
            Social feed{tall ? "" : " (tall frames)"}
          </option>
        </select>
        <select
          className="transport__select"
          value={options.backdrop}
          aria-label="Backdrop behind the picture"
          title="Backdrop behind the picture"
          onChange={(event) => onOptions({ backdrop: event.target.value as Backdrop })}
        >
          <option value="dark">Dark</option>
          <option value="grey">Grey</option>
          <option value="white">White</option>
          <option value="checker">Checker (see transparency)</option>
        </select>
        <IconButton
          icon="compose"
          label={placing ? "Cancel the note (Esc)" : "Add a review note: click the picture (N)"}
          pressed={placing}
          disabled={!ready}
          onClick={() => onPlacing(!placing)}
        />
        <IconButton
          icon="fit"
          label={theater ? "Leave full-window preview (Esc)" : "Full-window preview (F)"}
          pressed={theater}
          onClick={() => onTheater(!theater)}
        />
      </div>
    </div>
  );
}
