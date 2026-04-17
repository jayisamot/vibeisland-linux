import { useState } from "react";
import { useAgentState } from "@/hooks/useAgentState";
import type { AgentStatus } from "@/types/agent";
import { AgentCard } from "./AgentCard";

export function OverlayPanel() {
  const { sessions, agents, approve, deny, focusTerminal, installAgent, uninstallAgent } =
    useAgentState();

  const awaitingCount = sessions.filter(
    (s) => s.state === "awaiting_approval" || s.state === "awaiting_question",
  ).length;
  const anyInstalled = agents.some((a) => a.installed);

  return (
    <div className="h-screen w-screen p-2">
      <div className="h-full w-full bg-neutral-900/95 backdrop-blur rounded-2xl shadow-2xl ring-1 ring-white/5 flex flex-col overflow-hidden text-neutral-100">
        <header
          data-tauri-drag-region
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
            <EmptyState
              agents={agents}
              anyInstalled={anyInstalled}
              onInstall={installAgent}
              onUninstall={uninstallAgent}
            />
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
          <ConnectionDot agents={agents} />
          <span>v0.1</span>
        </footer>
      </div>
    </div>
  );
}

function EmptyState({
  agents,
  anyInstalled,
  onInstall,
  onUninstall,
}: {
  agents: AgentStatus[];
  anyInstalled: boolean;
  onInstall: (id: string) => Promise<void>;
  onUninstall: (id: string) => Promise<void>;
}) {
  if (agents.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2 text-center px-6 py-10">
        <div className="text-sm text-neutral-300">No agents registered</div>
        <div className="text-[11px] text-neutral-500">Backend hasn't finished starting.</div>
      </div>
    );
  }

  if (anyInstalled) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-3 text-center px-6 py-10">
        <div className="w-10 h-10 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-lg">
          ⌁
        </div>
        <div className="text-sm text-neutral-300">Waiting for activity</div>
        <div className="text-[11px] text-neutral-500 leading-relaxed max-w-[260px]">
          Run <code className="font-mono bg-white/5 px-1 rounded">claude</code> in a terminal — the
          first tool call will land here.
        </div>
        <AgentList agents={agents} onInstall={onInstall} onUninstall={onUninstall} />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col items-center justify-center gap-4 text-center px-6 py-8">
      <div className="w-10 h-10 rounded-full bg-white/5 flex items-center justify-center text-lg">
        ⌁
      </div>
      <div className="space-y-1">
        <div className="text-sm text-neutral-200 font-medium">Connect your first agent</div>
        <div className="text-[11px] text-neutral-500 leading-relaxed max-w-[280px]">
          VibeIsland adds hooks to your agent's config so every tool call routes through this
          overlay. Your existing config is backed up.
        </div>
      </div>
      <AgentList agents={agents} onInstall={onInstall} onUninstall={onUninstall} prominent />
    </div>
  );
}

function AgentList({
  agents,
  onInstall,
  onUninstall,
  prominent = false,
}: {
  agents: AgentStatus[];
  onInstall: (id: string) => Promise<void>;
  onUninstall: (id: string) => Promise<void>;
  prominent?: boolean;
}) {
  return (
    <div className="w-full max-w-[280px] space-y-1">
      {agents.map((a) => (
        <AgentRow
          key={a.id}
          agent={a}
          onInstall={onInstall}
          onUninstall={onUninstall}
          prominent={prominent}
        />
      ))}
    </div>
  );
}

function AgentRow({
  agent,
  onInstall,
  onUninstall,
  prominent,
}: {
  agent: AgentStatus;
  onInstall: (id: string) => Promise<void>;
  onUninstall: (id: string) => Promise<void>;
  prominent: boolean;
}) {
  const [busy, setBusy] = useState<null | "install" | "uninstall">(null);
  const [error, setError] = useState<string | null>(null);

  async function run(action: "install" | "uninstall") {
    setBusy(action);
    setError(null);
    try {
      await (action === "install" ? onInstall(agent.id) : onUninstall(agent.id));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  if (!agent.installed && prominent) {
    return (
      <button
        onClick={() => run("install")}
        disabled={busy !== null}
        className="w-full h-10 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-sm font-medium transition-colors"
      >
        {busy === "install" ? "Installing..." : `Connect ${agent.name}`}
      </button>
    );
  }

  return (
    <div className="flex items-center justify-between gap-2 text-[11px] px-2 py-1.5 rounded-md bg-white/5">
      <div className="flex items-center gap-2 min-w-0">
        <span
          className={`w-1.5 h-1.5 rounded-full ${agent.installed ? "bg-emerald-500" : "bg-neutral-500"}`}
        />
        <span className="truncate">{agent.name}</span>
      </div>
      <button
        onClick={() => run(agent.installed ? "uninstall" : "install")}
        disabled={busy !== null}
        className="text-[10px] uppercase tracking-wide text-neutral-400 hover:text-neutral-200 disabled:opacity-50"
      >
        {busy ?? (agent.installed ? "disconnect" : "connect")}
      </button>
      {error && <span className="text-red-400 text-[10px] truncate">{error}</span>}
    </div>
  );
}

function ConnectionDot({ agents }: { agents: AgentStatus[] }) {
  const installed = agents.filter((a) => a.installed);
  if (agents.length === 0) {
    return <span className="text-neutral-600">connecting…</span>;
  }
  if (installed.length === 0) {
    return (
      <span className="flex items-center gap-1.5">
        <span className="w-1.5 h-1.5 rounded-full bg-neutral-500" />
        not connected
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5">
      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
      {installed.map((a) => a.name).join(", ")}
    </span>
  );
}
