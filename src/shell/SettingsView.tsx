import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import { shortPath } from "../lib/format";
import { pickFolder } from "../projects/pickFolder";
import type { Settings, SetupReport } from "../lib/types";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";

interface SettingsViewProps {
  setup: SetupReport | null;
  onCheckSetup: () => void;
}

const MODELS: { value: Settings["assistantModel"]; label: string }[] = [
  { value: "", label: "Claude Code's default" },
  { value: "fable", label: "Fable" },
  { value: "opus", label: "Opus" },
  { value: "sonnet", label: "Sonnet" },
];
const EFFORTS: Settings["assistantEffort"][] = ["", "low", "medium", "high", "xhigh", "max"];

/** The app's preferences. Each change is saved as it is made. */
export function SettingsView({ setup, onCheckSetup }: SettingsViewProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    api.getSettings().then(setSettings, (cause) => setError(errorMessage(cause)));
  }, []);

  const change = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    setSaved(false);
    try {
      setSettings(await api.setSettings(next));
      setError(null);
      setSaved(true);
    } catch (cause) {
      setError(errorMessage(cause));
      setSettings(settings);
    }
  };

  const engine = setup?.checks.find((check) => check.id === "engine");

  return (
    <main className="screen settings">
      <header className="screen__head" data-tauri-drag-region>
        <h1 className="screen__title">Settings</h1>
        <span className="screen__meta" role="status">
          {saved ? "Saved" : ""}
        </span>
      </header>
      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      {!settings ? (
        <Spinner />
      ) : (
        <div className="settings__sections">
          <section className="settings__section" aria-labelledby="set-assistant">
            <h2 id="set-assistant">Assistant</h2>
            <p className="settings__hint">Every turn runs on your own Claude Code sign-in. These apply to new turns.</p>
            <label className="field">
              <span>Model</span>
              <select value={settings.assistantModel} onChange={(event) => void change({ assistantModel: event.target.value as Settings["assistantModel"] })}>
                {MODELS.map((model) => (
                  <option key={model.value} value={model.value}>
                    {model.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Effort</span>
              <select value={settings.assistantEffort} onChange={(event) => void change({ assistantEffort: event.target.value as Settings["assistantEffort"] })}>
                {EFFORTS.map((effort) => (
                  <option key={effort} value={effort}>
                    {effort || "Claude Code's default"}
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="settings__section" aria-labelledby="set-projects">
            <h2 id="set-projects">Projects</h2>
            <p className="settings__hint">Videos started from Home are made in this folder, one folder each.</p>
            <div className="field__row">
              <output className="field__value">{shortPath(settings.projectsFolder)}</output>
              <Button
                icon="folder"
                onClick={async () => {
                  const chosen = await pickFolder("Choose where new projects go", settings.projectsFolder);
                  if (chosen) void change({ projectsFolder: chosen });
                }}
              >
                Choose…
              </Button>
            </div>
          </section>

          <section className="settings__section" aria-labelledby="set-references">
            <h2 id="set-references">References</h2>
            <p className="settings__hint">
              Reference videos are downloaded to this Mac and studied for pacing and style. They are never placed in your
              video.
            </p>
            <label className="field">
              <span>Longest video to study</span>
              <select value={settings.referenceMaxMinutes} onChange={(event) => void change({ referenceMaxMinutes: Number(event.target.value) })}>
                {[3, 5, 10, 20, 30].map((minutes) => (
                  <option key={minutes} value={minutes}>
                    {minutes} minutes
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="settings__section" aria-labelledby="set-engine">
            <h2 id="set-engine">Video engine</h2>
            <p className="settings__hint">{engine ? `${engine.label}: ${engine.detail}` : "Checking…"}</p>
            <div>
              <Button onClick={onCheckSetup}>Check Setup…</Button>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}
