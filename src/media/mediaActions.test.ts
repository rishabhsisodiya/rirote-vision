import assert from "node:assert/strict";
import { test } from "node:test";

import type { MediaItem, MediaKind } from "../lib/types.ts";
import { canRemoveBackground, canTranscribe, placementRequest } from "./mediaActions.ts";

const item = (path: string, kind: MediaKind): MediaItem => ({
  path,
  name: path.split("/").pop()!,
  kind,
  bytes: 1,
  duration: null,
});

test("only sound and video can be transcribed", () => {
  assert.equal(canTranscribe(item("a.wav", "audio")), true);
  assert.equal(canTranscribe(item("a.mp4", "video")), true);
  assert.equal(canTranscribe(item("a.png", "image")), false);
  assert.equal(canTranscribe(item("a.srt", "captions")), false);
});

test("cut-outs are for video and the image formats the model reads", () => {
  assert.equal(canRemoveBackground(item("a.MOV", "video")), true);
  assert.equal(canRemoveBackground(item("me.JPG", "image")), true);
  assert.equal(canRemoveBackground(item("logo.svg", "image")), false);
  assert.equal(canRemoveBackground(item("vo.wav", "audio")), false);
});

test("the placement request names the file and what it is", () => {
  assert.match(placementRequest(item("assets/vo/line.wav", "audio")), /sound file "assets\/vo\/line\.wav".*playhead/);
  assert.match(placementRequest(item("assets/shot.png", "image")), /image file "assets\/shot\.png"/);
  assert.match(placementRequest(item("assets/talk.srt", "captions")), /^Add captions .*"assets\/talk\.srt"/);
});
