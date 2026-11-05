import "./references.css";

import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { ProjectSummary, ReferenceDetail } from "../lib/types";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { BlueprintView } from "./BlueprintView";
import { ReferenceVideo } from "./ReferenceVideo";
import { clock, platformName, progressLine } from "./describe";
import type { ReferencesState } from "./useReferences";
import { UseReference } from "./UseReference";

interface ReferencesScreenProps {
  library: ReferencesState;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onStarted: (project: ProjectSummary, brief: string) => void;
  onAdd: () => void;
}

/** The library of studied references, and what each study found. */
export function ReferencesScreen({ library, selectedId, onSelect, onStarted, onAdd }: ReferencesScreenProps) {
  const references = library.status === "ready" ? library.references : [];
  const current = references.find((reference) => reference.id === selectedId) ?? references[0] ?? null;

  return (
    <main className="screen references">
      <header className="screen__head" data-tauri-drag-region>
        <h1 className="screen__title">References</h1>
        <span className="screen__meta">
          {references.length} studied · kept on this Mac only
        </span>
      </header>
      <div className="references__body">
        <aside className="home__panel references__list" aria-label="Library">
          <div className="references__list-head">
            <Button icon="plus" onClick={onAdd}>
              Add a Reference
            </Button>
          </div>
          {library.status === "loading" ? <Spinner /> : null}
          {library.status === "error" ? <p className="home__state">{library.message}</p> : null}
          {library.status === "ready" && references.length === 0 ? (
            <p className="home__state">No references yet. Add one from a link or a file; it is studied for its format.</p>
          ) : null}
          <ul>
            {references.map((reference) => (
              <li key={reference.id}>
                <button
                  type="button"
                  className="references__row"
                  aria-current={reference.id === current?.id ? "true" : undefined}
                  onClick={() => onSelect(reference.id)}
                >
                  <span className="references__still">
                    {reference.poster ? <img src={convertFileSrc(reference.poster)} alt="" /> : <Icon name="reference" />}
                  </span>
                  <span className="references__text">
                    <strong>{reference.title ?? reference.source}</strong>
                    <span>
                      {platformName(reference.platform)} · <span className="mono">{clock(reference.duration)}</span>
                    </span>
                    <span className={`references__status references__status--${reference.status}`}>{progressLine(reference)}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </aside>
        <section className="home__panel references__detail" aria-label="Study">
          {current ? <Detail key={current.id} id={current.id} version={current.steps.map((s) => s.state).join()} onStarted={onStarted} onDeleted={() => onSelect(null)} /> : null}
        </section>
      </div>
    </main>
  );
}

function Detail({ id, version, onStarted, onDeleted }: { id: string; version: string; onStarted: ReferencesScreenProps["onStarted"]; onDeleted: () => void }) {
  const [detail, setDetail] = useState<ReferenceDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  // Read again whenever a step changes state.
  useEffect(() => {
    let current = true;
    api.getReference(id).then(
      (read) => current && setDetail(read),
      (cause) => current && setError(errorMessage(cause)),
    );
    return () => {
      current = false;
    };
  }, [id, version]);

  if (error) return <Notice message={error} />;
  if (!detail) return <Spinner />;
  const { reference, blueprint } = detail;
  const working = reference.status === "queued" || reference.status === "running";

  return (
    <div className="references__study">
      <header className="references__study-head">
        <div>
          <h2>{reference.title ?? reference.source}</h2>
          <p className="screen__meta">
            {platformName(reference.platform)}
            {reference.creator ? ` · ${reference.creator}` : ""} · {clock(reference.duration)}
            {reference.width ? ` · ${reference.width}×${reference.height}` : ""}
          </p>
          {reference.platform !== "file" ? <p className="references__source mono">{reference.source}</p> : null}
        </div>
        <div className="row">
          {working ? (
            <Button variant="ghost" onClick={() => void api.stopReference(id)}>
              Stop
            </Button>
          ) : null}
          {reference.status === "failed" ? <Button onClick={() => void api.retryReference(id).catch((cause) => setError(errorMessage(cause)))}>Try Again</Button> : null}
          {confirming ? (
            <>
              <Button variant="primary" className="button--danger" onClick={() => void api.deleteReference(id).then(onDeleted, (cause) => setError(errorMessage(cause)))}>
                Delete Study and Video
              </Button>
              <Button variant="ghost" onClick={() => setConfirming(false)}>
                Keep
              </Button>
            </>
          ) : (
            <Button variant="ghost" icon="trash" onClick={() => setConfirming(true)}>
              Delete
            </Button>
          )}
        </div>
      </header>

      {/* The video on the left, the study's progress and "make one like it" beside it. */}
      <div className={`references__lead ${reference.video ? "" : "references__lead--no-video"}`}>
        <ReferenceVideo key={reference.id} reference={reference} />
        <div className="references__lead-side">
          <ol className="steps" aria-label="Study steps">
            {reference.steps.map((step) => (
              <li key={step.name} className={`steps__item steps__item--${step.state}`} title={step.error ?? undefined}>
                <span className="steps__dot" aria-hidden="true" />
                {step.name}
                {step.seconds !== null && step.state === "done" ? <span className="mono"> {step.seconds.toFixed(1)}s</span> : null}
              </li>
            ))}
          </ol>
          {reference.status !== "ready" ? (
            <p className={`references__status references__status--${reference.status}`} role="status">
              {working ? <Spinner /> : null} {progressLine(reference)}
            </p>
          ) : null}
          {blueprint ? (
            <section className="references__use" aria-labelledby="use-heading">
              <h3 id="use-heading">Make a video in this format</h3>
              <UseReference reference={reference} onStarted={onStarted} />
            </section>
          ) : null}
        </div>
      </div>

      {blueprint ? <BlueprintView blueprint={blueprint} folder={reference.folder} /> : null}
    </div>
  );
}
