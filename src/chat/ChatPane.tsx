import "./chat.css";

import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { formatDuration } from "../lib/format";
import type { ApprovalAnswer, ApprovalRequest, ChatMessage, EditorContext } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Spinner } from "../ui/Spinner";
import { inlineRuns, toolVerb } from "./chatState";
import { trustLabel } from "./trust";
import { TrustChooser } from "./TrustChooser";
import type { ChatControls } from "./useChat";

const SUGGESTIONS = [
  "Describe what is on the timeline",
  "Tighten the gaps between scenes",
  "Add captions for the voiceover",
];

interface ChatPaneProps {
  chat: ChatControls;
  /** What is sent along with each request. */
  context: EditorContext;
  /** The name of the selected clip, for the line under the box. */
  selectedName: string | null;
}

/** The conversation with the assistant and the box that sends requests to it. */
export function ChatPane({ chat, context, selectedName }: ChatPaneProps) {
  const { state } = chat;
  const [draft, setDraft] = useState("");
  /** The level chooser is open to change a level already chosen. */
  const [changing, setChanging] = useState(false);
  const list = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);

  // Follow the conversation as it grows.
  useEffect(() => {
    const element = list.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [state.messages, state.streaming, state.approvals]);

  // The box grows with what is typed, up to a limit set in the stylesheet.
  useEffect(() => {
    const element = box.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [draft]);

  const submit = () => {
    const prompt = draft.trim();
    if (!prompt || state.busy || choosing) return;
    setDraft("");
    void chat.send(prompt, context);
  };

  const onKey = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  // A project with no level chosen asks for one before anything is sent.
  const choosing = state.loaded && (state.trust === null || changing);
  const empty = state.loaded && state.messages.length === 0 && !state.busy;

  return (
    <>
      <header className="chat__header">
        <h2 className="pane__heading">Assistant</h2>
        {state.trust && !choosing ? (
          <Button
            variant="ghost"
            className={`chat__level chat__level--${state.trust}`}
            disabled={state.busy}
            title="Change how much the assistant may do without asking"
            onClick={() => setChanging(true)}
          >
            {trustLabel(state.trust)}
          </Button>
        ) : null}
        {state.rules > 0 ? (
          <Button
            variant="ghost"
            disabled={state.busy}
            title="Withdraw every “always allow” given in this project"
            onClick={() => void chat.reset("permissions")}
          >
            Forget {state.rules} {state.rules === 1 ? "permission" : "permissions"}
          </Button>
        ) : null}
        <IconButton
          icon="compose"
          label="New conversation"
          disabled={state.busy || state.messages.length === 0}
          onClick={() => void chat.reset("conversation")}
        />
      </header>

      <div className="chat__list" ref={list} role="log" aria-label="Conversation" aria-live="polite">
        {!state.loaded ? (
          <p className="chat__status">
            <Spinner label="Loading the conversation" />
          </p>
        ) : choosing ? (
          <TrustChooser
            current={state.trust}
            onChoose={(level) => {
              setChanging(false);
              void chat.setTrust(level);
            }}
            onCancel={state.trust ? () => setChanging(false) : undefined}
          />
        ) : empty ? (
          <div className="chat__empty">
            <Icon name="spark" size={22} />
            <p className="chat__empty-title">Ask for an edit</p>
            <p className="chat__empty-text">
              Claude works on this project using your Claude plan. It asks before running commands
              or touching anything outside the project.
            </p>
            <ul className="chat__suggestions">
              {SUGGESTIONS.map((suggestion) => (
                <li key={suggestion}>
                  <button
                    type="button"
                    onClick={() => {
                      setDraft(suggestion);
                      box.current?.focus();
                    }}
                  >
                    {suggestion}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          state.messages.map((message, index) => <Message key={index} message={message} busy={state.busy} />)
        )}
        {state.streaming ? <p className="message message--assistant message--streaming">{state.streaming}</p> : null}
        {state.approvals.map((request) => (
          <Approval key={request.requestId} request={request} onAnswer={chat.answer} />
        ))}
        {state.busy && !state.streaming && state.approvals.length === 0 ? (
          <p className="chat__status" role="status">
            <Spinner />
            Working
          </p>
        ) : null}
      </div>

      <form
        className="composer"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="composer__box">
          <textarea
            ref={box}
            rows={1}
            value={draft}
            maxLength={20000}
            placeholder="Describe an edit"
            disabled={choosing}
            aria-label="Message to the assistant"
            aria-describedby="composer-context"
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={onKey}
          />
          {state.busy ? (
            <IconButton className="composer__action" icon="stop" label="Stop the assistant" onClick={chat.stop} />
          ) : (
            <IconButton
              className="composer__action composer__action--send"
              icon="send"
              label="Send (Return)"
              disabled={!draft.trim() || choosing}
              onClick={submit}
            />
          )}
        </div>
        <p id="composer-context" className="composer__context">
          {context.playhead === null ? "Preview not ready" : `Playhead ${formatDuration(context.playhead)}`}
          {selectedName ? ` · Selected ${selectedName}` : ""}
        </p>
      </form>
    </>
  );
}

function Message({ message, busy }: { message: ChatMessage; busy: boolean }) {
  if (message.role === "tool") {
    const waiting = message.ok === undefined;
    return (
      <p className={`tool-row ${message.ok === false ? "tool-row--failed" : ""}`}>
        {waiting && busy ? <Spinner /> : <Icon name={message.ok === false ? "missing" : "check"} size={13} />}
        <span className="tool-row__verb">
          {toolVerb(message.tool)}
          {message.ok === false ? " (did not finish)" : ""}
        </span>
        <span className="tool-row__detail" title={message.text}>
          {message.text}
        </span>
      </p>
    );
  }
  if (message.role === "assistant") {
    return (
      <p className="message message--assistant">
        {inlineRuns(message.text).map((run, index) =>
          run.style === "code" ? (
            <code key={index}>{run.text}</code>
          ) : run.style === "strong" ? (
            <strong key={index}>{run.text}</strong>
          ) : (
            run.text
          ),
        )}
      </p>
    );
  }
  return <p className={`message message--${message.role}`}>{message.text}</p>;
}

interface ApprovalProps {
  request: ApprovalRequest;
  onAnswer: (requestId: string, answer: ApprovalAnswer) => void;
}

function Approval({ request, onAnswer }: ApprovalProps) {
  const answer = (choice: ApprovalAnswer) => onAnswer(request.requestId, choice);
  return (
    <div className="approval" role="group" aria-label="The assistant is asking for permission">
      <p className="approval__title">
        <Icon name="alert" />
        Allow {toolVerb(request.tool) === "Ran" ? "this command" : `${request.tool}`}?
      </p>
      <pre className="approval__detail">{request.detail || request.tool}</pre>
      <div className="approval__actions">
        <Button onClick={() => answer("deny")}>Deny</Button>
        {request.rule ? (
          <Button onClick={() => answer("always")} title={`Stop asking in this project for: ${request.rule}`}>
            Always Allow
          </Button>
        ) : null}
        <Button variant="primary" onClick={() => answer("allow")}>
          Allow
        </Button>
      </div>
      {request.rule ? (
        <p className="approval__rule">
          Always Allow covers <code>{request.rule}</code> in this project.
        </p>
      ) : null}
    </div>
  );
}
