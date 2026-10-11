import assert from "node:assert/strict";
import { test } from "node:test";

import type { Clip } from "../lib/types.ts";
import {
  buildLanes,
  clampScale,
  dragSpan,
  editFor,
  editPoints,
  fitScale,
  formatRulerTime,
  nearest,
  neighbourPoint,
  newTrack,
  placementFor,
  rulerStep,
  rulerTicks,
  sameFamily,
  snapTargets,
} from "./model.ts";

const clip = (id: string, kind: string, track: number, start: number, end: number): Clip => ({
  id,
  ref: `#${id}`,
  label: null,
  kind,
  track,
  start,
  end,
  src: null,
  volume: kind === "audio" ? 1 : null,
  media: kind === "audio",
  editable: true,
});

test("lanes put picture on top, highest track first, then sound", () => {
  const lanes = buildLanes([
    clip("bed", "audio", 7, 0, 20),
    clip("s0", "graphics", 0, 0, 3),
    clip("cap", "captions", 4, 0, 1),
    clip("vo", "audio", 6, 0, 2),
    clip("s1", "graphics", 0, 3, 6),
  ]);
  assert.deepEqual(
    lanes.map((lane) => lane.label),
    ["Captions 4", "Graphics 0", "Audio 6", "Audio 7"],
  );
  assert.deepEqual(lanes[1]!.clips.map((member) => member.id), ["s0", "s1"]);
});

test("ruler marks keep their distance at any zoom", () => {
  assert.equal(rulerStep(40), 2);
  assert.equal(rulerStep(400), 0.25);
  assert.equal(rulerStep(5), 15);
  assert.deepEqual(rulerTicks(2, 0.5), [0, 0.5, 1, 1.5, 2]);
  assert.equal(formatRulerTime(65, 5), "1:05");
  assert.equal(formatRulerTime(5.5, 0.5), "0:05.5");
  assert.equal(formatRulerTime(0.25, 0.25), "0:00.25");
});

test("zoom stays between fitting and forty times closer", () => {
  const fit = fitScale(1000, 20);
  assert.equal(fit, 50);
  assert.equal(clampScale(10, fit), 50);
  assert.equal(clampScale(90, fit), 90);
  assert.equal(clampScale(99999, fit), 2000);
});

test("snapping picks the nearest target in reach", () => {
  assert.equal(nearest(4.9, [0, 5, 5.05], 0.2), 5);
  assert.equal(nearest(4.0, [0, 5], 0.2), null);
  const targets = snapTargets([clip("a", "graphics", 0, 1, 2), clip("b", "graphics", 0, 4, 6)], "#b", 3, 10);
  assert.deepEqual(targets, [0, 10, 3, 1, 2]);
});

test("moving keeps the length, snaps either edge and stays inside the film", () => {
  const options = { targets: [0, 10, 5], threshold: 0.2, duration: 10 };
  assert.deepEqual(dragSpan({ start: 1, end: 3 }, "move", 0.5, options), { start: 1.5, end: 3.5 });
  assert.deepEqual(dragSpan({ start: 1, end: 3 }, "move", 1.9, options), { start: 3, end: 5 }); // end snaps to 5
  assert.deepEqual(dragSpan({ start: 1, end: 3 }, "move", 3.9, options), { start: 5, end: 7 }); // start snaps to 5
  assert.deepEqual(dragSpan({ start: 1, end: 3 }, "move", -9, options), { start: 0, end: 2 });
  assert.deepEqual(dragSpan({ start: 1, end: 3 }, "move", 99, options), { start: 8, end: 10 });
});

test("trimming moves one edge and never inverts the clip", () => {
  const options = { targets: [5], threshold: 0.2, duration: 10 };
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "start", 2.9, options), { start: 5, end: 6 });
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "start", 50, options), { start: 5.95, end: 6 });
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "start", -50, options), { start: 0, end: 6 });
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "end", -0.9, options), { start: 2, end: 5 });
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "end", -50, options), { start: 2, end: 2.05 });
  assert.deepEqual(dragSpan({ start: 2, end: 6 }, "end", 50, options), { start: 2, end: 10 });
});

test("a drag becomes the matching edit, or none when nothing moved", () => {
  const title = clip("title", "graphics", 0, 2, 6);
  assert.deepEqual(editFor(title, "move", { start: 3, end: 7 }), { type: "move", clip: "#title", start: 3 });
  assert.deepEqual(editFor(title, "start", { start: 3, end: 6 }), { type: "trimStart", clip: "#title", start: 3 });
  assert.deepEqual(editFor(title, "end", { start: 2, end: 5 }), { type: "trimEnd", clip: "#title", end: 5 });
  assert.equal(editFor(title, "move", { start: 2.0004, end: 6.0004 }), null);
});

test("a clip dropped on another lane is placed there; otherwise it is a plain move", () => {
  const title = clip("title", "graphics", 0, 2, 6);
  assert.deepEqual(placementFor(title, "move", { start: 3, end: 7 }, 4), { type: "place", clip: "#title", start: 3, track: 4 });
  assert.deepEqual(placementFor(title, "move", { start: 2, end: 6 }, 4), { type: "place", clip: "#title", start: 2, track: 4 });
  assert.deepEqual(placementFor(title, "move", { start: 3, end: 7 }, 0), { type: "move", clip: "#title", start: 3 });
  assert.deepEqual(placementFor(title, "end", { start: 2, end: 5 }, 4), { type: "trimEnd", clip: "#title", end: 5 });
  assert.equal(placementFor(title, "move", { start: 2, end: 6 }, 0), null);
  assert.equal(sameFamily("graphics", "captions"), true);
  assert.equal(sameFamily("audio", "audio"), true);
  assert.equal(sameFamily("video", "audio"), false);
});

test("edit points are where cuts fall, and the playhead steps between them", () => {
  const clips = [clip("a", "graphics", 0, 0, 3), clip("b", "graphics", 0, 3, 7.5), clip("vo", "audio", 6, 0.2, 2.8)];
  const points = editPoints(clips, 10);
  assert.deepEqual(points, [0, 0.2, 2.8, 3, 7.5, 10]);
  assert.equal(neighbourPoint(points, 3, 1), 7.5);
  assert.equal(neighbourPoint(points, 3, -1), 2.8);
  assert.equal(neighbourPoint(points, 2.9, 1), 3);
  assert.equal(neighbourPoint(points, 10, 1), 10);
  assert.equal(neighbourPoint(points, 0, -1), 0);
  // Parked on the frame just before the 0.2 cut, "next" must not stay on it.
  assert.equal(neighbourPoint(points, 0.1667, 1), 2.8);
  assert.equal(neighbourPoint(points, 0.1667, -1), 0);
  assert.equal(neighbourPoint(points, 0, 1), 0.2);
  assert.equal(newTrack(clips), 7);
  assert.equal(newTrack([]), 0);
});

