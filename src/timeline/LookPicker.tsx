import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Clip, Look } from "../lib/types";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";

interface LookPickerProps {
  projectPath: string;
  clip: Clip;
  disabled: boolean;
}

let cached: Look[] | null = null;

/** A colour look for a video clip, from the engine's tested presets. */
export function LookPicker({ projectPath, clip, disabled }: LookPickerProps) {
  const [looks, setLooks] = useState<Look[]>(cached ?? []);
  const [preset, setPreset] = useState("");
  const [strength, setStrength] = useState(80);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  useEffect(() => {
    if (cached) return;
    api.listLooks().then(
      (found) => {
        cached = found;
        setLooks(found);
      },
      (cause) => setMessage({ text: errorMessage(cause), error: true }),
    );
  }, []);

  const apply = async (choice: string | null) => {
    setBusy(true);
    setMessage(null);
    try {
      await api.applyLook(projectPath, clip.id, choice, strength / 100);
      setMessage({ text: choice ? "Look applied. The preview shows it once it reloads." : "Look removed.", error: false });
    } catch (cause) {
      setMessage({ text: errorMessage(cause), error: true });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="look" aria-labelledby="look-heading">
      <h3 id="look-heading" className="pane__heading">
        Look
      </h3>
      <div className="look__row">
        <select className="transport__select look__select" value={preset} aria-label="Look" onChange={(event) => setPreset(event.target.value)}>
          <option value="">Choose a look…</option>
          {looks.map((look) => (
            <option key={look.id} value={look.id}>
              {look.label || look.id}
            </option>
          ))}
        </select>
        <label className="look__strength">
          <span className="mono">{strength}%</span>
          <input type="range" min={10} max={100} step={5} value={strength} aria-label="Strength" onChange={(event) => setStrength(Number(event.target.value))} />
        </label>
      </div>
      <div className="row">
        <Button variant="primary" busy={busy} disabled={disabled || !preset} onClick={() => void apply(preset)}>
          Apply Look
        </Button>
        <Button variant="ghost" disabled={disabled || busy} onClick={() => void apply(null)}>
          Remove
        </Button>
      </div>
      {message ? <Notice message={message.text} onDismiss={() => setMessage(null)} /> : null}
    </section>
  );
}
