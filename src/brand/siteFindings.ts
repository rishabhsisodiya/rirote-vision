// What a website read suggests for a kit, and how the ticked parts are
// merged into the form. Kept apart from the screen so it can be tested.

import type { BrandColour, BrandKit, SiteFindings } from "../lib/types.ts";

/** The colours a new kit starts with; replacing them loses nothing. */
export const DEFAULT_COLOURS: readonly BrandColour[] = [
  { name: "Background", hex: "#0A1628" },
  { name: "Accent", hex: "#00B4D8" },
  { name: "Text", hex: "#F0F4FF" },
];

export interface Picks {
  name: boolean;
  /** One tick per found colour, in order. */
  colours: boolean[];
  headingFont: boolean;
  bodyFont: boolean;
  tone: boolean;
  logo: boolean;
}

function sameColours(a: readonly BrandColour[], b: readonly BrandColour[]): boolean {
  return a.length === b.length && a.every((colour, at) => colour.hex.toUpperCase() === b[at]?.hex.toUpperCase() && colour.name === b[at]?.name);
}

const blank = (text: string) => text.trim().length === 0;

/** What starts ticked: whatever was found for a field that is still empty. */
export function initialPicks(found: SiteFindings, kit: BrandKit): Picks {
  const coloursEmpty = kit.colours.length === 0 || sameColours(kit.colours, DEFAULT_COLOURS);
  return {
    name: found.name !== null && blank(kit.name),
    colours: found.colours.map(() => coloursEmpty),
    headingFont: found.headingFont !== null && blank(kit.headingFont),
    bodyFont: found.bodyFont !== null && blank(kit.bodyFont),
    tone: found.tone !== null && blank(kit.tone),
    logo: found.logoFile !== null && kit.logo === null,
  };
}

export function anyPicked(picks: Picks): boolean {
  return picks.name || picks.headingFont || picks.bodyFont || picks.tone || picks.logo || picks.colours.some(Boolean);
}

/**
 * The form with the ticked findings in it. Ticked colours replace the kit's
 * colours rather than adding to them. The logo is returned separately: it is
 * copied into the kit when the kit is saved.
 */
export function applyPicks(kit: BrandKit, found: SiteFindings, picks: Picks): { kit: BrandKit; logo: string | null } {
  const colours = found.colours.filter((_, at) => picks.colours[at]);
  return {
    kit: {
      ...kit,
      name: picks.name && found.name ? found.name : kit.name,
      colours: colours.length > 0 ? colours.map((colour) => ({ ...colour })) : kit.colours,
      headingFont: picks.headingFont && found.headingFont ? found.headingFont : kit.headingFont,
      bodyFont: picks.bodyFont && found.bodyFont ? found.bodyFont : kit.bodyFont,
      tone: picks.tone && found.tone ? found.tone : kit.tone,
    },
    logo: picks.logo ? found.logoFile : null,
  };
}

/** The site's host, for headings: `theangaraa.com`. */
export function siteHost(url: string): string {
  return url.replace(/^https?:\/\//i, "").replace(/^www\./i, "").split(/[/?#]/)[0] ?? url;
}
