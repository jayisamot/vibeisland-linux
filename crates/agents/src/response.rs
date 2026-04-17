//! Shared schema for approve/deny/answer responses written by the app
//! (issue #13 IPC commands) and consumed by the blocking `hook`
//! subcommand (issue #14).
//!
//! Communication is one-shot file drop in `~/.vibeisland/responses/<id>.json`.
//! The hook polls the directory for the matching id and deletes the
//! file after reading. File polling beats FIFO / socket on robustness:
//! hook times out cleanly if the app crashes, no special filesystems
//! required, no mid-write race (write is atomic via tmp+rename).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default hook timeout before falling back to deny.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Poll interval while the hook waits for a decision.
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Grace period after which, if the app process is not even running,
/// we give up and deny. The hook does not actually check the app pid
/// (too fragile); instead the 10s is implicit in the caller choosing
/// not to install hooks when the app is absent — see issue #9.
pub const APP_ABSENT_GRACE: Duration = Duration::from_secs(10);

/// What the overlay decided for a pending action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum HookDecision {
    /// Tool call approved — Claude Code proceeds normally.
    Approve,
    /// Tool call denied — Claude Code skips the tool call. Optional
    /// free-form reason shown to Claude so it can adapt.
    Deny { reason: Option<String> },
    /// Answer to an `AskUserQuestion`. Encoded as a deny + the chosen
    /// option label carried in `reason` — Claude Code does not expose a
    /// structured answer channel on PreToolUse hooks yet. Good enough
    /// for MVP; refine in issue #28 when the tool protocol is richer.
    Answer { option_id: String, label: String },
}

/// Compute the response file path for a given action/question id.
pub fn response_path(base: &Path, id: &str) -> PathBuf {
    base.join(format!("{id}.json"))
}

/// Write the decision atomically (`tmp` + rename). Creates the parent
/// directory if missing.
pub async fn write_decision(base: &Path, id: &str, decision: &HookDecision) -> std::io::Result<()> {
    tokio::fs::create_dir_all(base).await?;
    let final_path = response_path(base, id);
    let tmp_path = base.join(format!(".{id}.json.tmp"));
    let bytes = serde_json::to_vec(decision).map_err(std::io::Error::other)?;
    tokio::fs::write(&tmp_path, bytes).await?;
    tokio::fs::rename(tmp_path, final_path).await
}

/// Blocking read (synchronous — used by the hook binary which doesn't
/// spin up tokio). Returns `None` if the file is not (yet) present or
/// cannot be parsed.
pub fn read_decision_blocking(base: &Path, id: &str) -> Option<HookDecision> {
    let path = response_path(base, id);
    let bytes = std::fs::read(&path).ok()?;
    let decision = serde_json::from_slice(&bytes).ok()?;
    // Best-effort remove; ignore errors (another process may have won
    // the race).
    let _ = std::fs::remove_file(&path);
    Some(decision)
}

/// Block the current thread polling for a decision, up to `timeout`.
pub fn wait_for_decision(base: &Path, id: &str, timeout: Duration) -> Option<HookDecision> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(decision) = read_decision_blocking(base, id) {
            return Some(decision);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        write_decision(dir.path(), "abc", &HookDecision::Approve)
            .await
            .unwrap();
        let decoded = read_decision_blocking(dir.path(), "abc").unwrap();
        assert!(matches!(decoded, HookDecision::Approve));
        // File is removed by read_decision_blocking.
        assert!(!response_path(dir.path(), "abc").exists());
    }

    #[tokio::test]
    async fn wait_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let result = tokio::task::spawn_blocking({
            let base = dir.path().to_path_buf();
            move || wait_for_decision(&base, "missing", Duration::from_millis(250))
        })
        .await
        .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn wait_returns_when_file_appears() {
        let dir = Arc::new(tempfile::tempdir().unwrap());
        let dir_clone = dir.clone();
        // Write after a small delay from another task.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            write_decision(
                dir_clone.path(),
                "id-1",
                &HookDecision::Deny {
                    reason: Some("because".into()),
                },
            )
            .await
            .unwrap();
        });
        let result = tokio::task::spawn_blocking({
            let base = dir.path().to_path_buf();
            move || wait_for_decision(&base, "id-1", Duration::from_secs(2))
        })
        .await
        .unwrap();
        match result {
            Some(HookDecision::Deny { reason }) => assert_eq!(reason.as_deref(), Some("because")),
            other => panic!("unexpected {other:?}"),
        }
    }
}
