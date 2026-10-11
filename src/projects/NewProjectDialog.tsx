import { useState, type FormEvent } from "react";

import { folderName, shortPath } from "../lib/format";
import { api, errorMessage } from "../lib/ipc";
import type { ProjectSummary, Resolution } from "../lib/types";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Notice } from "../ui/Notice";
import { pickFolder } from "./pickFolder";

const LOCATION_KEY = "rirote-vision.new-project-location";

const FORMATS: { value: Resolution; label: string; detail: string }[] = [
  { value: "landscape", label: "Landscape", detail: "16:9 · 1920 × 1080" },
  { value: "portrait", label: "Portrait", detail: "9:16 · 1080 × 1920" },
  { value: "square", label: "Square", detail: "1:1 · 1080 × 1080" },
];

interface NewProjectDialogProps {
  onClose: () => void;
  onCreated: (project: ProjectSummary) => void;
}

export function NewProjectDialog({ onClose, onCreated }: NewProjectDialogProps) {
  const [name, setName] = useState("");
  const [parent, setParent] = useState(() => localStorage.getItem(LOCATION_KEY) ?? "");
  const [resolution, setResolution] = useState<Resolution>("landscape");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const folder = folderName(name);
  const canCreate = folder.length > 0 && parent.length > 0;

  async function chooseLocation() {
    const chosen = await pickFolder("Choose where to keep the project", parent || undefined);
    if (chosen) setParent(chosen);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!canCreate || creating) return;
    setCreating(true);
    setError(null);
    try {
      const project = await api.createProject({ parent, name, resolution });
      localStorage.setItem(LOCATION_KEY, parent);
      onCreated(project);
    } catch (cause) {
      setError(errorMessage(cause));
      setCreating(false);
    }
  }

  return (
    <Dialog
      title="New Project"
      onClose={onClose}
      locked={creating}
      footer={
        <>
          <Button onClick={onClose} disabled={creating}>
            Cancel
          </Button>
          <Button
            variant="primary"
            type="submit"
            form="new-project"
            busy={creating}
            disabled={!canCreate}
          >
            {creating ? "Creating Project" : "Create Project"}
          </Button>
        </>
      }
    >
      <form id="new-project" className="form" onSubmit={submit}>
        <div className="field">
          <label htmlFor="project-name">Name</label>
          <input
            id="project-name"
            type="text"
            value={name}
            maxLength={64}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            placeholder="Launch film"
            aria-describedby="project-name-help"
            onChange={(event) => setName(event.target.value)}
          />
          <p id="project-name-help" className="field__help">
            {folder
              ? `Folder: ${folder}`
              : name
                ? "Use at least one letter or number."
                : "Letters, numbers and spaces. It becomes the folder name."}
          </p>
        </div>

        <div className="field">
          <span className="field__label" id="project-location-label">
            Location
          </span>
          <div className="field__row">
            <output className="field__value" aria-labelledby="project-location-label">
              {parent ? shortPath(parent) : "No folder chosen"}
            </output>
            <Button icon="folder" onClick={chooseLocation} disabled={creating}>
              Choose Folder
            </Button>
          </div>
        </div>

        <fieldset className="field">
          <legend>Format</legend>
          <div className="choices">
            {FORMATS.map((format) => (
              <label key={format.value} className="choice">
                <input
                  type="radio"
                  name="resolution"
                  value={format.value}
                  checked={resolution === format.value}
                  onChange={() => setResolution(format.value)}
                />
                <span className="choice__label">{format.label}</span>
                <span className="choice__detail">{format.detail}</span>
              </label>
            ))}
          </div>
        </fieldset>

        {error ? <Notice message={error} /> : null}
      </form>
    </Dialog>
  );
}
