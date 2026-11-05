import "./media.css";

import { useState } from "react";

import { formatBytes, formatDuration } from "../lib/format";
import type { Job, MediaItem } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Icon, type IconName } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { canRemoveBackground, canTranscribe } from "./mediaActions";
import type { JobControls } from "./useJobs";
import type { MediaControls } from "./useMedia";

const KIND_ICON: Record<MediaItem["kind"], IconName> = {
  video: "video",
  image: "media",
  audio: "sound",
  captions: "captions",
};

interface MediaBinProps {
  media: MediaControls;
  jobs: JobControls;
  /** Where the project's files are served, for image thumbnails. Null until the preview runs. */
  filesUrl: string | null;
  /** The assistant is busy, so it cannot be asked to place a file yet. */
  assistantBusy: boolean;
  onImport: () => void;
  onVoiceover: () => void;
  onMusic: () => void;
  onPlace: (item: MediaItem) => void;
  /** Opens the free sound search. */
  onSounds: () => void;
}

/** The project's media files, the jobs that make more, and what to do with each. */
export function MediaBin(props: MediaBinProps) {
  const { media, jobs, filesUrl } = props;
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [kind, setKind] = useState<MediaItem["kind"] | "all">("all");

  const start = async (item: MediaItem, type: "transcribe" | "removeBackground") => {
    const refused = await jobs.start({ type, source: item.path });
    if (refused) media.setNotice(refused);
  };

  return (
    <div className="media">
      <header className="media__header">
        <h2 className="pane__heading">Media</h2>
        <IconButton icon="mic" label="New voiceover" onClick={props.onVoiceover} />
        <IconButton icon="music" label="New music bed" onClick={props.onMusic} />
        <IconButton icon="search" label="Find free sounds and music" onClick={props.onSounds} />
        <IconButton icon="plus" label="Import media" disabled={media.importing} onClick={props.onImport} />
      </header>

      {media.notice ? <Notice message={media.notice} onDismiss={media.dismissNotice} /> : null}

      {media.state.status === "ready" && media.state.items.length > 4 ? (
        <div className="media__filter">
          <input type="search" value={filter} placeholder="Filter files" aria-label="Filter files" onChange={(event) => setFilter(event.target.value)} />
          <select className="transport__select" value={kind} aria-label="Kind" onChange={(event) => setKind(event.target.value as typeof kind)}>
            <option value="all">All</option>
            <option value="video">Video</option>
            <option value="audio">Sound</option>
            <option value="image">Images</option>
            <option value="captions">Captions</option>
          </select>
        </div>
      ) : null}

      <div className="media__list">
        {media.state.status === "loading" ? (
          <p className="media__state">
            <Spinner label="Reading the project's media" />
          </p>
        ) : media.state.status === "error" ? (
          <div className="media__state">
            <p>{media.state.message}</p>
            <Button onClick={() => void media.reload()}>Try Again</Button>
          </div>
        ) : media.state.items.length === 0 ? (
          <p className="media__state">
            No media yet. Import files, or drop them anywhere on the window.
          </p>
        ) : (
          <ul>
            {media.state.items
              .filter((item) => (kind === "all" || item.kind === kind) && item.name.toLowerCase().includes(filter.trim().toLowerCase()))
              .map((item) => (
              <MediaRow
                key={item.path}
                item={item}
                open={item.path === selected}
                filesUrl={filesUrl}
                assistantBusy={props.assistantBusy}
                onToggle={() => setSelected(item.path === selected ? null : item.path)}
                onPlace={() => props.onPlace(item)}
                onTranscribe={() => void start(item, "transcribe")}
                onCutOut={() => void start(item, "removeBackground")}
                onReveal={() => media.reveal(item.path)}
                usedBy={() => media.usedBy(item.path)}
                onTrash={async () => {
                  if (await media.trash(item.path)) setSelected(null);
                }}
              />
            ))}
          </ul>
        )}
        {media.importing ? (
          <p className="media__state" role="status">
            <Spinner />
            Importing
          </p>
        ) : null}
      </div>

      {jobs.jobs.length > 0 ? <JobList jobs={jobs} /> : null}
    </div>
  );
}

interface MediaRowProps {
  item: MediaItem;
  open: boolean;
  filesUrl: string | null;
  assistantBusy: boolean;
  onToggle: () => void;
  onPlace: () => void;
  onTranscribe: () => void;
  onCutOut: () => void;
  onReveal: () => void;
  /** The composition files that use this file. */
  usedBy: () => Promise<string[]>;
  onTrash: () => Promise<void>;
}

/** Deleting asks once; when the video uses the file, it says where. */
type Confirm = { status: "idle" } | { status: "checking" } | { status: "asking"; usedIn: string[] } | { status: "deleting" };

function MediaRow({ item, open, filesUrl, assistantBusy, ...actions }: MediaRowProps) {
  /** The picture could not be loaded; the kind's icon stands in. */
  const [broken, setBroken] = useState(false);
  const thumbnail =
    item.kind === "image" && filesUrl && !broken
      ? `${filesUrl}/${item.path.split("/").map(encodeURIComponent).join("/")}`
      : null;
  const facts = [item.duration === null ? null : formatDuration(item.duration), formatBytes(item.bytes)];
  const [confirm, setConfirm] = useState<Confirm>({ status: "idle" });

  const askToDelete = async () => {
    setConfirm({ status: "checking" });
    try {
      setConfirm({ status: "asking", usedIn: await actions.usedBy() });
    } catch {
      // Not knowing where it is used is no reason to refuse; ask plainly.
      setConfirm({ status: "asking", usedIn: [] });
    }
  };
  const remove = async () => {
    setConfirm({ status: "deleting" });
    await actions.onTrash();
    setConfirm({ status: "idle" });
  };
  return (
    <li className={`media-row ${open ? "media-row--open" : ""}`}>
      <button type="button" className="media-row__main" aria-expanded={open} title={item.path} onClick={actions.onToggle}>
        <span className="media-row__thumb">
          {thumbnail ? (
            <img src={thumbnail} alt="" loading="lazy" onError={() => setBroken(true)} />
          ) : (
            <Icon name={KIND_ICON[item.kind]} />
          )}
        </span>
        <span className="media-row__name">{item.name}</span>
        <span className="media-row__facts">{facts.filter(Boolean).join(" · ")}</span>
      </button>
      {open ? (
        <div className="media-row__actions">
          <Button
            icon="timeline"
            disabled={assistantBusy}
            title="Asks the assistant to add this file to the composition"
            onClick={actions.onPlace}
          >
            Add to Timeline
          </Button>
          {canTranscribe(item) ? (
            <Button icon="captions" onClick={actions.onTranscribe}>
              Make Captions
            </Button>
          ) : null}
          {canRemoveBackground(item) ? (
            <Button icon="cutout" onClick={actions.onCutOut}>
              Remove Background
            </Button>
          ) : null}
          <Button variant="ghost" icon="reveal" onClick={actions.onReveal}>
            Show in Finder
          </Button>
          {confirm.status === "idle" || confirm.status === "checking" ? (
            <Button variant="ghost" icon="trash" busy={confirm.status === "checking"} onClick={() => void askToDelete()}>
              Delete
            </Button>
          ) : null}
        </div>
      ) : null}
      {open && (confirm.status === "asking" || confirm.status === "deleting") ? (
        <div className="media-row__confirm" role="alertdialog" aria-label={`Delete ${item.name}?`}>
          <p>
            {confirm.status === "asking" && confirm.usedIn.length > 0 ? (
              <>
                <strong>The video uses this file</strong> (in {confirm.usedIn.join(", ")}). Deleting it leaves a missing clip there.
              </>
            ) : (
              <>Move {item.name} to the Trash? You can put it back from the Trash.</>
            )}
          </p>
          <div className="media-row__actions">
            <Button disabled={confirm.status === "deleting"} onClick={() => setConfirm({ status: "idle" })}>
              Cancel
            </Button>
            <Button variant="primary" icon="trash" busy={confirm.status === "deleting"} onClick={() => void remove()}>
              {confirm.status === "asking" && confirm.usedIn.length > 0 ? "Move to Trash Anyway" : "Move to Trash"}
            </Button>
          </div>
        </div>
      ) : null}
    </li>
  );
}

const WAITING = { queued: "Waiting", running: "Working" } as const;

function JobList({ jobs }: { jobs: JobControls }) {
  const anyFinished = jobs.jobs.some((job) => job.status !== "queued" && job.status !== "running");
  return (
    <section className="jobs" aria-label="Media jobs">
      <header className="jobs__header">
        <h3>Jobs</h3>
        {anyFinished ? (
          <Button variant="ghost" onClick={jobs.clearFinished}>
            Clear Finished
          </Button>
        ) : null}
      </header>
      <ul>
        {jobs.jobs.map((job) => (
          <JobRow
            key={job.id}
            job={job}
            onCancel={() => jobs.cancel(job.id)}
            onReveal={() => jobs.reveal(job.id)}
          />
        ))}
      </ul>
    </section>
  );
}

interface JobRowProps {
  job: Job;
  onCancel: () => void;
  onReveal: () => void;
}

/** One job: what it is, how far along, and what came of it. */
export function JobRow({ job, onCancel, onReveal }: JobRowProps) {
  const active = job.status === "queued" || job.status === "running";
  const detail = active
    ? (job.progress ?? WAITING[job.status as keyof typeof WAITING])
    : job.status === "done"
      ? (job.note ?? `Saved ${job.output ?? ""}`)
      : job.status === "cancelled"
        ? "Cancelled"
        : (job.error ?? "Failed");
  return (
    <li className={`job job--${job.status}`}>
      <span className="job__status">
        {job.status === "running" ? (
          <Spinner />
        ) : (
          <Icon name={job.status === "done" ? "check" : job.status === "queued" ? "timeline" : "missing"} size={13} />
        )}
      </span>
      <span className="job__text">
        <span className="job__label">{job.label}</span>
        <span className="job__detail" title={detail}>
          {job.percent === null ? "" : `${job.percent}% · `}
          {detail}
        </span>
        {job.percent === null ? null : (
          <progress className="job__bar" max={100} value={job.percent} aria-label={`${job.label} progress`} />
        )}
      </span>
      {active ? <IconButton icon="close" label={`Cancel ${job.label}`} onClick={onCancel} /> : null}
      {job.status === "done" && job.file ? (
        <IconButton icon="reveal" label={`Show ${job.output ?? "the file"} in Finder`} onClick={onReveal} />
      ) : null}
    </li>
  );
}
