import { useState, type FormEvent } from "react";

import type { MusicStyle } from "../lib/types";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Notice } from "../ui/Notice";
import {
  defaultImpact,
  defaultSeconds,
  MAX_SECONDS,
  MIN_SECONDS,
  MUSIC_STYLES,
  musicProblem,
} from "./musicOptions";

const STYLE_KEY = "rirote-vision.music-style";

interface MusicDialogProps {
  /** The composition's length, for the length first offered. Null when unknown. */
  filmSeconds: number | null;
  /** Queues the bed. Resolves to why it was refused, or null. */
  onCreate: (style: MusicStyle, seconds: number, impactAt: number | null) => Promise<string | null>;
  onClose: () => void;
}

/** A style and a length in, an original music file out. */
export function MusicDialog({ filmSeconds, onCreate, onClose }: MusicDialogProps) {
  const [style, setStyle] = useState<MusicStyle>(() => {
    const saved = localStorage.getItem(STYLE_KEY);
    return MUSIC_STYLES.some((option) => option.value === saved) ? (saved as MusicStyle) : "steady";
  });
  const [seconds, setSeconds] = useState(() => String(defaultSeconds(filmSeconds)));
  /** Null until the person types one; the suggestion follows the length until then. */
  const [impact, setImpact] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const length = Number(seconds);
  const cinematic = style === "cinematic";
  const suggested = Number.isFinite(length) ? defaultImpact(length) : MIN_SECONDS;
  const impactAt = cinematic ? Number(impact ?? suggested) : null;
  const problem = seconds.trim() === "" ? "Give the bed a length." : musicProblem(style, length, impactAt);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (problem || sending) return;
    setSending(true);
    setError(null);
    const refused = await onCreate(style, length, impactAt);
    if (refused) {
      setError(refused);
      setSending(false);
    } else {
      localStorage.setItem(STYLE_KEY, style);
      onClose();
    }
  }

  return (
    <Dialog
      title="New Music Bed"
      onClose={onClose}
      locked={sending}
      footer={
        <>
          <Button onClick={onClose} disabled={sending}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" form="music" busy={sending} disabled={!!problem}>
            Create Music
          </Button>
        </>
      }
    >
      <form id="music" className="form" onSubmit={submit}>
        <fieldset className="field">
          <legend>Style</legend>
          <div className="choices choices--two">
            {MUSIC_STYLES.map((option) => (
              <label key={option.value} className="choice">
                <input
                  type="radio"
                  name="style"
                  checked={style === option.value}
                  onChange={() => setStyle(option.value)}
                />
                <span className="choice__label">{option.label}</span>
                <span className="choice__detail choice__detail--plain">{option.detail}</span>
              </label>
            ))}
          </div>
        </fieldset>

        <div className="field__pair">
          <div className="field">
            <label htmlFor="music-seconds">Length in seconds</label>
            <input
              id="music-seconds"
              type="number"
              min={MIN_SECONDS}
              max={MAX_SECONDS}
              step={1}
              value={seconds}
              aria-describedby="music-help"
              onChange={(event) => setSeconds(event.target.value)}
            />
          </div>
          {cinematic ? (
            <div className="field">
              <label htmlFor="music-impact">Impact at second</label>
              <input
                id="music-impact"
                type="number"
                min={MIN_SECONDS}
                step={0.5}
                value={impact ?? String(suggested)}
                aria-describedby="music-help"
                onChange={(event) => setImpact(event.target.value)}
              />
            </div>
          ) : null}
        </div>

        <p id="music-help" className={`field__help ${problem ? "field__help--error" : ""}`}>
          {problem ??
            "Made on this Mac from scratch, so it is original and free to publish. Saved in the project's music folder; use Add to Timeline to place it."}
        </p>
        {error ? <Notice message={error} /> : null}
      </form>
    </Dialog>
  );
}
