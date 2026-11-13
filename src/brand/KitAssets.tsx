import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { BrandAsset, BrandKit } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";

const MAX_ASSETS = 30;
const PHOTO_TYPES = ["png", "jpg", "jpeg", "webp"];

const isPhoto = (file: string) => PHOTO_TYPES.includes(file.split(".").pop()?.toLowerCase() ?? "");

interface KitAssetsProps {
  kit: BrandKit;
  /** Called with the kit as saved after every change. */
  onChanged: (kit: BrandKit) => Promise<void>;
}

/** Photos, background-removed cut-outs and 3D avatars kept in a brand kit. */
export function KitAssets({ kit, onChanged }: KitAssetsProps) {
  const [alsoCut, setAlsoCut] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [cutting, setCutting] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const originals = kit.assets.filter((asset) => asset.kind !== "cutout");
  const full = kit.assets.length >= MAX_ASSETS;

  const cut = async (photo: string): Promise<BrandKit | null> => {
    setCutting((before) => new Set(before).add(photo));
    try {
      const saved = await api.cutBrandAsset(kit.id, photo);
      await onChanged(saved);
      return saved;
    } catch (cause) {
      setError(errorMessage(cause));
      return null;
    } finally {
      setCutting((before) => {
        const after = new Set(before);
        after.delete(photo);
        return after;
      });
    }
  };

  const add = async () => {
    const chosen = await open({
      multiple: true,
      title: "Add photos or 3D avatars",
      filters: [{ name: "Photos and 3D avatars", extensions: [...PHOTO_TYPES, "glb"] }],
    });
    const files = Array.isArray(chosen) ? chosen : typeof chosen === "string" ? [chosen] : [];
    if (files.length === 0) return;
    setError(null);
    const toCut: string[] = [];
    const failed: string[] = [];
    for (const [index, file] of files.entries()) {
      setBusy(`Adding ${index + 1} of ${files.length}…`);
      try {
        const saved = await api.addBrandAsset(kit.id, file, "", "");
        await onChanged(saved);
        const added = saved.assets[saved.assets.length - 1];
        if (alsoCut && added && isPhoto(file)) toCut.push(added.id);
      } catch (cause) {
        failed.push(`${file.split("/").pop()}: ${errorMessage(cause)}`);
      }
    }
    setBusy(null);
    if (failed.length) setError(failed.join("\n"));
    for (const photo of toCut) await cut(photo);
  };

  return (
    <section className="kit-assets" aria-label="Photos and 3D avatars">
      <div className="kit-assets__head">
        <div>
          <h2 className="kit-assets__title">Photos and 3D avatars</h2>
          <p className="field__help">
            Copied into every video that uses this kit. The note tells the assistant when to use each one.
          </p>
        </div>
        <div className="kit-assets__add">
          <label className="kit-assets__tick">
            <input type="checkbox" checked={alsoCut} onChange={(event) => setAlsoCut(event.target.checked)} />
            Also make a cut-out
          </label>
          <Button icon="plus" busy={busy !== null} disabled={full} title={full ? `A kit holds up to ${MAX_ASSETS}` : undefined} onClick={() => void add()}>
            Add Files
          </Button>
        </div>
      </div>
      {busy ? <p className="home__hint">{busy}</p> : null}
      {error ? <Notice message={error} onDismiss={() => setError(null)} /> : null}
      {originals.length === 0 ? (
        <p className="home__state kit-assets__empty">
          No photos yet. Add the founder, the product, the shop: PNG, JPEG or WebP, or a .glb 3D avatar.
        </p>
      ) : (
        <ul className="kit-assets__grid">
          {originals.map((asset) => (
            <AssetCard
              key={asset.id}
              kitId={kit.id}
              asset={asset}
              cutout={kit.assets.find((other) => other.cutoutOf === asset.id) ?? null}
              cutting={cutting.has(asset.id)}
              onCut={() => void cut(asset.id)}
              onChanged={onChanged}
              onError={setError}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

interface AssetCardProps {
  kitId: string;
  asset: BrandAsset;
  cutout: BrandAsset | null;
  cutting: boolean;
  onCut: () => void;
  onChanged: (kit: BrandKit) => Promise<void>;
  onError: (message: string) => void;
}

function AssetCard({ kitId, asset, cutout, cutting, onCut, onChanged, onError }: AssetCardProps) {
  const [label, setLabel] = useState(asset.label);
  const [note, setNote] = useState(asset.note);
  const [confirming, setConfirming] = useState(false);
  useEffect(() => {
    setLabel(asset.label);
    setNote(asset.note);
  }, [asset.label, asset.note]);

  const save = async () => {
    if (label === asset.label && note === asset.note) return;
    try {
      await onChanged(await api.updateBrandAsset(kitId, asset.id, label, note));
    } catch (cause) {
      onError(errorMessage(cause));
    }
  };
  const remove = async () => {
    try {
      await onChanged(await api.removeBrandAsset(kitId, asset.id));
    } catch (cause) {
      onError(errorMessage(cause));
    }
  };
  const name = asset.file.split("/").pop() ?? "";

  return (
    <li className="kit-asset">
      <div className="kit-asset__pictures">
        {asset.kind === "avatar3d" ? (
          <span className="kit-asset__thumb kit-asset__thumb--3d" title="3D avatar">
            <Icon name="cube" size={28} />
            <span className="mono">.glb</span>
          </span>
        ) : (
          <img className="kit-asset__thumb" src={convertFileSrc(asset.file)} alt={asset.label || name} />
        )}
        {cutout ? (
          <img className="kit-asset__thumb kit-asset__thumb--cut" src={`${convertFileSrc(cutout.file)}?v=${cutout.id}`} alt={`${asset.label || name}, background removed`} title="Cut-out" />
        ) : cutting ? (
          <span className="kit-asset__thumb kit-asset__thumb--wait">
            <Spinner label="Making cut-out" />
          </span>
        ) : null}
      </div>
      <span className="kit-asset__file mono" title={name}>
        {name}
      </span>
      <input type="text" className="kit-asset__input" value={label} maxLength={60} placeholder="Label, like Yuvraj" aria-label="Label" onChange={(event) => setLabel(event.target.value)} onBlur={() => void save()} />
      <input type="text" className="kit-asset__input" value={note} maxLength={200} placeholder="When to use it" aria-label="When to use it" onChange={(event) => setNote(event.target.value)} onBlur={() => void save()} />
      <div className="kit-asset__actions">
        {asset.kind === "photo" ? (
          <Button variant="ghost" icon="cutout" busy={cutting} onClick={onCut}>
            {cutout ? "Redo Cut-out" : "Make Cut-out"}
          </Button>
        ) : (
          <span />
        )}
        {confirming ? (
          <span className="row">
            <Button className="button--danger" onClick={() => void remove()}>
              Remove
            </Button>
            <Button variant="ghost" onClick={() => setConfirming(false)}>
              Keep
            </Button>
          </span>
        ) : (
          <IconButton icon="trash" label={cutout ? "Remove photo and its cut-out" : "Remove"} onClick={() => setConfirming(true)} />
        )}
      </div>
    </li>
  );
}
