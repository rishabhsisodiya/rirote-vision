import { formatDuration } from "../lib/format";
import type { Comment } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { fixBrief } from "./brief";
import type { CommentControls } from "./useComments";

interface NotesPanelProps {
  comments: CommentControls;
  assistantBusy: boolean;
  onSeek: (seconds: number) => void;
  onAsk: (prompt: string) => void;
}

/** The side panel's Notes tab: review notes in time order, fixed by hand or by the assistant. */
export function NotesPanel({ comments, assistantBusy, onSeek, onAsk }: NotesPanelProps) {
  const open = comments.notes.filter((note) => !note.resolved);
  const done = comments.notes.filter((note) => note.resolved);
  const row = (note: Comment, number: number | null) => (
    <li key={note.id} className={`note${note.resolved ? " note--resolved" : ""}`}>
      <button type="button" className="note__time mono" title="Go to this moment" onClick={() => onSeek(note.time)}>
        {number !== null ? <span className="note__number">{number}</span> : null}
        {formatDuration(note.time)}
      </button>
      <p className="note__text">{note.text}</p>
      <div className="row">
        {note.resolved ? (
          <Button variant="ghost" onClick={() => void comments.resolve(note.id, false)}>
            Reopen
          </Button>
        ) : (
          <>
            <Button disabled={assistantBusy} onClick={() => onAsk(fixBrief([note]))}>
              Fix with AI
            </Button>
            <Button variant="ghost" onClick={() => void comments.resolve(note.id, true)}>
              Resolve
            </Button>
          </>
        )}
        <IconButton icon="trash" label="Delete note" onClick={() => void comments.remove(note.id)} />
      </div>
    </li>
  );

  return (
    <div className="notes">
      {comments.error ? <Notice message={comments.error} onDismiss={comments.dismissError} /> : null}
      {open.length > 1 ? (
        <Button variant="primary" disabled={assistantBusy} onClick={() => onAsk(fixBrief(open))}>
          Fix All {open.length} with AI
        </Button>
      ) : null}
      {comments.notes.length === 0 ? (
        <p className="side__empty">
          No notes yet. Press <kbd>N</kbd> or the pen under the picture, then click the frame where something needs changing.
        </p>
      ) : null}
      <ol className="notes__list">{open.map((note, index) => row(note, index + 1))}</ol>
      {done.length ? (
        <>
          <h3 className="pane__heading">Resolved · {done.length}</h3>
          <ol className="notes__list">{done.map((note) => row(note, null))}</ol>
        </>
      ) : null}
    </div>
  );
}
