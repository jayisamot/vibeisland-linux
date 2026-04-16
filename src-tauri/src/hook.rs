//! CLI `hook <event>` subcommand.
//!
//! Claude Code (and future agents) invoke this binary as a hook — they
//! pipe a JSON payload on stdin and read the exit code + stdout. For
//! most events the hook writes one JSON file under `~/.vibeisland/events/`
//! and exits immediately. For `pre-tool-use`, the hook additionally
//! BLOCKS up to 5 minutes waiting for the user to approve / deny in the
//! overlay, then prints a Claude Code-compatible permission decision
//! on stdout.
//!
//! Response plumbing is described in [`vibeisland_agents::response`].
//!
//! Contract:
//! - stdin may be empty / invalid JSON — never crashes
//! - event file is written atomically (tmp + rename)
//! - non-pre-tool-use events exit in <100ms
//! - pre-tool-use blocks until response OR [`DEFAULT_TIMEOUT`] → deny
//! - failures are logged to `~/.vibeisland/hook.log` (append) and the
//!   process still exits cleanly so it cannot wedge the agent

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use chrono::Utc;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use vibeisland_agents::response::{self, HookDecision, DEFAULT_TIMEOUT};
use vibeisland_agents::HookPayload;

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    Stop,
    Notification,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "pre-tool-use",
            Self::PostToolUse => "post-tool-use",
            Self::UserPromptSubmit => "user-prompt-submit",
            Self::Stop => "stop",
            Self::Notification => "notification",
        }
    }
}

/// Entry point used by `main.rs` — returns the exit code.
pub fn run(event: HookEvent) -> i32 {
    let payload = match build_payload(event) {
        Ok(p) => p,
        Err(err) => {
            let _ = log_error(event, &err);
            return 0;
        }
    };

    let base = match vibeisland_home() {
        Some(h) => h,
        None => return 0,
    };

    if let Err(err) = write_event_file(&base, &payload) {
        let _ = log_error(event, &err);
        // Fall through — for pre-tool-use we can still try to block
        // since the response file is independent.
    }

    match event {
        HookEvent::PreToolUse => {
            let responses_dir = base.join("responses");
            let decision =
                response::wait_for_decision(&responses_dir, &payload.id, DEFAULT_TIMEOUT)
                    .unwrap_or(HookDecision::Deny {
                        reason: Some(format!(
                            "VibeIsland: no decision within {}s — default deny",
                            DEFAULT_TIMEOUT.as_secs()
                        )),
                    });
            print_claude_code_decision(&decision);
            0
        }
        _ => 0,
    }
}

fn build_payload(event: HookEvent) -> std::io::Result<HookPayload> {
    let mut stdin_buf = String::new();
    std::io::stdin().read_to_string(&mut stdin_buf).ok();

    let (payload, raw_payload) = if stdin_buf.trim().is_empty() {
        (None, None)
    } else {
        match serde_json::from_str::<serde_json::Value>(&stdin_buf) {
            Ok(v) => (Some(v), None),
            Err(_) => (None, Some(stdin_buf)),
        }
    };

    Ok(HookPayload {
        event: event.as_str().to_string(),
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        payload,
        raw_payload,
        session_id: env::var("CLAUDE_SESSION_ID")
            .ok()
            .or_else(|| env::var("VIBEISLAND_SESSION_ID").ok()),
        cwd: env::current_dir().ok().map(|p| p.display().to_string()),
        pid: process::id(),
        env: capture_env(),
    })
}

fn write_event_file(base: &Path, payload: &HookPayload) -> std::io::Result<()> {
    let events_dir = base.join("events");
    fs::create_dir_all(&events_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&events_dir, fs::Permissions::from_mode(0o700));
    }
    let ts = Utc::now();
    let filename = format!(
        "{}-{}.json",
        ts.format("%Y%m%dT%H%M%S%3f"),
        &payload.id[..8]
    );
    write_atomic(&events_dir, &filename, payload)
}

fn capture_env() -> BTreeMap<String, String> {
    let keys = [
        "TERM_PROGRAM",
        "TERM",
        "KITTY_PID",
        "KITTY_WINDOW_ID",
        "KONSOLE_VERSION",
        "GNOME_TERMINAL_SCREEN",
        "ALACRITTY_WINDOW_ID",
        "COLORTERM",
        "WINDOWID",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "PPID",
    ];
    keys.iter()
        .filter_map(|k| env::var(k).ok().map(|v| ((*k).to_string(), v)))
        .collect()
}

fn write_atomic(dir: &Path, filename: &str, record: &HookPayload) -> std::io::Result<()> {
    let final_path = dir.join(filename);
    let tmp_path = dir.join(format!(".{filename}.tmp"));
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)?;
        let bytes = serde_json::to_vec(record).map_err(std::io::Error::other)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

fn log_error(event: HookEvent, err: &std::io::Error) -> std::io::Result<()> {
    let base = match vibeisland_home() {
        Some(h) => h,
        None => return Ok(()),
    };
    fs::create_dir_all(&base)?;
    let log_path = base.join("hook.log");
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(
        f,
        "{} event={} pid={} err={}",
        Utc::now().to_rfc3339(),
        event.as_str(),
        process::id(),
        err,
    )
}

/// Emit a Claude Code PreToolUse decision to stdout. The shape matches
/// the `hookSpecificOutput` protocol Claude Code expects so it will
/// honor `allow` / `deny`.
fn print_claude_code_decision(decision: &HookDecision) {
    let (perm, reason) = match decision {
        HookDecision::Approve => ("allow", None),
        HookDecision::Deny { reason } => ("deny", reason.clone()),
        HookDecision::Answer { label, .. } => (
            "deny",
            Some(format!("User picked: {label} (via VibeIsland)")),
        ),
    };
    let mut output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": perm,
        }
    });
    if let Some(r) = reason {
        output["hookSpecificOutput"]["permissionDecisionReason"] = serde_json::Value::String(r);
    }
    println!("{}", serde_json::to_string(&output).unwrap_or_default());
}

fn vibeisland_home() -> Option<PathBuf> {
    if let Ok(h) = env::var("VIBEISLAND_HOME") {
        return Some(PathBuf::from(h).join(".vibeisland"));
    }
    directories::BaseDirs::new().map(|b| b.home_dir().join(".vibeisland"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kebab_matches_as_str() {
        for e in [
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::UserPromptSubmit,
            HookEvent::Stop,
            HookEvent::Notification,
        ] {
            let serialized = serde_json::to_string(&e).unwrap();
            let expected = format!("\"{}\"", e.as_str());
            assert_eq!(serialized, expected);
        }
    }

    #[test]
    fn write_atomic_creates_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let record = HookPayload {
            event: "pre-tool-use".into(),
            id: "abc-123".into(),
            timestamp: "2026-04-16T00:00:00Z".into(),
            payload: Some(serde_json::json!({"tool_name": "Bash"})),
            raw_payload: None,
            session_id: None,
            cwd: Some("/tmp".into()),
            pid: 42,
            env: Default::default(),
        };
        write_atomic(dir.path(), "x.json", &record).unwrap();
        let content = fs::read_to_string(dir.path().join("x.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["event"], "pre-tool-use");
        assert_eq!(v["payload"]["tool_name"], "Bash");
    }

    #[test]
    fn claude_code_json_for_approve_has_allow() {
        let buffer = render_decision(&HookDecision::Approve);
        let v: serde_json::Value = serde_json::from_str(&buffer).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "allow");
        assert!(v["hookSpecificOutput"]["permissionDecisionReason"].is_null());
    }

    #[test]
    fn claude_code_json_for_deny_includes_reason() {
        let buffer = render_decision(&HookDecision::Deny {
            reason: Some("nope".into()),
        });
        let v: serde_json::Value = serde_json::from_str(&buffer).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(v["hookSpecificOutput"]["permissionDecisionReason"], "nope");
    }

    #[test]
    fn claude_code_json_for_answer_is_deny_with_label() {
        let buffer = render_decision(&HookDecision::Answer {
            option_id: "o1".into(),
            label: "Option A".into(),
        });
        let v: serde_json::Value = serde_json::from_str(&buffer).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        let reason = v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap();
        assert!(reason.contains("Option A"));
    }

    /// Helper that mirrors `print_claude_code_decision` but returns the
    /// rendered string instead of writing to stdout, so we can assert
    /// on it without capturing global stdout.
    fn render_decision(decision: &HookDecision) -> String {
        let (perm, reason) = match decision {
            HookDecision::Approve => ("allow", None),
            HookDecision::Deny { reason } => ("deny", reason.clone()),
            HookDecision::Answer { label, .. } => (
                "deny",
                Some(format!("User picked: {label} (via VibeIsland)")),
            ),
        };
        let mut output = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": perm,
            }
        });
        if let Some(r) = reason {
            output["hookSpecificOutput"]["permissionDecisionReason"] = serde_json::Value::String(r);
        }
        serde_json::to_string(&output).unwrap()
    }
}
