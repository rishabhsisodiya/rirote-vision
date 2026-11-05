import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { BrandKit } from "../lib/types";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Notice";
import { restyleBrief, type BrandKitsState } from "./useBrandKits";

interface BrandPanelProps {
  projectPath: string;
  kits: BrandKitsState;
  assistantBusy: boolean;
  /** Sends a request to the assistant. */
  onAsk: (prompt: string) => void;
  onManage: () => void;
}

/** The side panel's Brand tab: which kit this video uses, and restyling to it. */
export function BrandPanel({ projectPath, kits, assistantBusy, onAsk, onManage }: BrandPanelProps) {
  const [kit, setKit] = useState<BrandKit | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.projectBrand(projectPath).then(setKit, (cause) => setError(errorMessage(cause)));
  }, [projectPath, kits]);

  const choose = async (id: string) => {
    setError(null);
    try {
      setKit(await api.setProjectBrand(projectPath, id || null));
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const list = kits.status === "ready" ? kits.kits : [];

  return (
    <div className="brand-panel">
      <label className="field">
        <span className="field__label">Brand kit for this video</span>
        <select value={kit?.id ?? ""} onChange={(event) => void choose(event.target.value)}>
          <option value="">None</option>
          {list.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      {list.length === 0 && kits.status === "ready" ? <p className="side__empty">No brand kits yet.</p> : null}
      {kit ? (
        <div className="brand-panel__kit">
          {kit.logo ? <img className="brand-panel__logo" src={convertFileSrc(kit.logo)} alt={`${kit.name} logo`} /> : null}
          <div className="swatches">
            {kit.colours.map((colour) => (
              <span key={colour.name + colour.hex} className="swatch" style={{ background: colour.hex }} title={`${colour.name} ${colour.hex}`} />
            ))}
          </div>
          <p className="brand-panel__fonts">
            {kit.headingFont || "—"} / {kit.bodyFont || "—"}
          </p>
          <Button
            variant="primary"
            disabled={assistantBusy}
            onClick={async () => {
              try {
                onAsk(restyleBrief(await api.brandBrief(kit.id)));
              } catch (cause) {
                setError(errorMessage(cause));
              }
            }}
          >
            Restyle to This Brand
          </Button>
          <p className="home__hint">The logo is copied into assets/brand/ when you choose the kit.</p>
        </div>
      ) : null}
      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      <Button variant="ghost" onClick={onManage}>
        Manage Brand Kits…
      </Button>
    </div>
  );
}
