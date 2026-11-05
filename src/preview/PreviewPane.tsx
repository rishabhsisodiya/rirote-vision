import "./preview.css";

import { useEffect, useState } from "react";

import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { guideZones, readOptions, saveOptions, type Guides, type PreviewOptions } from "./previewOptions";
import { NotesLayer } from "./NotesLayer";
import { Transport } from "./Transport";
import type { Comment } from "../lib/types";
import type { PlayerControls } from "./usePlayer";
import type { PreviewState } from "./usePreview";

interface PreviewPaneProps {
  preview: PreviewState;
  player: PlayerControls;
  /** The composition's size, for the guides; null until it is known. */
  frame: { width: number; height: number } | null;
  /** The picture fills the window. */
  theater: boolean;
  onTheater: (on: boolean) => void;
  onRestart: () => void;
  onCheckSetup: () => void;
  notes: Comment[];
  onAddNote: (x: number, y: number, text: string) => Promise<boolean>;
}

/** The picture and its playback controls. */
export function PreviewPane({ preview, player, frame, theater, onTheater, onRestart, onCheckSetup, notes, onAddNote }: PreviewPaneProps) {
  if (preview.status === "starting") {
    return (
      <div className="preview__state" role="status">
        <Spinner />
        <p>Starting the preview</p>
      </div>
    );
  }
  if (preview.status === "error") {
    return (
      <div className="preview__state">
        <p className="preview__state-title">The preview is not running</p>
        <p className="preview__state-text">{preview.message}</p>
        {preview.needsSetup ? (
          <Button onClick={onCheckSetup}>Check Setup</Button>
        ) : (
          <Button onClick={onRestart}>Start Preview Again</Button>
        )}
      </div>
    );
  }
  return <Player player={player} frame={frame} theater={theater} onTheater={onTheater} onRestart={onRestart} notes={notes} onAddNote={onAddNote} />;
}

interface PlayerProps {
  player: PlayerControls;
  frame: PreviewPaneProps["frame"];
  theater: boolean;
  onTheater: (on: boolean) => void;
  onRestart: () => void;
  notes: Comment[];
  onAddNote: PreviewPaneProps["onAddNote"];
}

function Player({ player, frame, theater, onTheater, onRestart, notes, onAddNote }: PlayerProps) {
  const [placing, setPlacing] = useState(false);
  const { ready, error } = player.playback;
  const [options, setOptions] = useState<PreviewOptions>(() => readOptions(globalThis.localStorage));
  const change = (patch: Partial<PreviewOptions>) => {
    const next = { ...options, ...patch };
    setOptions(next);
    saveOptions(globalThis.localStorage, next);
  };

  // Escape leaves the full-window picture; F toggles it.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, select, [contenteditable], dialog")) return;
      if (event.key === "Escape" && theater) onTheater(false);
      if ((event.key === "f" || event.key === "F") && !event.metaKey && !event.ctrlKey) onTheater(!theater);
      if ((event.key === "n" || event.key === "N") && !event.metaKey && !event.ctrlKey) {
        if (player.playback.playing) player.toggle();
        setPlacing((now) => !now);
      }
      if (event.key === "Escape") setPlacing(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [theater, onTheater, player]);

  return (
    <>
      <div className={`preview__picture preview__picture--${options.backdrop}`}>
        {player.source ? <hyperframes-player ref={player.attach} src={player.source} /> : null}
        {frame && ready ? <Guides guides={options.guides} frame={frame} /> : null}
        {frame && ready ? (
          <NotesLayer
            frame={frame}
            notes={notes}
            playhead={player.playback.currentTime}
            placing={placing}
            onPlace={(x, y, text) => onAddNote(x, y, text)}
            onDone={() => setPlacing(false)}
          />
        ) : null}
        {error ? (
          <div className="preview__state preview__state--over">
            <p className="preview__state-title">The composition could not be shown</p>
            <p className="preview__state-text">{error}</p>
            <Button onClick={onRestart}>Start Preview Again</Button>
          </div>
        ) : !ready ? (
          <div className="preview__state preview__state--over" role="status">
            <Spinner />
            <p>Loading the composition</p>
          </div>
        ) : null}
      </div>
      <Transport
        player={player}
        options={options}
        onOptions={change}
        theater={theater}
        onTheater={onTheater}
        tall={!!frame && frame.height > frame.width}
        placing={placing}
        onPlacing={(on) => {
          if (on && player.playback.playing) player.toggle();
          setPlacing(on);
        }}
      />
    </>
  );
}

/** Safe-zone guides over the picture, sized to the frame inside the player. */
function Guides({ guides, frame }: { guides: Guides; frame: { width: number; height: number } }) {
  const { safe, covered } = guideZones(guides, frame.width, frame.height);
  if (!safe) return null;
  const box = (zone: { left: number; top: number; width: number; height: number }) => ({
    left: `${zone.left * 100}%`,
    top: `${zone.top * 100}%`,
    width: `${zone.width * 100}%`,
    height: `${zone.height * 100}%`,
  });
  return (
    <div className="guides" aria-hidden="true">
      <div className="guides__frame" style={{ "--ratio": `${frame.width} / ${frame.height}`, "--aspect": frame.width / frame.height } as React.CSSProperties}>
        {covered.map((zone) => (
          <span key={zone.label} className="guides__covered" style={box(zone)}>
            {zone.label}
          </span>
        ))}
        <span className="guides__safe" style={box(safe)}>
          <span>{safe.label}</span>
        </span>
      </div>
    </div>
  );
}
