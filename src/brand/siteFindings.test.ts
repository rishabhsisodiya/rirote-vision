import assert from "node:assert/strict";
import { test } from "node:test";

import type { BrandKit, SiteFindings } from "../lib/types.ts";
import { anyPicked, applyPicks, DEFAULT_COLOURS, initialPicks, siteHost } from "./siteFindings.ts";

const kit = (change: Partial<BrandKit> = {}): BrandKit => ({
  id: "",
  name: "",
  colours: DEFAULT_COLOURS.map((colour) => ({ ...colour })),
  headingFont: "",
  bodyFont: "",
  logo: null,
  tone: "",
  website: "theangaraa.com",
  assets: [],
  updatedAt: 0,
  ...change,
});

const found: SiteFindings = {
  url: "https://www.theangaraa.com/",
  name: "ANGARAA",
  colours: [
    { name: "Background", hex: "#141110" },
    { name: "Accent", hex: "#FF6A2B" },
  ],
  headingFont: "Barlow Condensed",
  bodyFont: null,
  tone: "Plain and direct.",
  logoFile: "/data/brand-kits/_site-reads/read-1/logo.webp",
  aiUsed: true,
  notes: [],
};

test("a new kit takes everything that was found", () => {
  const picks = initialPicks(found, kit());
  assert.deepEqual(picks, { name: true, colours: [true, true], headingFont: true, bodyFont: false, tone: true, logo: true });
  const applied = applyPicks(kit(), found, picks);
  assert.equal(applied.kit.name, "ANGARAA");
  assert.deepEqual(applied.kit.colours, found.colours);
  assert.notEqual(applied.kit.colours[0], found.colours[0], "colours are copied, not shared");
  assert.equal(applied.kit.headingFont, "Barlow Condensed");
  assert.equal(applied.kit.website, "theangaraa.com", "the typed website is kept");
  assert.equal(applied.logo, found.logoFile);
});

test("fields already filled in start unticked and stay as they are", () => {
  const filled = kit({ id: "kit-1", name: "Angaraa", colours: [{ name: "Brand", hex: "#FF0000" }], headingFont: "Anton", tone: "Mine.", logo: "/kit/logo.svg" });
  const picks = initialPicks(found, filled);
  assert.deepEqual(picks, { name: false, colours: [false, false], headingFont: false, bodyFont: false, tone: false, logo: false });
  assert.equal(anyPicked(picks), false);
  assert.deepEqual(applyPicks(filled, found, picks), { kit: filled, logo: null });
});

test("ticking some colours replaces the kit's colours with just those", () => {
  const picks = { ...initialPicks(found, kit()), colours: [false, true] };
  assert.deepEqual(applyPicks(kit(), found, picks).kit.colours, [{ name: "Accent", hex: "#FF6A2B" }]);
});

test("hosts are shown without the scheme or www", () => {
  assert.equal(siteHost("https://www.theangaraa.com/shop?x=1"), "theangaraa.com");
  assert.equal(siteHost("http://riroteglobal.com"), "riroteglobal.com");
});
