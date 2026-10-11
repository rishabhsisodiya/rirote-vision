import "./export.css";

import { useEffect, useState, type FormEvent } from "react";

import { formatDuration, shortPath } from "../lib/format";
import { api, errorMessage } from "../lib/ipc";
import type {
  CheckSummary,
  ExportFormat,
  ExportOptions,
  ExportQuality,
  Job,
  ProjectInfo,
  ProjectSummary,
} from "../lib/types";
import { JobRow } from "../media/MediaBin";
import type { JobControls } from "../media/useJobs";
import { pickFolder } from "../projects/pickFolder";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { defaultName, FORMATS, FRAME_RATES, nameProblem, QUALITIES, sizeOptions } from "./exportOptions";

const FOLDER_KEY = "rirote-vision.export-folder";

/** Where the dialog is in an export: choosing, being checked, stopped by problems, or under way. */
type Step =
  | { at: "options" }
  | { at: "checking" }
  | { at: "problems"; summary: CheckSummary }
  | { at: "exporting"; jobId: number; warnings: number };

interface ExportDialogProps {
  project: ProjectSummary;
  info: ProjectInfo | null;
  jobs: JobControls;
  onClose: () => void;
}

/** Chooses how to export, checks the composition, then follows the render. */
export function ExportDialog({ project, info, jobs, onClose }: ExportDialogProps) {
  const [format, setFormat] = useState<ExportFormat>("mp4");
  const [quality, setQuality] = useState<ExportQuality>("standard");
  const [fps, setFps] = useState<number | null>(null);
  const [resolution, setResolution] = useState<string | null>(null);
  const [name, setName] = useState(() => defaultName(project.name));
  const [folder, setFolder] = useState<string | null>(() => localStorage.getItem(FOLDER_KEY));
  const [step, setStep] = useState<Step>({ at: "options" });
  const [error, setError] = useState<string | null>(null);

  const problem = nameProblem(name);
  const options: ExportOptions = { format, quality, fps, resolution, name: name.trim(), folder };
  const job = step.at === "exporting" ? jobs.jobs.find((candidate) => candidate.id === step.jobId) : undefined;

  async function chooseFolder() {
    const chosen = await pickFolder("Choose where to save the export", folder ?? undefined);
    if (chosen) setFolder(chosen);
  }

  /** Queues the render. `warnings` is what the check counted, for the record. */
  async function queue(warnings: number) {
    setError(null);
    try {
      const queued = await api.startJob(project.path, { type: "export", ...options });
      if (folder) localStorage.setItem(FOLDER_KEY, folder);
      else localStorage.removeItem(FOLDER_KEY);
      setStep({ at: "exporting", jobId: queued.id, warnings });
    } catch (cause) {
      setError(errorMessage(cause));
      setStep({ at: "options" });
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (problem || step.at !== "options") return;
    setError(null);
    setStep({ at: "checking" });
    try {
      const summary = await api.checkExport(project.path);
      if (summary.ok) await queue(summary.warnings);
      else setStep({ at: "problems", summary });
    } catch (cause) {
      setError(errorMessage(cause));
      setStep({ at: "options" });
    }
  }

  // Closing the dialog leaves a running export running; it stays in the jobs list.
  const busy = step.at === "checking";

  return (
    <Dialog
      title="Export"
      onClose={onClose}
      locked={busy}
      footer={
        step.at === "options" || step.at === "checking" ? (
          <>
            <Button onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button variant="primary" type="submit" form="export" busy={busy} disabled={!!problem}>
              {busy ? "Checking the Composition" : "Export"}
            </Button>
          </>
        ) : step.at === "problems" ? (
          <>
            <Button onClick={() => setStep({ at: "options" })}>Back</Button>
            <Button onClick={() => void queue(step.summary.warnings)}>Export Anyway</Button>
            <Button variant="primary" onClick={onClose}>
              Close and Fix
            </Button>
          </>
        ) : (
          <>
            {job && (job.status === "queued" || job.status === "running") ? (
              <Button onClick={() => jobs.cancel(job.id)}>Cancel Export</Button>
            ) : (
              <Button onClick={() => setStep({ at: "options" })}>Export Another</Button>
            )}
            <Button variant="primary" onClick={onClose}>
              {job && (job.status === "queued" || job.status === "running") ? "Continue Editing" : "Done"}
            </Button>
          </>
        )
      }
    >
      {error ? <Notice message={error} /> : null}

      {step.at === "problems" ? (
        <Problems summary={step.summary} />
      ) : step.at === "exporting" ? (
        <Progress job={job} warnings={step.warnings} jobs={jobs} />
      ) : (
        <form id="export" className="form" onSubmit={submit}>
          <fieldset className="field" disabled={busy}>
            <legend>Format</legend>
            <div className="choices choices--four">
              {FORMATS.map((option) => (
                <label key={option.value} className="choice" title={option.detail}>
                  <input
                    type="radio"
                    name="format"
                    checked={format === option.value}
                    onChange={() => setFormat(option.value)}
                  />
                  <span className="choice__label">{option.label}</span>
                </label>
              ))}
            </div>
            <p className="field__help">{FORMATS.find((option) => option.value === format)?.detail}</p>
          </fieldset>

          <fieldset className="field" disabled={busy}>
            <legend>Quality</legend>
            <div className="choices">
              {QUALITIES.map((option) => (
                <label key={option.value} className="choice">
                  <input
                    type="radio"
                    name="quality"
                    checked={quality === option.value}
                    onChange={() => setQuality(option.value)}
                  />
                  <span className="choice__label">{option.label}</span>
                  <span className="choice__detail choice__detail--plain">{option.detail}</span>
                </label>
              ))}
            </div>
          </fieldset>

          <div className="field__pair">
            <div className="field">
              <label htmlFor="export-size">Size</label>
              <select
                id="export-size"
                value={resolution ?? ""}
                disabled={busy}
                onChange={(event) => setResolution(event.target.value || null)}
              >
                {sizeOptions(info).map((option) => (
                  <option key={option.value ?? ""} value={option.value ?? ""}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="export-fps">Frame rate</label>
              <select
                id="export-fps"
                value={fps ?? ""}
                disabled={busy}
                onChange={(event) => setFps(event.target.value ? Number(event.target.value) : null)}
              >
                <option value="">Composition rate</option>
                {FRAME_RATES.map((rate) => (
                  <option key={rate} value={rate}>
                    {rate} frames a second
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="field">
            <label htmlFor="export-name">File name</label>
            <input
              id="export-name"
              type="text"
              value={name}
              maxLength={80}
              disabled={busy}
              autoComplete="off"
              spellCheck={false}
              aria-invalid={!!problem}
              aria-describedby="export-name-help"
              onChange={(event) => setName(event.target.value)}
            />
            <p id="export-name-help" className={`field__help ${problem ? "field__help--error" : ""}`}>
              {problem ?? `Saved as ${name.trim()}.${format}. An existing file is never replaced.`}
            </p>
          </div>

          <div className="field">
            <span className="field__label" id="export-folder-label">
              Save to
            </span>
            <div className="field__row">
              <output className="field__value" aria-labelledby="export-folder-label">
                {folder ? shortPath(folder) : "The project's renders folder"}
              </output>
              {folder ? (
                <Button variant="ghost" disabled={busy} onClick={() => setFolder(null)}>
                  Use Project Folder
                </Button>
              ) : null}
              <Button icon="folder" disabled={busy} onClick={() => void chooseFolder()}>
                Choose Folder
              </Button>
            </div>
          </div>
        </form>
      )}
    </Dialog>
  );
}

function Problems({ summary }: { summary: CheckSummary }) {
  const count = summary.errors.length + summary.more_errors;
  return (
    <div className="export-problems">
      <p className="export-problems__title">
        <Icon name="alert" />
        The composition has {count} {count === 1 ? "problem" : "problems"} that will show in the video
      </p>
      <ul>
        {summary.errors.map((finding, index) => (
          <li key={index}>
            <p>
              {finding.message}
              {finding.at === null ? "" : ` (at ${formatDuration(finding.at)})`}
            </p>
            {finding.hint ? <p className="export-problems__hint">{finding.hint}</p> : null}
          </li>
        ))}
      </ul>
      {summary.more_errors > 0 ? <p className="export-problems__hint">And {summary.more_errors} more.</p> : null}
      <p className="export-problems__hint">
        You can ask the assistant to fix these, or export anyway to see them in the file.
      </p>
    </div>
  );
}

interface ProgressProps {
  job: Job | undefined;
  warnings: number;
  jobs: JobControls;
}

function Progress({ job, warnings, jobs }: ProgressProps) {
  // The job leaves the list if finished jobs are cleared elsewhere.
  const [seen, setSeen] = useState<Job | undefined>(job);
  useEffect(() => {
    if (job) setSeen(job);
  }, [job]);
  const shown = job ?? seen;

  if (!shown) {
    return (
      <p className="export-progress__waiting" role="status">
        <Spinner />
        Starting the export
      </p>
    );
  }
  return (
    <div className="export-progress">
      <ul>
        <JobRow job={shown} onCancel={() => jobs.cancel(shown.id)} onReveal={() => jobs.reveal(shown.id)} />
      </ul>
      {shown.status === "queued" ? (
        <p className="export-problems__hint">Waiting for the job ahead of it to finish.</p>
      ) : null}
      {shown.status === "done" ? (
        <Button icon="reveal" onClick={() => jobs.reveal(shown.id)}>
          Show in Finder
        </Button>
      ) : null}
      {warnings > 0 ? (
        <p className="export-problems__hint">
          The check passed with {warnings} {warnings === 1 ? "warning" : "warnings"}.
        </p>
      ) : null}
    </div>
  );
}
