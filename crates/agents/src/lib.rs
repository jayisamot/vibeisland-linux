//! Agent adapters for VibeIsland Linux.
//!
//! The [`Agent`] trait is the contract every supported AI coding agent
//! implements (Claude Code, Codex, Gemini CLI, Cursor, ...). The session
//! types model the state of a running agent as observed from hooks +
//! file watchers.
//!
//! Concrete adapters live in child modules (e.g. `claude_code` — added by
//! issue #9). This crate only defines the trait and shared data types.

#![forbid(unsafe_code)]

pub mod claude_code;
pub mod event;
pub mod response;
pub mod session_store;
pub mod watcher;

pub use claude_code::ClaudeCodeAgent;
pub use event::HookPayload;
pub use response::{HookDecision, DEFAULT_TIMEOUT as HOOK_DEFAULT_TIMEOUT};
pub use session_store::{SessionDelta, SessionStore};
pub use watcher::EventWatcher;

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// High-level state of an agent session as presented in the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Agent is present but not doing anything.
    Idle,
    /// Agent is processing a prompt (thinking / tool calls in flight).
    Thinking,
    /// Agent asked to run a tool and is waiting for approve/deny.
    AwaitingApproval,
    /// Agent called `AskUserQuestion` and is waiting for an answer.
    AwaitingQuestion,
    /// Session terminated (will be GC'd after a grace period).
    Closed,
}

/// Terminal information captured at hook time (best-effort; all fields
/// are optional because detection on Linux is not always reliable —
/// see issue #26).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// E.g. `"kitty"`, `"konsole"`, `"gnome-terminal"`, `"alacritty"`.
    pub emulator: Option<String>,
    /// X11 window ID (hex string) when known.
    pub window_id: Option<String>,
    /// Terminal-specific tab id (Kitty / Konsole) when known.
    pub tab_id: Option<u32>,
    /// PID of the terminal emulator process when known.
    pub pid: Option<u32>,
}

/// A single option offered by `AskUserQuestion`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// What the agent is blocked on — the payload rendered by the UI when
/// the session is `AwaitingApproval` or `AwaitingQuestion`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PendingAction {
    /// Permission to execute a tool (Bash, Edit, Write, ...).
    ToolPermission {
        /// Unique id for this pending action (used by approve/deny).
        id: String,
        /// Tool name (e.g. `"Bash"`, `"Edit"`).
        tool: String,
        /// Raw tool arguments as JSON.
        args: serde_json::Value,
    },
    /// Multiple-choice question from `AskUserQuestion`.
    Question {
        id: String,
        question: String,
        options: Vec<QuestionOption>,
    },
}

impl PendingAction {
    pub fn id(&self) -> &str {
        match self {
            Self::ToolPermission { id, .. } | Self::Question { id, .. } => id,
        }
    }
}

/// A single supervised agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    /// Globally unique session id (UUID).
    pub id: String,
    /// Stable id of the agent impl (matches [`Agent::id`]).
    pub agent_id: String,
    /// Working directory where the agent runs.
    pub cwd: String,
    /// Best-effort terminal metadata.
    #[serde(default)]
    pub terminal: TerminalInfo,
    /// Current state.
    pub state: SessionState,
    /// What the UI should render as an action prompt.
    #[serde(default)]
    pub pending_action: Option<PendingAction>,
    /// Last time we observed activity in this session.
    pub last_activity: DateTime<Utc>,
}

impl AgentSession {
    pub fn new(agent_id: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            cwd: cwd.into(),
            terminal: TerminalInfo::default(),
            state: SessionState::Idle,
            pending_action: None,
            last_activity: Utc::now(),
        }
    }
}

/// Events the adapter can emit as the underlying CLI runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    Stop,
    Notification,
}

/// Errors every adapter must be able to raise.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("hook already installed for event `{0:?}` by another tool")]
    HookConflict(EventKind),

    #[error("config file not found: {0}")]
    ConfigNotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type AgentResult<T> = Result<T, AgentError>;

/// Contract every agent adapter implements.
///
/// Adapters are object-safe — the backend holds them as
/// `Vec<Box<dyn Agent>>` and routes approve/deny/answer calls through
/// the trait.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Stable identifier (e.g. `"claude-code"`).
    fn id(&self) -> &'static str;

    /// Human-readable name shown in the UI.
    fn name(&self) -> &'static str;

    /// Install hooks into the agent's user-global config.
    async fn install(&self) -> AgentResult<()>;

    /// Remove hooks previously written by [`install`]. Never touch
    /// hooks created by the user.
    async fn uninstall(&self) -> AgentResult<()>;

    /// True when hooks are already in place.
    async fn is_installed(&self) -> bool;

    /// Events this adapter can produce (diagnostics / capability probe).
    fn supported_events(&self) -> Vec<EventKind>;

    /// Approve a pending tool call. Called by the backend after user
    /// clicks approve in the UI.
    async fn approve(&self, session_id: &str, action_id: &str) -> AgentResult<()>;

    /// Deny a pending tool call.
    async fn deny(&self, session_id: &str, action_id: &str) -> AgentResult<()>;

    /// Provide an answer to a pending `AskUserQuestion`.
    async fn answer(&self, session_id: &str, question_id: &str, answer: &str) -> AgentResult<()>;
}

/// Registry of available adapters — populated by the app at startup.
#[derive(Default)]
pub struct AgentRegistry {
    agents: BTreeMap<&'static str, Box<dyn Agent>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, agent: Box<dyn Agent>) {
        self.agents.insert(agent.id(), agent);
    }

    pub fn get(&self, id: &str) -> Option<&dyn Agent> {
        self.agents.get(id).map(|b| b.as_ref())
    }

    pub fn ids(&self) -> impl Iterator<Item = &&'static str> {
        self.agents.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Object-safety smoke test: if this compiles, `dyn Agent` works.
    fn _accepts_trait_object(_: &dyn Agent) {}

    #[test]
    fn session_state_roundtrip() {
        let s = SessionState::AwaitingApproval;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"awaiting_approval\"");
        let back: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn session_serde_roundtrip() {
        let mut session = AgentSession::new("claude-code", "/home/jay/code");
        session.state = SessionState::AwaitingApproval;
        session.pending_action = Some(PendingAction::ToolPermission {
            id: "act-1".into(),
            tool: "Bash".into(),
            args: serde_json::json!({"command": "ls"}),
        });
        let json = serde_json::to_string(&session).unwrap();
        let back: AgentSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session, back);
    }

    #[test]
    fn pending_action_id_accessor() {
        let q = PendingAction::Question {
            id: "q-1".into(),
            question: "?".into(),
            options: vec![],
        };
        assert_eq!(q.id(), "q-1");
    }

    #[test]
    fn registry_registers_and_retrieves() {
        struct Fake;
        #[async_trait]
        impl Agent for Fake {
            fn id(&self) -> &'static str {
                "fake"
            }
            fn name(&self) -> &'static str {
                "Fake"
            }
            async fn install(&self) -> AgentResult<()> {
                Ok(())
            }
            async fn uninstall(&self) -> AgentResult<()> {
                Ok(())
            }
            async fn is_installed(&self) -> bool {
                false
            }
            fn supported_events(&self) -> Vec<EventKind> {
                vec![EventKind::PreToolUse]
            }
            async fn approve(&self, _: &str, _: &str) -> AgentResult<()> {
                Ok(())
            }
            async fn deny(&self, _: &str, _: &str) -> AgentResult<()> {
                Ok(())
            }
            async fn answer(&self, _: &str, _: &str, _: &str) -> AgentResult<()> {
                Ok(())
            }
        }
        let mut reg = AgentRegistry::new();
        reg.register(Box::new(Fake));
        assert_eq!(reg.get("fake").unwrap().name(), "Fake");
        assert!(reg.ids().any(|id| *id == "fake"));
    }
}
