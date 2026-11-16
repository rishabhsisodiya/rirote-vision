import { convertFileSrc } from "@tauri-apps/api/core";
import { useState, type ReactNode } from "react";

import type { SiteFindings } from "../lib/types";
import { Button } from "../ui/Button";
import { anyPicked, type Picks, siteHost } from "./siteFindings";

interface SiteFindingsCardProps {
  found: SiteFindings;
  initial: Picks;
  onApply: (picks: Picks) => void;
  onDismiss: () => void;
}

/** What a website read found, each part with a tick, to apply to the form. */
export function SiteFindingsCard({ found, initial, onApply, onDismiss }: SiteFindingsCardProps) {
  const [picks, setPicks] = useState<Picks>(initial);
  const toggle = (key: Exclude<keyof Picks, "colours">) => setPicks((before) => ({ ...before, [key]: !before[key] }));
  const nothing = !found.name && found.colours.length === 0 && !found.headingFont && !found.bodyFont && !found.tone && !found.logoFile;

  return (
    <section className="site-found" aria-label={`Found on ${siteHost(found.url)}`}>
      <header className="site-found__head">
        <h2 className="site-found__title">Found on {siteHost(found.url)}</h2>
        <span className="site-found__source mono">{found.aiUsed ? "Read with Claude Code" : "Plain read"}</span>
      </header>

      {nothing ? (
        <p className="home__state">Nothing usable was found on that page. Try the site's home page, or fill the kit in yourself.</p>
      ) : (
        <ul className="site-found__rows">
          {found.name ? (
            <Row label="Name" checked={picks.name} onChange={() => toggle("name")}>
              {found.name}
            </Row>
          ) : null}
          {found.logoFile ? (
            <Row label="Logo" checked={picks.logo} onChange={() => toggle("logo")}>
              <img className="site-found__logo" src={convertFileSrc(found.logoFile)} alt="Logo found on the site" />
            </Row>
          ) : null}
          {found.colours.length > 0 ? (
            <li className="site-found__row site-found__row--colours">
              <span className="field__label">Colours</span>
              <ul className="site-found__colours">
                {found.colours.map((colour, at) => (
                  <li key={colour.hex}>
                    <label className="site-found__colour">
                      <input
                        type="checkbox"
                        checked={picks.colours[at]}
                        onChange={() => setPicks((before) => ({ ...before, colours: before.colours.map((on, i) => (i === at ? !on : on)) }))}
                      />
                      <span className="swatch" style={{ background: colour.hex }} />
                      <span>{colour.name}</span>
                      <span className="mono site-found__hex">{colour.hex}</span>
                    </label>
                  </li>
                ))}
              </ul>
              <p className="field__help">Ticked colours replace the kit's colours.</p>
            </li>
          ) : null}
          {found.headingFont ? (
            <Row label="Heading font" checked={picks.headingFont} onChange={() => toggle("headingFont")}>
              {found.headingFont}
            </Row>
          ) : null}
          {found.bodyFont ? (
            <Row label="Body font" checked={picks.bodyFont} onChange={() => toggle("bodyFont")}>
              {found.bodyFont}
            </Row>
          ) : null}
          {found.tone ? (
            <Row label="Tone" checked={picks.tone} onChange={() => toggle("tone")}>
              {found.tone}
            </Row>
          ) : null}
        </ul>
      )}

      {found.notes.length > 0 ? (
        <ul className="site-found__notes">
          {found.notes.map((note) => (
            <li key={note} className="field__help">
              {note}
            </li>
          ))}
        </ul>
      ) : null}

      <div className="row">
        <Button variant="primary" icon="check" disabled={!anyPicked(picks)} onClick={() => onApply(picks)}>
          Apply to Kit
        </Button>
        <Button variant="ghost" onClick={onDismiss}>
          Dismiss
        </Button>
      </div>
    </section>
  );
}

function Row({ label, checked, onChange, children }: { label: string; checked: boolean; onChange: () => void; children: ReactNode }) {
  return (
    <li className="site-found__row">
      <label className="site-found__tick">
        <input type="checkbox" checked={checked} onChange={onChange} />
        <span className="field__label">{label}</span>
      </label>
      <span className="site-found__value">{children}</span>
    </li>
  );
}
