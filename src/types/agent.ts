/**
 * Shared types mirroring `crates/agents/src/lib.rs`.
 *
 * ⚠️ Keep in sync with the Rust structs. A future issue may switch to
 * `specta + tauri-specta` for auto-generation (see #19 discussion). For
 * now the surface is small enough to maintain by hand.
 */

export type SessionState =
  | "idle"
  | "thinking"
  | "awaiting_approval"
  | "awaiting_question"
  | "closed";

export interface TerminalInfo {
  emulator: string | null;
  window_id: string | null;
  tab_id: number | null;
  pid: number | null;
}

export interface QuestionOption {
  id: string;
  label: string;
  description: string | null;
}

export type PendingAction =
  | {
      type: "tool_permission";
      id: string;
      tool: string;
      args: unknown;
    }
  | {
      type: "question";
      id: string;
      question: string;
      options: QuestionOption[];
    };

export interface AgentSession {
  id: string;
  agent_id: string;
  cwd: string;
  terminal: TerminalInfo;
  state: SessionState;
  pending_action: PendingAction | null;
  last_activity: string; // ISO-8601
}

export type EventKind =
  | "pre_tool_use"
  | "post_tool_use"
  | "user_prompt_submit"
  | "stop"
  | "notification";

export interface AgentStatus {
  id: string;
  name: string;
  installed: boolean;
}

/** Helper to narrow a PendingAction. */
export function isApprovalAction(
  a: PendingAction | null | undefined,
): a is Extract<PendingAction, { type: "tool_permission" }> {
  return a?.type === "tool_permission";
}

export function isQuestionAction(
  a: PendingAction | null | undefined,
): a is Extract<PendingAction, { type: "question" }> {
  return a?.type === "question";
}
