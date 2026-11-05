import { convertFileSrc } from "@tauri-apps/api/core";

import type { Reference } from "../lib/types";
import { Spinner } from "../ui/Spinner";
import { clock, platformName } from "./describe";
import type { ReferencesState } from "./useReferences";

interface ReferenceGalleryProps {
  library: ReferencesState;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/** "Pick a reference": the videos already studied, to use again. */
export function ReferenceGallery({ library, selectedId, onSelect }: ReferenceGalleryProps) {
  const references = library.status === "ready" ? library.references : [];

  return (
    <section className="gallery" aria-labelledby="gallery-heading">
      <header className="home__panel-head">
        <h2 id="gallery-heading">
          Your references · <span className="mono">{references.length}</span>
        </h2>
      </header>

      <div className="gallery__scroll">
        {library.status === "loading" ? <Spinner /> : null}
        {library.status === "error" ? <p className="home__state">{library.message}</p> : null}
        {library.status === "ready" && references.length === 0 ? (
          <p className="home__state">
            None yet. Paste a link or drop a video on the left. Each one you study is kept here to use again.
          </p>
        ) : null}
        <ul className="gallery__grid">
          {references.map((reference, index) => (
            <li key={reference.id}>
              <LibraryCard reference={reference} panel={index + 1} selected={reference.id === selectedId} onSelect={onSelect} />
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function LibraryCard({ reference, panel, selected, onSelect }: { reference: Reference; panel: number; selected: boolean; onSelect: (id: string) => void }) {
  return (
    <button type="button" className="ref-card" aria-pressed={selected} onClick={() => onSelect(reference.id)}>
      <span className="ref-card__thumb" data-panel={`R${panel}`}>
        {reference.poster ? <img src={convertFileSrc(reference.poster)} alt="" loading="lazy" /> : null}
        <span className="ref-card__source">{platformName(reference.platform)}</span>
        <span className="ref-card__time mono">{clock(reference.duration)}</span>
        {reference.status !== "ready" ? <span className={`ref-card__state ref-card__state--${reference.status}`}>{reference.status}</span> : null}
      </span>
      <span className="ref-card__title">{reference.title ?? reference.source}</span>
      <span className="ref-card__meta">{reference.creator ?? "Your file"}</span>
    </button>
  );
}
