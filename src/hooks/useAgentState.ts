import { useCallback, useEffect, useMemo, useReducer } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentSession } from "@/types/agent";

/**
 * React hook subscribing to the backend session stream.
 *
 * - On mount, fetches the initial state via `invoke('list_sessions')`.
 * - Listens to `session:new` / `session:updated` / `session:closed`
 *   events emitted by `ipc.rs::spawn_delta_bridge`.
 * - Exposes memoized `approve`, `deny`, `answer` wrappers.
 */
export function useAgentState() {
  const [state, dispatch] = useReducer(sessionReducer, { byId: {} });

  useEffect(() => {
    let mounted = true;
    const unlisteners: UnlistenFn[] = [];

    (async () => {
      try {
        const initial = await invoke<AgentSession[]>("list_sessions");
        if (!mounted) return;
        dispatch({ kind: "hydrate", sessions: initial });
      } catch (err) {
        console.error("list_sessions failed", err);
      }

      unlisteners.push(
        await listen<AgentSession>("session:new", (e) => {
          dispatch({ kind: "upsert", session: e.payload });
        }),
      );
      unlisteners.push(
        await listen<AgentSession>("session:updated", (e) => {
          dispatch({ kind: "upsert", session: e.payload });
        }),
      );
      unlisteners.push(
        await listen<{ id: string }>("session:closed", (e) => {
          dispatch({ kind: "remove", id: e.payload.id });
        }),
      );
    })();

    return () => {
      mounted = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  const sessions = useMemo(() => {
    const arr = Object.values(state.byId);
    arr.sort((a, b) => b.last_activity.localeCompare(a.last_activity));
    return arr;
  }, [state]);

  const approve = useCallback(
    (session_id: string, action_id: string) =>
      invoke<void>("approve", { sessionId: session_id, actionId: action_id }),
    [],
  );
  const deny = useCallback(
    (session_id: string, action_id: string) =>
      invoke<void>("deny", { sessionId: session_id, actionId: action_id }),
    [],
  );
  const answer = useCallback(
    (session_id: string, question_id: string, answer: string) =>
      invoke<void>("answer_question", {
        sessionId: session_id,
        questionId: question_id,
        answer,
      }),
    [],
  );
  const focusTerminal = useCallback(
    (session_id: string) => invoke<void>("focus_terminal", { sessionId: session_id }),
    [],
  );

  return { sessions, approve, deny, answer, focusTerminal };
}

// ---------- reducer ----------

interface State {
  byId: Record<string, AgentSession>;
}

type Action =
  | { kind: "hydrate"; sessions: AgentSession[] }
  | { kind: "upsert"; session: AgentSession }
  | { kind: "remove"; id: string };

function sessionReducer(state: State, action: Action): State {
  switch (action.kind) {
    case "hydrate": {
      const byId: Record<string, AgentSession> = {};
      for (const s of action.sessions) byId[s.id] = s;
      return { byId };
    }
    case "upsert":
      return { byId: { ...state.byId, [action.session.id]: action.session } };
    case "remove": {
      if (!(action.id in state.byId)) return state;
      const { [action.id]: _removed, ...rest } = state.byId;
      return { byId: rest };
    }
  }
}
