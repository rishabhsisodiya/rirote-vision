import { useEffect, useState } from "react";

import { api } from "../lib/ipc";
import type { BrandKit, Settings } from "../lib/types";

interface SkillInfo {
  name: string;
  description: string;
}

const MODELS: { value: Settings["assistantModel"]; label: string }[] = [
  { value: "", label: "Default model" },
  { value: "fable", label: "Fable" },
  { value: "opus", label: "Opus" },
  { value: "sonnet", label: "Sonnet" },
];
const EFFORTS: Settings["assistantEffort"][] = ["", "low", "medium", "high", "xhigh", "max"];

interface ComposerToolsProps {
  onSkill: (name: string) => void;
  /** Brand kits that can go with the next message. */
  kits: BrandKit[];
  onKit: (id: string) => void;
  disabled: boolean;
}

/** Under the box: which model and effort the next turn uses, a skill to point it at, and a brand kit to send with it. */
export function ComposerTools({ onSkill, kits, onKit, disabled }: ComposerToolsProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [skills, setSkills] = useState<SkillInfo[]>([]);

  useEffect(() => {
    api.getSettings().then(setSettings, () => setSettings(null));
    api.listSkills().then(setSkills, () => setSkills([]));
  }, []);

  const change = (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    api.setSettings(next).catch(() => setSettings(settings));
  };

  return (
    <div className="composer__tools">
      <select
        className="transport__select"
        value={settings?.assistantModel ?? ""}
        disabled={!settings || disabled}
        aria-label="Model for the next request"
        title="Model for the next request (also in Settings)"
        onChange={(event) => change({ assistantModel: event.target.value as Settings["assistantModel"] })}
      >
        {MODELS.map((model) => (
          <option key={model.value} value={model.value}>
            {model.label}
          </option>
        ))}
      </select>
      <select
        className="transport__select"
        value={settings?.assistantEffort ?? ""}
        disabled={!settings || disabled}
        aria-label="Effort for the next request"
        title="How hard it thinks: more effort is slower and uses more of your plan"
        onChange={(event) => change({ assistantEffort: event.target.value as Settings["assistantEffort"] })}
      >
        {EFFORTS.map((effort) => (
          <option key={effort} value={effort}>
            {effort ? `Effort: ${effort}` : "Default effort"}
          </option>
        ))}
      </select>
      {skills.length ? (
        <select
          className="transport__select"
          value=""
          disabled={disabled}
          aria-label="Point the request at a skill"
          title="Point the request at one of the video skills"
          onChange={(event) => event.target.value && onSkill(event.target.value)}
        >
          <option value="">Skill…</option>
          {skills.map((skill) => (
            <option key={skill.name} value={skill.name} title={skill.description}>
              {skill.name}
            </option>
          ))}
        </select>
      ) : null}
      {kits.length ? (
        <select
          className="transport__select"
          value=""
          disabled={disabled}
          aria-label="Send a brand kit with the next message"
          title="Send a brand kit (colours, fonts, logo, photos, avatars) with the next message"
          onChange={(event) => event.target.value && onKit(event.target.value)}
        >
          <option value="">Brand kit…</option>
          {kits.map((kit) => (
            <option key={kit.id} value={kit.id}>
              {kit.name}
            </option>
          ))}
        </select>
      ) : null}
    </div>
  );
}
