import assert from "node:assert/strict";
import { test } from "node:test";

import type { AgentEvent } from "../lib/types.ts";
import { chatReducer, EMPTY_CHAT, inlineRuns, toolVerb, type ChatState } from "./chatState.ts";

const run = (state: ChatState, ...events: AgentEvent[]) =>
  events.reduce((now, event) => chatReducer(now, { type: "event", event }), state);

test("a turn streams, uses a tool and finishes", () => {
  let state = chatReducer(EMPTY_CHAT, { type: "sent", prompt: "trim the intro" });
  assert.equal(state.busy, true);
  state = run(state, { kind: "textDelta", text: "On " }, { kind: "textDelta", text: "it" });
  assert.equal(state.streaming, "On it");
  state = run(
    state,
    { kind: "text", text: "On it." },
    { kind: "tool", id: "t1", tool: "Edit", detail: "index.html" },
    { kind: "toolResult", id: "t1", ok: true },
    { kind: "text", text: "Trimmed." },
    { kind: "finished", ok: true, text: "Trimmed." },
  );
  assert.deepEqual(
    state.messages.map((message) => [message.role, message.text, message.ok]),
    [
      ["user", "trim the intro", undefined],
      ["assistant", "On it.", undefined],
      ["tool", "index.html", true],
      ["assistant", "Trimmed.", undefined],
    ],
  );
  assert.equal(state.busy, false);
  assert.equal(state.streaming, "");
});

test("a request waits until answered, and always-allow is counted", () => {
  let state = chatReducer(EMPTY_CHAT, { type: "sent", prompt: "render" });
  state = run(state, { kind: "approval", requestId: "r1", tool: "Bash", detail: "npm run render", rule: "npm run:*" });
  assert.deepEqual(state.approvals, [{ requestId: "r1", tool: "Bash", detail: "npm run render", rule: "npm run:*" }]);
  state = chatReducer(state, { type: "answered", requestId: "r1", always: true });
  assert.equal(state.approvals.length, 0);
  assert.equal(state.rules, 1);
});

test("stopping or failing ends the turn and closes what was open", () => {
  const open = run(
    chatReducer(EMPTY_CHAT, { type: "sent", prompt: "x" }),
    { kind: "tool", id: "t1", tool: "Bash", detail: "sleep 99" },
    { kind: "approval", requestId: "r1", tool: "Bash", detail: "rm x", rule: null },
    { kind: "textDelta", text: "half a sent" },
  );
  for (const [event, said] of [
    [{ kind: "stopped" }, "Stopped."],
    [{ kind: "failed", message: "Not signed in." }, "Not signed in."],
    [{ kind: "finished", ok: false, text: "Usage limit reached." }, "Usage limit reached."],
  ] as [AgentEvent, string][]) {
    const state = run(open, event);
    assert.equal(state.busy, false);
    assert.equal(state.streaming, "");
    assert.equal(state.approvals.length, 0);
    assert.equal(state.messages.find((message) => message.id === "t1")?.ok, false);
    assert.deepEqual(state.messages.at(-1), { role: "notice", text: said });
  }
});

test("a send that never started says why", () => {
  const sent = chatReducer(EMPTY_CHAT, { type: "sent", prompt: "x" });
  const state = chatReducer(sent, { type: "sendFailed", message: "claude was not found." });
  assert.equal(state.busy, false);
  assert.equal(state.messages.at(-1)?.text, "claude was not found.");
});

test("a saved conversation loads, still running or not", () => {
  const view = { messages: [{ role: "user" as const, text: "hi" }], busy: true, rules: [{ tool: "Bash", content: "ls" }], trust: "trusted" as const };
  const state = chatReducer(EMPTY_CHAT, { type: "loaded", view });
  assert.equal(state.loaded, true);
  assert.equal(state.busy, true);
  assert.equal(state.rules, 1);
  assert.equal(state.trust, "trusted");
  assert.equal(EMPTY_CHAT.trust, null, "a project starts with no level chosen");
});

test("tools and inline styles are put in words", () => {
  assert.equal(toolVerb("Edit"), "Edited");
  assert.equal(toolVerb("mcp__thing"), "mcp__thing");
  assert.equal(toolVerb(undefined), "Used a tool");
  assert.deepEqual(inlineRuns("Moved `s1` to **3 s**."), [
    { text: "Moved ", style: "plain" },
    { text: "s1", style: "code" },
    { text: " to ", style: "plain" },
    { text: "3 s", style: "strong" },
    { text: ".", style: "plain" },
  ]);
  assert.deepEqual(inlineRuns("plain"), [{ text: "plain", style: "plain" }]);
});
