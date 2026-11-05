import { useState } from "react";

import type { Clip, MediaItem } from "../lib/types";
import { Button } from "../ui/Button";

/** Swap a clip's media for another file in the project, by asking the assistant. */
export function ReplaceMedia({ clip, items, disabled, onReplace }: { clip: Clip; items: MediaItem[]; disabled: boolean; onReplace: (file: string) => void }) {
  const kind = clip.kind === "audio" ? "audio" : "video";
  const choices = items.filter((item) => item.kind === kind && item.path !== clip.src);
  const [file, setFile] = useState("");
  if (choices.length === 0) return null;
  return (
    <section className="look" aria-labelledby="replace-heading">
      <h3 id="replace-heading" className="pane__heading">
        Replace media
      </h3>
      <div className="look__row">
        <select className="transport__select look__select" value={file} aria-label="Replace with" onChange={(event) => setFile(event.target.value)}>
          <option value="">Replace with…</option>
          {choices.map((item) => (
            <option key={item.path} value={item.path}>
              {item.name}
            </option>
          ))}
        </select>
        <Button disabled={disabled || !file} onClick={() => onReplace(file)}>
          Replace
        </Button>
      </div>
    </section>
  );
}
