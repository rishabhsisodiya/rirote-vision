import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useReducer } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { AgentEvent, ApprovalAnswer, EditorContext, TrustLevel } from "../lib/types";
import { chatReducer, EMPTY_CHAT } from "./chatState";

/** One project's conversation with the assistant. */
export function useChat(projectPath: string) {
  const [state, dispatch] = useReducer(chatReducer, EMPTY_CHAT);

  useEffect(() => {
    let current = true;
    api
      .loadChat(projectPath)
      .then((view) => current && dispatch({ type: "loaded", view }))
      .catch((cause) => current && dispatch({ type: "sendFailed", message: errorMessage(cause) }));

    const events = listen<{ projectPath: string; event: AgentEvent }>("agent", ({ payload }) => {
      if (payload.projectPath === projectPath) dispatch({ type: "event", event: payload.event });
    });
    return () => {
      current = false;
      void events.then((unlisten) => unlisten());
    };
  }, [projectPath]);

  const send = useCallback(
    async (prompt: string, context: EditorContext) => {
      dispatch({ type: "sent", prompt });
      try {
        await api.sendChat(projectPath, prompt, context);
      } catch (cause) {
        dispatch({ type: "sendFailed", message: errorMessage(cause) });
      }
    },
    [projectPath],
  );

  const answer = useCallback(async (requestId: string, choice: ApprovalAnswer) => {
    dispatch({ type: "answered", requestId, always: choice === "always" });
    try {
      await api.answerChat(requestId, choice);
    } catch (cause) {
      dispatch({ type: "sendFailed", message: errorMessage(cause) });
    }
  }, []);

  const stop = useCallback(() => void api.stopChat().catch(() => {}), []);

  const reset = useCallback(
    async (what: "conversation" | "permissions") => {
      try {
        const view = await (what === "conversation"
          ? api.clearChat(projectPath)
          : api.forgetChatRules(projectPath));
        dispatch({ type: "loaded", view });
      } catch (cause) {
        dispatch({ type: "sendFailed", message: errorMessage(cause) });
      }
    },
    [projectPath],
  );

  const setTrust = useCallback(
    async (level: TrustLevel) => {
      try {
        dispatch({ type: "loaded", view: await api.setChatTrust(projectPath, level) });
      } catch (cause) {
        dispatch({ type: "sendFailed", message: errorMessage(cause) });
      }
    },
    [projectPath],
  );

  return { state, send, answer, stop, reset, setTrust };
}

export type ChatControls = ReturnType<typeof useChat>;
