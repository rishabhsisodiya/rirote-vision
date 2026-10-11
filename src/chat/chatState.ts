// How a conversation changes as the assistant reports what it is doing.
// Kept free of React so it can be tested.

import type { AgentEvent, ApprovalRequest, ChatMessage, ChatView, TrustLevel } from "../lib/types";

export interface ChatState {
  /** False until the saved conversation has been read. */
  loaded: boolean;
  messages: ChatMessage[];
  /** The reply being written, not yet a finished message. */
  streaming: string;
  busy: boolean;
  /** Requests waiting for the user's answer. */
  approvals: ApprovalRequest[];
  /** How many standing permissions this project has. */
  rules: number;
  /** How much the assistant may do unasked; null until chosen for this project. */
  trust: TrustLevel | null;
}

export const EMPTY_CHAT: ChatState = {
  loaded: false,
  messages: [],
  streaming: "",
  busy: false,
  approvals: [],
  rules: 0,
  trust: null,
};

export type ChatAction =
  | { type: "loaded"; view: ChatView }
  | { type: "sent"; prompt: string }
  | { type: "sendFailed"; message: string }
  | { type: "answered"; requestId: string; always: boolean }
  | { type: "event"; event: AgentEvent };

const notice = (text: string): ChatMessage => ({ role: "notice", text });

/** A turn that ended, however it ended, leaves nothing running or waiting. */
function ended(state: ChatState, extra: ChatMessage[] = []): ChatState {
  const messages = state.messages.map((message) =>
    message.role === "tool" && message.ok === undefined ? { ...message, ok: false } : message,
  );
  return { ...state, messages: [...messages, ...extra], streaming: "", busy: false, approvals: [] };
}

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "loaded":
      return {
        ...state,
        loaded: true,
        messages: action.view.messages,
        busy: action.view.busy,
        rules: action.view.rules.length,
        trust: action.view.trust,
        streaming: "",
        approvals: action.view.busy ? state.approvals : [],
      };
    case "sent":
      return {
        ...state,
        busy: true,
        streaming: "",
        messages: [...state.messages, { role: "user", text: action.prompt }],
      };
    case "sendFailed":
      return ended(state, [notice(action.message)]);
    case "answered":
      return {
        ...state,
        approvals: state.approvals.filter((request) => request.requestId !== action.requestId),
        rules: state.rules + (action.always ? 1 : 0),
      };
    case "event":
      return applyEvent(state, action.event);
  }
}

function applyEvent(state: ChatState, event: AgentEvent): ChatState {
  switch (event.kind) {
    case "textDelta":
      return { ...state, busy: true, streaming: state.streaming + event.text };
    case "text":
      return {
        ...state,
        streaming: "",
        messages: [...state.messages, { role: "assistant", text: event.text }],
      };
    case "tool":
      return {
        ...state,
        streaming: "",
        messages: [...state.messages, { role: "tool", text: event.detail, tool: event.tool, id: event.id }],
      };
    case "toolResult":
      return {
        ...state,
        messages: state.messages.map((message) =>
          message.id === event.id ? { ...message, ok: event.ok } : message,
        ),
      };
    case "approval": {
      const { kind: _kind, ...request } = event;
      return { ...state, approvals: [...state.approvals, request] };
    }
    case "finished":
      return ended(state, event.ok || !event.text ? [] : [notice(event.text)]);
    case "failed":
      return ended(state, [notice(event.message)]);
    case "stopped":
      return ended(state, [notice("Stopped.")]);
  }
}

const TOOL_VERBS: Record<string, string> = {
  Read: "Read",
  Write: "Wrote",
  Edit: "Edited",
  MultiEdit: "Edited",
  NotebookEdit: "Edited",
  Bash: "Ran",
  Glob: "Looked for",
  Grep: "Searched for",
  Skill: "Used skill",
  Task: "Handed off",
  Agent: "Handed off",
  WebFetch: "Fetched",
  WebSearch: "Searched the web for",
  TodoWrite: "Updated its plan",
};

/** "Edited" for Edit, and the tool's own name for one the app does not know. */
export function toolVerb(tool: string | undefined): string {
  return (tool && TOOL_VERBS[tool]) || tool || "Used a tool";
}

export type Inline = { text: string; style: "plain" | "code" | "strong" };

/** Splits a reply into plain text, `code` and **strong** runs. */
export function inlineRuns(text: string): Inline[] {
  const runs: Inline[] = [];
  const pattern = /`([^`\n]+)`|\*\*([^*\n]+)\*\*/g;
  let from = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index > from) runs.push({ text: text.slice(from, match.index), style: "plain" });
    runs.push(match[1] !== undefined ? { text: match[1], style: "code" } : { text: match[2]!, style: "strong" });
    from = match.index + match[0].length;
  }
  if (from < text.length) runs.push({ text: text.slice(from), style: "plain" });
  return runs;
}
