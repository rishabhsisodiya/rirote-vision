import assert from "node:assert/strict";
import { test } from "node:test";

import type { Comment } from "../lib/types.ts";
import { fixBrief } from "./brief.ts";

const note = (time: number, text: string, x: number | null = null, y: number | null = null, resolved = false): Comment => ({
  id: text, time, x, y, text, resolved, createdAt: 0,
});

test("open notes become one request in time order, with where each one is", () => {
  const brief = fixBrief([note(8.2, "Logo too close to the top", 0.72, 0.1), note(1.5, "Hook is slow"), note(3, "done", null, null, true)]);
  assert.match(brief, /^Fix these 2 review notes/);
  assert.match(brief, /1\. At 1\.50 s: Hook is slow/);
  assert.match(brief, /2\. At 8\.20 s near the top right of the frame \(72% across, 10% down\): Logo too close/);
  assert.doesNotMatch(brief, /done/);
  assert.match(fixBrief([note(2, "Centre it", 0.5, 0.5)]), /^Fix this review note[\s\S]*near the centre of the frame/);
});
