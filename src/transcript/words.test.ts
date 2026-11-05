import assert from "node:assert/strict";
import { test } from "node:test";

import type { Clip } from "../lib/types.ts";
import { clipWords, isFiller, pauses, rangesForWords, removedSeconds, shortenRanges } from "./words.ts";

const clip = (over: Partial<Clip> = {}): Clip => ({
  id: "t", ref: "#t", label: null, kind: "video", track: 1, start: 10, end: 16, src: "talk.mp4",
  volume: 1, rate: 1, mediaStart: 2, media: true, editable: true, ...over,
});

const file = [
  { text: "Before", start: 1.0, end: 1.6 },
  { text: "So,", start: 2.1, end: 2.4 },
  { text: "um", start: 2.5, end: 2.8 },
  { text: "tickets", start: 2.9, end: 3.4 },
  { text: "lie.", start: 4.6, end: 5.0 },
  { text: "After", start: 9.0, end: 9.4 },
];

test("only the words the clip plays are placed, on timeline time", () => {
  const words = clipWords(clip(), file);
  assert.deepEqual(words.map((w) => w.text), ["So,", "um", "tickets", "lie."]);
  assert.deepEqual([words[0]!.start, words[0]!.index], [10.1, 1]);
  assert.equal(words[1]!.filler, true);
  const fast = clipWords(clip({ rate: 2, end: 13 }), file);
  assert.equal(fast[0]!.start, 10.05, "at double speed a word comes twice as soon");
});

test("fillers are recognised whatever their case and punctuation", () => {
  assert.ok(isFiller("Um,") && isFiller("uhh") && isFiller("HMM."));
  assert.ok(!isFiller("umbrella") && !isFiller("so"));
});

test("chosen words become cuts, side-by-side words joined", () => {
  const words = clipWords(clip(), file);
  const ranges = rangesForWords(words, new Set([1, 2]));
  assert.equal(ranges.length, 1);
  assert.equal(ranges[0]![0], 10.1);
  assert.ok(Math.abs(ranges[0]![1] - 10.9) < 1e-9, "runs up to the next word, not past it");
});

test("long pauses are found and shortened to what is kept", () => {
  const words = clipWords(clip(), file);
  const found = pauses(words, 0.8);
  assert.equal(found.length, 1);
  assert.ok(Math.abs(found[0]!.start - 11.4) < 1e-9 && Math.abs(found[0]!.end - 12.6) < 1e-9);
  const cuts = shortenRanges(found, 0.3);
  assert.ok(Math.abs(removedSeconds(cuts) - 0.9) < 1e-9);
});
