//! End-to-end smoke test for the Phase 1 flow.
//!
//! Spawns the real `vibeisland-linux hook pre-tool-use` binary in a
//! subprocess with a simulated Claude Code payload, then in-process:
//!
//! 1. hook binary writes an event file under `$VIBEISLAND_HOME/.vibeisland/events/`
//! 2. our [`EventWatcher`] picks it up and feeds [`SessionStore`]
//! 3. a [`SessionDelta::New`] shows up on the broadcast channel with
//!    state == `AwaitingApproval`
//! 4. we write an approve `HookDecision` into `$VIBEISLAND_HOME/.vibeisland/responses/`
//! 5. the hook subprocess unblocks and prints the Claude Code permission
//!    decision JSON (`{"hookSpecificOutput":{"permissionDecision":"allow"}}`)
//! 6. process exits 0
//!
//! If this passes, the full hook → watcher → store → IPC response loop
//! works across process boundaries on the test machine.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use vibeisland_agents::response::{self, HookDecision};
use vibeisland_agents::{EventWatcher, PendingAction, SessionDelta, SessionState, SessionStore};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hook_to_store_to_response_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let base = home.join(".vibeisland");
    let events = base.join("events");
    let responses = base.join("responses");
    std::fs::create_dir_all(&events).unwrap();
    std::fs::create_dir_all(&responses).unwrap();

    // Simulate a running overlay so the hook takes the blocking-approval
    // path. Without this pidfile the hook now fails open and exits
    // silently, which is the correct behavior for users running `claude`
    // without VibeIsland — but not what this end-to-end test exercises.
    std::fs::write(base.join("overlay.pid"), std::process::id().to_string()).unwrap();

    let store = Arc::new(SessionStore::in_memory());
    let watcher = EventWatcher::start(events.clone(), store.clone())
        .await
        .unwrap();
    let mut rx = watcher.deltas.subscribe();

    let bin = env!("CARGO_BIN_EXE_vibeisland-linux");
    let mut child = Command::new(bin)
        .arg("hook")
        .arg("pre-tool-use")
        .env("VIBEISLAND_HOME", &home)
        .env("CLAUDE_SESSION_ID", "e2e-smoke")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook binary — did cargo build it?");

    // Pipe a Claude Code-like payload to the hook's stdin.
    {
        let stdin = child.stdin.as_mut().expect("stdin piped");
        stdin
            .write_all(br#"{"tool_name":"Bash","tool_input":{"command":"ls /tmp"}}"#)
            .unwrap();
    }
    drop(child.stdin.take());

    // The watcher should see the event within a few seconds.
    let delta = timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("watcher delta timeout")
        .expect("delta channel closed");
    let session = match delta {
        SessionDelta::New(s) => s,
        other => panic!("expected New delta, got {other:?}"),
    };
    assert_eq!(session.state, SessionState::AwaitingApproval);
    let action_id = match session.pending_action.as_ref().expect("pending action") {
        PendingAction::ToolPermission { id, tool, .. } => {
            assert_eq!(tool, "Bash");
            id.clone()
        }
        other => panic!("expected ToolPermission, got {other:?}"),
    };

    // Drop the approve decision file — hook should unblock.
    response::write_decision(&responses, &action_id, &HookDecision::Approve)
        .await
        .expect("write decision");

    // Hook subprocess is blocked on std::thread::sleep loops; the wait
    // has to happen on a blocking thread so tokio doesn't deadlock.
    let output =
        tokio::task::spawn_blocking(move || child.wait_with_output().expect("wait_with_output"))
            .await
            .expect("join blocking");

    assert!(
        output.status.success(),
        "hook exited non-zero: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(
        stdout.contains("\"permissionDecision\":\"allow\""),
        "stdout missing allow decision: {stdout}"
    );
    assert!(
        stdout.contains("\"hookEventName\":\"PreToolUse\""),
        "stdout missing hookEventName: {stdout}"
    );
}
