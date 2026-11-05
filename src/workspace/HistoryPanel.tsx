import { useCallback, useEffect, useState } from "react";

import { formatOpened } from "../lib/format";
import { api, errorMessage } from "../lib/ipc";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";

type Entry = Awaited<ReturnType<typeof api.listHistory>>[number];

interface HistoryPanelProps {
  projectPath: string;
  /** Read again whenever the project's files change. */
  signature: string | null;
  assistantBusy: boolean;
}

/** The side panel's History tab: each assistant turn as one step that can be undone. */
export function HistoryPanel({ projectPath, signature, assistantBusy }: HistoryPanelProps) {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [undoing, setUndoing] = useState<string | null>(null);

  const load = useCallback(() => {
    api.listHistory(projectPath).then(setEntries, (cause) => setError(errorMessage(cause)));
  }, [projectPath]);
  useEffect(load, [load, signature, assistantBusy]);

  const undo = async (entry: Entry) => {
    setUndoing(entry.id);
    setError(null);
    try {
      await api.undoHistory(projectPath, entry.id);
      load();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setUndoing(null);
    }
  };

  return (
    <div className="history">
      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      {entries === null ? <Spinner label="Reading the history" /> : null}
      {entries?.length === 0 ? <p className="side__empty">Nothing recorded yet. Each assistant turn shows here as one step you can undo.</p> : null}
      <ol className="history__list">
        {entries?.map((entry) => (
          <li key={entry.id} className={`history__entry${entry.undone ? " history__entry--undone" : ""}`}>
            <span className={`history__who history__who--${entry.by === "agent" ? "ai" : "you"}`}>{entry.by === "agent" ? "AI" : "You"}</span>
            <span className="history__text">
              <strong>{entry.label || "Edit"}</strong>
              <span>
                {entry.endedAt ? formatOpened(Math.floor(entry.endedAt / 1000)) : "in progress"} · {entry.files.length} file{entry.files.length === 1 ? "" : "s"}
                {entry.undone ? " · undone" : ""}
              </span>
            </span>
            {entry.undone || entry.label.startsWith("Undid:") ? null : (
              <Button variant="ghost" busy={undoing === entry.id} disabled={assistantBusy || undoing !== null || !entry.endedAt} onClick={() => void undo(entry)}>
                Undo
              </Button>
            )}
          </li>
        ))}
      </ol>
    </div>
  );
}
