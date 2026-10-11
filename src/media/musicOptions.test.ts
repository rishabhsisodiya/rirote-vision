import assert from "node:assert/strict";
import { test } from "node:test";

import { defaultImpact, defaultSeconds, musicProblem } from "./musicOptions.ts";

test("the bed starts out as long as the film", () => {
  assert.equal(defaultSeconds(24.452), 25);
  assert.equal(defaultSeconds(1.2), 3);
  assert.equal(defaultSeconds(4000), 600);
  assert.equal(defaultSeconds(null), 30);
  assert.equal(defaultSeconds(0), 30);
});

test("the impact starts about two thirds in, with room on both sides", () => {
  assert.equal(defaultImpact(30), 20);
  assert.equal(defaultImpact(12), 8);
  assert.equal(defaultImpact(5), 3);
  for (const seconds of [5, 6, 9, 30, 600]) {
    assert.equal(musicProblem("cinematic", seconds, defaultImpact(seconds)), null, String(seconds));
  }
});

test("lengths and impacts out of range are explained", () => {
  assert.equal(musicProblem("calm", 24, null), null);
  assert.match(musicProblem("calm", 2, null) ?? "", /3 to 600/);
  assert.match(musicProblem("bright", Number.NaN, null) ?? "", /3 to 600/);
  assert.match(musicProblem("cinematic", 4, 3) ?? "", /at least 5 seconds/);
  assert.match(musicProblem("cinematic", 12, null) ?? "", /which second/);
  assert.match(musicProblem("cinematic", 12, 11) ?? "", /between second 3 and second 10/);
  assert.match(musicProblem("cinematic", 12, 2) ?? "", /between second 3 and second 10/);
});
