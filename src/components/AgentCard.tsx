import type { AgentSession, SessionState } from "@/types/agent";
import { isApprovalAction } from "@/types/agent";
import { ApprovalPrompt } from "./ApprovalPrompt";

interface Props {
  session: AgentSession;
  onApprove: (sessionId: string, actionId: string) => Promise<void>;
  onDeny: (sessionId: string, actionId: string) => Promise<void>;
  onFocusTerminal: (sessionId: string) => Promise<void>;
}

export function AgentCard({ session, onApprove, onDeny, onFocusTerminal }: Props) {
  const terminalKnown = !!session.terminal.emulator;
  return (
    <article className="rounded-xl bg-white/5 border border-white/5 p-3 space-y-2">
      <header className="flex items-center justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold tracking-wide">
              {agentLabel(session.agent_id)}
            </span>
            <StateBadge state={session.state} />
          </div>
          <div className="text-[11px] text-neutral-400 font-mono truncate" title={session.cwd}>
            {session.cwd || "~"}
          </div>
        </div>
        <button
          className="shrink-0 h-7 px-2 rounded-md text-[11px] bg-white/5 hover:bg-white/10 disabled:opacity-30 disabled:hover:bg-white/5"
          onClick={() => onFocusTerminal(session.id)}
          disabled={!terminalKnown}
          title={terminalKnown ? `Focus ${session.terminal.emulator}` : "Terminal not detected"}
        >
          ↗ terminal
        </button>
      </header>

      {isApprovalAction(session.pending_action) && (
        <ApprovalPrompt
          action={session.pending_action}
          onApprove={() => onApprove(session.id, session.pending_action!.id)}
          onDeny={() => onDeny(session.id, session.pending_action!.id)}
        />
      )}

      {session.pending_action?.type === "question" && (
        <div className="border border-violet-500/30 bg-violet-500/10 rounded-md p-2 text-[11px] text-violet-200">
          Question pending ({session.pending_action.options.length} options) — UI lands in issue #29
        </div>
      )}
    </article>
  );
}

function agentLabel(id: string): string {
  switch (id) {
    case "claude-code":
      return "Claude Code";
    case "codex":
      return "Codex";
    case "gemini-cli":
      return "Gemini CLI";
    case "cursor":
      return "Cursor";
    default:
      return id;
  }
}

function StateBadge({ state }: { state: SessionState }) {
  const cfg = STATE_STYLES[state];
  return (
    <span
      className={`text-[9px] uppercase tracking-wider px-1.5 py-0.5 rounded ${cfg.bg} ${cfg.fg}`}
    >
      {cfg.label}
    </span>
  );
}

const STATE_STYLES: Record<SessionState, { label: string; bg: string; fg: string }> = {
  idle: { label: "idle", bg: "bg-neutral-700/50", fg: "text-neutral-300" },
  thinking: {
    label: "thinking",
    bg: "bg-blue-500/20 animate-pulse",
    fg: "text-blue-300",
  },
  awaiting_approval: {
    label: "approve?",
    bg: "bg-amber-500/20",
    fg: "text-amber-300",
  },
  awaiting_question: {
    label: "question",
    bg: "bg-violet-500/20",
    fg: "text-violet-300",
  },
  closed: { label: "closed", bg: "bg-neutral-800", fg: "text-neutral-500" },
};
