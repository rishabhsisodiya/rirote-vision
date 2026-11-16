import "./brand.css";

import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { BrandColour, BrandKit, SiteFindings } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { KitAssets } from "./KitAssets";
import { SiteFindingsCard } from "./SiteFindingsCard";
import { applyPicks, DEFAULT_COLOURS, initialPicks, type Picks } from "./siteFindings";
import type { BrandKitsControls } from "./useBrandKits";

/** Popular Google Fonts, offered as suggestions; any family name may be typed. */
const FONTS = ["Inter", "Poppins", "Montserrat", "Bebas Neue", "Anton", "Oswald", "Playfair Display", "DM Sans", "Space Grotesk", "Manrope", "Lora", "Noto Sans Devanagari", "Mukta", "Hind"];

const EMPTY: BrandKit = {
  id: "",
  name: "",
  colours: DEFAULT_COLOURS.map((colour) => ({ ...colour })),
  headingFont: "",
  bodyFont: "",
  logo: null,
  tone: "",
  website: "",
  assets: [],
  updatedAt: 0,
};

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;

/** The brand kit library: one kit per business, chosen per video. */
export function BrandKitsScreen({ kits }: { kits: BrandKitsControls }) {
  const list = kits.state.status === "ready" ? kits.state.kits : [];
  const [chosen, setChosen] = useState<string | null>(null);
  const [draft, setDraft] = useState<BrandKit | null>(null);
  // A problem found after saving (the form starts afresh once a kit is saved).
  const [savedNotice, setSavedNotice] = useState<string | null>(null);
  const current = list.find((kit) => kit.id === chosen) ?? null;

  // Kits can be added or changed outside the app; read the folder afresh on opening.
  const { reload } = kits;
  useEffect(() => {
    void reload();
  }, [reload]);

  // Opening a kit edits a copy of it; a new kit starts from the room's own colours.
  useEffect(() => {
    if (current) setDraft(current);
  }, [current]);

  return (
    <main className="screen brands">
      <header className="screen__head" data-tauri-drag-region>
        <h1 className="screen__title">Brand Kits</h1>
        <span className="screen__meta">{list.length} kits · used by Make, Script and the Brand tab</span>
      </header>
      <div className="references__body">
        <aside className="home__panel references__list" aria-label="Kits">
          <div className="references__list-head">
            <Button
              icon="plus"
              onClick={() => {
                setSavedNotice(null);
                setChosen(null);
                setDraft({ ...EMPTY, colours: EMPTY.colours.map((colour) => ({ ...colour })) });
              }}
            >
              New Brand Kit
            </Button>
          </div>
          {kits.state.status === "loading" ? <Spinner /> : null}
          {kits.state.status === "error" ? <p className="home__state">{kits.state.message}</p> : null}
          {kits.state.status === "ready" && list.length === 0 ? <p className="home__state">No kits yet. Make one for each business you make videos for.</p> : null}
          <ul>
            {list.map((kit) => (
              <li key={kit.id}>
                <button type="button" className="references__row" aria-current={kit.id === chosen ? "true" : undefined} onClick={() => {
                    setSavedNotice(null);
                    setChosen(kit.id);
                  }}>
                  <span className="references__still">
                    {kit.logo ? <img src={convertFileSrc(kit.logo)} alt="" className="brands__logo-thumb" /> : <span className="brands__initial">{kit.name.slice(0, 1)}</span>}
                  </span>
                  <span className="references__text">
                    <strong>{kit.name}</strong>
                    <span className="swatches">
                      {kit.colours.map((colour) => (
                        <span key={colour.name + colour.hex} className="swatch swatch--small" style={{ background: colour.hex }} />
                      ))}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </aside>
        <section className="home__panel references__detail" aria-label="Kit">
          {draft ? (
            <div className="brands__editor">
              <KitForm
                key={draft.id || "new"}
                kit={draft}
                notice={savedNotice}
                onSaved={async (saved, warning) => {
                  setSavedNotice(warning ?? null);
                  await kits.reload();
                  setChosen(saved.id);
                  setDraft(saved);
                }}
                onDeleted={async () => {
                  await kits.reload();
                  setChosen(null);
                  setDraft(null);
                }}
              />
              {draft.id ? (
                <KitAssets
                  kit={draft}
                  onChanged={async (saved) => {
                    setDraft(saved);
                    await kits.reload();
                  }}
                />
              ) : (
                <p className="field__help">Create the kit first, then add photos and 3D avatars to it.</p>
              )}
            </div>
          ) : (
            <p className="home__state">Choose a kit, or make a new one.</p>
          )}
        </section>
      </div>
    </main>
  );
}

interface KitFormProps {
  kit: BrandKit;
  /** Shown when the form opens, for something that went wrong while saving. */
  notice: string | null;
  /** `warning` is set when the kit saved but something after it did not. */
  onSaved: (kit: BrandKit, warning?: string) => Promise<void>;
  onDeleted: () => Promise<void>;
}

function KitForm({ kit, notice, onSaved, onDeleted }: KitFormProps) {
  const [form, setForm] = useState<BrandKit>(kit);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(notice);
  const site = useSiteRead();
  // A logo taken from the website, copied into the kit when it is saved.
  const [pendingLogo, setPendingLogo] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const badColour = form.colours.find((colour) => !HEX.test(colour.hex));
  const ready = form.name.trim().length > 0 && !badColour;

  const patch = (change: Partial<BrandKit>) => setForm((before) => ({ ...before, ...change }));
  const setColour = (index: number, change: Partial<BrandColour>) =>
    patch({ colours: form.colours.map((colour, at) => (at === index ? { ...colour, ...change } : colour)) });

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!ready || saving) return;
    setSaving(true);
    setError(null);
    try {
      const saved = await api.saveBrandKit(form);
      if (!pendingLogo) {
        await onSaved(saved);
        return;
      }
      try {
        const withLogo = await api.setBrandLogo(saved.id, pendingLogo);
        setPendingLogo(null);
        await onSaved(withLogo);
      } catch (cause) {
        await onSaved(saved, `The kit was saved, but the website's logo could not be added: ${errorMessage(cause)} Fetch from the website again, or add the logo yourself.`);
      }
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSaving(false);
    }
  };

  const chooseLogo = async () => {
    if (!form.id) return;
    const file = await open({ multiple: false, title: "Choose the logo", filters: [{ name: "Logo", extensions: ["svg", "png", "jpg", "jpeg", "webp"] }] });
    if (typeof file !== "string") return;
    try {
      const saved = await api.setBrandLogo(form.id, file);
      setPendingLogo(null);
      patch({ logo: saved.logo });
      await onSaved(saved);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  return (
    <form className="kit-form" onSubmit={submit}>
      <datalist id="font-suggestions">
        {FONTS.map((font) => (
          <option key={font} value={font} />
        ))}
      </datalist>
      <div className="kit-form__head">
        <label className="field kit-form__name">
          <span className="field__label">Name</span>
          <input type="text" value={form.name} maxLength={60} placeholder="Rirote Global" onChange={(event) => patch({ name: event.target.value })} />
        </label>
        <div className="kit-form__logo">
          {pendingLogo ? (
            <span className="kit-form__pending">
              <img src={convertFileSrc(pendingLogo)} alt="Logo from the website" />
              <span className="home__hint">Added when you save</span>
            </span>
          ) : form.logo ? (
            <img src={convertFileSrc(form.logo)} alt="Logo" />
          ) : (
            <span className="home__hint">No logo</span>
          )}
          <Button disabled={!form.id} title={form.id ? undefined : "Save the kit first"} onClick={() => void chooseLogo()}>
            {form.logo || pendingLogo ? "Replace Logo" : "Add Logo"}
          </Button>
        </div>
      </div>

      <fieldset className="field">
        <legend>Colours</legend>
        <ul className="kit-form__colours">
          {form.colours.map((colour, index) => (
            <li key={index} className="kit-form__colour">
              <input type="color" value={HEX.test(colour.hex) && colour.hex.length === 7 ? colour.hex : "#000000"} aria-label={`${colour.name || "Colour"} picker`} onChange={(event) => setColour(index, { hex: event.target.value.toUpperCase() })} />
              <input type="text" className="mono" value={colour.hex} aria-label={`${colour.name || "Colour"} hex`} aria-invalid={!HEX.test(colour.hex)} onChange={(event) => setColour(index, { hex: event.target.value })} />
              <input type="text" value={colour.name} placeholder="What it is for" aria-label="Colour role" onChange={(event) => setColour(index, { name: event.target.value })} />
              <IconButton icon="close" label="Remove colour" onClick={() => patch({ colours: form.colours.filter((_, at) => at !== index) })} />
            </li>
          ))}
        </ul>
        {form.colours.length < 8 ? (
          <Button variant="ghost" icon="plus" onClick={() => patch({ colours: [...form.colours, { name: "", hex: "#FFFFFF" }] })}>
            Add Colour
          </Button>
        ) : null}
        {badColour ? <p className="field__help kit-form__bad">“{badColour.hex}” is not a hex colour, like #00B4D8.</p> : null}
      </fieldset>

      <div className="kit-form__pair">
        <label className="field">
          <span className="field__label">Heading font</span>
          <input type="text" list="font-suggestions" value={form.headingFont} placeholder="Bebas Neue" onChange={(event) => patch({ headingFont: event.target.value })} />
        </label>
        <label className="field">
          <span className="field__label">Body font</span>
          <input type="text" list="font-suggestions" value={form.bodyFont} placeholder="Inter" onChange={(event) => patch({ bodyFont: event.target.value })} />
        </label>
      </div>
      <p className="field__help">Any Google Fonts family works; the engine embeds it in the export.</p>

      <label className="field">
        <span className="field__label">Tone</span>
        <textarea rows={3} maxLength={600} value={form.tone} placeholder="Plain and confident. Short sentences, no hype." onChange={(event) => patch({ tone: event.target.value })} />
      </label>
      <div className="field">
        <label className="field__label" htmlFor="kit-website">
          Website
        </label>
        <div className="kit-form__website">
          <input id="kit-website" type="text" value={form.website} maxLength={200} placeholder="rirote.com" onChange={(event) => patch({ website: event.target.value })} />
          <Button
            icon="spark"
            busy={site.reading}
            disabled={form.website.trim().length === 0}
            title="Read the name, colours, fonts, logo and tone from this website"
            onClick={() => {
              setPendingLogo(null);
              void site.read(form.website);
            }}
          >
            {site.reading ? "Reading Site…" : "Fetch from Website"}
          </Button>
          {site.reading ? (
            <Button variant="ghost" onClick={site.cancel}>
              Cancel
            </Button>
          ) : null}
        </div>
        {site.reading ? <p className="field__help">Reading the page and its stylesheets, then asking Claude Code to name the colours. This takes up to half a minute.</p> : null}
      </div>
      {site.error ? <Notice message={site.error} onDismiss={site.clear} /> : null}
      {site.found ? (
        <SiteFindingsCard
          key={site.found.url + site.generation}
          found={site.found}
          initial={initialPicks(site.found, { ...form, logo: form.logo ?? pendingLogo })}
          onApply={(picks: Picks) => {
            if (!site.found) return;
            const applied = applyPicks(form, site.found, picks);
            setForm(applied.kit);
            if (applied.logo) setPendingLogo(applied.logo);
            site.clear();
          }}
          onDismiss={site.clear}
        />
      ) : null}

      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      <div className="row">
        <Button variant="primary" type="submit" busy={saving} disabled={!ready}>
          {form.id ? "Save Changes" : "Create Kit"}
        </Button>
        {form.id ? (
          confirming ? (
            <>
              <Button className="button--danger" onClick={() => void api.deleteBrandKit(form.id).then(onDeleted, (cause) => setError(errorMessage(cause)))}>
                Delete Kit
              </Button>
              <Button variant="ghost" onClick={() => setConfirming(false)}>
                Keep
              </Button>
            </>
          ) : (
            <Button variant="ghost" icon="trash" onClick={() => setConfirming(true)}>
              Delete
            </Button>
          )
        ) : null}
      </div>
    </form>
  );
}

interface SiteRead {
  reading: boolean;
  found: SiteFindings | null;
  error: string | null;
  /** Counts reads, so a second read of the same site starts a fresh card. */
  generation: number;
  read: (website: string) => Promise<void>;
  cancel: () => void;
  clear: () => void;
}

/** One website read for the form; leaving the form stops a read under way. */
function useSiteRead(): SiteRead {
  const [reading, setReading] = useState(false);
  const [found, setFound] = useState<SiteFindings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [generation, setGeneration] = useState(0);
  const latest = useRef(0);
  const busy = useRef(false);

  useEffect(
    () => () => {
      latest.current += 1;
      if (busy.current) void api.cancelBrandSite();
    },
    [],
  );

  const read = async (website: string) => {
    const mine = ++latest.current;
    setGeneration(mine);
    busy.current = true;
    setReading(true);
    setFound(null);
    setError(null);
    try {
      const result = await api.readBrandSite(website);
      if (latest.current === mine) setFound(result);
    } catch (cause) {
      // A read stopped by Cancel, or by a newer read, says nothing.
      if (latest.current === mine && errorMessage(cause) !== "Stopped.") setError(errorMessage(cause));
    } finally {
      if (latest.current === mine) {
        busy.current = false;
        setReading(false);
      }
    }
  };

  const cancel = () => {
    latest.current += 1;
    busy.current = false;
    setReading(false);
    void api.cancelBrandSite();
  };

  const clear = () => {
    setFound(null);
    setError(null);
  };

  return { reading, found, error, generation, read, cancel, clear };
}
