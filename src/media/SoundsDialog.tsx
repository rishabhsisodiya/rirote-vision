import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { FreeSound } from "../lib/types";
import { clock } from "../references/describe";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";

const LICENCES: Record<string, string> = {
  cc0: "CC0 · no credit needed",
  pdm: "Public domain",
  by: "CC BY · credit the creator",
  "by-sa": "CC BY-SA · credit, share alike",
  "by-nc": "CC BY-NC · not for commercial use",
  "by-nd": "CC BY-ND · no changes",
  "by-nc-sa": "CC BY-NC-SA · not for commercial use",
  "by-nc-nd": "CC BY-NC-ND · not for commercial use",
  sampling: "Sampling+",
};

interface SoundsDialogProps {
  projectPath: string;
  onAdded: (path: string) => void;
  onClose: () => void;
}

/** Free sound effects and music from Openverse, saved into the project with their credits. */
export function SoundsDialog({ projectPath, onAdded, onClose }: SoundsDialogProps) {
  const [query, setQuery] = useState("");
  const [commercial, setCommercial] = useState(true);
  const [found, setFound] = useState<FreeSound[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const search = async (event: FormEvent) => {
    event.preventDefault();
    if (!query.trim()) return;
    setSearching(true);
    setError(null);
    try {
      setFound(await api.searchSounds(query, commercial));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSearching(false);
    }
  };

  const add = async (sound: FreeSound) => {
    setAdding(sound.id);
    setError(null);
    try {
      const path = await api.downloadSound(projectPath, sound);
      setAdded((before) => [...before, sound.id]);
      onAdded(path);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setAdding(null);
    }
  };

  return (
    <Dialog title="Free Sounds and Music" onClose={onClose} locked={adding !== null}>
      <form className="sounds__search" onSubmit={search}>
        <input type="search" value={query} autoFocus placeholder="whoosh, click, calm piano, crowd cheer…" aria-label="Search sounds" onChange={(event) => setQuery(event.target.value)} />
        <Button type="submit" variant="primary" busy={searching} disabled={!query.trim()}>
          Search
        </Button>
      </form>
      <label className="field field--check">
        <input type="checkbox" checked={commercial} onChange={(event) => setCommercial(event.target.checked)} />
        <span>Only sounds I may use commercially</span>
      </label>
      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      {searching ? <Spinner label="Searching Openverse" /> : null}
      {found && found.length === 0 ? <p className="side__empty">Nothing found. Try other words.</p> : null}
      <ul className="catalog__list">
        {found?.map((sound) => (
          <li key={sound.id} className="catalog__item">
            <div>
              <strong>{sound.title}</strong>
              <span className="mono"> · {clock(sound.duration)}</span>
              <p>
                {sound.creator ? `${sound.creator} · ` : ""}
                {LICENCES[sound.license] ?? sound.license.toUpperCase()}
              </p>
            </div>
            <Button busy={adding === sound.id} disabled={added.includes(sound.id) || (adding !== null && adding !== sound.id)} onClick={() => void add(sound)}>
              {added.includes(sound.id) ? "Added" : "Add"}
            </Button>
          </li>
        ))}
      </ul>
      <p className="home__hint">
        From Openverse. Each file goes into assets/sounds/, and its credit line into assets/sounds/CREDITS.txt; put credits in your video's description when the licence asks.
      </p>
    </Dialog>
  );
}
