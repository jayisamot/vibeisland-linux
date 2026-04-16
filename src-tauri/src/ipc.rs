//! Tauri commands exposed to the frontend.
//!
//! The overlay (React) talks to the backend through these commands.
//! Spec: [docs/architecture.md — IPC Frontend ↔ Backend].
//!
//! On startup, `AppState` holds the shared [`SessionStore`] and spawns
//! an [`EventWatcher`]; deltas are forwarded to the webview as Tauri
//! events `session:new` / `session:updated` / `session:closed`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use vibeisland_agents::{AgentRegistry, AgentSession, EventWatcher, SessionDelta, SessionStore};

/// Shared application state mounted on the Tauri app via `manage`.
pub struct AppState {
    pub store: Arc<SessionStore>,
    pub registry: Arc<AgentRegistry>,
    // Hold the watcher alive for the process lifetime.
    pub _watcher: EventWatcher,
}

impl AppState {
    pub async fn init(app: &AppHandle) -> std::io::Result<Self> {
        let home = home_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no HOME"))?;
        let base = home.join(".vibeisland");
        let events_dir = base.join("events");
        let sessions_path = base.join("sessions.json");

        let store = Arc::new(SessionStore::load(sessions_path).await?);
        let registry = Arc::new(AgentRegistry::new());

        let watcher = EventWatcher::start(events_dir, store.clone()).await?;
        spawn_delta_bridge(app.clone(), watcher.deltas.subscribe());

        Ok(Self {
            store,
            registry,
            _watcher: watcher,
        })
    }
}

fn spawn_delta_bridge(app: AppHandle, mut rx: tokio::sync::broadcast::Receiver<SessionDelta>) {
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(delta) => emit_delta(&app, &delta),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "delta channel lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn emit_delta(app: &AppHandle, delta: &SessionDelta) {
    let (event, payload) = match delta {
        SessionDelta::New(s) => ("session:new", serde_json::to_value(s).ok()),
        SessionDelta::Updated(s) => ("session:updated", serde_json::to_value(s).ok()),
        SessionDelta::Closed { id } => ("session:closed", Some(serde_json::json!({ "id": id }))),
    };
    if let Some(p) = payload {
        if let Err(e) = app.emit(event, p) {
            tracing::warn!(event, error = %e, "emit failed");
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("VIBEISLAND_HOME") {
        return Some(PathBuf::from(h));
    }
    directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
}

// ---------- Commands ----------

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<AgentSession>, String> {
    Ok(state.store.list().await)
}

#[tauri::command]
pub async fn approve(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    action_id: String,
) -> Result<(), String> {
    let session = state
        .store
        .get(&session_id)
        .await
        .ok_or_else(|| format!("unknown session: {session_id}"))?;

    if let Some(agent) = state.registry.get(&session.agent_id) {
        agent
            .approve(&session_id, &action_id)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tracing::warn!(
            session_id,
            action_id,
            agent = session.agent_id,
            "approve called but no adapter registered (ok for phase 1 until #9)"
        );
    }
    if let Some(delta) = state.store.mark_resolved(&session_id).await {
        emit_delta(&app, &delta);
    }
    Ok(())
}

#[tauri::command]
pub async fn deny(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    action_id: String,
) -> Result<(), String> {
    let session = state
        .store
        .get(&session_id)
        .await
        .ok_or_else(|| format!("unknown session: {session_id}"))?;
    if let Some(agent) = state.registry.get(&session.agent_id) {
        agent
            .deny(&session_id, &action_id)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tracing::warn!(session_id, action_id, "deny called with no adapter");
    }
    if let Some(delta) = state.store.mark_resolved(&session_id).await {
        emit_delta(&app, &delta);
    }
    Ok(())
}

#[tauri::command]
pub async fn answer_question(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    question_id: String,
    answer: String,
) -> Result<(), String> {
    let session = state
        .store
        .get(&session_id)
        .await
        .ok_or_else(|| format!("unknown session: {session_id}"))?;
    if let Some(agent) = state.registry.get(&session.agent_id) {
        agent
            .answer(&session_id, &question_id, &answer)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tracing::warn!(session_id, question_id, "answer called with no adapter");
    }
    if let Some(delta) = state.store.mark_resolved(&session_id).await {
        emit_delta(&app, &delta);
    }
    Ok(())
}

#[tauri::command]
pub async fn focus_terminal(_session_id: String) -> Result<(), String> {
    // Real implementation lands with #27 (frontend wiring) once the
    // terminal locators in #21-#25 exist.
    Err("focus_terminal not implemented yet (issue #27)".to_string())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
}

#[tauri::command]
pub async fn get_config() -> Result<Config, String> {
    Ok(Config { schema_version: 1 })
}

#[tauri::command]
pub async fn set_config(_config: Config) -> Result<(), String> {
    // Real implementation lands with #32.
    Ok(())
}
