import { useEffect, useRef, useState } from "react";

import type { Clip, EditOp } from "../lib/types";
import { MIN_CLIP } from "./model";

interface ClipInspectorProps {
  clip: Clip;
  duration: number;
  /** A track number nothing uses yet, offered as the upper limit. */
  freeTrack: number;
  disabled: boolean;
  onEdit: (op: EditOp) => void;
}

/** The selected clip's numbers, each one editable by typing. */
export function ClipInspector({ clip, duration, freeTrack, disabled, onEdit }: ClipInspectorProps) {
  const length = clip.end - clip.start;
  return (
    <div className="inspector" role="group" aria-label={`Settings for ${clip.label ?? clip.id}`}>
      <NumberField
        label="Start"
        unit="s"
        value={clip.start}
        min={0}
        max={Math.max(0, duration - length)}
        step={0.01}
        disabled={disabled}
        onCommit={(start) => onEdit({ type: "place", clip: clip.ref, start, track: clip.track })}
      />
      <NumberField
        label="Length"
        unit="s"
        value={length}
        min={MIN_CLIP}
        max={Math.max(MIN_CLIP, duration - clip.start)}
        step={0.01}
        disabled={disabled}
        onCommit={(next) => onEdit({ type: "trimEnd", clip: clip.ref, end: clip.start + next })}
      />
      <NumberField
        label="Track"
        value={clip.track}
        min={0}
        max={Math.max(freeTrack, clip.track)}
        step={1}
        disabled={disabled}
        onCommit={(track) => onEdit({ type: "place", clip: clip.ref, start: clip.start, track })}
      />
      {clip.volume === null ? null : (
        <NumberField
          label="Volume"
          unit="%"
          value={Math.round(clip.volume * 100)}
          min={0}
          max={200}
          step={1}
          disabled={disabled}
          onCommit={(percent) => onEdit({ type: "setVolume", clip: clip.ref, volume: percent / 100 })}
        />
      )}
    </div>
  );
}

interface NumberFieldProps {
  label: string;
  unit?: string;
  value: number;
  min: number;
  max: number;
  step: number;
  disabled: boolean;
  onCommit: (value: number) => void;
}

/**
 * A number that is sent when Return is pressed or the field is left, not on
 * every keystroke: each change is an edit to the project. Escape puts the
 * old number back.
 */
function NumberField({ label, unit, value, min, max, step, disabled, onCommit }: NumberFieldProps) {
  const shown = step >= 1 ? String(value) : value.toFixed(2);
  const [draft, setDraft] = useState(shown);
  useEffect(() => setDraft(shown), [shown]);
  /** Escape was pressed: the blur that follows must not send what was typed. */
  const abandoned = useRef(false);

  const commit = () => {
    if (abandoned.current) {
      abandoned.current = false;
      return;
    }
    const typed = Number(draft);
    if (draft.trim() === "" || !Number.isFinite(typed)) return setDraft(shown);
    const next = Math.min(max, Math.max(min, step >= 1 ? Math.round(typed) : typed));
    if (Math.abs(next - value) < step / 2) return setDraft(shown);
    onCommit(next);
  };

  return (
    <label className="inspector__field">
      <span>{label}</span>
      <input
        type="number"
        value={draft}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
          if (event.key === "Escape") {
            // Escape here means "put the number back", not "deselect the clip".
            event.stopPropagation();
            abandoned.current = true;
            setDraft(shown);
            event.currentTarget.blur();
          }
        }}
      />
      {unit ? <span className="inspector__unit">{unit}</span> : null}
    </label>
  );
}
