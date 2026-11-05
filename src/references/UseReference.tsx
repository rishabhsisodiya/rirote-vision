import { useState, type FormEvent } from "react";

import { createNamed } from "../home/createProject";
import { VoicePicker } from "../home/VoicePicker";
import { projectNameFrom } from "../home/briefs";
import { api, errorMessage } from "../lib/ipc";
import type { ProjectSummary, Reference, Resolution } from "../lib/types";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";

interface UseReferenceProps {
  reference: Reference;
  onStarted: (project: ProjectSummary, brief: string) => void;
}

/** The shape a reference suggests: portrait for tall video, square, else landscape. */
export function shapeOf(reference: Reference): Resolution {
  const { width, height } = reference;
  if (!width || !height) return "portrait";
  if (Math.abs(width - height) / Math.max(width, height) < 0.1) return "square";
  return height > width ? "portrait" : "landscape";
}

/** "Make my video in this format": a request, a shape, and a new project. */
export function UseReference({ reference, onStarted }: UseReferenceProps) {
  const [request, setRequest] = useState("");
  const [shape, setShape] = useState<Resolution>(() => shapeOf(reference));
  /** "" matches the reference, "none" is no voiceover, else a voice id. */
  const [voice, setVoice] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ready = request.trim().length >= 8;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!ready || busy) return;
    setBusy(true);
    setError(null);
    try {
      const brief = await api.referenceBrief(reference.id, request, voice);
      const project = await createNamed(projectNameFrom(request), shape);
      onStarted(project, brief);
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
    }
  };

  return (
    <form className="use-reference" onSubmit={submit}>
      <label className="field">
        <span className="field__label">What is your video about?</span>
        <textarea
          rows={4}
          value={request}
          placeholder="What your video is about: the product or topic, who it is for, and your website or brand name."
          onChange={(event) => setRequest(event.target.value)}
        />
        <span className="field__help">Your own subject, in this reference's format. Nothing of the reference is copied.</span>
      </label>
      <VoicePicker
        value={voice}
        onChange={setVoice}
        choices={[
          { value: "", label: "Match the reference" },
          { value: "none", label: "No voice: words on screen" },
        ]}
        help="Match the reference adds a voiceover when the reference is spoken. Voices are free and run on this Mac."
      />
      <div className="home__chips" role="radiogroup" aria-label="Shape">
        {(["portrait", "landscape", "square"] as const).map((value) => (
          <button key={value} type="button" role="radio" aria-checked={shape === value} className="chip" onClick={() => setShape(value)}>
            {value === "portrait" ? "Portrait 9:16" : value === "landscape" ? "Landscape 16:9" : "Square 1:1"}
          </button>
        ))}
      </div>
      {error ? <Notice message={error} /> : null}
      <Button variant="primary" type="submit" busy={busy} disabled={!ready}>
        Make my video in this format
      </Button>
    </form>
  );
}
