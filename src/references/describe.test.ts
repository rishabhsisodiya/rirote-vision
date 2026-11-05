import assert from "node:assert/strict";
import { test } from "node:test";

import type { Reference, ReferenceStep } from "../lib/types.ts";
import { clock, progressLine } from "./describe.ts";

const step = (name: ReferenceStep["name"], state: ReferenceStep["state"], error: string | null = null): ReferenceStep => ({
  name,
  state,
  seconds: null,
  error,
});

const reference = (status: Reference["status"], steps: ReferenceStep[]): Reference => ({
  id: "r", source: "s", platform: "youtube", title: null, creator: null, duration: null, width: null, height: null,
  addedAt: 0, steps, status, poster: null, video: null, folder: "/tmp/r",
});

test("progress says what the study is doing, why it stopped, or that it is ready", () => {
  const steps = [step("download", "done"), step("shots", "running"), step("keyframes", "pending"), step("transcribe", "pending"), step("analyze", "pending")];
  assert.equal(progressLine(reference("running", steps)), "Finding the cuts… (2 of 5)");
  const failed = [step("download", "failed", "That video is private.\nmore"), step("shots", "skipped")];
  assert.equal(progressLine(reference("failed", failed)), "Getting the video failed: That video is private.");
  assert.equal(progressLine(reference("failed", [step("analyze", "failed", "Stopped.")])), "Stopped");
  assert.equal(progressLine(reference("ready", [])), "Studied");
});

test("lengths read the way a video site shows them", () => {
  assert.equal(clock(66), "1:06");
  assert.equal(clock(3725), "1:02:05");
  assert.equal(clock(null), "–");
});
