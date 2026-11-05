import "./transcript.css";

import { useEffect, useMemo, useState, type ReactNode } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Clip, Job } from "../lib/types";
import { clipName } from "../timeline/model";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { clipWords, pauses, rangesForWords, removedSeconds, shortenRanges, type FileWord, type Range } from "./words";

interface TranscriptPanelProps {
  projectPath: string;
  /** The clips with sound the transcript can follow. */
  clips: Clip[];
  /** The selected clip, when it has sound. */
  selected: Clip | null;
  /** Changes when the project's files change, so the transcript is read again. */
  signature: string | null;
  playhead: number;
  busy: boolean;
  jobs: Job[];
  lead: ReactNode;
  onSeek: (seconds: number) => void;
  onCut: (clip: string, ranges: Range[]) => Promise<void>;
  onTranscribe: (src: string) => Promise<string | null>;
  /** Opens the caption styles, for captions made from this transcript. */
  onCaptions: (clip: Clip, transcriptFile: string) => void;
  /** Asks the assistant for the best short pieces of this clip. */
  onMoments: (clip: Clip, transcriptFile: string) => void;
}

type Loaded = { status: "loading" } | { status: "none" } | { status: "error"; message: string } | { status: "ready"; words: FileWord[]; file: string };

const PAUSE_CHOICES = [0.5, 0.8, 1.2];
const KEEP = 0.3;

/** Edit by text: the words of one clip; delete words to cut them, shorten the pauses. */
export function TranscriptPanel(props: TranscriptPanelProps) {
  const { clips, selected } = props;
  const [chosenRef, setChosenRef] = useState<string | null>(null);
  const clip = selected ?? clips.find((candidate) => candidate.ref === chosenRef) ?? clips[0] ?? null;
  const [loaded, setLoaded] = useState<Loaded>({ status: "loading" });
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [minPause, setMinPause] = useState(0.8);
  const [error, setError] = useState<string | null>(null);

  const src = clip?.src ?? null;
  const transcribing = props.jobs.some((job) => job.kind === "transcribe" && (job.status === "queued" || job.status === "running") && src && job.label.includes(src.split("/").pop() ?? "\u0000"));

  useEffect(() => {
    if (!src) return;
    let current = true;
    setLoaded({ status: "loading" });
    api.readTranscript(props.projectPath, src).then(
      (found) => current && setLoaded(found ? { status: "ready", words: found.words, file: found.file } : { status: "none" }),
      (cause) => current && setLoaded({ status: "error", message: errorMessage(cause) }),
    );
    return () => {
      current = false;
    };
  }, [props.projectPath, src, props.signature, transcribing]);

  const words = useMemo(() => (clip && loaded.status === "ready" ? clipWords(clip, loaded.words) : []), [clip, loaded]);
  const gaps = useMemo(() => pauses(words, minPause), [words, minPause]);
  const fillers = words.filter((word) => word.filler);
  useEffect(() => setPicked(new Set()), [clip?.ref, loaded]);

  const cut = async (ranges: Range[]) => {
    if (!clip || ranges.length === 0) return;
    setError(null);
    try {
      await props.onCut(clip.ref, ranges);
      setPicked(new Set());
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const choose = (index: number, extend: boolean) => {
    if (extend && anchor !== null) {
      const [from, to] = anchor < index ? [anchor, index] : [index, anchor];
      setPicked(new Set(words.filter((word) => word.index >= from && word.index <= to).map((word) => word.index)));
    } else {
      setPicked((before) => {
        const next = new Set(before);
        if (next.has(index)) next.delete(index);
        else next.add(index);
        return next;
      });
      setAnchor(index);
    }
  };

  const pickedRanges = rangesForWords(words, picked);
  const fillerRanges = rangesForWords(words, new Set(fillers.map((word) => word.index)));
  const pauseRanges = shortenRanges(gaps, KEEP);

  return (
    <div className="transcript">
      <div className="transcript__bar">
        {props.lead}
        {clips.length > 1 && !selected ? (
          <select className="transport__select transcript__clip" value={clip?.ref ?? ""} aria-label="Clip" onChange={(event) => setChosenRef(event.target.value)}>
            {clips.map((candidate) => (
              <option key={candidate.ref} value={candidate.ref}>
                {clipName(candidate)}
              </option>
            ))}
          </select>
        ) : clip ? (
          <span className="transcript__clip-name">{clipName(clip)}</span>
        ) : null}
        <span className="transcript__spacer" />
        {words.length ? (
          <>
            <Button
              disabled={props.busy || pickedRanges.length === 0}
              title="Cut the chosen words (⌫). Click a word to choose it; Shift-click to choose a run."
              onClick={() => void cut(pickedRanges)}
            >
              Cut {picked.size ? `${picked.size} word${picked.size === 1 ? "" : "s"}` : "words"}
            </Button>
            <Button
              disabled={props.busy || loaded.status !== "ready" || !clip}
              title="Pick a caption style; the assistant captions this clip from its transcript"
              onClick={() => clip && loaded.status === "ready" && props.onCaptions(clip, loaded.file)}
            >
              Add Captions…
            </Button>
            <Button
              variant="ghost"
              disabled={props.busy || loaded.status !== "ready" || !clip}
              title="Ask the assistant for the strongest 15–45 s moments to cut as shorts"
              onClick={() => clip && loaded.status === "ready" && props.onMoments(clip, loaded.file)}
            >
              Find Moments
            </Button>
            <Button disabled={props.busy || fillers.length === 0} title="Cut um, uh, er and the like" onClick={() => void cut(fillerRanges)}>
              Remove fillers <span className="mono">{fillers.length}</span>
            </Button>
            <select className="transport__select" value={minPause} aria-label="Pauses longer than" onChange={(event) => setMinPause(Number(event.target.value))}>
              {PAUSE_CHOICES.map((choice) => (
                <option key={choice} value={choice}>
                  Pauses over {choice} s
                </option>
              ))}
            </select>
            <Button
              disabled={props.busy || pauseRanges.length === 0}
              title={`Shorten each pause to ${KEEP} s`}
              onClick={() => void cut(pauseRanges)}
            >
              Shorten <span className="mono">{gaps.length}</span> · −{removedSeconds(pauseRanges).toFixed(1)} s
            </Button>
          </>
        ) : null}
      </div>

      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}

      <div
        className="transcript__body"
        tabIndex={-1}
        onKeyDown={(event) => {
          if ((event.key === "Backspace" || event.key === "Delete") && pickedRanges.length && !props.busy) {
            event.preventDefault();
            void cut(pickedRanges);
          }
          if (event.key === "Escape") setPicked(new Set());
        }}
      >
        {!clip ? (
          <p className="side__empty">There is no sound or video on the timeline to transcribe yet.</p>
        ) : loaded.status === "loading" ? (
          <Spinner label="Reading the transcript" />
        ) : loaded.status === "error" ? (
          <Notice message={loaded.message} />
        ) : loaded.status === "none" || transcribing ? (
          <div className="side__empty">
            <p>{transcribing ? "Transcribing… the words appear here when it finishes." : `${clipName(clip)} has no transcript yet. It is made on this Mac.`}</p>
            {transcribing ? (
              <Spinner />
            ) : (
              <Button
                variant="primary"
                onClick={async () => {
                  const refused = await props.onTranscribe(clip.src ?? "");
                  if (refused) setError(refused);
                }}
              >
                Transcribe
              </Button>
            )}
          </div>
        ) : words.length === 0 ? (
          <p className="side__empty">No words were heard in this part of the clip.</p>
        ) : (
          <p className="transcript__words">
            {words.map((word, position) => {
              const gap = gaps.find((pause) => pause.after === word.index);
              const speaking = props.playhead >= word.start && props.playhead < (words[position + 1]?.start ?? word.end);
              return (
                <span key={word.index}>
                  <span
                    role="button"
                    tabIndex={0}
                    className={["word", word.filler ? "word--filler" : "", picked.has(word.index) ? "word--picked" : "", speaking ? "word--now" : ""].join(" ")}
                    title={`${word.start.toFixed(2)} s · click to choose, double-click to play from here`}
                    onClick={(event) => choose(word.index, event.shiftKey)}
                    onDoubleClick={() => props.onSeek(word.start)}
                  >
                    {word.text}
                  </span>{" "}
                  {gap ? (
                    <button type="button" className="pause" title="Play from this pause" onClick={() => props.onSeek(gap.start)}>
                      {(gap.end - gap.start).toFixed(1)}s
                    </button>
                  ) : null}
                  {gap ? " " : null}
                </span>
              );
            })}
          </p>
        )}
      </div>
    </div>
  );
}
