import "./chat.css";

import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { restyleBrief, withAttachedKit, type BrandKitsState } from "../brand/useBrandKits";
import { formatDuration } from "../lib/format";
import { api, errorMessage } from "../lib/ipc";
import type { ApprovalAnswer, ApprovalRequest, BrandKit, ChatMessage, EditorContext } from "../lib/types";
import { Button, IconButton } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import { inlineRuns, toolVerb } from "./chatState";
import { trustLabel } from "./trust";
import { TrustChooser } from "./TrustChooser";
import { ComposerTools } from "./ComposerTools";
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
  projectPath: string;
  kits: BrandKitsState;
}

/** The conversation with the assistant and the box that sends requests to it. */
export function ChatPane({ chat, context, selectedName, projectPath, kits }: ChatPaneProps) {
  const { state } = chat;
  const [draft, setDraft] = useState("");
  /** A kit picked from the Brand kit menu: it goes with the next message only. */
  const [attached, setAttached] = useState<BrandKit | null>(null);
  const [kitBusy, setKitBusy] = useState(false);
  const [kitError, setKitError] = useState<string | null>(null);
  /** The level chooser is open to change a level already chosen. */
  const [changing, setChanging] = useState(false);
  /** Requests typed while a turn runs; each goes when the one before ends. */
  const [queued, setQueued] = useState<string[]>([]);
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

  const dispatch = (prompt: string) => {
    if (state.busy || queued.length) setQueued((before) => [...before, prompt]);
    else void chat.send(prompt, context);
  };

  /** The attached kit's summary, read fresh so it lists the kit as it is now. */
  const kitBrief = async (kit: BrandKit): Promise<string | null> => {
    try {
      return await api.brandBrief(kit.id);
    } catch (cause) {
      setKitError(errorMessage(cause));
      return null;
    }
  };

  const submit = async () => {
    const typed = draft.trim();
    if (!typed || choosing || kitBusy) return;
    let prompt = typed;
    if (attached) {
      setKitBusy(true);
      const brief = await kitBrief(attached);
      setKitBusy(false);
      if (brief === null) return;
      prompt = withAttachedKit(typed, brief);
      setAttached(null);
    }
    setDraft("");
    dispatch(prompt);
  };

  // Picking a kit makes it this video's kit (its files are copied into
  // assets/brand/) and attaches it to the next message.
  const pickKit = async (id: string) => {
    setKitError(null);
    setKitBusy(true);
    try {
      setAttached(await api.setProjectBrand(projectPath, id));
      box.current?.focus();
    } catch (cause) {
      setKitError(errorMessage(cause));
    } finally {
      setKitBusy(false);
    }
  };

  const restyle = async () => {
    if (!attached) return;
    setKitBusy(true);
    const brief = await kitBrief(attached);
    setKitBusy(false);
    if (brief === null) return;
    setAttached(null);
    dispatch(restyleBrief(brief));
  };

  // The next queued request goes as soon as the assistant is free.
  const { send } = chat;
  const latestContext = useRef(context);
  latestContext.current = context;
  useEffect(() => {
    if (state.busy || !state.loaded || queued.length === 0 || state.approvals.length) return;
    const [next, ...rest] = queued;
    setQueued(rest);
    void send(next!, latestContext.current);
  }, [state.busy, state.loaded, state.approvals.length, queued, send]);

  const onKey = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void submit();
    }
  };

  // The level is shown in the header; clicking it opens the chooser.
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

      {queued.length ? (
        <ul className="queued" aria-label="Waiting to be sent">
          {queued.map((item, index) => (
            <li key={index} className="queued__item">
              <span className="queued__label">Next</span>
              <span className="queued__text" title={item}>
                {item}
              </span>
              <IconButton icon="close" label="Do not send this" onClick={() => setQueued((before) => before.filter((_, at) => at !== index))} />
            </li>
          ))}
        </ul>
      ) : null}
      <form
        className="composer"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        {attached ? (
          <div className="composer__kit" role="status">
            <Icon name="brand" size={14} />
            <span className="composer__kit-text">
              <strong>{attached.name}</strong> kit attached
            </span>
            <Button variant="ghost" busy={kitBusy} title="Restyle this whole video to the kit now" onClick={() => void restyle()}>
              Restyle
            </Button>
            <IconButton icon="close" label="Do not send the kit" onClick={() => setAttached(null)} />
          </div>
        ) : null}
        {kitError ? <Notice message={kitError} onDismiss={() => setKitError(null)} /> : null}
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
          {state.busy && !draft.trim() ? (
            <IconButton className="composer__action" icon="stop" label="Stop the assistant" onClick={chat.stop} />
          ) : (
            <IconButton
              className="composer__action composer__action--send"
              icon="send"
              label={state.busy ? "Send when this turn ends (Return)" : "Send (Return)"}
              disabled={!draft.trim() || choosing || kitBusy}
              onClick={() => void submit()}
            />
          )}
        </div>
        <ComposerTools
          disabled={choosing || kitBusy}
          kits={kits.status === "ready" ? kits.kits : []}
          onKit={(id) => void pickKit(id)}
          onSkill={(name) => {
            setDraft((before) => `Use the ${name} skill. ${before}`.trimEnd() + (before ? "" : " "));
            box.current?.focus();
          }}
        />
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
    const verb = toolVerb(message.tool);
    const failed = message.ok === false ? " (did not finish)" : "";
    return (
      <p className={`tool-row ${message.ok === false ? "tool-row--failed" : ""}`}>
        {waiting && busy ? <Spinner /> : <Icon name={message.ok === false ? "missing" : "check"} size={13} />}
        {verb ? (
          <>
            <span className="tool-row__verb">
              {verb}
              {failed}
            </span>
            <span className="tool-row__detail" title={message.text}>
              {message.text}
            </span>
          </>
        ) : (
          // A step described in its own words, like "Checked the reel for errors".
          <span className="tool-row__step">
            {message.text || "Working in the project"}
            {failed}
          </span>
        )}
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
        Allow {request.tool === "Bash" ? "this command" : request.tool}?
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
