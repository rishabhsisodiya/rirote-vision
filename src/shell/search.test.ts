import assert from "node:assert/strict";
import { test } from "node:test";

import { matches } from "./search.ts";

test("every typed word must appear, in any order and case", () => {
  assert.equal(matches("", "anything"), true);
  assert.equal(matches("launch rirote", "Rirote Launch", "/Users/me/films"), true);
  assert.equal(matches("films launch", "Rirote Launch", "/Users/me/films"), true, "words may come from any text");
  assert.equal(matches("launch erptick", "Rirote Launch"), false);
});
