//! Shape of an event file written by `vibeisland-linux hook <event>`
//! (issue #10) and consumed by [`crate::session_store::SessionStore`]
//! (issue #12).
//!
//! Keeping the schema here (not inside `src-tauri`) means the hook
//! binary and the watcher agree on exactly one type — serde drift is
//! impossible.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// JSON payload persisted under `~/.vibeisland/events/<ts>-<id>.json`.
///
/// Every field except `event` is optional — the hook records whatever
/// the underlying agent happens to provide. The store treats unknown
/// or missing fields as soft failures (log & continue).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    /// Hook event name (kebab-case: `pre-tool-use`, `post-tool-use`,
    /// `user-prompt-submit`, `stop`, `notification`).
    pub event: String,

    /// Unique id for this event (uuid v4).
    pub id: String,

    /// ISO-8601 UTC timestamp.
    pub timestamp: String,

    /// Parsed JSON body piped to the hook on stdin.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,

    /// Verbatim stdin text when it did not parse as JSON.
    #[serde(default)]
    pub raw_payload: Option<String>,

    /// Agent-side session id (`CLAUDE_SESSION_ID`, etc.) when present.
    #[serde(default)]
    pub session_id: Option<String>,

    /// Working directory of the hook process when known.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Hook process PID.
    pub pid: u32,

    /// Curated env snapshot captured for terminal detection.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl HookPayload {
    /// Best-effort agent id inferred from the hook event shape.
    /// Until more agents are wired up (phase 4+) every hook is treated
    /// as Claude Code.
    pub fn agent_id(&self) -> &str {
        "claude-code"
    }

    /// Stable session key: prefer the agent-side session id, fall back
    /// to cwd so two terminals in the same folder share a row.
    pub fn session_key(&self) -> String {
        self.session_id.clone().unwrap_or_else(|| {
            self.cwd
                .clone()
                .unwrap_or_else(|| format!("pid:{}", self.pid))
        })
    }

    /// Tool name when the payload is from a `PreToolUse` / `PostToolUse`.
    pub fn tool_name(&self) -> Option<&str> {
        self.payload
            .as_ref()?
            .get("tool_name")
            .or_else(|| self.payload.as_ref()?.get("tool"))
            .and_then(|v| v.as_str())
    }

    /// Tool args when the payload is from a `PreToolUse` / `PostToolUse`.
    pub fn tool_args(&self) -> Option<&serde_json::Value> {
        self.payload
            .as_ref()?
            .get("tool_input")
            .or_else(|| self.payload.as_ref()?.get("args"))
    }
}
