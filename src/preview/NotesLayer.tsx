import { useState, type MouseEvent } from "react";

import type { Comment } from "../lib/types";
import { Button } from "../ui/Button";

interface NotesLayerProps {
  frame: { width: number; height: number };
  notes: Comment[];
  playhead: number;
  /** Clicking the frame places a note. */
  placing: boolean;
  onPlace: (x: number, y: number, text: string) => Promise<boolean>;
  onDone: () => void;
}

/** Grease-pencil marks for the notes at this moment, and the click that adds one. */
export function NotesLayer({ frame, notes, playhead, placing, onPlace, onDone }: NotesLayerProps) {
  const [draft, setDraft] = useState<{ x: number; y: number; text: string } | null>(null);
  const open = notes.filter((note) => !note.resolved);
  const shown = open.filter((note) => note.x !== null && note.y !== null && Math.abs(note.time - playhead) < 0.75);

  const place = (event: MouseEvent<HTMLDivElement>) => {
    if (!placing || draft) return;
    const box = event.currentTarget.getBoundingClientRect();
    setDraft({ x: (event.clientX - box.left) / box.width, y: (event.clientY - box.top) / box.height, text: "" });
  };

  return (
    <div className={`notes-layer${placing ? " notes-layer--placing" : ""}`}>
      <div
        className="guides__frame notes-layer__frame"
        style={{ "--ratio": `${frame.width} / ${frame.height}`, "--aspect": frame.width / frame.height } as React.CSSProperties}
        onClick={place}
      >
        {shown.map((note) => (
          <span key={note.id} className="pencil" style={{ left: `${note.x! * 100}%`, top: `${note.y! * 100}%` }} title={note.text}>
            {open.indexOf(note) + 1}
          </span>
        ))}
        {draft ? (
          <form
            className="note-draft"
            style={{ left: `${Math.min(draft.x, 0.6) * 100}%`, top: `${Math.min(draft.y, 0.75) * 100}%` }}
            onClick={(event) => event.stopPropagation()}
            onSubmit={async (event) => {
              event.preventDefault();
              if (!draft.text.trim()) return;
              if (await onPlace(draft.x, draft.y, draft.text)) {
                setDraft(null);
                onDone();
              }
            }}
          >
            <span className="pencil pencil--draft" style={{ left: 0, top: 0 }} />
            <textarea
              autoFocus
              rows={2}
              value={draft.text}
              placeholder="What should change here?"
              onChange={(event) => setDraft({ ...draft, text: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  setDraft(null);
                  onDone();
                }
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
            />
            <div className="row">
              <Button variant="primary" type="submit" disabled={!draft.text.trim()}>
                Add Note
              </Button>
              <Button
                variant="ghost"
                onClick={() => {
                  setDraft(null);
                  onDone();
                }}
              >
                Cancel
              </Button>
            </div>
          </form>
        ) : null}
      </div>
    </div>
  );
}
