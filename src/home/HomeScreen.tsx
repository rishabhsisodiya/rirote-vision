import "./home.css";
import "../references/references.css";

import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { ProjectSummary, RecentProject, Resolution, SetupReport } from "../lib/types";
import { ReferenceForm } from "../references/ReferenceForm";
import { ReferenceGallery } from "../references/ReferenceGallery";
import type { ReferencesState } from "../references/useReferences";
import type { BrandKitsState } from "../brand/useBrandKits";
import type { BrandKit } from "../lib/types";
import type { RecentState } from "../projects/useRecentProjects";
import { Button } from "../ui/Button";
import { Icon, type IconName } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { makeBrief, projectNameFrom, scriptBrief } from "./briefs";
import { createNamed } from "./createProject";
import { ProjectGrid } from "./ProjectGrid";
import { VoicePicker } from "./VoicePicker";

export type HomeTab = "make" | "reference" | "script";

interface HomeScreenProps {
  recent: RecentState;
  setup: SetupReport | null;
  notice: string | null;
  onDismissNotice: () => void;
  /** A new project made from Home, with the brief for its first assistant turn. */
  onStarted: (project: ProjectSummary, brief: string) => void;
  onOpen: () => void;
  onOpenRecent: (project: RecentProject) => void;
  onForget: (project: RecentProject) => void;
  onCheckSetup: () => void;
  onRetryRecent: () => void;
  tab: HomeTab;
  onTab: (tab: HomeTab) => void;
  library: ReferencesState;
  kits: BrandKitsState;
  /** The reference chosen on the Reference tab. */
  referenceId: string | null;
  onSelectReference: (id: string | null) => void;
}

const TABS: { id: HomeTab; label: string; icon: IconName }[] = [
  { id: "make", label: "Make", icon: "spark" },
  { id: "reference", label: "Reference", icon: "reference" },
  { id: "script", label: "Script", icon: "captions" },
];

const SHAPES: { value: Resolution; label: string }[] = [
  { value: "portrait", label: "Portrait 9:16" },
  { value: "landscape", label: "Landscape 16:9" },
  { value: "square", label: "Square 1:1" },
];

export function HomeScreen(props: HomeScreenProps) {
  const { recent, setup, notice, tab, onTab: setTab, library, referenceId, onSelectReference } = props;
  const references = library.status === "ready" ? library.references : [];
  const selectedReference = references.find((reference) => reference.id === referenceId) ?? null;
  const kitList = props.kits.status === "ready" ? props.kits.kits : [];
  const projects = recent.status === "ready" ? recent.projects : [];
  const latest = projects.find((project) => !project.missing);

  return (
    <main className="screen home">
      <header className="screen__head" data-tauri-drag-region>
        <h1 className="screen__title home__title">What are we cutting?</h1>
        <span className="screen__meta">
          {projects.length} {projects.length === 1 ? "project" : "projects"}
          {latest ? ` · last opened ${new Date(latest.openedAt * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}` : ""}
        </span>
      </header>

      {notice ? <Notice message={notice} onDismiss={props.onDismissNotice} /> : null}
      {setup && !setup.ready ? <SetupBanner setup={setup} onCheck={props.onCheckSetup} /> : null}

      <div className="home__body">
        <section className="home__panel home__make" aria-label="Start a video">
          <div className="home__tabs" role="tablist" aria-label="How to start">
            {TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                role="tab"
                id={`home-tab-${item.id}`}
                aria-selected={tab === item.id}
                aria-controls={`home-panel-${item.id}`}
                onClick={() => setTab(item.id)}
              >
                <Icon name={item.icon} />
                {item.label}
              </button>
            ))}
          </div>
          <div className="home__tabpanel" role="tabpanel" id={`home-panel-${tab}`} aria-labelledby={`home-tab-${tab}`}>
            {tab === "make" ? <MakeForm onStarted={props.onStarted} kits={kitList} /> : null}
            {tab === "script" ? <ScriptForm onStarted={props.onStarted} kits={kitList} /> : null}
            {tab === "reference" ? (
              <ReferenceForm
                selected={selectedReference}
                onSelect={onSelectReference}
                onStarted={props.onStarted}
                onCheckSetup={props.onCheckSetup}
              />
            ) : null}
          </div>
        </section>

        <div className="home__panel home__side">
          {tab === "reference" ? (
            <ReferenceGallery
              library={library}
              selectedId={referenceId}
              onSelect={onSelectReference}
            />
          ) : (
            <ProjectGrid
              state={recent}
              onOpen={props.onOpenRecent}
              onForget={props.onForget}
              onOpenFolder={props.onOpen}
              onRetry={props.onRetryRecent}
            />
          )}
        </div>
      </div>
    </main>
  );
}

function ShapePicker({ value, onChange }: { value: Resolution; onChange: (shape: Resolution) => void }) {
  return (
    <div className="home__chips" role="radiogroup" aria-label="Shape">
      {SHAPES.map((shape) => (
        <button
          key={shape.value}
          type="button"
          role="radio"
          aria-checked={value === shape.value}
          className="chip"
          onClick={() => onChange(shape.value)}
        >
          {shape.label}
        </button>
      ))}
    </div>
  );
}

function useStart(onStarted: HomeScreenProps["onStarted"]) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const start = async (name: string, shape: Resolution, brief: string, kitId: string) => {
    setBusy(true);
    setError(null);
    try {
      const kitBrief = kitId ? await api.brandBrief(kitId) : null;
      const project = await createNamed(name, shape);
      if (kitId) await api.setProjectBrand(project.path, kitId);
      onStarted(project, kitBrief ? `${brief}\n\nUse this brand throughout:\n${kitBrief}` : brief);
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
    }
  };
  return { busy, error, start };
}

function KitPicker({ kits, value, onChange }: { kits: BrandKit[]; value: string; onChange: (id: string) => void }) {
  if (kits.length === 0) return null;
  return (
    <label className="field">
      <span className="field__label">Brand kit</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">None</option>
        {kits.map((kit) => (
          <option key={kit.id} value={kit.id}>
            {kit.name}
          </option>
        ))}
      </select>
    </label>
  );
}

function MakeForm({ onStarted, kits }: { onStarted: HomeScreenProps["onStarted"]; kits: BrandKit[] }) {
  const [kit, setKit] = useState("");
  const [request, setRequest] = useState("");
  const [shape, setShape] = useState<Resolution>("portrait");
  const { busy, error, start } = useStart(onStarted);
  const ready = request.trim().length >= 8;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready || busy) return;
    void start(projectNameFrom(request), shape, makeBrief(request, shape), kit);
  };

  return (
    <form className="home__form" onSubmit={submit}>
      <label className="field">
        <span className="field__label">What should it be?</span>
        <textarea
          rows={7}
          value={request}
          placeholder="A 25-second launch video for rirote.com: a hook, what we build, two highlights and a closing card with the URL. Use real screenshots of the site."
          onChange={(event) => setRequest(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) submit(event);
          }}
        />
      </label>
      <KitPicker kits={kits} value={kit} onChange={setKit} />
      <ShapePicker value={shape} onChange={setShape} />
      {error ? <Notice message={error} /> : null}
      <div className="home__go">
        <Button variant="primary" type="submit" busy={busy} disabled={!ready}>
          Start the video
        </Button>
        <span className="home__hint">
          <kbd>⌘</kbd> <kbd>↵</kbd> · The assistant plans it, builds it and shows you the timeline.
        </span>
      </div>
    </form>
  );
}

function ScriptForm({ onStarted, kits }: { onStarted: HomeScreenProps["onStarted"]; kits: BrandKit[] }) {
  const [kit, setKit] = useState("");
  const [script, setScript] = useState("");
  const [shape, setShape] = useState<Resolution>("portrait");
  const [voice, setVoice] = useState<string>("");
  const { busy, error, start } = useStart(onStarted);
  const ready = script.trim().length >= 20;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready || busy) return;
    void start(projectNameFrom(script, "Script video"), shape, scriptBrief(script, shape, voice || null), kit);
  };

  return (
    <form className="home__form" onSubmit={submit}>
      <label className="field">
        <span className="field__label">Your script</span>
        <textarea
          rows={10}
          value={script}
          placeholder={"Most people skip breakfast.\nHere are three easy breakfasts you can make in five minutes.\nFollow for more quick recipes."}
          onChange={(event) => setScript(event.target.value)}
        />
        <span className="field__help">
          {script.trim() ? `${script.trim().split(/\s+/).length} words · about ${Math.max(1, Math.round(script.trim().split(/\s+/).length / 2.5))} s spoken` : "The words stay exactly as you write them."}
        </span>
      </label>
      <VoicePicker value={voice} onChange={setVoice} choices={[{ value: "", label: "No voice: words on screen" }]} />
      <KitPicker kits={kits} value={kit} onChange={setKit} />
      <ShapePicker value={shape} onChange={setShape} />
      {error ? <Notice message={error} /> : null}
      <div className="home__go">
        <Button variant="primary" type="submit" busy={busy} disabled={!ready}>
          Turn script into a video
        </Button>
      </div>
    </form>
  );
}

function SetupBanner({ setup, onCheck }: { setup: SetupReport; onCheck: () => void }) {
  const open = setup.checks.filter((check) => check.required && check.status !== "ok");
  return (
    <div className="setup-banner" role="status">
      <Icon name="alert" />
      <p>
        <strong>Setup is not finished.</strong> {open.map((check) => check.label).join(", ")}{" "}
        {open.length === 1 ? "needs" : "need"} attention before you can edit or export.
      </p>
      <Button onClick={onCheck}>Check Setup</Button>
    </div>
  );
}
