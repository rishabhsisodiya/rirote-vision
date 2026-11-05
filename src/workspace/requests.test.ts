import assert from "node:assert/strict";
import { test } from "node:test";

import type { Clip } from "../lib/types.ts";
import { momentsBrief, replaceBrief } from "./requests.ts";

const clip: Clip = {
  id: "talk", ref: "#talk", label: null, kind: "video", track: 1, start: 2, end: 30.5, src: "assets/talk.mp4",
  volume: 1, rate: 1, mediaStart: 0, media: true, editable: true,
};

test("replacing names both files and keeps the clip's place", () => {
  const brief = replaceBrief(clip, "assets/talk-take2.mp4");
  assert.match(brief, /#talk \(now assets\/talk\.mp4\) with assets\/talk-take2\.mp4/);
  assert.match(brief, /2\.00–30\.50 s on track 1/);
});

test("finding moments asks for a list, not an edit", () => {
  const brief = momentsBrief(clip, "assets/talk.transcript.json");
  assert.match(brief, /3 strongest moments in #talk/);
  assert.match(brief, /Do not change the project yet/);
});
