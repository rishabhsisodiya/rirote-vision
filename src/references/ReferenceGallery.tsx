import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { FoundVideo, Reference } from "../lib/types";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { clock, platformName, viewsLabel } from "./describe";
import type { ReferencesState } from "./useReferences";

type Source = "youtube" | "instagram";

type Results =
  | { status: "idle" }
  | { status: "searching"; query: string }
  | { status: "error"; message: string }
  | { status: "ready"; query: string; found: FoundVideo[] };

interface ReferenceGalleryProps {
  library: ReferencesState;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onPickFound: (video: FoundVideo) => Promise<void>;
}

/** "Pick a reference": search YouTube (or Instagram, if allowed), or reuse one already studied. */
export function ReferenceGallery({ library, selectedId, onSelect, onPickFound }: ReferenceGalleryProps) {
  const [query, setQuery] = useState("");
  const [source, setSource] = useState<Source>("youtube");
  const [instagram, setInstagram] = useState(false);
  const [results, setResults] = useState<Results>({ status: "idle" });
  const [picking, setPicking] = useState<string | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then((settings) => setInstagram(settings.instagramSearch), () => setInstagram(false));
  }, []);

  const search = async (event?: FormEvent) => {
    event?.preventDefault();
    const words = query.trim();
    if (!words) {
      setResults({ status: "idle" });
      return;
    }
    setResults({ status: "searching", query: words });
    try {
      setResults({ status: "ready", query: words, found: await api.searchReferences(words, source) });
    } catch (cause) {
      setResults({ status: "error", message: errorMessage(cause) });
    }
  };

  const pick = async (video: FoundVideo) => {
    setPicking(video.id);
    setPickError(null);
    try {
      await onPickFound(video);
    } catch (cause) {
      setPickError(errorMessage(cause));
    } finally {
      setPicking(null);
    }
  };

  const references = library.status === "ready" ? library.references : [];

  return (
    <section className="gallery" aria-labelledby="gallery-heading">
      <header className="home__panel-head">
        <h2 id="gallery-heading">Pick a reference</h2>
        <form className="gallery__search" role="search" onSubmit={search}>
          <Icon name="search" />
          <input
            type="search"
            value={query}
            aria-label={source === "instagram" ? "Instagram hashtag" : "Search YouTube"}
            placeholder={source === "instagram" ? "One hashtag, like motiongraphics" : "Search YouTube: product launch shorts"}
            onChange={(event) => setQuery(event.target.value)}
          />
        </form>
        <div className="home__chips" role="radiogroup" aria-label="Where to search">
          <button type="button" role="radio" className="chip" aria-checked={source === "youtube"} onClick={() => setSource("youtube")}>
            YouTube
          </button>
          <button
            type="button"
            role="radio"
            className="chip"
            aria-checked={source === "instagram"}
            disabled={!instagram}
            title={instagram ? "Searches with your browser's Instagram sign-in" : "Turn on Instagram search in Settings first"}
            onClick={() => setSource("instagram")}
          >
            Instagram <span className="tag">beta</span>
          </button>
        </div>
      </header>

      {pickError ? <Notice message={pickError} onDismiss={() => setPickError(null)} /> : null}

      <div className="gallery__scroll">
        {results.status === "searching" ? (
          <p className="home__state">
            <Spinner label={`Searching for “${results.query}”`} />
          </p>
        ) : null}
        {results.status === "error" ? <p className="home__state home__state--error">{results.message}</p> : null}
        {results.status === "ready" ? (
          results.found.length === 0 ? (
            <p className="home__state">Nothing found for “{results.query}”. Try other words.</p>
          ) : (
            <>
              <h3 className="gallery__label">
                Results for “{results.query}” · <span className="mono">{results.found.length}</span>
              </h3>
              <ul className="gallery__grid">
                {results.found.map((video, index) => (
                  <li key={`${video.platform}-${video.id}`}>
                    <button type="button" className="ref-card" disabled={picking !== null} onClick={() => void pick(video)}>
                      <span className="ref-card__thumb" data-panel={`P${index + 1}`}>
                        {video.thumbnail ? <img src={convertFileSrc(video.thumbnail)} alt="" loading="lazy" /> : null}
                        <span className="ref-card__source">{platformName(video.platform)}</span>
                        <span className="ref-card__time mono">{clock(video.duration)}</span>
                        {picking === video.id ? (
                          <span className="ref-card__busy">
                            <Spinner label="Adding" />
                          </span>
                        ) : null}
                      </span>
                      <span className="ref-card__title">{video.title}</span>
                      <span className="ref-card__meta">
                        {[video.creator, viewsLabel(video.views)].filter(Boolean).join(" · ")}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )
        ) : null}

        <h3 className="gallery__label">
          Your references · <span className="mono">{references.length}</span>
        </h3>
        {library.status === "loading" ? <Spinner /> : null}
        {library.status === "error" ? <p className="home__state">{library.message}</p> : null}
        {library.status === "ready" && references.length === 0 ? (
          <p className="home__state">
            None yet. Search above, paste a link, or drop a video on the left. Each one you study is kept here to use
            again.
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
