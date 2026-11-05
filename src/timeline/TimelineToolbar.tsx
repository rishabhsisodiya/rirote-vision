import { formatDuration } from "../lib/format";
import type { Clip } from "../lib/types";
import type { ReactNode } from "react";

import { Button, IconButton } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { clipName } from "./model";

interface TimelineToolbarProps {
  selected: Clip | null;
  canUndo: boolean;
  canRedo: boolean;
  canSplit: boolean;
  busy: boolean;
  assistantBusy: boolean;
  zoomed: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onSplit: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onZoom: (factor: number) => void;
  onFit: () => void;
  /** Opens the catalogue: transitions, titles, overlays and more. */
  onAdd: () => void;
  /** Shown in place of the title: the Timeline / Transcript switch. */
  lead?: ReactNode;
}

export function TimelineToolbar(props: TimelineToolbarProps) {
  const { selected, busy } = props;
  const editable = !!selected?.editable && !busy;
  return (
    <div className="timeline__toolbar" role="toolbar" aria-label="Timeline">
      {props.lead ?? <h2 className="timeline__title">Timeline</h2>}
      <div className="timeline__group">
        <IconButton icon="undo" label="Undo (⌘Z)" disabled={!props.canUndo || busy} onClick={props.onUndo} />
        <IconButton icon="redo" label="Redo (⇧⌘Z)" disabled={!props.canRedo || busy} onClick={props.onRedo} />
      </div>
      <div className="timeline__group">
        <IconButton
          icon="split"
          label="Split at playhead (⌘B)"
          disabled={!editable || !props.canSplit}
          onClick={props.onSplit}
        />
        <IconButton icon="duplicate" label="Duplicate (⌘D)" disabled={!editable} onClick={props.onDuplicate} />
        <IconButton icon="trash" label="Delete (⌫)" disabled={!editable} onClick={props.onDelete} />
      </div>
      <div className="timeline__group">
        <Button
          variant="ghost"
          icon="plus"
          disabled={props.assistantBusy}
          title={selected ? "Add a transition, title or overlay after the selected clip" : "Add a transition, title or overlay at the playhead"}
          onClick={props.onAdd}
        >
          {selected ? "Add After" : "Add"}
        </Button>
      </div>

      <p className="timeline__selection" aria-live="polite">
        {busy ? <Spinner label={props.assistantBusy ? undefined : "Applying the edit"} /> : null}
        {props.assistantBusy ? (
          <span>The assistant is editing</span>
        ) : selected ? (
          <>
            <strong>{clipName(selected)}</strong>
            <span>
              {formatDuration(selected.start)} – {formatDuration(selected.end)} ·{" "}
              {formatDuration(selected.end - selected.start)}
            </span>
            {selected.editable ? null : <span>Nested, view only</span>}
          </>
        ) : null}
      </p>

      <div className="timeline__group">
        <IconButton icon="zoomOut" label="Zoom out" disabled={!props.zoomed} onClick={() => props.onZoom(1 / 1.5)} />
        <IconButton icon="zoomIn" label="Zoom in" onClick={() => props.onZoom(1.5)} />
        <IconButton icon="fit" label="Fit the whole timeline" disabled={!props.zoomed} onClick={props.onFit} />
      </div>
    </div>
  );
}
