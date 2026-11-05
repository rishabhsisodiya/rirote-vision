import { convertFileSrc } from "@tauri-apps/api/core";
import { useState, type FormEvent } from "react";

import { useFileDrop } from "../media/useFileDrop";
import { api, errorMessage } from "../lib/ipc";
import type { ProjectSummary, Reference } from "../lib/types";
import { pickVideo } from "../projects/pickFolder";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { clock, platformName, progressLine } from "./describe";
import { UseReference } from "./UseReference";

const VIDEO = /\.(mp4|mov|webm|mkv|m4v)$/i;

interface ReferenceFormProps {
  /** The reference picked here or in the gallery. */
  selected: Reference | null;
  onSelect: (id: string | null) => void;
  onStarted: (project: ProjectSummary, brief: string) => void;
  onCheckSetup: () => void;
}

/** Home's Reference tab: bring a video in, watch it being studied, then use it. */
export function ReferenceForm({ selected, onSelect, onStarted, onCheckSetup }: ReferenceFormProps) {
  const [link, setLink] = useState("");
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<{ message: string; setup: boolean } | null>(null);

  const add = async (input: { link?: string; file?: string }) => {
    setAdding(true);
    setError(null);
    try {
      const reference = await api.importReference(input);
      onSelect(reference.id);
      setLink("");
    } catch (cause) {
      const missing = cause instanceof Error && "code" in cause && cause.code === "tool_missing";
      setError({ message: errorMessage(cause), setup: missing });
    } finally {
      setAdding(false);
    }
  };

  const dropping = useFileDrop((paths) => {
    const video = paths.find((path) => VIDEO.test(path));
    if (video) void add({ file: video });
    else setError({ message: "Drop a video file: MP4, MOV, WebM or MKV.", setup: false });
  });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (link.trim() && !adding) void add({ link });
  };

  if (selected) {
    return <Selected reference={selected} onClear={() => onSelect(null)} onStarted={onStarted} />;
  }

  return (
    <div className="home__form">
      <button
        type="button"
        className={`drop-zone${dropping ? " drop-zone--over" : ""}`}
        disabled={adding}
        onClick={async () => {
          const file = await pickVideo("Choose a reference video");
          if (file) void add({ file });
        }}
      >
        <Icon name="export" size={20} />
        <strong>{dropping ? "Drop to study it" : "Drop a reference video"}</strong>
        <span>or click to choose one · MP4, MOV or WebM</span>
      </button>

      <div className="or-rule" aria-hidden="true">
        <span>or</span>
      </div>

      <form className="field" onSubmit={submit}>
        <label className="field__label" htmlFor="reference-link">
          Paste a link
        </label>
        <div className="field__row">
          <input
            id="reference-link"
            type="text"
            value={link}
            placeholder="youtube.com/shorts/…  instagram.com/reel/…  tiktok.com/…"
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setLink(event.target.value)}
          />
          <Button type="submit" busy={adding} disabled={!link.trim()}>
            Study it
          </Button>
        </div>
        <span className="field__help">YouTube, Instagram, TikTok, X and most video sites.</span>
      </form>

      {error ? (
        <div className="reference-error">
          <Notice message={error.message} onDismiss={() => setError(null)} />
          {error.setup ? <Button onClick={onCheckSetup}>Open Check Setup</Button> : null}
        </div>
      ) : null}

      <p className="home__hint">
        It is downloaded to this Mac and studied for its shots, pacing, hook and captions. Your video is built new from
        that study: the reference's footage, voice, music and words are never used.
      </p>
    </div>
  );
}

function Selected({ reference, onClear, onStarted }: { reference: Reference; onClear: () => void; onStarted: ReferenceFormProps["onStarted"] }) {
  const working = reference.status === "queued" || reference.status === "running";
  return (
    <div className="home__form">
      <div className="reference-pick">
        <span className="reference-pick__still">
          {reference.poster ? <img src={convertFileSrc(reference.poster)} alt="" /> : working ? <Spinner /> : <Icon name="reference" size={20} />}
        </span>
        <div className="reference-pick__text">
          <strong>{reference.title ?? reference.source}</strong>
          <span>
            {platformName(reference.platform)}
            {reference.creator ? ` · ${reference.creator}` : ""} · <span className="mono">{clock(reference.duration)}</span>
          </span>
          <span className={`reference-pick__status reference-pick__status--${reference.status}`} role="status">
            {working ? <Spinner /> : null}
            {progressLine(reference)}
          </span>
        </div>
        <Button variant="ghost" onClick={onClear}>
          Change
        </Button>
      </div>

      {reference.status === "ready" ? <UseReference reference={reference} onStarted={onStarted} /> : null}
      {reference.status === "failed" ? (
        <div className="row">
          <Button onClick={() => void api.retryReference(reference.id).catch(() => {})}>Try Again</Button>
        </div>
      ) : null}
      {working ? (
        <div className="row">
          <Button variant="ghost" onClick={() => void api.stopReference(reference.id)}>
            Stop
          </Button>
          <span className="home__hint">A study takes a minute or two; you can leave this screen.</span>
        </div>
      ) : null}
    </div>
  );
}
