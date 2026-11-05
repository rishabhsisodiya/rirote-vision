import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { Voice } from "../lib/types";
import { Button } from "../ui/Button";

export interface VoiceChoice {
  value: string;
  label: string;
}

interface VoicePickerProps {
  /** A voice id, or one of the `choices` values. */
  value: string;
  onChange: (value: string) => void;
  /** Choices listed before the voices, such as "No voice". */
  choices: VoiceChoice[];
  help?: string;
}

/** The free on-device voices, grouped female and male, with a sample to hear. */
export function VoicePicker({ value, onChange, choices, help }: VoicePickerProps) {
  const [voices, setVoices] = useState<Voice[]>([]);
  const [hearing, setHearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const playing = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    api.listVoices().then(setVoices, () => setVoices([]));
    return () => playing.current?.pause();
  }, []);

  const isVoice = voices.some((voice) => voice.id === value);
  const groups = [
    { label: "Female", list: voices.filter((voice) => voice.gender.toLowerCase().startsWith("f")) },
    { label: "Male", list: voices.filter((voice) => voice.gender.toLowerCase().startsWith("m")) },
    { label: "Other", list: voices.filter((voice) => !/^[fm]/i.test(voice.gender)) },
  ].filter((group) => group.list.length > 0);

  const hear = async () => {
    if (!isVoice) return;
    setHearing(true);
    setError(null);
    try {
      // The first sample of a voice is made on this Mac, so it can take a moment.
      const file = await api.voiceSample(value);
      playing.current?.pause();
      const audio = new Audio(convertFileSrc(file));
      playing.current = audio;
      await audio.play();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setHearing(false);
    }
  };

  return (
    <label className="field">
      <span className="field__label">Narration</span>
      <span className="field__row">
        <select value={value} onChange={(event) => onChange(event.target.value)}>
          {choices.map((choice) => (
            <option key={choice.value} value={choice.value}>
              {choice.label}
            </option>
          ))}
          {groups.map((group) => (
            <optgroup key={group.label} label={group.label}>
              {group.list.map((voice) => (
                <option key={voice.id} value={voice.id}>
                  {voice.label} · {voice.language}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
        <Button icon="play" busy={hearing} disabled={!isVoice} title="Hear this voice" onClick={() => void hear()}>
          Hear
        </Button>
      </span>
      <span className="field__help">{error ?? help ?? "Free voices that run on this Mac."}</span>
    </label>
  );
}
