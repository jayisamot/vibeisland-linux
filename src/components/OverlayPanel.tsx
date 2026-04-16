import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAgentState } from "@/hooks/useAgentState";
import { AgentCard } from "./AgentCard";

async function beginDrag(e: React.MouseEvent) {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest("button, a, input, textarea, select")) return;
  try {
    await getCurrentWindow().startDragging();
  } catch (err) {
    console.error("startDragging failed", err);
  }
}

export function OverlayPanel() {
  const { sessions, approve, deny, focusTerminal } = useAgentState();
  const awaitingCount = sessions.filter(
    (s) => s.state === "awaiting_approval" || s.state === "awaiting_question",
  ).length;

  return (
    <div className="h-screen w-screen p-2">
      <div className="h-full w-full bg-neutral-900/95 backdrop-blur rounded-2xl shadow-2xl ring-1 ring-white/5 flex flex-col overflow-hidden text-neutral-100">
        <header
          onMouseDown={beginDrag}
          className="h-10 px-4 flex items-center justify-between select-none cursor-grab active:cursor-grabbing border-b border-white/5"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">VibeIsland</span>
            {awaitingCount > 0 && (
              <span className="text-[10px] bg-amber-500 text-black rounded-full h-4 min-w-4 px-1 flex items-center justify-center font-semibold">
                {awaitingCount}
              </span>
            )}
          </div>
          <span className="text-[10px] uppercase tracking-wide text-neutral-500">
            {sessions.length} session{sessions.length === 1 ? "" : "s"}
          </span>
        </header>

        <main className="flex-1 overflow-y-auto p-2 space-y-2">
          {sessions.length === 0 ? (
            <EmptyState />
          ) : (
            sessions.map((s) => (
              <AgentCard
                key={s.id}
                session={s}
                onApprove={async (sid, aid) => {
                  await approve(sid, aid);
                }}
                onDeny={async (sid, aid) => {
                  await deny(sid, aid);
                }}
                onFocusTerminal={async (sid) => {
                  await focusTerminal(sid);
                }}
              />
            ))
          )}
        </main>

        <footer className="h-7 px-4 flex items-center justify-between text-[10px] text-neutral-600 border-t border-white/5">
          <span>dev/v0.1</span>
          <span>VibeIsland Linux</span>
        </footer>
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="h-full flex flex-col items-center justify-center gap-3 text-center px-6 py-10">
      <div className="w-10 h-10 rounded-full bg-white/5 flex items-center justify-center text-lg">
        ⌁
      </div>
      <div className="text-sm text-neutral-300">No active agents</div>
      <div className="text-[11px] text-neutral-500 leading-relaxed max-w-[260px]">
        Run <code className="font-mono bg-white/5 px-1 rounded">claude</code> in a terminal — hooks
        will auto-install once the Claude Code adapter lands (issue&nbsp;#9).
      </div>
    </div>
  );
}
