import assert from "node:assert/strict";
import { test } from "node:test";

import { makeBrief, nextName, projectNameFrom, scriptBrief } from "./briefs.ts";

test("a project name comes from the telling words of the request", () => {
  assert.equal(projectNameFrom("Make a 25-second launch video for https://rirote.com with five scenes"), "25-second launch rirote.com five");
  assert.equal(projectNameFrom("Diwali candle drop for Samarth Crafts"), "Diwali candle drop Samarth");
  assert.equal(projectNameFrom("!!! ???"), "New video");
  assert.equal(projectNameFrom("", "Script video"), "Script video");
});

test("a taken name gets the next number", () => {
  assert.equal(nextName("Launch"), "Launch 2");
  assert.equal(nextName("Launch 2"), "Launch 3");
  assert.equal(nextName("Day 9"), "Day 10");
});

test("briefs carry the request, the shape and, for a script, the words and the voice", () => {
  const made = makeBrief("  A launch film  ", "portrait");
  assert.match(made, /A launch film/);
  assert.match(made, /portrait 9:16/);
  const narrated = scriptBrief("Line one.\nLine two.", "square", "af_bella");
  assert.match(narrated, /--- script ---\nLine one.\nLine two.\n--- end of script ---/);
  assert.match(narrated, /voice "af_bella"/);
  assert.match(scriptBrief("Hi.", "landscape", null), /no narration/);
});
