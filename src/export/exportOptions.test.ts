import assert from "node:assert/strict";
import { test } from "node:test";

import { defaultName, nameProblem, sizeOptions } from "./exportOptions.ts";

test("a standard shape can also be exported in 4K", () => {
  assert.deepEqual(sizeOptions({ width: 1080, height: 1920 }), [
    { value: null, label: "Composition size · 1080 × 1920" },
    { value: "portrait-4k", label: "4K Portrait · 2160 × 3840" },
  ]);
  assert.equal(sizeOptions({ width: 1920, height: 1080 })[1]?.value, "landscape-4k");
  assert.equal(sizeOptions({ width: 1080, height: 1080 })[1]?.value, "square-4k");
});

test("any other shape is exported at its own size only", () => {
  assert.deepEqual(sizeOptions({ width: 1600, height: 1000 }), [
    { value: null, label: "Composition size · 1600 × 1000" },
  ]);
  assert.deepEqual(sizeOptions(null), [{ value: null, label: "Composition size" }]);
});

test("the default name is one the backend accepts", () => {
  assert.equal(defaultName("launch-website"), "launch-website");
  assert.equal(defaultName("  ERPTick: Launch / Film!  "), "ERPTick Launch Film");
  assert.equal(defaultName("../.."), "export");
  assert.equal(defaultName("-o"), "o");
  for (const name of ["launch-website", "ERPTick: Launch / Film!", "../..", "-o", "रिरोट फ़िल्म"]) {
    assert.equal(nameProblem(defaultName(name)), null, name);
  }
});

test("a typed name is checked before it is sent", () => {
  assert.equal(nameProblem("Cut 1 (final)"), null);
  assert.match(nameProblem("") ?? "", /name/);
  assert.match(nameProblem("a/b") ?? "", /letters/);
  assert.match(nameProblem(".hidden") ?? "", /letters/);
  assert.match(nameProblem("x".repeat(81)) ?? "", /80/);
});
