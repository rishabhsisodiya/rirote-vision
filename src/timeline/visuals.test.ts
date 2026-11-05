import assert from "node:assert/strict";
import { test } from "node:test";

import type { Clip } from "../lib/types.ts";
import { liveClips, placedStills, placementBrief, sourceTime, waveBars } from "./visuals.ts";

const clip = (over: Partial<Clip> = {}): Clip => ({
  id: "v", ref: "#v", label: null, kind: "video", track: 1, start: 10, end: 14, src: "a.mp4",
  volume: 1, rate: 1, mediaStart: 0, media: true, editable: true, ...over,
});

test("a trimmed or sped-up clip reads its media from the right place", () => {
  assert.equal(sourceTime(clip(), 2), 2);
  assert.equal(sourceTime(clip({ mediaStart: 5 }), 2), 7);
  assert.equal(sourceTime(clip({ mediaStart: 5, rate: 2 }), 2), 9);
});

test("waveform bars take the loudest peak under each bar", () => {
  const peaks = { step: 0.5, values: [0, 255, 51, 102, 0, 0, 0, 0] };
  // 10 px a second and a 5 px gap: each bar covers half a second, one peak.
  assert.deepEqual(waveBars(clip(), peaks, 10, 20, 5), [0, 1, 0.2, 0.4]);
  // Starting 1 s into the file skips the first two peaks.
  assert.deepEqual(waveBars(clip({ mediaStart: 1 }), peaks, 10, 10, 5), [0.2, 0.4]);
  assert.deepEqual(waveBars(clip(), peaks, 0, 10), []);
});

test("only the stills inside the clip are placed", () => {
  const stills = [0.5, 3, 6, 9].map((time) => ({ time, path: `/s/${time}.jpg` }));
  const placed = placedStills(clip({ mediaStart: 2 }), stills, 20);
  assert.deepEqual(placed.map((item) => item.still.time), [3, 6]);
});

test("the placement brief names the block, the spot and the snippet", () => {
  const brief = placementBrief({ name: "cinematic-zoom", snippet: "<div data-composition-id=\"cinematic-zoom\"></div>" }, "Cinematic Zoom", 3, clip());
  assert.match(brief, /“Cinematic Zoom” \(cinematic-zoom\)/);
  assert.match(brief, /right after #v ends, at 14.00 s/);
  assert.match(brief, /data-composition-id="cinematic-zoom"/);
  assert.match(placementBrief({ name: "x", snippet: "" }, "X", 3.5, null), /at the playhead, 3.50 s/);
});

test("the clips lit as on air are those whose file is being written", () => {
  const scene = clip({ ref: "#scene-2", src: "compositions/scene-2.html", media: false });
  const other = clip({ ref: "#scene-3", src: "compositions/scene-3.html", media: false });
  const writing = [
    { role: "tool" as const, tool: "Read", text: "compositions/scene-3.html" },
    { role: "tool" as const, tool: "Edit", text: "/Users/me/film/compositions/scene-2.html" },
    { role: "tool" as const, tool: "Edit", text: "index.html" },
  ];
  assert.deepEqual([...liveClips(writing, [scene, other], true)], ["#scene-2"]);
  assert.equal(liveClips(writing, [scene, other], false).size, 0, "nothing is on air between turns");
  const done = [{ role: "tool" as const, tool: "Edit", text: "compositions/scene-2.html", ok: true }];
  assert.equal(liveClips(done, [scene], true).size, 0);
});
