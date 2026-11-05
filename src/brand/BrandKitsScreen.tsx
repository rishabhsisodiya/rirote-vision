import "./brand.css";

import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type FormEvent } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { BrandColour, BrandKit } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import type { BrandKitsControls } from "./useBrandKits";

/** Popular Google Fonts, offered as suggestions; any family name may be typed. */
const FONTS = ["Inter", "Poppins", "Montserrat", "Bebas Neue", "Anton", "Oswald", "Playfair Display", "DM Sans", "Space Grotesk", "Manrope", "Lora", "Noto Sans Devanagari", "Mukta", "Hind"];

const EMPTY: BrandKit = {
  id: "",
  name: "",
  colours: [
    { name: "Background", hex: "#0A1628" },
    { name: "Accent", hex: "#00B4D8" },
    { name: "Text", hex: "#F0F4FF" },
  ],
  headingFont: "",
  bodyFont: "",
  logo: null,
  tone: "",
  website: "",
  updatedAt: 0,
};

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;

/** The brand kit library: one kit per business, chosen per video. */
export function BrandKitsScreen({ kits }: { kits: BrandKitsControls }) {
  const list = kits.state.status === "ready" ? kits.state.kits : [];
  const [chosen, setChosen] = useState<string | null>(null);
  const [draft, setDraft] = useState<BrandKit | null>(null);
  const current = list.find((kit) => kit.id === chosen) ?? null;

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
                <button type="button" className="references__row" aria-current={kit.id === chosen ? "true" : undefined} onClick={() => setChosen(kit.id)}>
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
            <KitForm
              key={draft.id || "new"}
              kit={draft}
              onSaved={async (saved) => {
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
  onSaved: (kit: BrandKit) => Promise<void>;
  onDeleted: () => Promise<void>;
}

function KitForm({ kit, onSaved, onDeleted }: KitFormProps) {
  const [form, setForm] = useState<BrandKit>(kit);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
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
      await onSaved(await api.saveBrandKit(form));
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
          {form.logo ? <img src={convertFileSrc(form.logo)} alt="Logo" /> : <span className="home__hint">No logo</span>}
          <Button disabled={!form.id} title={form.id ? undefined : "Save the kit first"} onClick={() => void chooseLogo()}>
            {form.logo ? "Replace Logo" : "Add Logo"}
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
      <label className="field">
        <span className="field__label">Website</span>
        <input type="text" value={form.website} placeholder="rirote.com" onChange={(event) => patch({ website: event.target.value })} />
      </label>

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
