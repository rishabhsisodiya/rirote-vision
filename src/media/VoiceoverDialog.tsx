import { useEffect, useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Voice } from "../lib/types";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Notice } from "../ui/Notice";

const VOICE_KEY = "rirote-vision.voice";
const MAX_TEXT = 5000;

interface VoiceoverDialogProps {
  /** Queues the voiceover. Resolves to why it was refused, or null. */
  onCreate: (text: string, voice: string) => Promise<string | null>;
  onClose: () => void;
}

/** Words in, a narration file out. */
export function VoiceoverDialog({ onCreate, onClose }: VoiceoverDialogProps) {
  const [text, setText] = useState("");
  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [voice, setVoice] = useState(() => localStorage.getItem(VOICE_KEY) ?? "af_heart");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  useEffect(() => {
    let current = true;
    api
      .listVoices()
      .then((list) => current && setVoices(list))
      .catch((cause) => current && setError(errorMessage(cause)));
    return () => {
      current = false;
    };
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    setError(null);
    const refused = await onCreate(text, voice);
    if (refused) {
      setError(refused);
      setSending(false);
    } else {
      localStorage.setItem(VOICE_KEY, voice);
      onClose();
    }
  }

  return (
    <Dialog
      title="New Voiceover"
      onClose={onClose}
      locked={sending}
      footer={
        <>
          <Button onClick={onClose} disabled={sending}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" form="voiceover" busy={sending} disabled={!text.trim() || !voices}>
            Create Voiceover
          </Button>
        </>
      }
    >
      <form id="voiceover" className="form" onSubmit={submit}>
        <div className="field">
          <label htmlFor="voiceover-text">Words to speak</label>
          <textarea
            id="voiceover-text"
            rows={6}
            value={text}
            maxLength={MAX_TEXT}
            autoFocus
            placeholder="Type the narration"
            aria-describedby="voiceover-help"
            onChange={(event) => setText(event.target.value)}
          />
          <p id="voiceover-help" className="field__help">
            Spoken on this Mac by the Kokoro voice model. The sound file is saved in the project's
            voiceover folder. {text.length > MAX_TEXT * 0.8 ? `${MAX_TEXT - text.length} characters left.` : ""}
          </p>
        </div>
        <div className="field">
          <label htmlFor="voiceover-voice">Voice</label>
          <select
            id="voiceover-voice"
            value={voice}
            disabled={!voices}
            onChange={(event) => setVoice(event.target.value)}
          >
            {voices ? (
              voices.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label} · {option.language} · {option.gender}
                </option>
              ))
            ) : (
              <option value={voice}>Loading voices</option>
            )}
          </select>
        </div>
        {error ? <Notice message={error} /> : null}
      </form>
    </Dialog>
  );
}
