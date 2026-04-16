//! Filesystem watcher for `~/.vibeisland/events/`.
//!
//! The `hook` subcommand (#10) writes one JSON file per event. This
//! module watches the directory, parses each file into a [`HookPayload`],
//! feeds it into the [`SessionStore`] (#12), and forwards the resulting
//! [`SessionDelta`] onto a broadcast channel that the Tauri layer
//! subscribes to (wired up in #13).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::event::{CreateKind, ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::event::HookPayload;
use crate::session_store::{SessionDelta, SessionStore};

const BROADCAST_CAPACITY: usize = 256;
const DEBOUNCE: Duration = Duration::from_millis(50);

/// A running watcher. Drop the struct to stop the background task.
pub struct EventWatcher {
    /// Subscribe to get [`SessionDelta`]s as they happen.
    pub deltas: broadcast::Sender<SessionDelta>,
    _fs_watcher: notify::RecommendedWatcher,
    _task: JoinHandle<()>,
}

impl EventWatcher {
    /// Start watching `events_dir`. Any files already sitting in the
    /// directory are drained first — so events buffered while the app
    /// was closed are not lost.
    pub async fn start(events_dir: PathBuf, store: Arc<SessionStore>) -> std::io::Result<Self> {
        tokio::fs::create_dir_all(&events_dir).await?;

        let (tx, rx) = mpsc::unbounded_channel::<notify::Event>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })
        .map_err(std::io::Error::other)?;
        watcher
            .watch(&events_dir, RecursiveMode::NonRecursive)
            .map_err(std::io::Error::other)?;

        let (delta_tx, _) = broadcast::channel(BROADCAST_CAPACITY);

        // Drain files already on disk before spawning the run loop so
        // we don't race the watcher on the same entries.
        Self::drain_once(&events_dir, &store, &delta_tx).await?;

        let task = tokio::spawn(run_loop(events_dir, rx, store, delta_tx.clone()));

        Ok(Self {
            deltas: delta_tx,
            _fs_watcher: watcher,
            _task: task,
        })
    }

    /// Process every valid event file currently on disk — one-shot. Used
    /// internally by [`start`] and exposed for tests.
    pub async fn drain_once(
        events_dir: &Path,
        store: &SessionStore,
        delta_tx: &broadcast::Sender<SessionDelta>,
    ) -> std::io::Result<()> {
        let mut read = tokio::fs::read_dir(events_dir).await?;
        while let Some(entry) = read.next_entry().await? {
            let path = entry.path();
            if !is_event_file(&path) {
                continue;
            }
            if let Some(delta) = process_file(&path, store).await {
                let _ = delta_tx.send(delta);
            }
        }
        Ok(())
    }
}

async fn run_loop(
    events_dir: PathBuf,
    mut rx: mpsc::UnboundedReceiver<notify::Event>,
    store: Arc<SessionStore>,
    delta_tx: broadcast::Sender<SessionDelta>,
) {
    while let Some(event) = rx.recv().await {
        if !is_creation_like(&event.kind) {
            continue;
        }
        for path in event.paths {
            if !is_event_file(&path) {
                continue;
            }
            // Small debounce so notify doesn't fire mid-write.
            tokio::time::sleep(DEBOUNCE).await;
            if let Some(delta) = process_file(&path, &store).await {
                let _ = delta_tx.send(delta);
            }
        }
    }
    tracing::debug!(path = ?events_dir, "event watcher stopped");
}

fn is_creation_like(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(CreateKind::File | CreateKind::Any)
            | EventKind::Modify(ModifyKind::Name(RenameMode::To))
            | EventKind::Modify(ModifyKind::Data(_))
    )
}

fn is_event_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    !name.starts_with('.') && name.ends_with(".json")
}

async fn process_file(path: &Path, store: &SessionStore) -> Option<SessionDelta> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(path = ?path, error = %e, "read event file failed");
            return None;
        }
    };
    let payload: HookPayload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(path = ?path, error = %e, "parse event file failed");
            let _ = move_to_malformed(path).await;
            return None;
        }
    };
    let delta = store.apply_event(&payload).await;
    if let Err(e) = tokio::fs::remove_file(path).await {
        tracing::debug!(path = ?path, error = %e, "remove event file failed (probably already gone)");
    }
    delta
}

async fn move_to_malformed(path: &Path) -> std::io::Result<()> {
    let parent = match path.parent() {
        Some(p) => p,
        None => return Ok(()),
    };
    let bad_dir = parent.join(".malformed");
    tokio::fs::create_dir_all(&bad_dir).await?;
    let filename = path.file_name().unwrap_or_default();
    let dest = bad_dir.join(filename);
    tokio::fs::rename(path, dest).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn write_payload(dir: &Path, payload: &HookPayload) -> PathBuf {
        let filename = format!("{}.json", payload.id);
        let path = dir.join(filename);
        std::fs::write(&path, serde_json::to_vec(payload).unwrap()).unwrap();
        path
    }

    fn mk_payload() -> HookPayload {
        HookPayload {
            event: "pre-tool-use".into(),
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            payload: Some(serde_json::json!({
                "tool_name": "Bash",
                "tool_input": {"command": "ls"}
            })),
            raw_payload: None,
            session_id: Some("sess-1".into()),
            cwd: Some("/home/jay/code".into()),
            pid: 1,
            env: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn drain_once_processes_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().to_path_buf();
        write_payload(&events, &mk_payload());

        let store = SessionStore::in_memory();
        let (tx, mut rx) = broadcast::channel(8);
        EventWatcher::drain_once(&events, &store, &tx)
            .await
            .unwrap();

        let delta = rx.try_recv().unwrap();
        assert!(matches!(delta, SessionDelta::New(_)));
        assert_eq!(store.list().await.len(), 1);
        let remaining: Vec<_> = std::fs::read_dir(&events).unwrap().collect();
        assert_eq!(remaining.len(), 0);
    }

    #[tokio::test]
    async fn malformed_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().to_path_buf();
        std::fs::write(events.join("broken.json"), b"{ not json }").unwrap();

        let store = SessionStore::in_memory();
        let (tx, _rx) = broadcast::channel(8);
        EventWatcher::drain_once(&events, &store, &tx)
            .await
            .unwrap();

        assert_eq!(store.list().await.len(), 0);
        assert!(events.join(".malformed/broken.json").exists());
    }

    #[tokio::test]
    async fn start_watches_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().to_path_buf();
        let store = Arc::new(SessionStore::in_memory());
        let watcher = EventWatcher::start(events.clone(), store.clone())
            .await
            .unwrap();
        let mut rx = watcher.deltas.subscribe();

        write_payload(&events, &mk_payload());

        let delta = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("watcher should emit a delta")
            .expect("delta should be received");
        assert!(matches!(delta, SessionDelta::New(_)));
        assert_eq!(store.list().await.len(), 1);
    }

    #[tokio::test]
    async fn start_drains_preexisting_files() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().to_path_buf();
        // Write the file BEFORE starting the watcher.
        write_payload(&events, &mk_payload());

        let store = Arc::new(SessionStore::in_memory());
        let watcher = EventWatcher::start(events.clone(), store.clone())
            .await
            .unwrap();
        let _ = watcher.deltas.subscribe();

        assert_eq!(store.list().await.len(), 1);
    }

    #[test]
    fn is_event_file_filters_hidden_and_non_json() {
        assert!(is_event_file(Path::new("20260416-abc.json")));
        assert!(!is_event_file(Path::new(".hidden.json")));
        assert!(!is_event_file(Path::new("foo.txt")));
        assert!(!is_event_file(Path::new(".x.json.tmp")));
    }
}
