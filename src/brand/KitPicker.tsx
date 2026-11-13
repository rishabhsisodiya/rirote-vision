import type { BrandKit } from "../lib/types";

/** Chooses a brand kit for a new video, or none. Hidden when there are no kits. */
export function KitPicker({ kits, value, onChange }: { kits: BrandKit[]; value: string; onChange: (id: string) => void }) {
  if (kits.length === 0) return null;
  return (
    <label className="field">
      <span className="field__label">Brand kit</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">None</option>
        {kits.map((kit) => (
          <option key={kit.id} value={kit.id}>
            {kit.name}
            {kit.assets.length ? ` · ${kit.assets.length} assets` : ""}
          </option>
        ))}
      </select>
    </label>
  );
}
