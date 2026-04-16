import { useState } from "react";
import type { PendingAction } from "@/types/agent";

interface Props {
  action: Extract<PendingAction, { type: "tool_permission" }>;
  onApprove: () => Promise<void>;
  onDeny: () => Promise<void>;
}

export function ApprovalPrompt({ action, onApprove, onDeny }: Props) {
  const [pending, setPending] = useState<"approve" | "deny" | null>(null);
  const disabled = pending !== null;

  async function run(kind: "approve" | "deny") {
    if (disabled) return;
    setPending(kind);
    try {
      await (kind === "approve" ? onApprove() : onDeny());
    } finally {
      setPending(null);
    }
  }

  return (
    <div
      className="border border-white/5 rounded-lg bg-black/30 p-3 space-y-2"
      onKeyDown={(e) => {
        if (e.key === "a") run("approve");
        if (e.key === "d") run("deny");
      }}
      tabIndex={0}
    >
      <div className="text-[11px] uppercase tracking-wide text-amber-400">
        Approval needed · {action.tool}
      </div>
      <ArgsPreview tool={action.tool} args={action.args} />
      <div className="flex gap-2 pt-1">
        <button
          onClick={() => run("approve")}
          disabled={disabled}
          className="flex-1 h-8 rounded-md bg-emerald-600 hover:bg-emerald-500 text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="Approve (a)"
        >
          {pending === "approve" ? "..." : "Approve"}
          <span className="ml-2 text-[10px] opacity-60">a</span>
        </button>
        <button
          onClick={() => run("deny")}
          disabled={disabled}
          className="flex-1 h-8 rounded-md bg-red-600 hover:bg-red-500 text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="Deny (d)"
        >
          {pending === "deny" ? "..." : "Deny"}
          <span className="ml-2 text-[10px] opacity-60">d</span>
        </button>
      </div>
    </div>
  );
}

function ArgsPreview({ tool, args }: { tool: string; args: unknown }) {
  if (tool === "Bash" && isRecord(args) && typeof args.command === "string") {
    return (
      <pre className="text-[11px] bg-neutral-950 p-2 rounded border border-white/5 overflow-x-auto whitespace-pre-wrap">
        $ {args.command}
      </pre>
    );
  }
  if ((tool === "Edit" || tool === "Write") && isRecord(args)) {
    const path = typeof args.file_path === "string" ? args.file_path : undefined;
    return (
      <div className="text-[11px] space-y-1">
        {path && <div className="text-neutral-400 font-mono truncate">{path}</div>}
        <pre className="bg-neutral-950 p-2 rounded border border-white/5 overflow-x-auto text-[10px] max-h-32">
          {stringify(args)}
        </pre>
      </div>
    );
  }
  if (tool === "Read" && isRecord(args) && typeof args.file_path === "string") {
    return <div className="text-[11px] font-mono text-neutral-400 truncate">{args.file_path}</div>;
  }
  return (
    <pre className="text-[11px] bg-neutral-950 p-2 rounded border border-white/5 overflow-x-auto max-h-32">
      {stringify(args)}
    </pre>
  );
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function stringify(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}
