import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_OPTIONS, guideZones, readOptions } from "./previewOptions.ts";

test("saved options are read back, and anything unknown falls back", () => {
  const storage = (value: string | null) => ({ getItem: () => value });
  assert.deepEqual(readOptions(storage(JSON.stringify({ backdrop: "checker", guides: "social" }))), { backdrop: "checker", guides: "social" });
  assert.deepEqual(readOptions(storage(JSON.stringify({ backdrop: "purple" }))), DEFAULT_OPTIONS);
  assert.deepEqual(readOptions(storage("{ broken")), DEFAULT_OPTIONS);
  assert.deepEqual(readOptions(undefined), DEFAULT_OPTIONS);
});

test("social guides only apply to tall frames", () => {
  assert.equal(guideZones("off", 1080, 1920).safe, null);
  assert.equal(guideZones("social", 1920, 1080).covered.length, 0, "a wide frame gets the action margin only");
  const tall = guideZones("social", 1080, 1920);
  assert.equal(tall.covered.length, 3);
  for (const zone of [tall.safe!, ...tall.covered]) {
    assert.ok(zone.left >= 0 && zone.top >= 0 && zone.left + zone.width <= 1.0001 && zone.top + zone.height <= 1.0001, zone.label);
  }
});
