import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { CatalogInstalled, CatalogItem, CatalogShelf } from "../lib/types";
import { matches } from "../shell/search";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";

const SHELVES: { id: CatalogShelf; label: string }[] = [
  { id: "transition", label: "Transitions" },
  { id: "title-card", label: "Titles" },
  { id: "overlay", label: "Overlays" },
  { id: "captions", label: "Captions" },
  { id: "texture", label: "Textures" },
];

interface CatalogDialogProps {
  projectPath: string;
  /** Where it will go, in words: "after scene-2" or "at 0:04". */
  where: string;
  onInstalled: (installed: CatalogInstalled, item: CatalogItem) => void;
  onClose: () => void;
  initialShelf?: CatalogShelf;
}

type ShelfState = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; items: CatalogItem[] };

/** The engine's ready-made blocks. Choosing one installs it and asks the assistant to place it. */
export function CatalogDialog({ projectPath, where, onInstalled, onClose, initialShelf = "transition" }: CatalogDialogProps) {
  const [shelf, setShelf] = useState<CatalogShelf>(initialShelf);
  const [state, setState] = useState<ShelfState>({ status: "loading" });
  const [query, setQuery] = useState("");
  const [adding, setAdding] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setState({ status: "loading" });
    api.listCatalog(shelf).then(
      (items) => current && setState({ status: "ready", items }),
      (cause) => current && setState({ status: "error", message: errorMessage(cause) }),
    );
    return () => {
      current = false;
    };
  }, [shelf]);

  const add = async (item: CatalogItem) => {
    setAdding(item.name);
    setError(null);
    try {
      onInstalled(await api.addFromCatalog(projectPath, item.name), item);
    } catch (cause) {
      setError(errorMessage(cause));
      setAdding(null);
    }
  };

  const items = state.status === "ready" ? state.items.filter((item) => matches(query, item.title, item.description, item.name)) : [];

  return (
    <Dialog title={`Add ${where}`} onClose={onClose} locked={adding !== null}>
      <div className="catalog">
        <div className="home__tabs" role="tablist" aria-label="Kind">
          {SHELVES.map((item) => (
            <button key={item.id} type="button" role="tab" aria-selected={shelf === item.id} onClick={() => setShelf(item.id)}>
              {item.label}
            </button>
          ))}
        </div>
        <input
          className="catalog__search"
          type="search"
          value={query}
          placeholder="Filter: zoom, glitch, whip…"
          aria-label="Filter the catalogue"
          onChange={(event) => setQuery(event.target.value)}
        />
        {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
        {state.status === "loading" ? <Spinner label="Reading the catalogue" /> : null}
        {state.status === "error" ? <Notice message={state.message} /> : null}
        {state.status === "ready" && items.length === 0 ? <p className="side__empty">Nothing matches “{query}”.</p> : null}
        <ul className="catalog__list">
          {items.map((item) => (
            <li key={item.name} className="catalog__item">
              <div>
                <strong>{item.title || item.name}</strong>
                {item.duration ? <span className="mono"> · {item.duration} s</span> : null}
                <p>{item.description}</p>
              </div>
              <Button busy={adding === item.name} disabled={adding !== null && adding !== item.name} onClick={() => void add(item)}>
                Add
              </Button>
            </li>
          ))}
        </ul>
        <p className="home__hint">The block's files are added to the project; the assistant then places it and checks the result.</p>
      </div>
    </Dialog>
  );
}
