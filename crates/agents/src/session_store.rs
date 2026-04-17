//! In-memory session store persisted to `~/.vibeisland/sessions.json`.
//!
//! The store is the single source of truth for what the UI should
//! display. It consumes [`HookPayload`]s produced by the `hook`
//! subcommand (#10) via a file watcher (#11) and emits [`SessionDelta`]s
//! so callers can translate them into Tauri events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::event::HookPayload;
use crate::{AgentSession, PendingAction, QuestionOption, SessionState, TerminalInfo};

/// What changed when an event was applied — callers translate these
/// into Tauri events (`session:new`, `session:updated`, `session:closed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionDelta {
    New(AgentSession),
    Updated(AgentSession),
    Closed { id: String },
}

/// Sessions a closed entry is kept for, to allow the UI to animate the
/// close transition before we drop it.
const CLOSED_GRACE: Duration = Duration::from_secs(5 * 60);

/// Any non-closed session that hasn't had a hook event in this long is
/// presumed dead. Covers the common case where the user kills the
/// terminal mid-`Thinking` or -`Idle` — Claude Code never emits a
/// `stop` in that path, so the row would otherwise live forever.
const STALE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Default, Serialize, Deserialize)]
struct SnapshotV1 {
    schema_version: u32,
    sessions: HashMap<String, AgentSession>,
}

pub struct SessionStore {
    path: Option<PathBuf>,
    inner: Arc<RwLock<HashMap<String, AgentSession>>>,
}

impl SessionStore {
    /// In-memory store, not persisted. Useful for tests.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            inner: Arc::default(),
        }
    }

    /// Load from `sessions.json` (creating parent dirs + returning empty
    /// state if the file is missing or corrupt).
    pub async fn load(path: PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let sessions = match tokio::fs::read(&path).await {
            Ok(bytes) => match serde_json::from_slice::<SnapshotV1>(&bytes) {
                Ok(snap) if snap.schema_version == 1 => snap.sessions,
                Ok(_) | Err(_) => {
                    // Back up the corrupt file and start fresh.
                    let backup = path.with_extension("json.corrupted");
                    let _ = tokio::fs::rename(&path, &backup).await;
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            path: Some(path),
            inner: Arc::new(RwLock::new(sessions)),
        })
    }

    pub async fn list(&self) -> Vec<AgentSession> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<AgentSession> {
        self.inner.read().await.get(id).cloned()
    }

    pub async fn remove(&self, id: &str) -> bool {
        let removed = self.inner.write().await.remove(id).is_some();
        if removed {
            self.persist().await;
        }
        removed
    }

    /// Mark the pending action on `id` as resolved — state goes to
    /// [`SessionState::Thinking`] and `pending_action` is cleared. Used
    /// immediately after approve/deny/answer so the UI doesn't wait for
    /// the next hook event to refresh.
    pub async fn mark_resolved(&self, id: &str) -> Option<SessionDelta> {
        let delta = {
            let mut map = self.inner.write().await;
            let session = map.get_mut(id)?;
            session.state = SessionState::Thinking;
            session.pending_action = None;
            session.last_activity = Utc::now();
            SessionDelta::Updated(session.clone())
        };
        self.persist().await;
        Some(delta)
    }

    pub async fn apply_event(&self, payload: &HookPayload) -> Option<SessionDelta> {
        let key = payload.session_key();
        let delta = {
            let mut map = self.inner.write().await;
            let is_new = !map.contains_key(&key);
            let entry = map.entry(key.clone()).or_insert_with(|| {
                let mut s = AgentSession::new(
                    payload.agent_id().to_string(),
                    payload.cwd.clone().unwrap_or_default(),
                );
                s.id = key.clone();
                s
            });

            apply_transition(entry, payload);
            entry.terminal = merge_terminal(entry.terminal.clone(), payload);
            entry.last_activity = Utc::now();

            if is_new {
                SessionDelta::New(entry.clone())
            } else if matches!(entry.state, SessionState::Closed) {
                SessionDelta::Closed { id: key.clone() }
            } else {
                SessionDelta::Updated(entry.clone())
            }
        };
        self.persist().await;
        Some(delta)
    }

    /// Drop closed sessions older than `CLOSED_GRACE`. Returns the ids
    /// removed.
    pub async fn gc(&self) -> Vec<String> {
        let cutoff = Utc::now() - chrono::Duration::from_std(CLOSED_GRACE).unwrap();
        let mut removed = Vec::new();
        {
            let mut map = self.inner.write().await;
            map.retain(|id, s| {
                let stale = matches!(s.state, SessionState::Closed) && s.last_activity < cutoff;
                if stale {
                    removed.push(id.clone());
                }
                !stale
            });
        }
        if !removed.is_empty() {
            self.persist().await;
        }
        removed
    }

    /// Drop any session — regardless of state — whose `last_activity`
    /// is older than `STALE_TTL`. Intended to run once on overlay
    /// startup to clear ghost rows left behind by killed terminals.
    pub async fn prune_stale(&self) -> Vec<String> {
        let cutoff = Utc::now() - chrono::Duration::from_std(STALE_TTL).unwrap();
        let mut removed = Vec::new();
        {
            let mut map = self.inner.write().await;
            map.retain(|id, s| {
                let stale = s.last_activity < cutoff;
                if stale {
                    removed.push(id.clone());
                }
                !stale
            });
        }
        if !removed.is_empty() {
            self.persist().await;
        }
        removed
    }

    async fn persist(&self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let snapshot = SnapshotV1 {
            schema_version: 1,
            sessions: self.inner.read().await.clone(),
        };
        let bytes = match serde_json::to_vec_pretty(&snapshot) {
            Ok(b) => b,
            Err(_) => return,
        };
        let tmp = path.with_extension("json.tmp");
        if tokio::fs::write(&tmp, bytes).await.is_ok() {
            let _ = tokio::fs::rename(tmp, path).await;
        }
    }
}

fn apply_transition(session: &mut AgentSession, payload: &HookPayload) {
    match payload.event.as_str() {
        "pre-tool-use" => {
            let tool = payload.tool_name().unwrap_or("unknown").to_string();
            let args = payload
                .tool_args()
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if tool == "AskUserQuestion" {
                session.state = SessionState::AwaitingQuestion;
                session.pending_action = Some(parse_question(&payload.id, &args));
            } else {
                session.state = SessionState::AwaitingApproval;
                session.pending_action = Some(PendingAction::ToolPermission {
                    id: payload.id.clone(),
                    tool,
                    args,
                });
            }
        }
        "post-tool-use" => {
            session.state = SessionState::Thinking;
            session.pending_action = None;
        }
        "user-prompt-submit" => {
            session.state = SessionState::Thinking;
            session.pending_action = None;
        }
        "stop" => {
            session.state = SessionState::Idle;
            session.pending_action = None;
        }
        "notification" => {
            // Just bumps last_activity — no state change.
        }
        _ => {
            // Unknown event kind — leave state alone, log upstream.
        }
    }
}

fn parse_question(event_id: &str, args: &serde_json::Value) -> PendingAction {
    let question = args
        .get("question")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let options = args
        .get("options")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    let label = v.get("label").and_then(|x| x.as_str())?.to_string();
                    let description = v
                        .get("description")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    Some(QuestionOption {
                        id: format!("{event_id}-opt-{i}"),
                        label,
                        description,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    PendingAction::Question {
        id: event_id.to_string(),
        question,
        options,
    }
}

fn merge_terminal(mut current: TerminalInfo, payload: &HookPayload) -> TerminalInfo {
    let env = &payload.env;
    if current.emulator.is_none() {
        current.emulator = detect_emulator(env);
    }
    if current.window_id.is_none() {
        current.window_id = env
            .get("KITTY_WINDOW_ID")
            .or_else(|| env.get("WINDOWID"))
            .or_else(|| env.get("ALACRITTY_WINDOW_ID"))
            .cloned();
    }
    if current.pid.is_none() {
        current.pid = env.get("PPID").and_then(|p| p.parse().ok());
    }
    current
}

fn detect_emulator(env: &std::collections::BTreeMap<String, String>) -> Option<String> {
    if env.contains_key("KITTY_PID") || env.contains_key("KITTY_WINDOW_ID") {
        return Some("kitty".into());
    }
    if env.contains_key("KONSOLE_VERSION") {
        return Some("konsole".into());
    }
    if env.contains_key("GNOME_TERMINAL_SCREEN") {
        return Some("gnome-terminal".into());
    }
    if env.contains_key("ALACRITTY_WINDOW_ID") {
        return Some("alacritty".into());
    }
    env.get("TERM_PROGRAM").cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn mk_payload(event: &str, session_id: Option<&str>, tool: Option<&str>) -> HookPayload {
        let payload_json = tool.map(|t| {
            serde_json::json!({
                "tool_name": t,
                "tool_input": { "command": "ls" }
            })
        });
        HookPayload {
            event: event.to_string(),
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            payload: payload_json,
            raw_payload: None,
            session_id: session_id.map(String::from),
            cwd: Some("/home/jay/code".into()),
            pid: 12345,
            env: {
                let mut m = BTreeMap::new();
                m.insert("KITTY_WINDOW_ID".into(), "1".into());
                m.insert("KITTY_PID".into(), "4321".into());
                m.insert("TERM_PROGRAM".into(), "kitty".into());
                m
            },
        }
    }

    #[tokio::test]
    async fn pre_tool_use_creates_session_awaiting_approval() {
        let store = SessionStore::in_memory();
        let delta = store
            .apply_event(&mk_payload(
                "pre-tool-use",
                Some("claude-xyz"),
                Some("Bash"),
            ))
            .await
            .unwrap();
        assert!(
            matches!(delta, SessionDelta::New(ref s) if s.state == SessionState::AwaitingApproval)
        );
        let sessions = store.list().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].terminal.emulator.as_deref(), Some("kitty"));
    }

    #[tokio::test]
    async fn ask_user_question_maps_to_awaiting_question() {
        let store = SessionStore::in_memory();
        let mut p = mk_payload("pre-tool-use", Some("s1"), Some("AskUserQuestion"));
        p.payload = Some(serde_json::json!({
            "tool_name": "AskUserQuestion",
            "tool_input": {
                "question": "Pick one",
                "options": [
                    {"label": "A", "description": "choice A"},
                    {"label": "B"}
                ]
            }
        }));
        store.apply_event(&p).await.unwrap();
        let s = store.get("s1").await.unwrap();
        assert_eq!(s.state, SessionState::AwaitingQuestion);
        match s.pending_action.unwrap() {
            PendingAction::Question {
                question, options, ..
            } => {
                assert_eq!(question, "Pick one");
                assert_eq!(options.len(), 2);
                assert_eq!(options[0].label, "A");
                assert_eq!(options[0].description.as_deref(), Some("choice A"));
                assert_eq!(options[1].description, None);
            }
            _ => panic!("expected Question"),
        }
    }

    #[tokio::test]
    async fn stop_event_returns_to_idle() {
        let store = SessionStore::in_memory();
        store
            .apply_event(&mk_payload("pre-tool-use", Some("s1"), Some("Bash")))
            .await;
        let delta = store
            .apply_event(&mk_payload("stop", Some("s1"), None))
            .await
            .unwrap();
        assert!(matches!(delta, SessionDelta::Updated(ref s) if s.state == SessionState::Idle));
        let s = store.get("s1").await.unwrap();
        assert_eq!(s.state, SessionState::Idle);
        assert!(s.pending_action.is_none());
    }

    #[tokio::test]
    async fn session_key_falls_back_to_cwd_when_no_session_id() {
        let store = SessionStore::in_memory();
        store
            .apply_event(&mk_payload("pre-tool-use", None, Some("Bash")))
            .await;
        let sessions = store.list().await;
        assert_eq!(sessions.len(), 1);
        // key = cwd
        assert_eq!(sessions[0].id, "/home/jay/code");
    }

    #[tokio::test]
    async fn session_key_reads_session_id_from_payload_body() {
        // Claude Code puts its session UUID inside the JSON it pipes to
        // the hook on stdin, not in an env var. Two terminals in the
        // same cwd must therefore be kept distinct by that field.
        let store = SessionStore::in_memory();
        let mut p1 = mk_payload("pre-tool-use", None, Some("Bash"));
        p1.payload = Some(serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls" },
            "session_id": "uuid-tab-1",
        }));
        let mut p2 = mk_payload("pre-tool-use", None, Some("Bash"));
        p2.payload = Some(serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls" },
            "session_id": "uuid-tab-2",
        }));

        store.apply_event(&p1).await;
        store.apply_event(&p2).await;

        let sessions = store.list().await;
        assert_eq!(sessions.len(), 2, "two distinct sessions expected");
        let ids: std::collections::BTreeSet<_> = sessions.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("uuid-tab-1"));
        assert!(ids.contains("uuid-tab-2"));
    }

    #[tokio::test]
    async fn prune_stale_drops_sessions_past_ttl_regardless_of_state() {
        let store = SessionStore::in_memory();
        // Seed two sessions via real events.
        store
            .apply_event(&mk_payload("pre-tool-use", Some("fresh"), Some("Bash")))
            .await;
        store
            .apply_event(&mk_payload("pre-tool-use", Some("ancient"), Some("Bash")))
            .await;
        // Rewind one of them past the TTL. We reach into the inner
        // map directly — there's no public setter, and that's fine
        // since this is the test module.
        {
            let mut map = store.inner.write().await;
            let ancient = map.get_mut("ancient").unwrap();
            ancient.last_activity = Utc::now() - chrono::Duration::hours(1);
        }
        let removed = store.prune_stale().await;
        assert_eq!(removed, vec!["ancient".to_string()]);
        let list = store.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "fresh");
    }

    #[tokio::test]
    async fn top_level_session_id_wins_over_payload() {
        // If both are present the top-level (env-var origin) takes precedence.
        let store = SessionStore::in_memory();
        let mut p = mk_payload("pre-tool-use", Some("env-wins"), Some("Bash"));
        p.payload = Some(serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls" },
            "session_id": "payload-loses",
        }));
        store.apply_event(&p).await;
        let sessions = store.list().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "env-wins");
    }

    #[tokio::test]
    async fn persistence_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sessions.json");
        let store = SessionStore::load(path.clone()).await.unwrap();
        store
            .apply_event(&mk_payload("pre-tool-use", Some("sss"), Some("Bash")))
            .await;
        drop(store);
        let reloaded = SessionStore::load(path).await.unwrap();
        let list = reloaded.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "sss");
        assert_eq!(list[0].state, SessionState::AwaitingApproval);
    }

    #[tokio::test]
    async fn load_with_corrupt_file_starts_fresh_and_keeps_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sessions.json");
        tokio::fs::write(&path, b"{ not json }").await.unwrap();
        let store = SessionStore::load(path.clone()).await.unwrap();
        assert_eq!(store.list().await.len(), 0);
        assert!(path.with_extension("json.corrupted").exists());
    }
}
